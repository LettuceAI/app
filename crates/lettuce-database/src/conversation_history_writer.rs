//! Historical conversation insertion.
//!
//! Backup restore and legacy import write finished conversations with their
//! own ids and timestamps. Rows are inserted in the order the 0008 triggers
//! require, turns walk their legal status path instead of being inserted
//! terminal, and the create operation plus its outbox event come last so later
//! live mutations see a normal conversation.

use std::collections::{BTreeMap, BTreeSet};

use lettuce_conversations::{
    BranchStatus, ConversationAggregate, ConversationBranch, ConversationKind,
    ConversationOutboxEvent, ConversationOutboxRecord, ConversationRepositoryError,
    GenerationAttempt, GenerationAttemptStatus, GenerationInput, GenerationTarget, GenerationTurn,
    GenerationTurnStatus, InitialMessageOrigin, MessageCandidate, MessagePart, MessageRenderSource,
    MessageRevision, OperationKind, OperationResultRef, OperationToken, ProtectedSnapshotRef,
    SnapshotArtifactDraft, SnapshotSelection, ValidationError,
};
use lettuce_transfer::{
    BackupConversation, BackupMemoryProjection, BackupMemoryProjectionState, BackupMemorySpace,
    BackupMessage,
};
use lettuce_types::{
    ConversationBranchId, ConversationId, GenerationAttemptId, MessageId, OutboxEventId, Revision,
    SnapshotArtifactId,
};
use lettuce_usage::UsageEvent;
use rusqlite::{Transaction, params};

use super::{
    conversation_artifact_adapter, conversation_creator, conversation_mutation_kernel as kernel,
    conversation_vertical_slice as slice,
};

pub(crate) struct HistoricalConversation<'a> {
    pub history: &'a BackupConversation,
    pub turns: &'a [GenerationTurn],
    pub usage: &'a [UsageEvent],
    pub snapshots: Vec<SnapshotArtifactDraft>,
    pub operation: OperationToken,
    pub memory: Option<&'a BackupMemorySpace>,
    pub memory_projections: &'a [BackupMemoryProjection],
}

fn invalid(field: &'static str) -> ConversationRepositoryError {
    ConversationRepositoryError::Invalid(ValidationError::InvalidValue { field })
}

pub(crate) fn insert_historical_conversation(
    transaction: &Transaction<'_>,
    input: HistoricalConversation<'_>,
) -> Result<ConversationAggregate, ConversationRepositoryError> {
    let aggregate = &input.history.aggregate;
    aggregate
        .validate()
        .map_err(ConversationRepositoryError::Invalid)?;
    let conversation = &aggregate.conversation;
    let conversation_id = conversation.id;
    let is_group = conversation.kind.is_group();
    for draft in input.snapshots {
        conversation_artifact_adapter::stage_snapshot_in_transaction(
            transaction,
            draft,
            conversation.created_at,
        )
        .map_err(ConversationRepositoryError::ArtifactReference)?;
    }
    slice::save_conversation(transaction, conversation)?;
    match input.memory {
        Some(space) if space.conversation_id == conversation_id => {
            crate::memory_adapter::insert_space_in(transaction, conversation_id, &space.snapshot)?;
        }
        Some(_) => return Err(invalid("history.memory_space")),
        None if conversation_creator::conversation_uses_memory(&conversation.kind) => {
            crate::memory_adapter::create_conversation_space_in(transaction, conversation_id)?;
        }
        None => {}
    }
    insert_snapshot_refs(transaction, input.history)?;

    let branches = aggregate
        .branches
        .iter()
        .map(|branch| (branch.id, branch))
        .collect::<BTreeMap<_, _>>();
    let mut turns_by_message = BTreeMap::<MessageId, Vec<&GenerationTurn>>::new();
    for turn in input.turns {
        validate_turn(turn, conversation_id, is_group)?;
        let target = match turn.target {
            GenerationTarget::NewAssistant { message_id, .. }
            | GenerationTarget::ExistingCandidate { message_id, .. } => message_id,
        };
        turns_by_message.entry(target).or_default().push(turn);
    }
    let usage = input
        .usage
        .iter()
        .map(|event| (event.record.attempt_id, event))
        .collect::<BTreeMap<_, _>>();
    if usage.len() != input.usage.len() {
        return Err(invalid("history.usage"));
    }

    let mut messages = input.history.messages.iter().collect::<Vec<_>>();
    messages.sort_by_key(|message| (message.timeline_ordinal, message.message.id));
    let mut inserted_branches = BTreeSet::new();
    for backup in &messages {
        let branch = branches
            .get(&backup.message.branch_id)
            .ok_or_else(|| invalid("history.message_branch"))?;
        if inserted_branches.insert(branch.id) {
            insert_branch(transaction, branch)?;
        }
        let turns = turns_by_message
            .remove(&backup.message.id)
            .unwrap_or_default();
        insert_message_with_turns(transaction, backup, &turns, &usage)?;
        set_branch_head(
            transaction,
            conversation_id,
            branch.id,
            Some(backup.message.id),
        )?;
    }
    if !turns_by_message.is_empty() {
        return Err(invalid("history.turn_target"));
    }
    for branch in &aggregate.branches {
        if inserted_branches.insert(branch.id) {
            insert_branch(transaction, branch)?;
        }
        set_branch_head(
            transaction,
            conversation_id,
            branch.id,
            branch.head_message_id,
        )?;
    }
    let next_ordinal = messages.last().map_or(Ok(1), |message| {
        message
            .timeline_ordinal
            .checked_add(1)
            .and_then(|value| i64::try_from(value).ok())
            .ok_or(ConversationRepositoryError::Storage)
    })?;
    transaction
        .execute(
            "UPDATE conversations SET next_timeline_ordinal = ?1 WHERE id = ?2",
            params![next_ordinal, conversation_id.to_string()],
        )
        .map_err(slice::db)?;

    insert_memory_state(transaction, input.memory, input.memory_projections)?;
    insert_creation_record(transaction, aggregate, &messages, &input.operation)?;
    let stored = slice::hydrate_conversation(transaction, conversation_id, || {})?;
    let mut expected = aggregate.clone();
    expected
        .conversation
        .participants
        .sort_by_key(|participant| (participant.ordinal, participant.id));
    expected
        .branches
        .sort_by_key(|branch| (branch.created_at, branch.id));
    if stored != expected {
        return Err(ConversationRepositoryError::Storage);
    }
    Ok(stored)
}

fn insert_memory_state(
    transaction: &Transaction<'_>,
    memory: Option<&BackupMemorySpace>,
    projections: &[BackupMemoryProjection],
) -> Result<(), ConversationRepositoryError> {
    let Some(space) = memory else {
        return if projections.is_empty() {
            Ok(())
        } else {
            Err(invalid("history.memory_projections"))
        };
    };
    let space_id = space.snapshot.id;
    if let Some(summary) = &space.summary {
        crate::memory_adapter::replace_summary_in(transaction, space_id, Some(summary))
            .map_err(|_| ConversationRepositoryError::Storage)?;
    }
    for projection in projections {
        if projection.space_id != space_id {
            return Err(invalid("history.memory_projection_space"));
        }
        let (status, vector) = match &projection.state {
            BackupMemoryProjectionState::Ready { vector_le_hex } => (
                "ready",
                Some(
                    crate::hex_decode(vector_le_hex)
                        .map_err(|()| invalid("history.memory_projection_vector"))?,
                ),
            ),
            BackupMemoryProjectionState::RepairNeeded => ("repair_needed", None),
        };
        transaction
            .execute(
                "INSERT INTO memory_embedding_projections (space_id, memory_id, source_revision, dimensions, source_text, status, vector, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    space_id.to_string(),
                    projection.memory_id.to_string(),
                    projection.source_revision,
                    i64::from(projection.dimensions),
                    projection.source_text,
                    status,
                    vector,
                    projection.updated_at.get(),
                ],
            )
            .map_err(kernel::map_constraint)?;
    }
    Ok(())
}

fn validate_turn(
    turn: &GenerationTurn,
    conversation_id: ConversationId,
    is_group: bool,
) -> Result<(), ConversationRepositoryError> {
    turn.validate(is_group)
        .map_err(ConversationRepositoryError::Invalid)?;
    if turn.conversation_id != conversation_id
        || !matches!(
            turn.status,
            GenerationTurnStatus::Succeeded
                | GenerationTurnStatus::Failed
                | GenerationTurnStatus::Cancelled
        )
        || turn.selected_speaker.is_some()
        || !turn.lorebooks.is_empty()
        || turn.memory.is_some()
        || turn.candidate_ids.is_empty()
    {
        return Err(invalid("history.turn"));
    }
    Ok(())
}

fn insert_snapshot_refs(
    transaction: &Transaction<'_>,
    history: &BackupConversation,
) -> Result<(), ConversationRepositoryError> {
    let conversation = &history.aggregate.conversation;
    let mut references = BTreeMap::<SnapshotArtifactId, &ProtectedSnapshotRef>::new();
    for reference in lettuce_conversations::conversation_snapshot_references(&conversation.kind) {
        references.insert(reference.artifact_id, reference);
    }
    for participant in &conversation.participants {
        if let SnapshotSelection::Inherited(model) | SnapshotSelection::Explicit(model) =
            &participant.model_selection
        {
            references.insert(model.snapshot_ref.artifact_id, &model.snapshot_ref);
        }
    }
    if let ConversationKind::Group(details) = &conversation.kind {
        for policy in &details.initial_participant_policy.members {
            if let SnapshotSelection::Inherited(model) | SnapshotSelection::Explicit(model) =
                &policy.model_override
            {
                references.insert(model.snapshot_ref.artifact_id, &model.snapshot_ref);
            }
        }
    }
    for message in &history.messages {
        if let Some(
            InitialMessageOrigin::SelectedScene { snapshot_ref }
            | InitialMessageOrigin::StarterMessage { snapshot_ref, .. },
        ) = &message.initial_origin
        {
            references.insert(snapshot_ref.artifact_id, snapshot_ref);
        }
    }
    for reference in references.values() {
        conversation_artifact_adapter::verify_snapshot_in_transaction(transaction, reference)
            .map_err(ConversationRepositoryError::ArtifactReference)?;
        transaction
            .execute(
                "INSERT OR IGNORE INTO conversation_snapshot_refs (conversation_id, artifact_id) VALUES (?1, ?2)",
                params![conversation.id.to_string(), reference.artifact_id.to_string()],
            )
            .map_err(slice::db)?;
    }
    Ok(())
}

const fn branch_status_name(status: BranchStatus) -> &'static str {
    match status {
        BranchStatus::Active => "active",
        BranchStatus::Archived => "archived",
        BranchStatus::Tombstoned => "tombstoned",
    }
}

fn insert_branch(
    transaction: &Transaction<'_>,
    branch: &ConversationBranch,
) -> Result<(), ConversationRepositoryError> {
    transaction
        .execute(
            "INSERT INTO conversation_branches (conversation_id, id, parent_branch_id, fork_message_id, head_message_id, status, revision, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6, ?7, ?8)",
            params![
                branch.conversation_id.to_string(),
                branch.id.to_string(),
                branch.parent_branch_id.map(|id| id.to_string()),
                branch.fork_message_id.map(|id| id.to_string()),
                branch_status_name(branch.status),
                slice::sql_revision(branch.revision)?,
                branch.created_at.get(),
                branch.updated_at.get(),
            ],
        )
        .map_err(kernel::map_constraint)?;
    Ok(())
}

fn set_branch_head(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    branch_id: ConversationBranchId,
    head: Option<MessageId>,
) -> Result<(), ConversationRepositoryError> {
    transaction
        .execute(
            "UPDATE conversation_branches SET head_message_id = ?1 WHERE conversation_id = ?2 AND id = ?3",
            params![
                head.map(|id| id.to_string()),
                conversation_id.to_string(),
                branch_id.to_string(),
            ],
        )
        .map_err(kernel::map_constraint)?;
    Ok(())
}

fn insert_message_with_turns(
    transaction: &Transaction<'_>,
    backup: &BackupMessage,
    turns: &[&GenerationTurn],
    usage: &BTreeMap<GenerationAttemptId, &UsageEvent>,
) -> Result<(), ConversationRepositoryError> {
    let candidate_ids = backup
        .candidates
        .iter()
        .map(|candidate| candidate.id)
        .collect::<BTreeSet<_>>();
    let turn_candidate_ids = turns
        .iter()
        .flat_map(|turn| turn.candidate_ids.iter().copied())
        .collect::<BTreeSet<_>>();
    let attempts = turns
        .iter()
        .flat_map(|turn| turn.attempts.iter().map(|attempt| (turn.id, attempt.id)))
        .collect::<BTreeSet<_>>();
    if candidate_ids != turn_candidate_ids
        || backup
            .candidates
            .iter()
            .any(|candidate| !attempts.contains(&(candidate.turn_id, candidate.attempt_id)))
        || backup
            .revisions
            .iter()
            .any(|revision| revision.provider_replay.is_some())
        || backup
            .candidates
            .iter()
            .any(|candidate| candidate.provider_replay.is_some())
    {
        return Err(invalid("history.message_candidates"));
    }
    if let Some(first) = turns.first() {
        if !matches!(first.target, GenerationTarget::NewAssistant { .. }) {
            return Err(invalid("history.first_turn"));
        }
        insert_turn(transaction, first)?;
    }
    insert_message(transaction, backup)?;
    for revision in &backup.revisions {
        insert_revision(transaction, backup, revision)?;
    }
    if let Some(origin) = &backup.initial_origin {
        insert_origin(transaction, backup, origin)?;
    }
    for (index, turn) in turns.iter().enumerate() {
        if index > 0 {
            insert_turn(transaction, turn)?;
        }
        settle_turn(transaction, backup, turn, usage)?;
    }
    Ok(())
}

fn insert_message(
    transaction: &Transaction<'_>,
    backup: &BackupMessage,
) -> Result<(), ConversationRepositoryError> {
    let message = &backup.message;
    let (revision_id, candidate_id) = match message.active_render_source {
        MessageRenderSource::Revision(id) => (Some(id.to_string()), None),
        MessageRenderSource::Candidate(id) => (None, Some(id.to_string())),
    };
    transaction
        .execute(
            "INSERT INTO conversation_messages (conversation_id, id, branch_id, parent_message_id, author_participant_id, role, timeline_ordinal, logical_time, effective_time, visibility, pinned, scene_edited, active_revision_id, active_candidate_id, revision, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
            params![
                message.conversation_id.to_string(),
                message.id.to_string(),
                message.branch_id.to_string(),
                message.parent_message_id.map(|id| id.to_string()),
                message.author_participant_id.map(|id| id.to_string()),
                kernel::message_role_name(message.role),
                i64::try_from(backup.timeline_ordinal)
                    .map_err(|_| invalid("history.timeline_ordinal"))?,
                message.logical_time.get(),
                message.effective_time.get(),
                kernel::message_visibility_name(message.visibility),
                i64::from(message.pinned),
                i64::from(message.scene_edited),
                revision_id,
                candidate_id,
                slice::sql_revision(message.revision)?,
                message.created_at.get(),
                message.updated_at.get(),
            ],
        )
        .map_err(kernel::map_constraint)?;
    Ok(())
}

fn insert_revision(
    transaction: &Transaction<'_>,
    backup: &BackupMessage,
    revision: &MessageRevision,
) -> Result<(), ConversationRepositoryError> {
    let message = &backup.message;
    transaction
        .execute(
            "INSERT INTO conversation_message_revisions (conversation_id, id, message_id, branch_id, sequence, parts_json, authored_at, source_turn_id, provider_replay_artifact_id, provider_replay_retention) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, NULL)",
            params![
                message.conversation_id.to_string(),
                revision.id.to_string(),
                message.id.to_string(),
                message.branch_id.to_string(),
                slice::sql_revision(revision.sequence)?,
                slice::encode(&revision.parts)?,
                revision.authored_at.get(),
                revision.source_turn_id.map(|id| id.to_string()),
            ],
        )
        .map_err(kernel::map_constraint)?;
    for (ordinal, asset_id, role) in media_parts(&revision.parts)? {
        transaction
            .execute(
                "INSERT INTO revision_media_refs (conversation_id, message_revision_id, part_ordinal, asset_id, media_role, state, created_at) VALUES (?1, ?2, ?3, ?4, ?5, 'active', ?6)",
                params![
                    message.conversation_id.to_string(),
                    revision.id.to_string(),
                    ordinal,
                    asset_id,
                    role,
                    revision.authored_at.get(),
                ],
            )
            .map_err(kernel::map_constraint)?;
    }
    Ok(())
}

fn media_parts(
    parts: &[MessagePart],
) -> Result<Vec<(i64, String, &'static str)>, ConversationRepositoryError> {
    parts
        .iter()
        .enumerate()
        .filter_map(|(ordinal, part)| match part {
            MessagePart::MediaAsset { asset_id, role } => Some(
                i64::try_from(ordinal)
                    .map(|ordinal| {
                        (
                            ordinal,
                            asset_id.to_string(),
                            conversation_creator::media_role_name(*role),
                        )
                    })
                    .map_err(|_| ConversationRepositoryError::Storage),
            ),
            _ => None,
        })
        .collect()
}

fn insert_origin(
    transaction: &Transaction<'_>,
    backup: &BackupMessage,
    origin: &InitialMessageOrigin,
) -> Result<(), ConversationRepositoryError> {
    let (source_kind, starter_id, reference) = match origin {
        InitialMessageOrigin::SelectedScene { snapshot_ref } => ("scene", None, snapshot_ref),
        InitialMessageOrigin::StarterMessage {
            snapshot_ref,
            starter_message_id,
        } => (
            "starter",
            Some(starter_message_id.to_string()),
            snapshot_ref,
        ),
    };
    transaction
        .execute(
            "INSERT INTO conversation_initial_message_origins (conversation_id, message_id, snapshot_artifact_id, source_kind, starter_message_id) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                backup.message.conversation_id.to_string(),
                backup.message.id.to_string(),
                reference.artifact_id.to_string(),
                source_kind,
                starter_id,
            ],
        )
        .map_err(kernel::map_constraint)?;
    Ok(())
}

fn insert_turn(
    transaction: &Transaction<'_>,
    turn: &GenerationTurn,
) -> Result<(), ConversationRepositoryError> {
    let (input_kind, user_message_id, head_message_id, candidate_message_id, candidate_id) =
        match turn.input {
            GenerationInput::UserMessage { message_id } => (
                "user_message",
                Some(message_id.to_string()),
                None,
                None,
                None,
            ),
            GenerationInput::ExistingHead { head_message_id } => (
                "existing_head",
                None,
                Some(head_message_id.to_string()),
                None,
                None,
            ),
            GenerationInput::ExistingCandidate {
                message_id,
                candidate_id,
            } => (
                "existing_candidate",
                None,
                None,
                Some(message_id.to_string()),
                Some(candidate_id.to_string()),
            ),
        };
    let (target_kind, target_message_id, target_parent_message_id, target_prior_candidate_id) =
        match turn.target {
            GenerationTarget::NewAssistant {
                message_id,
                parent_message_id,
            } => (
                "new_assistant",
                message_id.to_string(),
                parent_message_id.map(|id| id.to_string()),
                None,
            ),
            GenerationTarget::ExistingCandidate {
                message_id,
                prior_candidate_id,
            } => (
                "existing_candidate",
                message_id.to_string(),
                None,
                Some(prior_candidate_id.to_string()),
            ),
        };
    transaction
        .execute(
            "INSERT INTO conversation_turns (conversation_id, id, branch_id, operation, input_kind, user_message_id, head_message_id, candidate_message_id, candidate_id, idempotency_key, correlation_id, status, target_kind, target_message_id, target_parent_message_id, target_prior_candidate_id, retry_of_turn_id, guidance, requested_model_override_json, forced_speaker_participant_id, swap_roles, revision, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 'created', ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, 1, ?21, ?21)",
            params![
                turn.conversation_id.to_string(),
                turn.id.to_string(),
                turn.branch_id.to_string(),
                kernel::generation_operation_name(turn.operation),
                input_kind,
                user_message_id,
                head_message_id,
                candidate_message_id,
                candidate_id,
                turn.idempotency_key.as_str(),
                turn.correlation_id.map(|id| id.to_string()),
                target_kind,
                target_message_id,
                target_parent_message_id,
                target_prior_candidate_id,
                turn.retry_of_turn_id.map(|id| id.to_string()),
                turn.guidance,
                turn.requested_model_override
                    .as_ref()
                    .map(slice::encode)
                    .transpose()?,
                turn.forced_speaker.map(|id| id.to_string()),
                i64::from(turn.swap_roles),
                turn.created_at.get(),
            ],
        )
        .map_err(kernel::map_constraint)?;
    Ok(())
}

fn settle_turn(
    transaction: &Transaction<'_>,
    backup: &BackupMessage,
    turn: &GenerationTurn,
    usage: &BTreeMap<GenerationAttemptId, &UsageEvent>,
) -> Result<(), ConversationRepositoryError> {
    let mut attempts = turn.attempts.iter().collect::<Vec<_>>();
    attempts.sort_by_key(|attempt| attempt.ordinal);
    for attempt in attempts {
        insert_attempt(transaction, turn, attempt)?;
        for candidate in backup
            .candidates
            .iter()
            .filter(|candidate| candidate.attempt_id == attempt.id)
        {
            if candidate.turn_id != turn.id || !attempt.candidate_ids.contains(&candidate.id) {
                return Err(invalid("history.candidate_attempt"));
            }
            insert_candidate(transaction, backup, candidate)?;
        }
        settle_attempt(transaction, turn, attempt, usage.get(&attempt.id).copied())?;
    }
    let path: &[&str] = match turn.status {
        GenerationTurnStatus::Succeeded => {
            &["preparing", "context_prepared", "running", "finalizing"]
        }
        GenerationTurnStatus::Failed => &["preparing"],
        GenerationTurnStatus::Cancelled => &["cancellation_requested"],
        _ => return Err(invalid("history.turn_status")),
    };
    for status in path {
        transaction
            .execute(
                "UPDATE conversation_turns SET status = ?1 WHERE conversation_id = ?2 AND id = ?3",
                params![
                    *status,
                    turn.conversation_id.to_string(),
                    turn.id.to_string()
                ],
            )
            .map_err(kernel::map_constraint)?;
    }
    let prompt_entry_ids = turn
        .prompt
        .as_ref()
        .map(|prompt| slice::encode(&prompt.selected_entry_ids))
        .transpose()?;
    transaction
        .execute(
            "UPDATE conversation_turns SET status = ?1, selected_candidate_id = ?2, failure = ?3, resolved_model_json = ?4, prompt_document_id = ?5, prompt_revision = ?6, prompt_entry_ids_json = ?7, revision = ?8, updated_at = ?9 WHERE conversation_id = ?10 AND id = ?11",
            params![
                kernel::generation_status_name(turn.status),
                turn.selected_candidate_id.map(|id| id.to_string()),
                turn.failure.map(kernel::failure_name),
                turn.resolved_model.as_ref().map(slice::encode).transpose()?,
                turn.prompt.as_ref().map(|prompt| prompt.document_id.to_string()),
                turn.prompt
                    .as_ref()
                    .map(|prompt| slice::sql_revision(prompt.revision))
                    .transpose()?,
                prompt_entry_ids,
                slice::sql_revision(turn.revision)?,
                turn.updated_at.get(),
                turn.conversation_id.to_string(),
                turn.id.to_string(),
            ],
        )
        .map_err(kernel::map_constraint)?;
    Ok(())
}

fn insert_attempt(
    transaction: &Transaction<'_>,
    turn: &GenerationTurn,
    attempt: &GenerationAttempt,
) -> Result<(), ConversationRepositoryError> {
    transaction
        .execute(
            "INSERT INTO generation_attempts (conversation_id, turn_id, id, ordinal, parent_attempt_id, status, job_idempotency_key, job_id, started_at, finished_at, usage_event_id, usage_outcome, failure) VALUES (?1, ?2, ?3, ?4, ?5, 'created', ?6, ?7, NULL, NULL, NULL, NULL, NULL)",
            params![
                turn.conversation_id.to_string(),
                turn.id.to_string(),
                attempt.id.to_string(),
                i64::from(attempt.ordinal),
                attempt.parent_attempt_id.map(|id| id.to_string()),
                attempt.job_idempotency_key.as_str(),
                attempt.job_id.map(|id| id.to_string()),
            ],
        )
        .map_err(kernel::map_constraint)?;
    Ok(())
}

fn settle_attempt(
    transaction: &Transaction<'_>,
    turn: &GenerationTurn,
    attempt: &GenerationAttempt,
    event: Option<&UsageEvent>,
) -> Result<(), ConversationRepositoryError> {
    let usage_event_id = attempt
        .usage_event_id
        .ok_or_else(|| invalid("history.attempt_usage"))?;
    let finished_at = attempt
        .finished_at
        .ok_or_else(|| invalid("history.attempt_finished_at"))?;
    let started_at = attempt.started_at.unwrap_or(finished_at);
    if event.is_some_and(|event| event.id != usage_event_id || event.record.turn_id != turn.id)
        || !matches!(
            attempt.status,
            GenerationAttemptStatus::Succeeded
                | GenerationAttemptStatus::Failed
                | GenerationAttemptStatus::Cancelled
                | GenerationAttemptStatus::Interrupted
        )
    {
        return Err(invalid("history.attempt"));
    }
    let outcome = kernel::attempt_status_name(attempt.status);
    transaction
        .execute(
            "INSERT INTO conversation_usage_refs (conversation_id, turn_id, attempt_id, usage_event_id, outcome, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                turn.conversation_id.to_string(),
                turn.id.to_string(),
                attempt.id.to_string(),
                usage_event_id.to_string(),
                outcome,
                finished_at.get(),
            ],
        )
        .map_err(kernel::map_constraint)?;
    if let Some(event) = event {
        crate::usage_adapter::insert_usage_event_in(
            transaction,
            &turn.conversation_id.to_string(),
            event,
        )
        .map_err(|_| ConversationRepositoryError::Storage)?;
    }
    transaction
        .execute(
            "UPDATE generation_attempts SET status = ?1, usage_event_id = ?2, usage_outcome = ?1, failure = ?3, started_at = ?4, finished_at = ?5 WHERE conversation_id = ?6 AND turn_id = ?7 AND id = ?8",
            params![
                outcome,
                usage_event_id.to_string(),
                attempt.failure.map(kernel::failure_name),
                started_at.get(),
                finished_at.get(),
                turn.conversation_id.to_string(),
                turn.id.to_string(),
                attempt.id.to_string(),
            ],
        )
        .map_err(kernel::map_constraint)?;
    Ok(())
}

fn insert_candidate(
    transaction: &Transaction<'_>,
    backup: &BackupMessage,
    candidate: &MessageCandidate,
) -> Result<(), ConversationRepositoryError> {
    let message = &backup.message;
    transaction
        .execute(
            "INSERT INTO conversation_message_candidates (conversation_id, id, message_id, branch_id, turn_id, attempt_id, author_participant_id, ordinal, parts_json, model_json, created_at, provider_replay_artifact_id, provider_replay_retention) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, NULL, NULL)",
            params![
                message.conversation_id.to_string(),
                candidate.id.to_string(),
                message.id.to_string(),
                message.branch_id.to_string(),
                candidate.turn_id.to_string(),
                candidate.attempt_id.to_string(),
                candidate.author_participant_id.to_string(),
                i64::from(candidate.ordinal),
                slice::encode(&candidate.parts)?,
                slice::encode(&candidate.model)?,
                candidate.created_at.get(),
            ],
        )
        .map_err(kernel::map_constraint)?;
    for (ordinal, asset_id, role) in media_parts(&candidate.parts)? {
        transaction
            .execute(
                "INSERT INTO candidate_media_refs (conversation_id, candidate_id, part_ordinal, asset_id, media_role, state, created_at) VALUES (?1, ?2, ?3, ?4, ?5, 'active', ?6)",
                params![
                    message.conversation_id.to_string(),
                    candidate.id.to_string(),
                    ordinal,
                    asset_id,
                    role,
                    candidate.created_at.get(),
                ],
            )
            .map_err(kernel::map_constraint)?;
    }
    Ok(())
}

fn insert_creation_record(
    transaction: &Transaction<'_>,
    aggregate: &ConversationAggregate,
    messages: &[&BackupMessage],
    token: &OperationToken,
) -> Result<(), ConversationRepositoryError> {
    let conversation = &aggregate.conversation;
    let root_branch_id = aggregate
        .branches
        .iter()
        .find(|branch| branch.parent_branch_id.is_none())
        .map(|branch| branch.id)
        .ok_or_else(|| invalid("history.root_branch"))?;
    let origins = messages
        .iter()
        .filter(|message| message.initial_origin.is_some())
        .collect::<Vec<_>>();
    let operation = kernel::insert_operation(
        transaction,
        conversation.id,
        OperationKind::Create,
        token,
        &OperationResultRef::Conversation(conversation.id),
        conversation.created_at,
    )?;
    let sequence =
        kernel::next_outbox_sequence(transaction, conversation.id, OperationKind::Create)?;
    if sequence != 1 {
        return Err(ConversationRepositoryError::Storage);
    }
    let outbox = ConversationOutboxRecord {
        format_version: 1,
        id: OutboxEventId::new(),
        conversation_id: conversation.id,
        conversation_revision: Revision::INITIAL,
        sequence,
        operation_record_id: operation.id,
        at: conversation.created_at,
        event: ConversationOutboxEvent::ConversationCreated {
            conversation_id: conversation.id,
            root_branch_id,
            head_message_id: origins.last().map(|message| message.message.id),
            initial_message_count: u16::try_from(origins.len())
                .map_err(|_| invalid("history.initial_messages"))?,
            at: conversation.created_at,
        },
    };
    kernel::insert_outbox(transaction, &outbox)
}
