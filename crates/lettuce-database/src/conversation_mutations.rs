//! Conversation mutations that open a generation turn.
//!
//! Every method here runs through the shared mutation kernel: an Immediate
//! transaction, a replay check on the operation token, the staged writes, the
//! operation record, and the outbox events. The generation-begin family adds
//! one rule of its own: a conversation may hold at most one live turn, so a
//! second begin while a turn is unsettled is a conflict.
//!
//! `append_event` is deliberately shaped differently: its signature carries no
//! conversation revision, so it reads the lifecycle instead of comparing and
//! swapping it, and it leaves the conversation revision and outbox untouched.

use lettuce_conversations::{
    ArchiveConversation, ArchiveConversationResult, AssetReferenceDelta, AssetReferenceState,
    AttachAttemptJob, AttachAttemptJobResult, BeginGeneration, BranchResult, CancelGeneration,
    ChooseCandidate, ChooseCandidateResult, ContinueConversation, ContinueConversationResult,
    ConversationBranch, ConversationLifecycle, ConversationOutboxEvent, ConversationRepository,
    ConversationRepositoryError, DescendantPolicy, EditMessage, EditMessageResult, EditResult,
    FinalizationDraft, ForkBranch, ForkBranchResult, GenerationAttempt, GenerationAttemptStatus,
    GenerationCancellation, GenerationCheckpointEnvelope, GenerationCheckpointEvent,
    GenerationFailure, GenerationFailureCode, GenerationFailureResult, GenerationFinalization,
    GenerationFinalizationResult, GenerationInput, GenerationInterruptionResult,
    GenerationOperation, GenerationRecovery, GenerationRecoveryResult, GenerationTarget,
    GenerationTurn, GenerationTurnStatus, Message, MessageCandidate, MessagePart, OperationKind,
    OperationResultRef, OperationToken, ParticipantPolicyResult, RegenerateCandidate,
    RegenerateCandidateResult, RequestCancellationResult, RestoreConversation,
    RestoreConversationResult, RetryGeneration, RetryGenerationResult, SelectBranch,
    SelectBranchResult, SendConversation, SendConversationResult, SettingsCasRequirement,
    SettingsResult, SettleCancellation, SettleCancellationResult, SnapshotSelection,
    TombstoneMessage, TombstoneMessageResult, TombstoneResult, UpdateConversationSettings,
    UpdateMessageFlags, UpdateMessageFlagsResult, UpdateParticipantPolicy,
    attempt_job_idempotency_key,
};
use lettuce_types::{
    AssetId, ConversationBranchId, ConversationId, ConversationParticipantId, GenerationAttemptId,
    GenerationTurnId, MessageCandidateId, MessageId, MessageRevisionId, Revision, TimestampMillis,
    UsageEventId,
};
use rusqlite::{OptionalExtension, Transaction, params};

use super::{
    Database, conversation_artifact_adapter, conversation_creator,
    conversation_mutation_kernel as kernel, conversation_query,
    conversation_vertical_slice as slice,
};

const SQLITE_CONSTRAINT_UNIQUE: i32 = 2067;
const SQLITE_CONSTRAINT_PRIMARYKEY: i32 = 1555;

const MESSAGE_SELECT_SQL: &str = "SELECT conversation_id, id, branch_id, parent_message_id, author_participant_id, role, logical_time, effective_time, visibility, pinned, scene_edited, timeline_ordinal, active_revision_id, active_candidate_id, revision, created_at, updated_at FROM conversation_messages WHERE conversation_id = ?1 AND id = ?2";

/// The attach path owns the repository-wide job uniqueness rule, so a hit on
/// the partial unique index is `JobInUse` rather than a storage fault.
fn map_attach_constraint(error: rusqlite::Error) -> ConversationRepositoryError {
    if let rusqlite::Error::SqliteFailure(code, Some(message)) = &error
        && code.code == rusqlite::ErrorCode::ConstraintViolation
        && code.extended_code == SQLITE_CONSTRAINT_UNIQUE
        && message == "UNIQUE constraint failed: generation_attempts.job_id"
    {
        return ConversationRepositoryError::JobInUse;
    }
    kernel::map_constraint(error)
}

/// A usage event is recorded once per conversation, so a reused id is a
/// caller conflict rather than a storage fault.
fn map_usage_constraint(error: rusqlite::Error) -> ConversationRepositoryError {
    if let rusqlite::Error::SqliteFailure(code, Some(message)) = &error
        && code.code == rusqlite::ErrorCode::ConstraintViolation
        && matches!(
            code.extended_code,
            SQLITE_CONSTRAINT_PRIMARYKEY | SQLITE_CONSTRAINT_UNIQUE
        )
        && message.contains("conversation_usage_refs")
    {
        return ConversationRepositoryError::Conflict;
    }
    kernel::map_constraint(error)
}

fn invalid(field: &'static str) -> ConversationRepositoryError {
    ConversationRepositoryError::Invalid(lettuce_conversations::ValidationError::InvalidReference {
        field,
    })
}

/// The composed turn key must survive a round trip through `IdempotencyKey`,
/// whose character cap is tighter than the column's, so an over-long caller
/// key is rejected before it can be written and then fail to hydrate.
const MAX_TURN_KEY_CHARS: usize = 128;

fn turn_key(
    kind: OperationKind,
    token: &OperationToken,
) -> Result<String, ConversationRepositoryError> {
    let key = kernel::turn_idempotency_key(kind, token);
    if key.chars().count() > MAX_TURN_KEY_CHARS {
        return Err(ConversationRepositoryError::Invalid(
            lettuce_conversations::ValidationError::TooLarge {
                field: "generation_turn.idempotency_key",
            },
        ));
    }
    Ok(key)
}

/// A checkpoint may only drive the turn through the stages the runtime owns.
/// Cancellation, settlement, and recovery each have their own mutation, which
/// is where the usage rows and outbox events a terminal turn needs are minted.
fn is_appendable_stage(status: GenerationTurnStatus) -> bool {
    matches!(
        status,
        GenerationTurnStatus::Preparing
            | GenerationTurnStatus::SelectingSpeaker
            | GenerationTurnStatus::ContextPrepared
            | GenerationTurnStatus::Running
            | GenerationTurnStatus::Finalizing
    )
}

fn is_group(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
) -> Result<bool, ConversationRepositoryError> {
    transaction
        .query_row(
            "SELECT kind = 'group' FROM conversations WHERE id = ?1",
            [conversation_id.to_string()],
            |row| row.get(0),
        )
        .optional()
        .map_err(slice::db)?
        .ok_or(ConversationRepositoryError::NotFound)
}

/// Speaker selection belongs to group conversations and role swapping to
/// direct ones. The 0008 triggers enforce both; checking here keeps the
/// caller's error class a domain validation rather than a storage conflict.
fn require_speaker_shape(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    forced_speaker: Option<ConversationParticipantId>,
    swap_roles: bool,
) -> Result<(), ConversationRepositoryError> {
    let group = is_group(transaction, conversation_id)?;
    if !group && forced_speaker.is_some() {
        return Err(invalid("generation_turn.direct_speaker"));
    }
    if group && swap_roles {
        return Err(invalid("generation_turn.swap_roles"));
    }
    Ok(())
}

/// Work already in flight settles even after the conversation is archived:
/// archiving is metadata, and a turn that cannot finish would strand its
/// attempt forever. Admission is the half that requires an active aggregate.
fn require_settleable(lifecycle: ConversationLifecycle) -> Result<(), ConversationRepositoryError> {
    match lifecycle {
        ConversationLifecycle::Active | ConversationLifecycle::Archived => Ok(()),
        ConversationLifecycle::Tombstoned => Err(ConversationRepositoryError::Conflict),
    }
}

/// Settings carry no conversation revision, so admission reads the lifecycle
/// directly instead of swapping it.
fn require_active_conversation(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
) -> Result<(), ConversationRepositoryError> {
    let lifecycle: String = transaction
        .query_row(
            "SELECT lifecycle FROM conversations WHERE id = ?1",
            [conversation_id.to_string()],
            |row| row.get(0),
        )
        .optional()
        .map_err(slice::db)?
        .ok_or(ConversationRepositoryError::NotFound)?;
    match slice::lifecycle_from_name(&lifecycle)? {
        ConversationLifecycle::Active => Ok(()),
        _ => Err(ConversationRepositoryError::Conflict),
    }
}

/// `append_event` has no expected revision to compare, so it reads the
/// lifecycle directly instead of swapping it.
fn require_settleable_conversation(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
) -> Result<(), ConversationRepositoryError> {
    let lifecycle: String = transaction
        .query_row(
            "SELECT lifecycle FROM conversations WHERE id = ?1",
            [conversation_id.to_string()],
            |row| row.get(0),
        )
        .optional()
        .map_err(slice::db)?
        .ok_or(ConversationRepositoryError::NotFound)?;
    require_settleable(slice::lifecycle_from_name(&lifecycle)?)
}

/// A generation may only start on the branch the conversation currently
/// renders, so a stale branch reference is a conflict rather than a not-found.
fn require_active_branch(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    branch_id: ConversationBranchId,
) -> Result<(), ConversationRepositoryError> {
    let active: String = transaction
        .query_row(
            "SELECT active_branch_id FROM conversations WHERE id = ?1",
            [conversation_id.to_string()],
            |row| row.get(0),
        )
        .optional()
        .map_err(slice::db)?
        .ok_or(ConversationRepositoryError::NotFound)?;
    if active == branch_id.to_string() {
        Ok(())
    } else {
        Err(ConversationRepositoryError::Conflict)
    }
}

/// One live turn per conversation. Every unsettled status counts, including
/// the recovery states, so a crashed turn blocks new work until it settles.
fn require_no_live_turn(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
) -> Result<(), ConversationRepositoryError> {
    let live: bool = transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM conversation_turns WHERE conversation_id = ?1 AND status NOT IN ('succeeded', 'failed', 'cancelled', 'interrupted'))",
            [conversation_id.to_string()],
            |row| row.get(0),
        )
        .map_err(slice::db)?;
    if live {
        Err(ConversationRepositoryError::Conflict)
    } else {
        Ok(())
    }
}

fn cas_turn(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    turn_id: GenerationTurnId,
    expected: Revision,
) -> Result<(), ConversationRepositoryError> {
    let revision: Option<i64> = transaction
        .query_row(
            "SELECT revision FROM conversation_turns WHERE conversation_id = ?1 AND id = ?2",
            params![conversation_id.to_string(), turn_id.to_string()],
            |row| row.get(0),
        )
        .optional()
        .map_err(slice::db)?;
    let actual = slice::rev(revision.ok_or(ConversationRepositoryError::NotFound)?)?;
    if actual == expected {
        Ok(())
    } else {
        Err(ConversationRepositoryError::StaleRevision { expected, actual })
    }
}

/// Where the next message on a branch attaches. A fresh fork has no head of
/// its own yet, so its first message hangs off the fork point and keeps the
/// inherited ancestry reachable; the topology trigger accepts that parent.
fn branch_parent(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    branch_id: ConversationBranchId,
) -> Result<Option<MessageId>, ConversationRepositoryError> {
    let parent: Option<Option<String>> = transaction
        .query_row(
            "SELECT COALESCE(head_message_id, fork_message_id) FROM conversation_branches WHERE conversation_id = ?1 AND id = ?2",
            params![conversation_id.to_string(), branch_id.to_string()],
            |row| row.get(0),
        )
        .optional()
        .map_err(slice::db)?;
    conversation_query::parse_opt(parent.ok_or(ConversationRepositoryError::NotFound)?)
}

fn branch_head(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    branch_id: ConversationBranchId,
) -> Result<Option<MessageId>, ConversationRepositoryError> {
    let head: Option<String> = transaction
        .query_row(
            "SELECT head_message_id FROM conversation_branches WHERE conversation_id = ?1 AND id = ?2",
            params![conversation_id.to_string(), branch_id.to_string()],
            |row| row.get(0),
        )
        .optional()
        .map_err(slice::db)?
        .ok_or(ConversationRepositoryError::NotFound)?;
    conversation_query::parse_opt(head)
}

struct TurnDraft {
    conversation_id: ConversationId,
    turn_id: GenerationTurnId,
    branch_id: ConversationBranchId,
    operation: GenerationOperation,
    input: GenerationInput,
    target: GenerationTarget,
    idempotency_key: String,
    guidance: Option<String>,
    model_override: Option<String>,
    forced_speaker: Option<ConversationParticipantId>,
    swap_roles: bool,
}

fn insert_turn(
    transaction: &Transaction<'_>,
    draft: &TurnDraft,
    now: TimestampMillis,
) -> Result<(), ConversationRepositoryError> {
    let (input_kind, user_message_id, head_message_id, candidate_message_id, candidate_id) =
        match draft.input {
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
        match draft.target {
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
            "INSERT INTO conversation_turns (conversation_id, id, branch_id, operation, input_kind, user_message_id, head_message_id, candidate_message_id, candidate_id, idempotency_key, correlation_id, status, target_kind, target_message_id, target_parent_message_id, target_prior_candidate_id, retry_of_turn_id, guidance, requested_model_override_json, forced_speaker_participant_id, swap_roles, revision, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, NULL, 'created', ?11, ?12, ?13, ?14, NULL, ?15, ?16, ?17, ?18, 1, ?19, ?19)",
            params![
                draft.conversation_id.to_string(),
                draft.turn_id.to_string(),
                draft.branch_id.to_string(),
                kernel::generation_operation_name(draft.operation),
                input_kind,
                user_message_id,
                head_message_id,
                candidate_message_id,
                candidate_id,
                draft.idempotency_key,
                target_kind,
                target_message_id,
                target_parent_message_id,
                target_prior_candidate_id,
                draft.guidance,
                draft.model_override,
                draft.forced_speaker.map(|id| id.to_string()),
                i64::from(draft.swap_roles),
                now.get(),
            ],
        )
        .map_err(kernel::map_constraint)?;
    Ok(())
}

/// Every begin opens attempt zero. Later attempts belong to recovery, which
/// links them to their interrupted predecessor.
fn insert_attempt(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    turn_id: GenerationTurnId,
    ordinal: u16,
    parent_attempt_id: Option<GenerationAttemptId>,
) -> Result<GenerationAttemptId, ConversationRepositoryError> {
    let attempt_id = GenerationAttemptId::new();
    transaction
        .execute(
            "INSERT INTO generation_attempts (conversation_id, turn_id, id, ordinal, parent_attempt_id, status, job_idempotency_key, job_id, started_at, finished_at, usage_event_id, usage_outcome, failure) VALUES (?1, ?2, ?3, ?4, ?5, 'created', ?6, NULL, NULL, NULL, NULL, NULL, NULL)",
            params![
                conversation_id.to_string(),
                turn_id.to_string(),
                attempt_id.to_string(),
                i64::from(ordinal),
                parent_attempt_id.map(|id| id.to_string()),
                attempt_job_idempotency_key(turn_id, attempt_id).as_str(),
            ],
        )
        .map_err(kernel::map_constraint)?;
    Ok(attempt_id)
}

fn insert_first_attempt(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    turn_id: GenerationTurnId,
) -> Result<GenerationAttemptId, ConversationRepositoryError> {
    insert_attempt(transaction, conversation_id, turn_id, 0, None)
}

/// Settlement writes in the order the 0008 triggers require: the usage row
/// first, then the attempt, so the turn's own terminal update already sees
/// every attempt settled with its usage reference.
struct AttemptSettlement {
    outcome: GenerationAttemptStatus,
    failure: Option<GenerationFailureCode>,
    usage_event_id: UsageEventId,
}

fn settle_attempt_terminal(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    turn_id: GenerationTurnId,
    attempt_id: GenerationAttemptId,
    settlement: AttemptSettlement,
    now: TimestampMillis,
) -> Result<(), ConversationRepositoryError> {
    let AttemptSettlement {
        outcome,
        failure,
        usage_event_id,
    } = settlement;
    let outcome_name = kernel::attempt_status_name(outcome);
    transaction
        .execute(
            "INSERT INTO conversation_usage_refs (conversation_id, turn_id, attempt_id, usage_event_id, outcome, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                conversation_id.to_string(),
                turn_id.to_string(),
                attempt_id.to_string(),
                usage_event_id.to_string(),
                outcome_name,
                now.get(),
            ],
        )
        .map_err(map_usage_constraint)?;
    let changed = transaction
        .execute(
            "UPDATE generation_attempts SET status = ?4, usage_event_id = ?5, usage_outcome = ?4, failure = ?6, started_at = COALESCE(started_at, ?7), finished_at = ?7 WHERE conversation_id = ?1 AND turn_id = ?2 AND id = ?3",
            params![
                conversation_id.to_string(),
                turn_id.to_string(),
                attempt_id.to_string(),
                outcome_name,
                usage_event_id.to_string(),
                failure.map(kernel::failure_name),
                now.get(),
            ],
        )
        .map_err(kernel::map_constraint)?;
    if changed == 0 {
        return Err(ConversationRepositoryError::NotFound);
    }
    Ok(())
}

/// The named attempt must still be open. A settled one would either regress
/// through the terminal-attempt trigger or double-count its usage.
fn require_live_attempt(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    turn_id: GenerationTurnId,
    attempt_id: GenerationAttemptId,
) -> Result<(), ConversationRepositoryError> {
    let status: Option<String> = transaction
        .query_row(
            "SELECT status FROM generation_attempts WHERE conversation_id = ?1 AND turn_id = ?2 AND id = ?3",
            params![
                conversation_id.to_string(),
                turn_id.to_string(),
                attempt_id.to_string(),
            ],
            |row| row.get(0),
        )
        .optional()
        .map_err(slice::db)?;
    let status = status.ok_or(ConversationRepositoryError::NotFound)?;
    if is_terminal_attempt(&status) {
        return Err(ConversationRepositoryError::Conflict);
    }
    Ok(())
}

/// A terminal turn requires every attempt settled, so a live sibling is a
/// conflict rather than a trigger abort.
fn require_no_other_live_attempt(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    turn_id: GenerationTurnId,
    attempt_id: GenerationAttemptId,
) -> Result<(), ConversationRepositoryError> {
    let live: bool = transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM generation_attempts WHERE conversation_id = ?1 AND turn_id = ?2 AND id <> ?3 AND status NOT IN ('succeeded', 'failed', 'cancelled', 'interrupted'))",
            params![
                conversation_id.to_string(),
                turn_id.to_string(),
                attempt_id.to_string(),
            ],
            |row| row.get(0),
        )
        .map_err(slice::db)?;
    if live {
        Err(ConversationRepositoryError::Conflict)
    } else {
        Ok(())
    }
}

fn is_terminal_attempt(status: &str) -> bool {
    matches!(status, "succeeded" | "failed" | "cancelled" | "interrupted")
}

/// The domain's transition table is the same one migration 0008 encodes, so
/// an illegal hop is reported as a conflict before the trigger fires.
fn require_transition(
    from: GenerationTurnStatus,
    to: GenerationTurnStatus,
) -> Result<(), ConversationRepositoryError> {
    if from.can_transition_to(to) {
        Ok(())
    } else {
        Err(ConversationRepositoryError::Conflict)
    }
}

fn fail_turn(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    turn_id: GenerationTurnId,
    failure: GenerationFailureCode,
    now: TimestampMillis,
) -> Result<(), ConversationRepositoryError> {
    transaction
        .execute(
            "UPDATE conversation_turns SET status = 'failed', failure = ?3, revision = revision + 1, updated_at = ?4 WHERE conversation_id = ?1 AND id = ?2",
            params![
                conversation_id.to_string(),
                turn_id.to_string(),
                kernel::failure_name(failure),
                now.get(),
            ],
        )
        .map_err(kernel::map_constraint)?;
    Ok(())
}

fn succeed_turn(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    turn_id: GenerationTurnId,
    candidate_id: MessageCandidateId,
    now: TimestampMillis,
) -> Result<(), ConversationRepositoryError> {
    transaction
        .execute(
            "UPDATE conversation_turns SET status = 'succeeded', selected_candidate_id = ?3, revision = revision + 1, updated_at = ?4 WHERE conversation_id = ?1 AND id = ?2",
            params![
                conversation_id.to_string(),
                turn_id.to_string(),
                candidate_id.to_string(),
                now.get(),
            ],
        )
        .map_err(kernel::map_constraint)?;
    Ok(())
}

fn load_candidate(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    candidate_id: MessageCandidateId,
) -> Result<MessageCandidate, ConversationRepositoryError> {
    transaction
        .query_row(
            "SELECT conversation_id, id, message_id, branch_id, turn_id, attempt_id, ordinal, parts_json, model_json, created_at, provider_replay_artifact_id, provider_replay_retention, author_participant_id FROM conversation_message_candidates WHERE conversation_id = ?1 AND id = ?2",
            params![conversation_id.to_string(), candidate_id.to_string()],
            |row| {
                conversation_query::hydrate_candidate_row(transaction, row)
                    .map_err(|_| rusqlite::Error::InvalidQuery)
            },
        )
        .optional()
        .map_err(slice::db)?
        .ok_or(ConversationRepositoryError::NotFound)
}

fn load_message(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    message_id: MessageId,
) -> Result<Message, ConversationRepositoryError> {
    let (item, _) = transaction
        .query_row(
            MESSAGE_SELECT_SQL,
            params![conversation_id.to_string(), message_id.to_string()],
            |row| {
                conversation_query::message_row(transaction, row)
                    .map_err(|_| rusqlite::Error::InvalidQuery)
            },
        )
        .optional()
        .map_err(slice::db)?
        .ok_or(ConversationRepositoryError::NotFound)?;
    Ok(item.message)
}

/// Reads one candidate's media projection back as retention deltas, so the
/// committed rows are the single source for both the result and the replay.
fn candidate_deltas(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    candidate_id: MessageCandidateId,
) -> Result<Vec<AssetReferenceDelta>, ConversationRepositoryError> {
    let mut statement = transaction
        .prepare("SELECT asset_id, state FROM candidate_media_refs WHERE conversation_id = ?1 AND candidate_id = ?2 ORDER BY part_ordinal")
        .map_err(slice::db)?;
    let rows: Vec<(String, String)> = statement
        .query_map(
            params![conversation_id.to_string(), candidate_id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(slice::db)?
        .collect::<rusqlite::Result<_>>()
        .map_err(slice::db)?;
    drop(statement);
    let mut deltas = Vec::with_capacity(rows.len());
    for (asset_id, state) in rows {
        deltas.push(AssetReferenceDelta {
            asset_id: slice::parse_id(asset_id)?,
            retainer: lettuce_media::AssetRetainer::MessageCandidate(candidate_id),
            state: match state.as_str() {
                "active" => AssetReferenceState::Active,
                "historical" => AssetReferenceState::Historical,
                _ => return Err(ConversationRepositoryError::Storage),
            },
        });
    }
    Ok(deltas)
}

/// Group turns carry the speaker their selection resolved; a direct turn has
/// exactly one character participant and no speaker columns at all. A
/// regenerated message keeps its author unless the turn names another.
fn resolve_candidate_author(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    turn: &GenerationTurn,
) -> Result<ConversationParticipantId, ConversationRepositoryError> {
    if let Some(speaker) = turn.selected_speaker.as_ref() {
        return Ok(speaker.participant_id);
    }
    if let Some(forced) = turn.forced_speaker {
        return Ok(forced);
    }
    if let GenerationTarget::ExistingCandidate { message_id, .. } = turn.target {
        let author: Option<Option<String>> = transaction
            .query_row(
                "SELECT author_participant_id FROM conversation_messages WHERE conversation_id = ?1 AND id = ?2",
                params![conversation_id.to_string(), message_id.to_string()],
                |row| row.get(0),
            )
            .optional()
            .map_err(slice::db)?;
        let author = author
            .ok_or(ConversationRepositoryError::NotFound)?
            .ok_or(ConversationRepositoryError::Conflict)?;
        return slice::parse_id(author);
    }
    let mut statement = transaction
        .prepare("SELECT id FROM conversation_participants WHERE conversation_id = ?1 AND role = 'character' ORDER BY ordinal, id LIMIT 2")
        .map_err(slice::db)?;
    let candidates: Vec<String> = statement
        .query_map([conversation_id.to_string()], |row| row.get(0))
        .map_err(slice::db)?
        .collect::<rusqlite::Result<_>>()
        .map_err(slice::db)?;
    drop(statement);
    let [only] = candidates.as_slice() else {
        return Err(ConversationRepositoryError::Conflict);
    };
    slice::parse_id(only.clone())
}

struct CandidateIdentity {
    candidate_id: MessageCandidateId,
    message_id: MessageId,
    turn_id: GenerationTurnId,
    attempt_id: GenerationAttemptId,
    author: ConversationParticipantId,
}

/// The draft's own bounds are checked before the transaction opens so an
/// oversized part or a non-conversation replay never reaches a CHECK.
fn validate_finalization_draft(
    draft: &FinalizationDraft,
) -> Result<(), ConversationRepositoryError> {
    if !matches!(draft.outcome, GenerationCheckpointEvent::Completed) {
        return Err(invalid("finalization.outcome"));
    }
    for part in &draft.parts {
        part.validate()
            .map_err(ConversationRepositoryError::Invalid)?;
    }
    draft
        .model
        .validate()
        .map_err(ConversationRepositoryError::Invalid)?;
    if let Some(replay) = &draft.replay {
        replay
            .validate()
            .map_err(ConversationRepositoryError::Invalid)?;
        if replay.retention != lettuce_conversations::ReplayRetention::Conversation {
            return Err(invalid("finalization.replay_retention"));
        }
    }
    Ok(())
}

/// Materializes the assistant message a `new_assistant` turn reserved at
/// begin. Its identity was minted then, so finalization only fills the row.
#[allow(clippy::too_many_arguments)]
fn insert_assistant_message(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    branch_id: ConversationBranchId,
    message_id: MessageId,
    parent_message_id: Option<MessageId>,
    author: ConversationParticipantId,
    candidate_id: MessageCandidateId,
    now: TimestampMillis,
) -> Result<(), ConversationRepositoryError> {
    let ordinal = allocate_timeline_ordinal(transaction, conversation_id)?;
    transaction
        .execute(
            "INSERT INTO conversation_messages (conversation_id, id, branch_id, parent_message_id, author_participant_id, role, timeline_ordinal, logical_time, effective_time, visibility, pinned, scene_edited, active_revision_id, active_candidate_id, revision, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, 'assistant', ?6, ?7, ?7, 'visible', 0, 0, NULL, ?8, 1, ?7, ?7)",
            params![
                conversation_id.to_string(),
                message_id.to_string(),
                branch_id.to_string(),
                parent_message_id.map(|id| id.to_string()),
                author.to_string(),
                ordinal,
                now.get(),
                candidate_id.to_string(),
            ],
        )
        .map_err(kernel::map_constraint)?;
    Ok(())
}

/// Candidate ordinals are dense per message, so the repository derives the
/// next one and treats the draft's value as an expectation to confirm.
fn next_candidate_ordinal(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    message_id: MessageId,
) -> Result<u16, ConversationRepositoryError> {
    let next: i64 = transaction
        .query_row(
            "SELECT COALESCE(max(ordinal) + 1, 0) FROM conversation_message_candidates WHERE conversation_id = ?1 AND message_id = ?2",
            params![conversation_id.to_string(), message_id.to_string()],
            |row| row.get(0),
        )
        .map_err(slice::db)?;
    u16::try_from(next).map_err(|_| ConversationRepositoryError::Storage)
}

fn insert_candidate(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    branch_id: ConversationBranchId,
    draft: &FinalizationDraft,
    identity: CandidateIdentity,
    now: TimestampMillis,
) -> Result<(), ConversationRepositoryError> {
    let ordinal = next_candidate_ordinal(transaction, conversation_id, identity.message_id)?;
    if ordinal != draft.ordinal {
        return Err(invalid("finalization.ordinal"));
    }
    let (replay_artifact_id, replay_retention) = match &draft.replay {
        Some(replay) => (Some(replay.artifact_id.to_string()), Some("conversation")),
        None => (None, None),
    };
    transaction
        .execute(
            "INSERT INTO conversation_message_candidates (conversation_id, id, message_id, branch_id, turn_id, attempt_id, author_participant_id, ordinal, parts_json, model_json, created_at, provider_replay_artifact_id, provider_replay_retention) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                conversation_id.to_string(),
                identity.candidate_id.to_string(),
                identity.message_id.to_string(),
                branch_id.to_string(),
                identity.turn_id.to_string(),
                identity.attempt_id.to_string(),
                identity.author.to_string(),
                i64::from(ordinal),
                slice::encode(&draft.parts)?,
                slice::encode(&draft.model)?,
                now.get(),
                replay_artifact_id,
                replay_retention,
            ],
        )
        .map_err(kernel::map_constraint)?;
    for (part_ordinal, part) in draft.parts.iter().enumerate() {
        let MessagePart::MediaAsset { asset_id, role } = part else {
            continue;
        };
        transaction
            .execute(
                "INSERT INTO candidate_media_refs (conversation_id, candidate_id, part_ordinal, asset_id, media_role, state, created_at) VALUES (?1, ?2, ?3, ?4, ?5, 'active', ?6)",
                params![
                    conversation_id.to_string(),
                    identity.candidate_id.to_string(),
                    i64::try_from(part_ordinal).map_err(|_| ConversationRepositoryError::Storage)?,
                    asset_id.to_string(),
                    conversation_creator::media_role_name(*role),
                    now.get(),
                ],
            )
            .map_err(kernel::map_constraint)?;
    }
    Ok(())
}

/// Rebuilds the finalization result from the committed rows, so the first
/// commit and any later replay read the same way.
fn finalization_value(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    turn_id: GenerationTurnId,
    candidate_id: MessageCandidateId,
    usage_event_id: UsageEventId,
    asset_reference_deltas: Vec<AssetReferenceDelta>,
) -> Result<GenerationFinalization, ConversationRepositoryError> {
    let turn = load_turn(transaction, conversation_id, turn_id)?;
    let candidate = load_candidate(transaction, conversation_id, candidate_id)?;
    if candidate.turn_id != turn_id {
        return Err(ConversationRepositoryError::Conflict);
    }
    let assistant_message = load_message(transaction, conversation_id, candidate.message_id)?;
    Ok(GenerationFinalization {
        turn,
        assistant_message,
        candidate,
        revision: None,
        asset_reference_deltas,
        usage_event_id,
    })
}

/// Retention deltas are event-shaped facts about one commit. A replay reads
/// them back from the operation's own outbox records, because the live media
/// rows keep moving as later regenerations retire candidates.
fn recorded_deltas(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    operation: &lettuce_conversations::OperationRecord,
) -> Result<Vec<AssetReferenceDelta>, ConversationRepositoryError> {
    let records = kernel::read_outbox_for_operation(transaction, conversation_id, operation.id)?;
    let mut deltas = Vec::new();
    for record in records {
        if let ConversationOutboxEvent::AssetReferencesChanged { changes, .. } = record.event {
            deltas.extend(changes);
        }
    }
    Ok(deltas)
}

/// The guard sequence every terminal settlement shares: the conversation and
/// turn are compare-and-swapped, then the attempt is proven open and alone.
fn settle_preamble(
    transaction: &Transaction<'_>,
    context: &kernel::MutationCtx,
    turn_id: GenerationTurnId,
    attempt_id: GenerationAttemptId,
    expected_conversation_revision: Revision,
    expected_turn_revision: Revision,
) -> Result<GenerationTurn, ConversationRepositoryError> {
    let conversation = kernel::cas_conversation(
        transaction,
        context.conversation_id,
        expected_conversation_revision,
    )?;
    require_settleable(conversation.lifecycle)?;
    cas_turn(
        transaction,
        context.conversation_id,
        turn_id,
        expected_turn_revision,
    )?;
    require_live_attempt(transaction, context.conversation_id, turn_id, attempt_id)?;
    require_no_other_live_attempt(transaction, context.conversation_id, turn_id, attempt_id)?;
    load_turn(transaction, context.conversation_id, turn_id)
}

/// A replayed settlement must describe the turn the caller asked about.
fn replayed_settlement(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    operation: &lettuce_conversations::OperationRecord,
    turn_id: GenerationTurnId,
) -> Result<GenerationTurn, ConversationRepositoryError> {
    let replayed = replayed_turn(operation)?;
    if replayed != turn_id {
        return Err(ConversationRepositoryError::Conflict);
    }
    load_turn(transaction, conversation_id, turn_id)
}

fn recovery_value(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    turn_id: GenerationTurnId,
) -> Result<GenerationRecovery, ConversationRepositoryError> {
    let turn = load_turn(transaction, conversation_id, turn_id)?;
    let attempt = turn
        .attempts
        .last()
        .cloned()
        .ok_or(ConversationRepositoryError::Storage)?;
    Ok(GenerationRecovery { turn, attempt })
}

/// Which side of a message's render sources owns a media projection. The two
/// tables are shaped identically; only the owning column differs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MediaOwner {
    Revision(MessageRevisionId),
    Candidate(MessageCandidateId),
}

impl MediaOwner {
    fn table(self) -> &'static str {
        match self {
            Self::Revision(_) => "revision_media_refs",
            Self::Candidate(_) => "candidate_media_refs",
        }
    }

    fn column(self) -> &'static str {
        match self {
            Self::Revision(_) => "message_revision_id",
            Self::Candidate(_) => "candidate_id",
        }
    }

    fn id(self) -> String {
        match self {
            Self::Revision(id) => id.to_string(),
            Self::Candidate(id) => id.to_string(),
        }
    }

    fn retainer(self) -> lettuce_media::AssetRetainer {
        match self {
            Self::Revision(id) => lettuce_media::AssetRetainer::MessageRevision(id),
            Self::Candidate(id) => lettuce_media::AssetRetainer::MessageCandidate(id),
        }
    }
}

fn owner_assets(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    owner: MediaOwner,
) -> Result<Vec<AssetId>, ConversationRepositoryError> {
    let sql = format!(
        "SELECT asset_id FROM {} WHERE conversation_id = ?1 AND {} = ?2 ORDER BY part_ordinal",
        owner.table(),
        owner.column()
    );
    let mut statement = transaction.prepare(&sql).map_err(slice::db)?;
    let rows: Vec<String> = statement
        .query_map(params![conversation_id.to_string(), owner.id()], |row| {
            row.get(0)
        })
        .map_err(slice::db)?
        .collect::<rusqlite::Result<_>>()
        .map_err(slice::db)?;
    drop(statement);
    rows.into_iter().map(slice::parse_id).collect()
}

fn set_owner_media_state(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    owner: MediaOwner,
    state: &str,
) -> Result<(), ConversationRepositoryError> {
    let sql = format!(
        "UPDATE {} SET state = ?3 WHERE conversation_id = ?1 AND {} = ?2",
        owner.table(),
        owner.column()
    );
    transaction
        .execute(
            &sql,
            params![conversation_id.to_string(), owner.id(), state],
        )
        .map_err(kernel::map_constraint)?;
    Ok(())
}

fn owner_deltas(
    assets: &[AssetId],
    owner: MediaOwner,
    state: AssetReferenceState,
) -> Vec<AssetReferenceDelta> {
    assets
        .iter()
        .map(|asset_id| AssetReferenceDelta {
            asset_id: *asset_id,
            retainer: owner.retainer(),
            state,
        })
        .collect()
}

fn owner_event(
    conversation_id: ConversationId,
    owner: MediaOwner,
    changes: Vec<AssetReferenceDelta>,
    at: TimestampMillis,
) -> ConversationOutboxEvent {
    let (message_revision_id, candidate_id) = match owner {
        MediaOwner::Revision(id) => (Some(id), None),
        MediaOwner::Candidate(id) => (None, Some(id)),
    };
    ConversationOutboxEvent::AssetReferencesChanged {
        conversation_id,
        message_revision_id,
        candidate_id,
        changes,
        at,
    }
}

type MessageStateRow = (
    String,
    Option<String>,
    String,
    String,
    Option<String>,
    Option<String>,
);

#[derive(Debug, Clone)]
struct MessageState {
    branch_id: ConversationBranchId,
    parent_message_id: Option<MessageId>,
    role: String,
    visibility: String,
    active_revision_id: Option<MessageRevisionId>,
    active_candidate_id: Option<MessageCandidateId>,
}

impl MessageState {
    fn render_owner(&self) -> Option<MediaOwner> {
        match (self.active_revision_id, self.active_candidate_id) {
            (Some(id), None) => Some(MediaOwner::Revision(id)),
            (None, Some(id)) => Some(MediaOwner::Candidate(id)),
            _ => None,
        }
    }
}

fn message_state(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    message_id: MessageId,
) -> Result<MessageState, ConversationRepositoryError> {
    let row: Option<MessageStateRow> = transaction
        .query_row(
            "SELECT branch_id, parent_message_id, role, visibility, active_revision_id, active_candidate_id FROM conversation_messages WHERE conversation_id = ?1 AND id = ?2",
            params![conversation_id.to_string(), message_id.to_string()],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .optional()
        .map_err(slice::db)?;
    let (branch_id, parent_message_id, role, visibility, active_revision_id, active_candidate_id) =
        row.ok_or(ConversationRepositoryError::NotFound)?;
    Ok(MessageState {
        branch_id: slice::parse_id(branch_id)?,
        parent_message_id: conversation_query::parse_opt(parent_message_id)?,
        role,
        visibility,
        active_revision_id: conversation_query::parse_opt(active_revision_id)?,
        active_candidate_id: conversation_query::parse_opt(active_candidate_id)?,
    })
}

/// A live generation owns every message it reads or writes: its target, the
/// user message it answers, the head it continues, and the candidate it
/// regenerates. Content mutations wait for it to settle.
fn require_no_live_turn_for_message(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    message_id: MessageId,
) -> Result<(), ConversationRepositoryError> {
    let live: bool = transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM conversation_turns WHERE conversation_id = ?1 AND status NOT IN ('succeeded', 'failed', 'cancelled', 'interrupted') AND (target_message_id = ?2 OR user_message_id = ?2 OR head_message_id = ?2 OR candidate_message_id = ?2))",
            params![conversation_id.to_string(), message_id.to_string()],
            |row| row.get(0),
        )
        .map_err(slice::db)?;
    if live {
        Err(ConversationRepositoryError::Conflict)
    } else {
        Ok(())
    }
}

fn load_branch(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    branch_id: ConversationBranchId,
) -> Result<ConversationBranch, ConversationRepositoryError> {
    transaction
        .query_row(
            "SELECT id, parent_branch_id, fork_message_id, head_message_id, status, revision, created_at, updated_at FROM conversation_branches WHERE conversation_id = ?1 AND id = ?2",
            params![conversation_id.to_string(), branch_id.to_string()],
            |row| slice::read_branch(row, conversation_id),
        )
        .optional()
        .map_err(slice::db)?
        .ok_or(ConversationRepositoryError::NotFound)
}

fn select_active_branch(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    branch_id: ConversationBranchId,
) -> Result<(), ConversationRepositoryError> {
    transaction
        .execute(
            "UPDATE conversations SET active_branch_id = ?2 WHERE id = ?1",
            params![conversation_id.to_string(), branch_id.to_string()],
        )
        .map_err(kernel::map_constraint)?;
    Ok(())
}

fn conversation_value(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
) -> Result<lettuce_conversations::Conversation, ConversationRepositoryError> {
    Ok(slice::hydrate_conversation(transaction, conversation_id, || {})?.conversation)
}

/// A replay is only the same mutation when it names the same target. Keys are
/// unique per conversation and kind, so this is what separates two edits of
/// different messages that reused one caller key.
fn replayed_message(
    operation: &lettuce_conversations::OperationRecord,
    message_id: MessageId,
) -> Result<(), ConversationRepositoryError> {
    match operation.result {
        OperationResultRef::Message(id) if id == message_id => Ok(()),
        _ => Err(ConversationRepositoryError::Conflict),
    }
}

fn recorded_events(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    operation: &lettuce_conversations::OperationRecord,
) -> Result<Vec<ConversationOutboxEvent>, ConversationRepositoryError> {
    Ok(
        kernel::read_outbox_for_operation(transaction, conversation_id, operation.id)?
            .into_iter()
            .map(|record| record.event)
            .collect(),
    )
}

fn verify_current_snapshot(
    transaction: &Transaction<'_>,
    reference: &lettuce_conversations::ProtectedSnapshotRef,
) -> Result<(), ConversationRepositoryError> {
    conversation_artifact_adapter::verify_snapshot_in_transaction(transaction, reference)
        .map_err(ConversationRepositoryError::ArtifactReference)
}

fn edit_result(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    message_id: MessageId,
    revision_id: MessageRevisionId,
    asset_reference_deltas: Vec<AssetReferenceDelta>,
) -> Result<EditResult, ConversationRepositoryError> {
    let revision = transaction
        .query_row(
            "SELECT conversation_id, id, message_id, branch_id, sequence, parts_json, authored_at, provider_replay_artifact_id, provider_replay_retention, source_turn_id FROM conversation_message_revisions WHERE conversation_id = ?1 AND id = ?2",
            params![conversation_id.to_string(), revision_id.to_string()],
            |row| {
                conversation_query::hydrate_revision_row(transaction, row)
                    .map_err(|_| rusqlite::Error::InvalidQuery)
            },
        )
        .optional()
        .map_err(slice::db)?
        .ok_or(ConversationRepositoryError::NotFound)?;
    if revision.message_id != message_id {
        return Err(ConversationRepositoryError::Conflict);
    }
    Ok(EditResult {
        message: load_message(transaction, conversation_id, message_id)?,
        revision,
        asset_reference_deltas,
    })
}

/// Descendants of one message on its own branch. The policy is branch-local
/// by contract: a fork's subtree belongs to that fork, not to this tombstone.
fn branch_descendants(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    branch_id: ConversationBranchId,
    message_id: MessageId,
) -> Result<Vec<MessageId>, ConversationRepositoryError> {
    let mut statement = transaction
        .prepare(
            "WITH RECURSIVE subtree(id) AS (
                 SELECT id FROM conversation_messages
                 WHERE conversation_id = ?1 AND branch_id = ?2 AND parent_message_id = ?3
                 UNION
                 SELECT message.id FROM conversation_messages AS message
                 JOIN subtree ON message.parent_message_id = subtree.id
                 WHERE message.conversation_id = ?1 AND message.branch_id = ?2
             )
             SELECT subtree.id FROM subtree
             JOIN conversation_messages AS message
               ON message.conversation_id = ?1 AND message.id = subtree.id
             WHERE message.visibility <> 'tombstoned'
             ORDER BY message.timeline_ordinal",
        )
        .map_err(slice::db)?;
    let rows: Vec<String> = statement
        .query_map(
            params![
                conversation_id.to_string(),
                branch_id.to_string(),
                message_id.to_string()
            ],
            |row| row.get(0),
        )
        .map_err(slice::db)?
        .collect::<rusqlite::Result<_>>()
        .map_err(slice::db)?;
    drop(statement);
    rows.into_iter().map(slice::parse_id).collect()
}

fn read_current_settings(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
) -> Result<Option<lettuce_conversations::CurrentConversationSettings>, ConversationRepositoryError>
{
    transaction
        .query_row(
            "SELECT revision, author_note, author_note_provenance, memory_json, memory_provenance, model_override_json, model_provenance, voice_json, voice_provenance FROM conversation_settings WHERE conversation_id = ?1",
            [conversation_id.to_string()],
            slice::read_settings,
        )
        .optional()
        .map_err(slice::db)
}

fn write_settings(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    settings: &lettuce_conversations::CurrentConversationSettings,
    create: bool,
    now: TimestampMillis,
) -> Result<(), ConversationRepositoryError> {
    let revision = slice::sql_revision(settings.revision)?;
    let memory = settings.memory.as_ref().map(slice::encode).transpose()?;
    let model = settings
        .model_override
        .as_ref()
        .map(slice::encode)
        .transpose()?;
    let voice = settings.voice.as_ref().map(slice::encode).transpose()?;
    if create {
        transaction
            .execute(
                "INSERT INTO conversation_settings (conversation_id, revision, author_note, author_note_provenance, memory_json, memory_provenance, model_override_json, model_provenance, voice_json, voice_provenance, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)",
                params![
                    conversation_id.to_string(),
                    revision,
                    settings.author_note,
                    slice::provenance_name(settings.author_note_provenance),
                    memory,
                    slice::provenance_name(settings.memory_provenance),
                    model,
                    slice::provenance_name(settings.model_provenance),
                    voice,
                    slice::provenance_name(settings.voice_provenance),
                    now.get(),
                ],
            )
            .map_err(kernel::map_constraint)?;
        return Ok(());
    }
    let changed = transaction
        .execute(
            "UPDATE conversation_settings SET revision = ?2, author_note = ?3, author_note_provenance = ?4, memory_json = ?5, memory_provenance = ?6, model_override_json = ?7, model_provenance = ?8, voice_json = ?9, voice_provenance = ?10, updated_at = ?11 WHERE conversation_id = ?1 AND revision = ?12",
            params![
                conversation_id.to_string(),
                revision,
                settings.author_note,
                slice::provenance_name(settings.author_note_provenance),
                memory,
                slice::provenance_name(settings.memory_provenance),
                model,
                slice::provenance_name(settings.model_provenance),
                voice,
                slice::provenance_name(settings.voice_provenance),
                now.get(),
                revision - 1,
            ],
        )
        .map_err(kernel::map_constraint)?;
    if changed == 0 {
        return Err(ConversationRepositoryError::Conflict);
    }
    Ok(())
}

fn memory_revision_ids(turn: &GenerationTurn) -> Vec<lettuce_types::MemoryRevisionId> {
    turn.memory
        .as_ref()
        .map(|memory| memory.revision_id)
        .into_iter()
        .collect()
}

fn load_turn(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    turn_id: GenerationTurnId,
) -> Result<GenerationTurn, ConversationRepositoryError> {
    let sql = format!(
        "{} WHERE conversation_id = ?1 AND id = ?2",
        conversation_query::turn_select_sql()
    );
    transaction
        .query_row(
            &sql,
            params![conversation_id.to_string(), turn_id.to_string()],
            |row| {
                conversation_query::hydrate_turn_row(transaction, row)
                    .map_err(|_| rusqlite::Error::InvalidQuery)
            },
        )
        .optional()
        .map_err(slice::db)?
        .ok_or(ConversationRepositoryError::NotFound)
}

fn load_attempt(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    turn_id: GenerationTurnId,
    attempt_id: GenerationAttemptId,
) -> Result<GenerationAttempt, ConversationRepositoryError> {
    let (attempts, _) =
        conversation_query::hydrate_attempts(transaction, conversation_id, turn_id)?;
    attempts
        .into_iter()
        .find(|attempt| attempt.id == attempt_id)
        .ok_or(ConversationRepositoryError::NotFound)
}

/// The begin result carries the post-mutation conversation plus the turn and
/// its opening attempt, all read back from the rows that were just written.
fn begin_generation(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    turn_id: GenerationTurnId,
) -> Result<BeginGeneration, ConversationRepositoryError> {
    let conversation =
        slice::hydrate_conversation(transaction, conversation_id, || {})?.conversation;
    let turn = load_turn(transaction, conversation_id, turn_id)?;
    let attempt = turn
        .attempts
        .first()
        .cloned()
        .ok_or(ConversationRepositoryError::Storage)?;
    Ok(BeginGeneration {
        conversation,
        turn,
        attempt,
    })
}

fn replayed_turn(
    operation: &lettuce_conversations::OperationRecord,
) -> Result<GenerationTurnId, ConversationRepositoryError> {
    match operation.result {
        OperationResultRef::Turn(turn_id) => Ok(turn_id),
        _ => Err(ConversationRepositoryError::Storage),
    }
}

fn allocate_timeline_ordinal(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
) -> Result<i64, ConversationRepositoryError> {
    transaction
        .query_row(
            "UPDATE conversations SET next_timeline_ordinal = next_timeline_ordinal + 1 WHERE id = ?1 RETURNING next_timeline_ordinal - 1",
            [conversation_id.to_string()],
            |row| row.get(0),
        )
        .map_err(kernel::map_constraint)
}

/// Writes the user message, its first revision, and the media projection for
/// that revision, then advances the branch head onto it.
fn insert_user_message(
    transaction: &Transaction<'_>,
    command: &SendConversation,
    parent_message_id: Option<MessageId>,
    now: TimestampMillis,
) -> Result<(MessageId, MessageRevisionId), ConversationRepositoryError> {
    let message_id = MessageId::new();
    let revision_id = MessageRevisionId::new();
    let ordinal = allocate_timeline_ordinal(transaction, command.conversation_id)?;
    transaction
        .execute(
            "INSERT INTO conversation_messages (conversation_id, id, branch_id, parent_message_id, author_participant_id, role, timeline_ordinal, logical_time, effective_time, visibility, pinned, scene_edited, active_revision_id, active_candidate_id, revision, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8, ?9, ?10, ?11, ?12, NULL, 1, ?8, ?8)",
            params![
                command.conversation_id.to_string(),
                message_id.to_string(),
                command.branch_id.to_string(),
                parent_message_id.map(|id| id.to_string()),
                command.message.author_participant_id.map(|id| id.to_string()),
                kernel::message_role_name(command.message.role),
                ordinal,
                now.get(),
                kernel::message_visibility_name(command.message.visibility),
                i64::from(command.message.pinned),
                i64::from(command.message.scene_edited),
                revision_id.to_string(),
            ],
        )
        .map_err(kernel::map_constraint)?;
    transaction
        .execute(
            "INSERT INTO conversation_message_revisions (conversation_id, id, message_id, branch_id, sequence, parts_json, authored_at, source_turn_id, provider_replay_artifact_id, provider_replay_retention) VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6, NULL, NULL, NULL)",
            params![
                command.conversation_id.to_string(),
                revision_id.to_string(),
                message_id.to_string(),
                command.branch_id.to_string(),
                slice::encode(&command.message.parts)?,
                now.get(),
            ],
        )
        .map_err(kernel::map_constraint)?;
    for (part_ordinal, part) in command.message.parts.iter().enumerate() {
        let MessagePart::MediaAsset { asset_id, role } = part else {
            continue;
        };
        transaction
            .execute(
                "INSERT INTO revision_media_refs (conversation_id, message_revision_id, part_ordinal, asset_id, media_role, state, created_at) VALUES (?1, ?2, ?3, ?4, ?5, 'active', ?6)",
                params![
                    command.conversation_id.to_string(),
                    revision_id.to_string(),
                    i64::try_from(part_ordinal).map_err(|_| ConversationRepositoryError::Storage)?,
                    asset_id.to_string(),
                    conversation_creator::media_role_name(*role),
                    now.get(),
                ],
            )
            .map_err(kernel::map_constraint)?;
    }
    transaction
        .execute(
            "UPDATE conversation_branches SET head_message_id = ?1, updated_at = ?2 WHERE conversation_id = ?3 AND id = ?4",
            params![
                message_id.to_string(),
                now.get(),
                command.conversation_id.to_string(),
                command.branch_id.to_string(),
            ],
        )
        .map_err(kernel::map_constraint)?;
    Ok((message_id, revision_id))
}

/// Advances the turn revision, optionally applying a lifecycle transition in
/// the same statement. The column list stays narrow: the intent-immutability
/// trigger aborts an update that names an intent column, even unchanged.
fn advance_turn(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    turn_id: GenerationTurnId,
    status: Option<GenerationTurnStatus>,
    now: TimestampMillis,
) -> Result<(), ConversationRepositoryError> {
    let changed = match status {
        Some(status) => transaction
            .execute(
                "UPDATE conversation_turns SET status = ?3, revision = revision + 1, updated_at = ?4 WHERE conversation_id = ?1 AND id = ?2",
                params![
                    conversation_id.to_string(),
                    turn_id.to_string(),
                    kernel::generation_status_name(status),
                    now.get(),
                ],
            )
            .map_err(kernel::map_constraint)?,
        None => transaction
            .execute(
                "UPDATE conversation_turns SET revision = revision + 1, updated_at = ?3 WHERE conversation_id = ?1 AND id = ?2",
                params![
                    conversation_id.to_string(),
                    turn_id.to_string(),
                    now.get(),
                ],
            )
            .map_err(kernel::map_constraint)?,
    };
    if changed == 0 {
        return Err(ConversationRepositoryError::NotFound);
    }
    Ok(())
}

impl Database {
    /// Archive and restore are the same write with different endpoints, and
    /// the domain's own transition table decides which one is legal now.
    fn change_lifecycle(
        &self,
        conversation_id: ConversationId,
        expected_revision: Revision,
        operation: &OperationToken,
        kind: OperationKind,
        lifecycle: ConversationLifecycle,
        now: TimestampMillis,
    ) -> Result<
        lettuce_conversations::MutationCommit<lettuce_conversations::Conversation>,
        ConversationRepositoryError,
    > {
        kernel::run_mutation(
            self,
            conversation_id,
            kind,
            operation,
            now,
            |transaction, context| {
                let conversation = kernel::cas_conversation(
                    transaction,
                    context.conversation_id,
                    expected_revision,
                )?;
                if !conversation.lifecycle.can_transition_to(lifecycle) {
                    return Err(ConversationRepositoryError::Conflict);
                }
                transaction
                    .execute(
                        "UPDATE conversations SET lifecycle = ?2 WHERE id = ?1",
                        params![
                            context.conversation_id.to_string(),
                            slice::lifecycle_name(lifecycle),
                        ],
                    )
                    .map_err(kernel::map_constraint)?;
                let revision =
                    kernel::bump_conversation(transaction, context.conversation_id, context.now)?;
                Ok(kernel::Staged {
                    value: conversation_value(transaction, context.conversation_id)?,
                    result: OperationResultRef::Conversation(context.conversation_id),
                    events: vec![kernel::StagedEvent {
                        conversation_revision: revision,
                        at: context.now,
                        event: ConversationOutboxEvent::ConversationLifecycleChanged {
                            conversation_id: context.conversation_id,
                            lifecycle,
                            at: context.now,
                        },
                    }],
                })
            },
            |transaction, _| {
                let value = conversation_value(transaction, conversation_id)?;
                if value.lifecycle != lifecycle {
                    return Err(ConversationRepositoryError::Conflict);
                }
                Ok(value)
            },
        )
    }

    /// Checkpoints address a turn directly, so the owning conversation is
    /// resolved before the mutation transaction opens.
    fn conversation_for_turn(
        &self,
        turn_id: GenerationTurnId,
    ) -> Result<ConversationId, ConversationRepositoryError> {
        let connection = self
            .connection()
            .map_err(|_| ConversationRepositoryError::Storage)?;
        let mut statement = connection
            .prepare("SELECT conversation_id FROM conversation_turns WHERE id = ?1 LIMIT 2")
            .map_err(slice::db)?;
        let mut owners = Vec::new();
        for row in statement
            .query_map([turn_id.to_string()], |row| row.get::<_, String>(0))
            .map_err(slice::db)?
        {
            owners.push(row.map_err(slice::db)?);
        }
        drop(statement);
        drop(connection);
        if owners.len() > 1 {
            return Err(ConversationRepositoryError::Storage);
        }
        slice::parse_id(owners.pop().ok_or(ConversationRepositoryError::NotFound)?)
    }
}

/// Every port method is implemented here; the kernel owns the shared
/// transaction, idempotency and outbox order they all run through.
impl ConversationRepository for Database {
    fn artifact_store(&self) -> &dyn lettuce_conversations::ConversationArtifactStore {
        self
    }

    fn begin_send(
        &self,
        command: &SendConversation,
        now: TimestampMillis,
    ) -> Result<SendConversationResult, ConversationRepositoryError> {
        command
            .validate()
            .map_err(ConversationRepositoryError::Invalid)?;
        let turn_key = turn_key(OperationKind::Send, &command.operation)?;
        kernel::run_mutation(
            self,
            command.conversation_id,
            OperationKind::Send,
            &command.operation,
            now,
            |transaction, context| {
                let conversation = kernel::cas_conversation(
                    transaction,
                    context.conversation_id,
                    command.expected_revision,
                )?;
                kernel::require_active(&conversation)?;
                require_active_branch(transaction, context.conversation_id, command.branch_id)?;
                require_no_live_turn(transaction, context.conversation_id)?;
                require_speaker_shape(
                    transaction,
                    context.conversation_id,
                    None,
                    command.swap_roles,
                )?;
                let parent_message_id =
                    branch_parent(transaction, context.conversation_id, command.branch_id)?;
                let (message_id, revision_id) =
                    insert_user_message(transaction, command, parent_message_id, context.now)?;
                let turn_id = GenerationTurnId::new();
                insert_turn(
                    transaction,
                    &TurnDraft {
                        conversation_id: context.conversation_id,
                        turn_id,
                        branch_id: command.branch_id,
                        operation: GenerationOperation::Send,
                        input: GenerationInput::UserMessage { message_id },
                        target: GenerationTarget::NewAssistant {
                            message_id: MessageId::new(),
                            parent_message_id: Some(message_id),
                        },
                        idempotency_key: turn_key,
                        guidance: None,
                        model_override: None,
                        forced_speaker: None,
                        swap_roles: command.swap_roles,
                    },
                    context.now,
                )?;
                insert_first_attempt(transaction, context.conversation_id, turn_id)?;
                let revision =
                    kernel::bump_conversation(transaction, context.conversation_id, context.now)?;
                let value = begin_generation(transaction, context.conversation_id, turn_id)?;
                Ok(kernel::Staged {
                    value,
                    result: OperationResultRef::Turn(turn_id),
                    events: vec![kernel::StagedEvent {
                        conversation_revision: revision,
                        at: context.now,
                        event: lettuce_conversations::ConversationOutboxEvent::MessageCommitted {
                            conversation_id: context.conversation_id,
                            branch_id: command.branch_id,
                            message_id,
                            revision_id: Some(revision_id),
                            candidate_id: None,
                            at: context.now,
                        },
                    }],
                })
            },
            |transaction, operation| {
                begin_generation(
                    transaction,
                    command.conversation_id,
                    replayed_turn(operation)?,
                )
            },
        )
    }

    fn begin_continue(
        &self,
        command: &ContinueConversation,
        now: TimestampMillis,
    ) -> Result<ContinueConversationResult, ConversationRepositoryError> {
        let turn_key = turn_key(OperationKind::Continue, &command.operation)?;
        kernel::run_mutation(
            self,
            command.conversation_id,
            OperationKind::Continue,
            &command.operation,
            now,
            |transaction, context| {
                let conversation = kernel::cas_conversation(
                    transaction,
                    context.conversation_id,
                    command.expected_revision,
                )?;
                kernel::require_active(&conversation)?;
                require_active_branch(transaction, context.conversation_id, command.branch_id)?;
                require_no_live_turn(transaction, context.conversation_id)?;
                require_speaker_shape(
                    transaction,
                    context.conversation_id,
                    command.forced_speaker,
                    command.swap_roles,
                )?;
                let head_message_id =
                    branch_head(transaction, context.conversation_id, command.branch_id)?
                        .ok_or(ConversationRepositoryError::Conflict)?;
                let turn_id = GenerationTurnId::new();
                insert_turn(
                    transaction,
                    &TurnDraft {
                        conversation_id: context.conversation_id,
                        turn_id,
                        branch_id: command.branch_id,
                        operation: GenerationOperation::Continue,
                        input: GenerationInput::ExistingHead { head_message_id },
                        target: GenerationTarget::NewAssistant {
                            message_id: MessageId::new(),
                            parent_message_id: Some(head_message_id),
                        },
                        idempotency_key: turn_key,
                        guidance: None,
                        model_override: None,
                        forced_speaker: command.forced_speaker,
                        swap_roles: command.swap_roles,
                    },
                    context.now,
                )?;
                insert_first_attempt(transaction, context.conversation_id, turn_id)?;
                kernel::bump_conversation(transaction, context.conversation_id, context.now)?;
                let value = begin_generation(transaction, context.conversation_id, turn_id)?;
                Ok(kernel::Staged {
                    value,
                    result: OperationResultRef::Turn(turn_id),
                    events: Vec::new(),
                })
            },
            |transaction, operation| {
                begin_generation(
                    transaction,
                    command.conversation_id,
                    replayed_turn(operation)?,
                )
            },
        )
    }

    fn begin_regenerate(
        &self,
        command: &RegenerateCandidate,
        now: TimestampMillis,
    ) -> Result<RegenerateCandidateResult, ConversationRepositoryError> {
        lettuce_conversations::ConversationMutation::Regenerate(command.clone())
            .validate()
            .map_err(ConversationRepositoryError::Invalid)?;
        let turn_key = turn_key(OperationKind::Regenerate, &command.operation)?;
        kernel::run_mutation(
            self,
            command.conversation_id,
            OperationKind::Regenerate,
            &command.operation,
            now,
            |transaction, context| {
                let conversation = kernel::cas_conversation(
                    transaction,
                    context.conversation_id,
                    command.expected_revision,
                )?;
                kernel::require_active(&conversation)?;
                require_active_branch(transaction, context.conversation_id, command.branch_id)?;
                require_no_live_turn(transaction, context.conversation_id)?;
                cas_turn(
                    transaction,
                    context.conversation_id,
                    command.turn_id,
                    command.expected_turn_revision,
                )?;
                let owned_candidate: bool = transaction
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM conversation_message_candidates WHERE conversation_id = ?1 AND id = ?2 AND message_id = ?3 AND turn_id = ?4)",
                        params![
                            context.conversation_id.to_string(),
                            command.active_candidate_id.to_string(),
                            command.message_id.to_string(),
                            command.turn_id.to_string(),
                        ],
                        |row| row.get(0),
                    )
                    .map_err(slice::db)?;
                if !owned_candidate {
                    return Err(ConversationRepositoryError::Conflict);
                }
                require_speaker_shape(
                    transaction,
                    context.conversation_id,
                    command.forced_speaker,
                    command.swap_roles,
                )?;
                let aggregate =
                    slice::hydrate_conversation(transaction, context.conversation_id, || {})?;
                let (item, _) = transaction
                    .query_row(
                        MESSAGE_SELECT_SQL,
                        params![
                            context.conversation_id.to_string(),
                            command.message_id.to_string(),
                        ],
                        |row| {
                            conversation_query::message_row(transaction, row)
                                .map_err(|_| rusqlite::Error::InvalidQuery)
                        },
                    )
                    .optional()
                    .map_err(slice::db)?
                    .ok_or(ConversationRepositoryError::NotFound)?;
                let head = branch_head(transaction, context.conversation_id, command.branch_id)?;
                command
                    .validate_target_context(
                        &item.message,
                        aggregate.conversation.active_branch_id,
                        head,
                        &aggregate.conversation.participants,
                        aggregate.conversation.kind.is_group(),
                    )
                    .map_err(ConversationRepositoryError::Invalid)?;
                let turn_id = GenerationTurnId::new();
                insert_turn(
                    transaction,
                    &TurnDraft {
                        conversation_id: context.conversation_id,
                        turn_id,
                        branch_id: command.branch_id,
                        operation: GenerationOperation::Regenerate,
                        input: GenerationInput::ExistingCandidate {
                            message_id: command.message_id,
                            candidate_id: command.active_candidate_id,
                        },
                        target: GenerationTarget::ExistingCandidate {
                            message_id: command.message_id,
                            prior_candidate_id: command.active_candidate_id,
                        },
                        idempotency_key: turn_key,
                        guidance: command.guidance.clone(),
                        model_override: command
                            .model_override
                            .as_ref()
                            .map(slice::encode)
                            .transpose()?,
                        forced_speaker: command.forced_speaker,
                        swap_roles: command.swap_roles,
                    },
                    context.now,
                )?;
                insert_first_attempt(transaction, context.conversation_id, turn_id)?;
                kernel::bump_conversation(transaction, context.conversation_id, context.now)?;
                let value = begin_generation(transaction, context.conversation_id, turn_id)?;
                Ok(kernel::Staged {
                    value,
                    result: OperationResultRef::Turn(turn_id),
                    events: Vec::new(),
                })
            },
            |transaction, operation| {
                begin_generation(
                    transaction,
                    command.conversation_id,
                    replayed_turn(operation)?,
                )
            },
        )
    }

    fn begin_retry(
        &self,
        command: &RetryGeneration,
        now: TimestampMillis,
    ) -> Result<RetryGenerationResult, ConversationRepositoryError> {
        let turn_key = turn_key(OperationKind::Retry, &command.operation)?;
        kernel::run_mutation(
            self,
            command.conversation_id,
            OperationKind::Retry,
            &command.operation,
            now,
            |transaction, context| {
                let conversation = kernel::cas_conversation(
                    transaction,
                    context.conversation_id,
                    command.expected_revision,
                )?;
                kernel::require_active(&conversation)?;
                require_active_branch(transaction, context.conversation_id, command.branch_id)?;
                require_no_live_turn(transaction, context.conversation_id)?;
                cas_turn(
                    transaction,
                    context.conversation_id,
                    command.turn_id,
                    command.expected_turn_revision,
                )?;
                let source: (String, String, String, Option<String>) = transaction
                    .query_row(
                        "SELECT status, branch_id, target_kind, target_parent_message_id FROM conversation_turns WHERE conversation_id = ?1 AND id = ?2",
                        params![
                            context.conversation_id.to_string(),
                            command.turn_id.to_string(),
                        ],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                    )
                    .optional()
                    .map_err(slice::db)?
                    .ok_or(ConversationRepositoryError::NotFound)?;
                let (status, branch_id, target_kind, target_parent_message_id) = source;
                if branch_id != command.branch_id.to_string()
                    || !matches!(status.as_str(), "failed" | "cancelled")
                {
                    return Err(ConversationRepositoryError::Conflict);
                }
                if target_kind == "new_assistant" {
                    let head =
                        branch_head(transaction, context.conversation_id, command.branch_id)?
                            .map(|id| id.to_string());
                    if head != target_parent_message_id {
                        return Err(ConversationRepositoryError::Conflict);
                    }
                }
                let turn_id = GenerationTurnId::new();
                transaction
                    .execute(
                        "INSERT INTO conversation_turns (conversation_id, id, branch_id, operation, input_kind, user_message_id, head_message_id, candidate_message_id, candidate_id, idempotency_key, correlation_id, status, target_kind, target_message_id, target_parent_message_id, target_prior_candidate_id, retry_of_turn_id, guidance, requested_model_override_json, forced_speaker_participant_id, swap_roles, revision, created_at, updated_at) SELECT conversation_id, ?3, branch_id, operation, input_kind, user_message_id, head_message_id, candidate_message_id, candidate_id, ?4, correlation_id, 'created', target_kind, target_message_id, target_parent_message_id, target_prior_candidate_id, id, guidance, requested_model_override_json, forced_speaker_participant_id, swap_roles, 1, ?5, ?5 FROM conversation_turns WHERE conversation_id = ?1 AND id = ?2",
                        params![
                            context.conversation_id.to_string(),
                            command.turn_id.to_string(),
                            turn_id.to_string(),
                            turn_key,
                            context.now.get(),
                        ],
                    )
                    .map_err(kernel::map_constraint)?;
                insert_first_attempt(transaction, context.conversation_id, turn_id)?;
                kernel::bump_conversation(transaction, context.conversation_id, context.now)?;
                let value = begin_generation(transaction, context.conversation_id, turn_id)?;
                Ok(kernel::Staged {
                    value,
                    result: OperationResultRef::Turn(turn_id),
                    events: Vec::new(),
                })
            },
            |transaction, operation| {
                begin_generation(
                    transaction,
                    command.conversation_id,
                    replayed_turn(operation)?,
                )
            },
        )
    }

    fn append_event(
        &self,
        turn_id: GenerationTurnId,
        expected_turn_revision: Revision,
        operation: &OperationToken,
        event: GenerationCheckpointEnvelope,
        now: TimestampMillis,
    ) -> Result<lettuce_conversations::AppendCheckpointResult, ConversationRepositoryError> {
        if event.turn_id != turn_id {
            return Err(invalid("checkpoint.turn_id"));
        }
        event
            .event
            .validate()
            .map_err(ConversationRepositoryError::Invalid)?;
        let conversation_id = self.conversation_for_turn(turn_id)?;
        kernel::run_mutation(
            self,
            conversation_id,
            OperationKind::Checkpoint,
            operation,
            now,
            |transaction, context| {
                require_settleable_conversation(transaction, context.conversation_id)?;
                cas_turn(
                    transaction,
                    context.conversation_id,
                    turn_id,
                    expected_turn_revision,
                )?;
                let attempt: Option<(Option<String>, String)> = transaction
                    .query_row(
                        "SELECT job_id, status FROM generation_attempts WHERE conversation_id = ?1 AND turn_id = ?2 AND id = ?3",
                        params![
                            context.conversation_id.to_string(),
                            turn_id.to_string(),
                            event.attempt_id.to_string(),
                        ],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .optional()
                    .map_err(slice::db)?;
                let (attempt_job_id, attempt_status) =
                    attempt.ok_or(ConversationRepositoryError::NotFound)?;
                if matches!(
                    attempt_status.as_str(),
                    "succeeded" | "failed" | "cancelled" | "interrupted"
                ) {
                    return Err(ConversationRepositoryError::Conflict);
                }
                if attempt_job_id != event.job_id.map(|id| id.to_string()) {
                    return Err(ConversationRepositoryError::Conflict);
                }
                let stage = match event.event {
                    GenerationCheckpointEvent::Stage { status } => {
                        if !is_appendable_stage(status) {
                            return Err(ConversationRepositoryError::Conflict);
                        }
                        Some(status)
                    }
                    _ => None,
                };
                let sequence = i64::try_from(event.sequence)
                    .map_err(|_| ConversationRepositoryError::Storage)?;
                if sequence < 1 {
                    return Err(invalid("checkpoint.sequence"));
                }
                transaction
                    .execute(
                        "INSERT INTO generation_checkpoints (conversation_id, turn_id, attempt_id, sequence, job_id, correlation_id, event_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                        params![
                            context.conversation_id.to_string(),
                            turn_id.to_string(),
                            event.attempt_id.to_string(),
                            sequence,
                            event.job_id.map(|id| id.to_string()),
                            event.correlation_id.map(|id| id.to_string()),
                            slice::encode(&event.event)?,
                            context.now.get(),
                        ],
                    )
                    .map_err(kernel::map_constraint)?;
                advance_turn(
                    transaction,
                    context.conversation_id,
                    turn_id,
                    stage,
                    context.now,
                )?;
                let value = load_turn(transaction, context.conversation_id, turn_id)?;
                Ok(kernel::Staged {
                    value,
                    result: OperationResultRef::Turn(turn_id),
                    events: Vec::new(),
                })
            },
            |transaction, operation| {
                let replayed = replayed_turn(operation)?;
                if replayed != turn_id {
                    return Err(ConversationRepositoryError::Conflict);
                }
                load_turn(transaction, conversation_id, replayed)
            },
        )
    }

    fn attach_attempt_job(
        &self,
        command: &AttachAttemptJob,
        now: TimestampMillis,
    ) -> Result<AttachAttemptJobResult, ConversationRepositoryError> {
        command
            .validate()
            .map_err(ConversationRepositoryError::Invalid)?;
        kernel::run_mutation(
            self,
            command.conversation_id,
            OperationKind::AttachJob,
            &command.operation,
            now,
            |transaction, context| {
                let conversation = kernel::cas_conversation(
                    transaction,
                    context.conversation_id,
                    command.expected_revision,
                )?;
                require_settleable(conversation.lifecycle)?;
                cas_turn(
                    transaction,
                    context.conversation_id,
                    command.turn_id,
                    command.expected_turn_revision,
                )?;
                let attached = transaction
                    .execute(
                        "UPDATE generation_attempts SET job_id = ?4 WHERE conversation_id = ?1 AND turn_id = ?2 AND id = ?3 AND job_id IS NULL AND status IN ('created', 'preparing')",
                        params![
                            context.conversation_id.to_string(),
                            command.turn_id.to_string(),
                            command.attempt_id.to_string(),
                            command.job_id.to_string(),
                        ],
                    )
                    .map_err(map_attach_constraint)?;
                if attached == 0 {
                    let current: Option<(Option<String>, String)> = transaction
                        .query_row(
                            "SELECT job_id, status FROM generation_attempts WHERE conversation_id = ?1 AND turn_id = ?2 AND id = ?3",
                            params![
                                context.conversation_id.to_string(),
                                command.turn_id.to_string(),
                                command.attempt_id.to_string(),
                            ],
                            |row| Ok((row.get(0)?, row.get(1)?)),
                        )
                        .optional()
                        .map_err(slice::db)?;
                    return match current {
                        None => Err(ConversationRepositoryError::NotFound),
                        Some((Some(_), _)) => Err(ConversationRepositoryError::JobAlreadyAttached),
                        Some((None, _)) => Err(ConversationRepositoryError::Conflict),
                    };
                }
                kernel::bump_conversation(transaction, context.conversation_id, context.now)?;
                let value = load_attempt(
                    transaction,
                    context.conversation_id,
                    command.turn_id,
                    command.attempt_id,
                )?;
                Ok(kernel::Staged {
                    value,
                    result: OperationResultRef::Turn(command.turn_id),
                    events: Vec::new(),
                })
            },
            |transaction, operation| {
                let replayed = replayed_turn(operation)?;
                if replayed != command.turn_id {
                    return Err(ConversationRepositoryError::Conflict);
                }
                let attempt = load_attempt(
                    transaction,
                    command.conversation_id,
                    replayed,
                    command.attempt_id,
                )?;
                if attempt.job_id != Some(command.job_id) {
                    return Err(ConversationRepositoryError::Conflict);
                }
                Ok(attempt)
            },
        )
    }

    fn finalize_generation(
        &self,
        turn_id: GenerationTurnId,
        attempt_id: GenerationAttemptId,
        expected_conversation_revision: Revision,
        expected_turn_revision: Revision,
        operation: &OperationToken,
        draft: FinalizationDraft,
        usage_event_id: UsageEventId,
        now: TimestampMillis,
    ) -> Result<GenerationFinalizationResult, ConversationRepositoryError> {
        validate_finalization_draft(&draft)?;
        let conversation_id = self.conversation_for_turn(turn_id)?;
        kernel::run_mutation(
            self,
            conversation_id,
            OperationKind::Finalize,
            operation,
            now,
            |transaction, context| {
                let conversation = kernel::cas_conversation(
                    transaction,
                    context.conversation_id,
                    expected_conversation_revision,
                )?;
                require_settleable(conversation.lifecycle)?;
                cas_turn(
                    transaction,
                    context.conversation_id,
                    turn_id,
                    expected_turn_revision,
                )?;
                if let Some(reference) = &draft.replay {
                    conversation_artifact_adapter::verify_replay_in_transaction(
                        transaction,
                        reference,
                    )
                    .map_err(ConversationRepositoryError::ArtifactReference)?;
                }
                let turn = load_turn(transaction, context.conversation_id, turn_id)?;
                if !matches!(
                    turn.status,
                    GenerationTurnStatus::Running | GenerationTurnStatus::Finalizing
                ) {
                    return Err(ConversationRepositoryError::Conflict);
                }
                require_live_attempt(transaction, context.conversation_id, turn_id, attempt_id)?;
                require_no_other_live_attempt(
                    transaction,
                    context.conversation_id,
                    turn_id,
                    attempt_id,
                )?;
                let author = resolve_candidate_author(transaction, context.conversation_id, &turn)?;
                let candidate_id = MessageCandidateId::new();
                let (message_id, prior_candidate_id) = match turn.target {
                    GenerationTarget::NewAssistant {
                        message_id,
                        parent_message_id,
                    } => {
                        insert_assistant_message(
                            transaction,
                            context.conversation_id,
                            turn.branch_id,
                            message_id,
                            parent_message_id,
                            author,
                            candidate_id,
                            context.now,
                        )?;
                        (message_id, None)
                    }
                    GenerationTarget::ExistingCandidate {
                        message_id,
                        prior_candidate_id,
                    } => (message_id, Some(prior_candidate_id)),
                };
                insert_candidate(
                    transaction,
                    context.conversation_id,
                    turn.branch_id,
                    &draft,
                    CandidateIdentity {
                        candidate_id,
                        message_id,
                        turn_id,
                        attempt_id,
                        author,
                    },
                    context.now,
                )?;
                if let Some(prior_candidate_id) = prior_candidate_id {
                    transaction
                        .execute(
                            "UPDATE candidate_media_refs SET state = 'historical' WHERE conversation_id = ?1 AND candidate_id = ?2 AND state = 'active'",
                            params![
                                context.conversation_id.to_string(),
                                prior_candidate_id.to_string(),
                            ],
                        )
                        .map_err(kernel::map_constraint)?;
                    let flipped = transaction
                        .execute(
                            "UPDATE conversation_messages SET active_candidate_id = ?3, author_participant_id = ?4, revision = revision + 1, updated_at = ?5 WHERE conversation_id = ?1 AND id = ?2 AND active_candidate_id = ?6",
                            params![
                                context.conversation_id.to_string(),
                                message_id.to_string(),
                                candidate_id.to_string(),
                                author.to_string(),
                                context.now.get(),
                                prior_candidate_id.to_string(),
                            ],
                        )
                        .map_err(kernel::map_constraint)?;
                    if flipped == 0 {
                        return Err(ConversationRepositoryError::Conflict);
                    }
                } else {
                    let parent = match turn.target {
                        GenerationTarget::NewAssistant {
                            parent_message_id, ..
                        } => parent_message_id,
                        GenerationTarget::ExistingCandidate { .. } => {
                            return Err(ConversationRepositoryError::Storage);
                        }
                    };
                    let advanced = transaction
                        .execute(
                            "UPDATE conversation_branches SET head_message_id = ?1, updated_at = ?2 WHERE conversation_id = ?3 AND id = ?4 AND head_message_id IS ?5",
                            params![
                                message_id.to_string(),
                                context.now.get(),
                                context.conversation_id.to_string(),
                                turn.branch_id.to_string(),
                                parent.map(|id| id.to_string()),
                            ],
                        )
                        .map_err(kernel::map_constraint)?;
                    if advanced == 0 {
                        return Err(ConversationRepositoryError::Conflict);
                    }
                }
                settle_attempt_terminal(
                    transaction,
                    context.conversation_id,
                    turn_id,
                    attempt_id,
                    AttemptSettlement {
                        outcome: GenerationAttemptStatus::Succeeded,
                        failure: None,
                        usage_event_id,
                    },
                    context.now,
                )?;
                if turn.status == GenerationTurnStatus::Running {
                    advance_turn(
                        transaction,
                        context.conversation_id,
                        turn_id,
                        Some(GenerationTurnStatus::Finalizing),
                        context.now,
                    )?;
                }
                succeed_turn(
                    transaction,
                    context.conversation_id,
                    turn_id,
                    candidate_id,
                    context.now,
                )?;
                let revision =
                    kernel::bump_conversation(transaction, context.conversation_id, context.now)?;
                let mut owned_deltas = vec![(
                    candidate_id,
                    candidate_deltas(transaction, context.conversation_id, candidate_id)?,
                )];
                if let Some(prior_candidate_id) = prior_candidate_id {
                    owned_deltas.push((
                        prior_candidate_id,
                        candidate_deltas(transaction, context.conversation_id, prior_candidate_id)?,
                    ));
                }
                let value = finalization_value(
                    transaction,
                    context.conversation_id,
                    turn_id,
                    candidate_id,
                    usage_event_id,
                    owned_deltas
                        .iter()
                        .flat_map(|(_, deltas)| deltas.iter().cloned())
                        .collect(),
                )?;
                let mut events = vec![kernel::StagedEvent {
                    conversation_revision: revision,
                    at: context.now,
                    event: ConversationOutboxEvent::TurnFinalized {
                        conversation_id: context.conversation_id,
                        branch_id: turn.branch_id,
                        turn_id,
                        attempt_id,
                        message_id,
                        candidate_id,
                        revision_id: None,
                        effective_time: context.now,
                        usage_event_id,
                        used_memory_revision_ids: memory_revision_ids(&turn),
                    },
                }];
                if prior_candidate_id.is_none() {
                    events.push(kernel::StagedEvent {
                        conversation_revision: revision,
                        at: context.now,
                        event: ConversationOutboxEvent::MessageCommitted {
                            conversation_id: context.conversation_id,
                            branch_id: turn.branch_id,
                            message_id,
                            revision_id: None,
                            candidate_id: Some(candidate_id),
                            at: context.now,
                        },
                    });
                }
                for (owner, changes) in owned_deltas {
                    if changes.is_empty() {
                        continue;
                    }
                    events.push(kernel::StagedEvent {
                        conversation_revision: revision,
                        at: context.now,
                        event: ConversationOutboxEvent::AssetReferencesChanged {
                            conversation_id: context.conversation_id,
                            message_revision_id: None,
                            candidate_id: Some(owner),
                            changes,
                            at: context.now,
                        },
                    });
                }
                Ok(kernel::Staged {
                    value,
                    result: OperationResultRef::Candidate(candidate_id),
                    events,
                })
            },
            |transaction, operation| {
                let OperationResultRef::Candidate(candidate_id) = operation.result else {
                    return Err(ConversationRepositoryError::Storage);
                };
                let deltas = recorded_deltas(transaction, conversation_id, operation)?;
                let value = finalization_value(
                    transaction,
                    conversation_id,
                    turn_id,
                    candidate_id,
                    usage_event_id,
                    deltas,
                )?;
                if value.turn.id != turn_id {
                    return Err(ConversationRepositoryError::Conflict);
                }
                let settled = value
                    .turn
                    .attempts
                    .iter()
                    .find(|attempt| attempt.id == value.candidate.attempt_id)
                    .ok_or(ConversationRepositoryError::Storage)?;
                if settled.usage_event_id != Some(usage_event_id) {
                    return Err(ConversationRepositoryError::Conflict);
                }
                Ok(value)
            },
        )
    }

    fn fail_generation(
        &self,
        turn_id: GenerationTurnId,
        attempt_id: GenerationAttemptId,
        expected_conversation_revision: Revision,
        expected_turn_revision: Revision,
        operation: &OperationToken,
        failure: GenerationFailureCode,
        usage_event_id: UsageEventId,
        now: TimestampMillis,
    ) -> Result<GenerationFailureResult, ConversationRepositoryError> {
        let conversation_id = self.conversation_for_turn(turn_id)?;
        kernel::run_mutation(
            self,
            conversation_id,
            OperationKind::Fail,
            operation,
            now,
            |transaction, context| {
                let turn = settle_preamble(
                    transaction,
                    context,
                    turn_id,
                    attempt_id,
                    expected_conversation_revision,
                    expected_turn_revision,
                )?;
                require_transition(turn.status, GenerationTurnStatus::Failed)?;
                settle_attempt_terminal(
                    transaction,
                    context.conversation_id,
                    turn_id,
                    attempt_id,
                    AttemptSettlement {
                        outcome: GenerationAttemptStatus::Failed,
                        failure: Some(failure),
                        usage_event_id,
                    },
                    context.now,
                )?;
                fail_turn(
                    transaction,
                    context.conversation_id,
                    turn_id,
                    failure,
                    context.now,
                )?;
                let revision =
                    kernel::bump_conversation(transaction, context.conversation_id, context.now)?;
                let settled = load_turn(transaction, context.conversation_id, turn_id)?;
                let events = vec![kernel::StagedEvent {
                    conversation_revision: revision,
                    at: context.now,
                    event: ConversationOutboxEvent::TurnFailed {
                        conversation_id: context.conversation_id,
                        branch_id: settled.branch_id,
                        turn_id,
                        attempt_id,
                        usage_event_id,
                        used_memory_revision_ids: memory_revision_ids(&settled),
                        at: context.now,
                    },
                }];
                Ok(kernel::Staged {
                    value: GenerationFailure {
                        turn: settled,
                        failure,
                        usage_event_id,
                    },
                    result: OperationResultRef::Turn(turn_id),
                    events,
                })
            },
            |transaction, operation| {
                let turn = replayed_settlement(transaction, conversation_id, operation, turn_id)?;
                if turn.failure != Some(failure) {
                    return Err(ConversationRepositoryError::Conflict);
                }
                Ok(GenerationFailure {
                    turn,
                    failure,
                    usage_event_id,
                })
            },
        )
    }

    fn interrupt_generation(
        &self,
        turn_id: GenerationTurnId,
        attempt_id: GenerationAttemptId,
        expected_conversation_revision: Revision,
        expected_turn_revision: Revision,
        operation: &OperationToken,
        usage_event_id: UsageEventId,
        now: TimestampMillis,
    ) -> Result<GenerationInterruptionResult, ConversationRepositoryError> {
        let conversation_id = self.conversation_for_turn(turn_id)?;
        kernel::run_mutation(
            self,
            conversation_id,
            OperationKind::Interrupt,
            operation,
            now,
            |transaction, context| {
                let turn = settle_preamble(
                    transaction,
                    context,
                    turn_id,
                    attempt_id,
                    expected_conversation_revision,
                    expected_turn_revision,
                )?;
                require_transition(turn.status, GenerationTurnStatus::Interrupted)?;
                settle_attempt_terminal(
                    transaction,
                    context.conversation_id,
                    turn_id,
                    attempt_id,
                    AttemptSettlement {
                        outcome: GenerationAttemptStatus::Interrupted,
                        failure: None,
                        usage_event_id,
                    },
                    context.now,
                )?;
                advance_turn(
                    transaction,
                    context.conversation_id,
                    turn_id,
                    Some(GenerationTurnStatus::Interrupted),
                    context.now,
                )?;
                kernel::bump_conversation(transaction, context.conversation_id, context.now)?;
                Ok(kernel::Staged {
                    value: load_turn(transaction, context.conversation_id, turn_id)?,
                    result: OperationResultRef::Turn(turn_id),
                    events: Vec::new(),
                })
            },
            |transaction, operation| {
                replayed_settlement(transaction, conversation_id, operation, turn_id)
            },
        )
    }

    /// Cancellation is two mutations under one operation kind. The request and
    /// the settlement carry different tokens by contract, so the operations
    /// `UNIQUE (conversation, kind, key)` still separates them.
    fn request_cancellation(
        &self,
        command: &CancelGeneration,
        now: TimestampMillis,
    ) -> Result<RequestCancellationResult, ConversationRepositoryError> {
        command
            .validate()
            .map_err(ConversationRepositoryError::Invalid)?;
        kernel::run_mutation(
            self,
            command.conversation_id,
            OperationKind::Cancel,
            &command.operation,
            now,
            |transaction, context| {
                let conversation = kernel::cas_conversation(
                    transaction,
                    context.conversation_id,
                    command.expected_revision,
                )?;
                require_settleable(conversation.lifecycle)?;
                cas_turn(
                    transaction,
                    context.conversation_id,
                    command.turn_id,
                    command.expected_turn_revision,
                )?;
                let turn = load_turn(transaction, context.conversation_id, command.turn_id)?;
                require_transition(turn.status, GenerationTurnStatus::CancellationRequested)?;
                require_live_attempt(
                    transaction,
                    context.conversation_id,
                    command.turn_id,
                    command.attempt_id,
                )?;
                advance_turn(
                    transaction,
                    context.conversation_id,
                    command.turn_id,
                    Some(GenerationTurnStatus::CancellationRequested),
                    context.now,
                )?;
                let revision =
                    kernel::bump_conversation(transaction, context.conversation_id, context.now)?;
                let events = vec![kernel::StagedEvent {
                    conversation_revision: revision,
                    at: context.now,
                    event: ConversationOutboxEvent::TurnCancellationRequested {
                        conversation_id: context.conversation_id,
                        branch_id: turn.branch_id,
                        turn_id: command.turn_id,
                        attempt_id: command.attempt_id,
                        at: context.now,
                    },
                }];
                Ok(kernel::Staged {
                    value: load_turn(transaction, context.conversation_id, command.turn_id)?,
                    result: OperationResultRef::Turn(command.turn_id),
                    events,
                })
            },
            |transaction, operation| {
                let turn = replayed_settlement(
                    transaction,
                    command.conversation_id,
                    operation,
                    command.turn_id,
                )?;
                if !matches!(
                    turn.status,
                    GenerationTurnStatus::CancellationRequested | GenerationTurnStatus::Cancelled
                ) {
                    return Err(ConversationRepositoryError::Conflict);
                }
                Ok(turn)
            },
        )
    }

    /// Only `cancellation_requested` and `created` settle here. Migration 0008
    /// also permits finalizing, interrupted and recovering to reach cancelled,
    /// but those edges are reserved: interrupted and recovering settle through
    /// fail or recover, and finalizing's cancel arrives with a later slice.
    fn settle_cancellation(
        &self,
        command: &SettleCancellation,
        now: TimestampMillis,
    ) -> Result<SettleCancellationResult, ConversationRepositoryError> {
        command
            .validate()
            .map_err(ConversationRepositoryError::Invalid)?;
        kernel::run_mutation(
            self,
            command.conversation_id,
            OperationKind::Cancel,
            &command.operation,
            now,
            |transaction, context| {
                let conversation = kernel::cas_conversation(
                    transaction,
                    context.conversation_id,
                    command.expected_revision,
                )?;
                require_settleable(conversation.lifecycle)?;
                cas_turn(
                    transaction,
                    context.conversation_id,
                    command.turn_id,
                    command.expected_turn_revision,
                )?;
                let turn = load_turn(transaction, context.conversation_id, command.turn_id)?;
                if !matches!(
                    turn.status,
                    GenerationTurnStatus::CancellationRequested | GenerationTurnStatus::Created
                ) {
                    return Err(ConversationRepositoryError::Conflict);
                }
                require_live_attempt(
                    transaction,
                    context.conversation_id,
                    command.turn_id,
                    command.attempt_id,
                )?;
                require_no_other_live_attempt(
                    transaction,
                    context.conversation_id,
                    command.turn_id,
                    command.attempt_id,
                )?;
                settle_attempt_terminal(
                    transaction,
                    context.conversation_id,
                    command.turn_id,
                    command.attempt_id,
                    AttemptSettlement {
                        outcome: GenerationAttemptStatus::Cancelled,
                        failure: None,
                        usage_event_id: command.usage_event_id,
                    },
                    context.now,
                )?;
                advance_turn(
                    transaction,
                    context.conversation_id,
                    command.turn_id,
                    Some(GenerationTurnStatus::Cancelled),
                    context.now,
                )?;
                let revision =
                    kernel::bump_conversation(transaction, context.conversation_id, context.now)?;
                let settled = load_turn(transaction, context.conversation_id, command.turn_id)?;
                let events = vec![kernel::StagedEvent {
                    conversation_revision: revision,
                    at: context.now,
                    event: ConversationOutboxEvent::TurnCancelled {
                        conversation_id: context.conversation_id,
                        branch_id: settled.branch_id,
                        turn_id: command.turn_id,
                        attempt_id: command.attempt_id,
                        usage_event_id: command.usage_event_id,
                        used_memory_revision_ids: memory_revision_ids(&settled),
                        at: context.now,
                    },
                }];
                Ok(kernel::Staged {
                    value: GenerationCancellation {
                        turn: settled,
                        attempt_id: command.attempt_id,
                        usage_event_id: command.usage_event_id,
                    },
                    result: OperationResultRef::Turn(command.turn_id),
                    events,
                })
            },
            |transaction, operation| {
                let turn = replayed_settlement(
                    transaction,
                    command.conversation_id,
                    operation,
                    command.turn_id,
                )?;
                if turn.status != GenerationTurnStatus::Cancelled {
                    return Err(ConversationRepositoryError::Conflict);
                }
                let settled = turn
                    .attempts
                    .iter()
                    .find(|attempt| attempt.id == command.attempt_id)
                    .ok_or(ConversationRepositoryError::Conflict)?;
                if settled.status != GenerationAttemptStatus::Cancelled
                    || settled.usage_event_id != Some(command.usage_event_id)
                {
                    return Err(ConversationRepositoryError::Conflict);
                }
                Ok(GenerationCancellation {
                    turn,
                    attempt_id: command.attempt_id,
                    usage_event_id: command.usage_event_id,
                })
            },
        )
    }

    fn recover_generation(
        &self,
        turn_id: GenerationTurnId,
        attempt_id: GenerationAttemptId,
        expected_conversation_revision: Revision,
        expected_turn_revision: Revision,
        operation: &OperationToken,
        now: TimestampMillis,
    ) -> Result<GenerationRecoveryResult, ConversationRepositoryError> {
        let conversation_id = self.conversation_for_turn(turn_id)?;
        kernel::run_mutation(
            self,
            conversation_id,
            OperationKind::Recover,
            operation,
            now,
            |transaction, context| {
                let conversation = kernel::cas_conversation(
                    transaction,
                    context.conversation_id,
                    expected_conversation_revision,
                )?;
                require_settleable(conversation.lifecycle)?;
                cas_turn(
                    transaction,
                    context.conversation_id,
                    turn_id,
                    expected_turn_revision,
                )?;
                let interrupted = load_turn(transaction, context.conversation_id, turn_id)?;
                if interrupted.status != GenerationTurnStatus::Interrupted {
                    return Err(ConversationRepositoryError::Conflict);
                }
                let previous = interrupted
                    .attempts
                    .last()
                    .cloned()
                    .ok_or(ConversationRepositoryError::Storage)?;
                if previous.id != attempt_id
                    || previous.status != GenerationAttemptStatus::Interrupted
                {
                    return Err(ConversationRepositoryError::Conflict);
                }
                let ordinal = previous
                    .ordinal
                    .checked_add(1)
                    .ok_or(ConversationRepositoryError::Storage)?;
                insert_attempt(
                    transaction,
                    context.conversation_id,
                    turn_id,
                    ordinal,
                    Some(previous.id),
                )?;
                advance_turn(
                    transaction,
                    context.conversation_id,
                    turn_id,
                    Some(GenerationTurnStatus::Recovering),
                    context.now,
                )?;
                kernel::bump_conversation(transaction, context.conversation_id, context.now)?;
                let value = recovery_value(transaction, context.conversation_id, turn_id)?;
                value
                    .validate_against(&previous, &interrupted)
                    .map_err(ConversationRepositoryError::Invalid)?;
                Ok(kernel::Staged {
                    value,
                    result: OperationResultRef::Turn(turn_id),
                    events: Vec::new(),
                })
            },
            |transaction, operation| {
                let replayed = replayed_turn(operation)?;
                if replayed != turn_id {
                    return Err(ConversationRepositoryError::Conflict);
                }
                recovery_value(transaction, conversation_id, turn_id)
            },
        )
    }

    fn choose_candidate(
        &self,
        command: &ChooseCandidate,
        now: TimestampMillis,
    ) -> Result<ChooseCandidateResult, ConversationRepositoryError> {
        lettuce_conversations::ConversationMutation::Choose(command.clone())
            .validate()
            .map_err(ConversationRepositoryError::Invalid)?;
        kernel::run_mutation(
            self,
            command.conversation_id,
            OperationKind::ChooseCandidate,
            &command.operation,
            now,
            |transaction, context| {
                let conversation = kernel::cas_conversation(
                    transaction,
                    context.conversation_id,
                    command.expected_revision,
                )?;
                kernel::require_active(&conversation)?;
                let author: Option<String> = transaction
                    .query_row(
                        "SELECT author_participant_id FROM conversation_message_candidates WHERE conversation_id = ?1 AND id = ?2 AND message_id = ?3",
                        params![
                            context.conversation_id.to_string(),
                            command.candidate_id.to_string(),
                            command.message_id.to_string(),
                        ],
                        |row| row.get(0),
                    )
                    .optional()
                    .map_err(slice::db)?;
                let author = author.ok_or(ConversationRepositoryError::NotFound)?;
                require_no_live_turn_for_message(
                    transaction,
                    context.conversation_id,
                    command.message_id,
                )?;
                let state =
                    message_state(transaction, context.conversation_id, command.message_id)?;
                if state.visibility == "tombstoned" {
                    return Err(ConversationRepositoryError::Conflict);
                }
                let chosen = MediaOwner::Candidate(command.candidate_id);
                let retired = state.render_owner().filter(|owner| *owner != chosen);
                let changed = transaction
                    .execute(
                        "UPDATE conversation_messages SET active_candidate_id = ?3, active_revision_id = NULL, author_participant_id = ?4, revision = revision + 1, updated_at = ?5 WHERE conversation_id = ?1 AND id = ?2",
                        params![
                            context.conversation_id.to_string(),
                            command.message_id.to_string(),
                            command.candidate_id.to_string(),
                            author,
                            context.now.get(),
                        ],
                    )
                    .map_err(kernel::map_constraint)?;
                if changed == 0 {
                    return Err(ConversationRepositoryError::NotFound);
                }
                let mut owned_deltas = Vec::new();
                if let Some(retired) = retired {
                    set_owner_media_state(
                        transaction,
                        context.conversation_id,
                        retired,
                        "historical",
                    )?;
                    let assets = owner_assets(transaction, context.conversation_id, retired)?;
                    owned_deltas.push((
                        retired,
                        owner_deltas(&assets, retired, AssetReferenceState::Historical),
                    ));
                }
                set_owner_media_state(transaction, context.conversation_id, chosen, "active")?;
                let assets = owner_assets(transaction, context.conversation_id, chosen)?;
                owned_deltas.insert(
                    0,
                    (
                        chosen,
                        owner_deltas(&assets, chosen, AssetReferenceState::Active),
                    ),
                );
                let revision =
                    kernel::bump_conversation(transaction, context.conversation_id, context.now)?;
                let mut events = vec![kernel::StagedEvent {
                    conversation_revision: revision,
                    at: context.now,
                    event: ConversationOutboxEvent::CandidateChosen {
                        conversation_id: context.conversation_id,
                        message_id: command.message_id,
                        candidate_id: command.candidate_id,
                        at: context.now,
                    },
                }];
                for (owner, changes) in owned_deltas {
                    if changes.is_empty() {
                        continue;
                    }
                    events.push(kernel::StagedEvent {
                        conversation_revision: revision,
                        at: context.now,
                        event: owner_event(context.conversation_id, owner, changes, context.now),
                    });
                }
                Ok(kernel::Staged {
                    value: load_message(transaction, context.conversation_id, command.message_id)?,
                    result: OperationResultRef::Message(command.message_id),
                    events,
                })
            },
            |transaction, operation| {
                replayed_message(operation, command.message_id)?;
                let recorded = recorded_events(transaction, command.conversation_id, operation)?
                    .into_iter()
                    .any(|event| {
                        matches!(
                            event,
                            ConversationOutboxEvent::CandidateChosen { candidate_id, .. }
                                if candidate_id == command.candidate_id
                        )
                    });
                if !recorded {
                    return Err(ConversationRepositoryError::Conflict);
                }
                load_message(transaction, command.conversation_id, command.message_id)
            },
        )
    }

    fn fork_branch(
        &self,
        command: &ForkBranch,
        now: TimestampMillis,
    ) -> Result<ForkBranchResult, ConversationRepositoryError> {
        lettuce_conversations::ConversationMutation::Fork(command.clone())
            .validate()
            .map_err(ConversationRepositoryError::Invalid)?;
        kernel::run_mutation(
            self,
            command.conversation_id,
            OperationKind::Fork,
            &command.operation,
            now,
            |transaction, context| {
                let conversation = kernel::cas_conversation(
                    transaction,
                    context.conversation_id,
                    command.expected_revision,
                )?;
                kernel::require_active(&conversation)?;
                require_no_live_turn(transaction, context.conversation_id)?;
                let source = load_branch(
                    transaction,
                    context.conversation_id,
                    command.source_branch_id,
                )?;
                if source.status != lettuce_conversations::BranchStatus::Active {
                    return Err(ConversationRepositoryError::Conflict);
                }
                let fork_message_id = match command.at_message_id {
                    Some(message_id) => {
                        let state =
                            message_state(transaction, context.conversation_id, message_id)?;
                        if state.branch_id != command.source_branch_id {
                            return Err(ConversationRepositoryError::Conflict);
                        }
                        message_id
                    }
                    None => source
                        .head_message_id
                        .ok_or(ConversationRepositoryError::Conflict)?,
                };
                let branch_id = ConversationBranchId::new();
                transaction
                    .execute(
                        "INSERT INTO conversation_branches (conversation_id, id, parent_branch_id, fork_message_id, head_message_id, status, revision, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, NULL, 'active', 1, ?5, ?5)",
                        params![
                            context.conversation_id.to_string(),
                            branch_id.to_string(),
                            command.source_branch_id.to_string(),
                            fork_message_id.to_string(),
                            context.now.get(),
                        ],
                    )
                    .map_err(kernel::map_constraint)?;
                select_active_branch(transaction, context.conversation_id, branch_id)?;
                let revision =
                    kernel::bump_conversation(transaction, context.conversation_id, context.now)?;
                let value = BranchResult {
                    branch: load_branch(transaction, context.conversation_id, branch_id)?,
                    conversation: conversation_value(transaction, context.conversation_id)?,
                };
                value
                    .validate()
                    .map_err(ConversationRepositoryError::Invalid)?;
                Ok(kernel::Staged {
                    value,
                    result: OperationResultRef::Branch(branch_id),
                    events: vec![kernel::StagedEvent {
                        conversation_revision: revision,
                        at: context.now,
                        event: ConversationOutboxEvent::BranchForked {
                            conversation_id: context.conversation_id,
                            branch_id,
                            at: context.now,
                        },
                    }],
                })
            },
            |transaction, operation| {
                let OperationResultRef::Branch(branch_id) = operation.result else {
                    return Err(ConversationRepositoryError::Storage);
                };
                let branch = load_branch(transaction, command.conversation_id, branch_id)?;
                if branch.parent_branch_id != Some(command.source_branch_id) {
                    return Err(ConversationRepositoryError::Conflict);
                }
                Ok(BranchResult {
                    branch,
                    conversation: conversation_value(transaction, command.conversation_id)?,
                })
            },
        )
    }

    fn select_branch(
        &self,
        command: &SelectBranch,
        now: TimestampMillis,
    ) -> Result<SelectBranchResult, ConversationRepositoryError> {
        lettuce_conversations::ConversationMutation::SelectBranch(command.clone())
            .validate()
            .map_err(ConversationRepositoryError::Invalid)?;
        kernel::run_mutation(
            self,
            command.conversation_id,
            OperationKind::SelectBranch,
            &command.operation,
            now,
            |transaction, context| {
                let conversation = kernel::cas_conversation(
                    transaction,
                    context.conversation_id,
                    command.expected_revision,
                )?;
                kernel::require_active(&conversation)?;
                require_no_live_turn(transaction, context.conversation_id)?;
                let branch = load_branch(transaction, context.conversation_id, command.branch_id)?;
                if branch.status != lettuce_conversations::BranchStatus::Active {
                    return Err(ConversationRepositoryError::Conflict);
                }
                select_active_branch(transaction, context.conversation_id, command.branch_id)?;
                let revision =
                    kernel::bump_conversation(transaction, context.conversation_id, context.now)?;
                Ok(kernel::Staged {
                    value: conversation_value(transaction, context.conversation_id)?,
                    result: OperationResultRef::Conversation(context.conversation_id),
                    events: vec![kernel::StagedEvent {
                        conversation_revision: revision,
                        at: context.now,
                        event: ConversationOutboxEvent::BranchSelected {
                            conversation_id: context.conversation_id,
                            branch_id: command.branch_id,
                            at: context.now,
                        },
                    }],
                })
            },
            |transaction, operation| {
                let selected = recorded_events(transaction, command.conversation_id, operation)?
                    .into_iter()
                    .any(|event| {
                        matches!(
                            event,
                            ConversationOutboxEvent::BranchSelected { branch_id, .. }
                                if branch_id == command.branch_id
                        )
                    });
                if !selected {
                    return Err(ConversationRepositoryError::Conflict);
                }
                conversation_value(transaction, command.conversation_id)
            },
        )
    }

    fn edit_message(
        &self,
        command: &EditMessage,
        now: TimestampMillis,
    ) -> Result<EditMessageResult, ConversationRepositoryError> {
        lettuce_conversations::ConversationMutation::Edit(command.clone())
            .validate()
            .map_err(ConversationRepositoryError::Invalid)?;
        kernel::run_mutation(
            self,
            command.conversation_id,
            OperationKind::Edit,
            &command.operation,
            now,
            |transaction, context| {
                let conversation = kernel::cas_conversation(
                    transaction,
                    context.conversation_id,
                    command.expected_revision,
                )?;
                kernel::require_active(&conversation)?;
                let state =
                    message_state(transaction, context.conversation_id, command.message_id)?;
                if state.visibility == "tombstoned" {
                    return Err(ConversationRepositoryError::Conflict);
                }
                if command.draft.scene_edited && state.role != "scene" {
                    return Err(invalid("message_edit.scene_edited"));
                }
                require_no_live_turn_for_message(
                    transaction,
                    context.conversation_id,
                    command.message_id,
                )?;
                let sequence: i64 = transaction
                    .query_row(
                        "SELECT COALESCE(max(sequence) + 1, 1) FROM conversation_message_revisions WHERE conversation_id = ?1 AND message_id = ?2",
                        params![
                            context.conversation_id.to_string(),
                            command.message_id.to_string(),
                        ],
                        |row| row.get(0),
                    )
                    .map_err(slice::db)?;
                let revision_id = MessageRevisionId::new();
                transaction
                    .execute(
                        "INSERT INTO conversation_message_revisions (conversation_id, id, message_id, branch_id, sequence, parts_json, authored_at, source_turn_id, provider_replay_artifact_id, provider_replay_retention) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, NULL, NULL)",
                        params![
                            context.conversation_id.to_string(),
                            revision_id.to_string(),
                            command.message_id.to_string(),
                            state.branch_id.to_string(),
                            sequence,
                            slice::encode(&command.draft.parts)?,
                            context.now.get(),
                        ],
                    )
                    .map_err(kernel::map_constraint)?;
                for (part_ordinal, part) in command.draft.parts.iter().enumerate() {
                    let MessagePart::MediaAsset { asset_id, role } = part else {
                        continue;
                    };
                    transaction
                        .execute(
                            "INSERT INTO revision_media_refs (conversation_id, message_revision_id, part_ordinal, asset_id, media_role, state, created_at) VALUES (?1, ?2, ?3, ?4, ?5, 'active', ?6)",
                            params![
                                context.conversation_id.to_string(),
                                revision_id.to_string(),
                                i64::try_from(part_ordinal)
                                    .map_err(|_| ConversationRepositoryError::Storage)?,
                                asset_id.to_string(),
                                conversation_creator::media_role_name(*role),
                                context.now.get(),
                            ],
                        )
                        .map_err(kernel::map_constraint)?;
                }
                transaction
                    .execute(
                        "UPDATE conversation_messages SET active_revision_id = ?3, active_candidate_id = NULL, visibility = ?4, pinned = ?5, scene_edited = ?6, revision = revision + 1, updated_at = ?7 WHERE conversation_id = ?1 AND id = ?2",
                        params![
                            context.conversation_id.to_string(),
                            command.message_id.to_string(),
                            revision_id.to_string(),
                            kernel::message_visibility_name(command.draft.visibility),
                            i64::from(command.draft.pinned),
                            i64::from(command.draft.scene_edited),
                            context.now.get(),
                        ],
                    )
                    .map_err(kernel::map_constraint)?;
                let created = MediaOwner::Revision(revision_id);
                let mut owned_deltas = vec![(
                    created,
                    owner_deltas(
                        &owner_assets(transaction, context.conversation_id, created)?,
                        created,
                        AssetReferenceState::Active,
                    ),
                )];
                if let Some(retired) = state.render_owner() {
                    set_owner_media_state(
                        transaction,
                        context.conversation_id,
                        retired,
                        "historical",
                    )?;
                    let assets = owner_assets(transaction, context.conversation_id, retired)?;
                    owned_deltas.push((
                        retired,
                        owner_deltas(&assets, retired, AssetReferenceState::Historical),
                    ));
                }
                let revision =
                    kernel::bump_conversation(transaction, context.conversation_id, context.now)?;
                let mut events = vec![kernel::StagedEvent {
                    conversation_revision: revision,
                    at: context.now,
                    event: ConversationOutboxEvent::MessageRevised {
                        conversation_id: context.conversation_id,
                        branch_id: state.branch_id,
                        message_id: command.message_id,
                        revision_id,
                        at: context.now,
                    },
                }];
                for (owner, changes) in &owned_deltas {
                    if changes.is_empty() {
                        continue;
                    }
                    events.push(kernel::StagedEvent {
                        conversation_revision: revision,
                        at: context.now,
                        event: owner_event(
                            context.conversation_id,
                            *owner,
                            changes.clone(),
                            context.now,
                        ),
                    });
                }
                Ok(kernel::Staged {
                    value: edit_result(
                        transaction,
                        context.conversation_id,
                        command.message_id,
                        revision_id,
                        owned_deltas
                            .into_iter()
                            .flat_map(|(_, changes)| changes)
                            .collect(),
                    )?,
                    result: OperationResultRef::Message(command.message_id),
                    events,
                })
            },
            |transaction, operation| {
                replayed_message(operation, command.message_id)?;
                let mut recorded = None;
                for event in recorded_events(transaction, command.conversation_id, operation)? {
                    match event {
                        ConversationOutboxEvent::MessageRevised {
                            message_id,
                            revision_id,
                            ..
                        } if message_id == command.message_id => recorded = Some(revision_id),
                        _ => {}
                    }
                }
                let revision_id = recorded.ok_or(ConversationRepositoryError::Conflict)?;
                let deltas = recorded_deltas(transaction, command.conversation_id, operation)?;
                edit_result(
                    transaction,
                    command.conversation_id,
                    command.message_id,
                    revision_id,
                    deltas,
                )
            },
        )
    }

    fn update_message_flags(
        &self,
        command: &UpdateMessageFlags,
        now: TimestampMillis,
    ) -> Result<UpdateMessageFlagsResult, ConversationRepositoryError> {
        lettuce_conversations::ConversationMutation::Flags(command.clone())
            .validate()
            .map_err(ConversationRepositoryError::Invalid)?;
        kernel::run_mutation(
            self,
            command.conversation_id,
            OperationKind::Flags,
            &command.operation,
            now,
            |transaction, context| {
                let conversation = kernel::cas_conversation(
                    transaction,
                    context.conversation_id,
                    command.expected_revision,
                )?;
                kernel::require_active(&conversation)?;
                let state =
                    message_state(transaction, context.conversation_id, command.message_id)?;
                if state.visibility == "tombstoned" {
                    return Err(ConversationRepositoryError::Conflict);
                }
                transaction
                    .execute(
                        "UPDATE conversation_messages SET pinned = COALESCE(?3, pinned), visibility = COALESCE(?4, visibility), revision = revision + 1, updated_at = ?5 WHERE conversation_id = ?1 AND id = ?2",
                        params![
                            context.conversation_id.to_string(),
                            command.message_id.to_string(),
                            command.pinned.map(i64::from),
                            command.visibility.map(kernel::message_visibility_name),
                            context.now.get(),
                        ],
                    )
                    .map_err(kernel::map_constraint)?;
                let message =
                    load_message(transaction, context.conversation_id, command.message_id)?;
                command
                    .validate_result(&message)
                    .map_err(ConversationRepositoryError::Invalid)?;
                let revision =
                    kernel::bump_conversation(transaction, context.conversation_id, context.now)?;
                Ok(kernel::Staged {
                    result: OperationResultRef::Message(command.message_id),
                    events: vec![kernel::StagedEvent {
                        conversation_revision: revision,
                        at: context.now,
                        event: ConversationOutboxEvent::MessageFlagsChanged {
                            conversation_id: context.conversation_id,
                            message_id: command.message_id,
                            pinned: message.pinned,
                            visibility: message.visibility,
                            at: context.now,
                        },
                    }],
                    value: message,
                })
            },
            |transaction, operation| {
                replayed_message(operation, command.message_id)?;
                load_message(transaction, command.conversation_id, command.message_id)
            },
        )
    }

    fn tombstone_message(
        &self,
        command: &TombstoneMessage,
        now: TimestampMillis,
    ) -> Result<TombstoneMessageResult, ConversationRepositoryError> {
        lettuce_conversations::ConversationMutation::Tombstone(command.clone())
            .validate()
            .map_err(ConversationRepositoryError::Invalid)?;
        kernel::run_mutation(
            self,
            command.conversation_id,
            OperationKind::Tombstone,
            &command.operation,
            now,
            |transaction, context| {
                let conversation = kernel::cas_conversation(
                    transaction,
                    context.conversation_id,
                    command.expected_revision,
                )?;
                kernel::require_active(&conversation)?;
                let state =
                    message_state(transaction, context.conversation_id, command.message_id)?;
                if state.visibility == "tombstoned" {
                    return Err(ConversationRepositoryError::Conflict);
                }
                require_no_live_turn_for_message(
                    transaction,
                    context.conversation_id,
                    command.message_id,
                )?;
                let descendants = match command.descendants {
                    DescendantPolicy::Tombstone => branch_descendants(
                        transaction,
                        context.conversation_id,
                        state.branch_id,
                        command.message_id,
                    )?,
                    DescendantPolicy::Preserve | DescendantPolicy::Fork => Vec::new(),
                };
                for descendant in &descendants {
                    require_no_live_turn_for_message(
                        transaction,
                        context.conversation_id,
                        *descendant,
                    )?;
                }
                let mut owned_deltas = Vec::new();
                let mut affected_revision_ids = Vec::new();
                for message_id in std::iter::once(command.message_id).chain(descendants.clone()) {
                    let affected = message_state(transaction, context.conversation_id, message_id)?;
                    transaction
                        .execute(
                            "UPDATE conversation_messages SET visibility = 'tombstoned', revision = revision + 1, updated_at = ?3 WHERE conversation_id = ?1 AND id = ?2",
                            params![
                                context.conversation_id.to_string(),
                                message_id.to_string(),
                                context.now.get(),
                            ],
                        )
                        .map_err(kernel::map_constraint)?;
                    if let Some(revision_id) = affected.active_revision_id {
                        affected_revision_ids.push(revision_id);
                    }
                    if let Some(owner) = affected.render_owner() {
                        set_owner_media_state(
                            transaction,
                            context.conversation_id,
                            owner,
                            "historical",
                        )?;
                        let assets = owner_assets(transaction, context.conversation_id, owner)?;
                        if !assets.is_empty() {
                            owned_deltas.push((
                                owner,
                                owner_deltas(&assets, owner, AssetReferenceState::Released),
                            ));
                        }
                    }
                }
                let forked_branch = match command.descendants {
                    DescendantPolicy::Fork => {
                        let parent_message_id = state
                            .parent_message_id
                            .ok_or(ConversationRepositoryError::Conflict)?;
                        let parent =
                            message_state(transaction, context.conversation_id, parent_message_id)?;
                        let branch_id = ConversationBranchId::new();
                        transaction
                            .execute(
                                "INSERT INTO conversation_branches (conversation_id, id, parent_branch_id, fork_message_id, head_message_id, status, revision, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, NULL, 'active', 1, ?5, ?5)",
                                params![
                                    context.conversation_id.to_string(),
                                    branch_id.to_string(),
                                    parent.branch_id.to_string(),
                                    parent_message_id.to_string(),
                                    context.now.get(),
                                ],
                            )
                            .map_err(kernel::map_constraint)?;
                        select_active_branch(transaction, context.conversation_id, branch_id)?;
                        Some(load_branch(
                            transaction,
                            context.conversation_id,
                            branch_id,
                        )?)
                    }
                    DescendantPolicy::Preserve | DescendantPolicy::Tombstone => None,
                };
                let revision =
                    kernel::bump_conversation(transaction, context.conversation_id, context.now)?;
                let asset_reference_deltas: Vec<AssetReferenceDelta> = owned_deltas
                    .iter()
                    .flat_map(|(_, changes)| changes.iter().cloned())
                    .collect();
                let value = TombstoneResult {
                    conversation: conversation_value(transaction, context.conversation_id)?,
                    message: load_message(
                        transaction,
                        context.conversation_id,
                        command.message_id,
                    )?,
                    descendant_count: u32::try_from(descendants.len())
                        .map_err(|_| ConversationRepositoryError::Storage)?,
                    asset_reference_deltas: asset_reference_deltas.clone(),
                    forked_branch: forked_branch.clone(),
                };
                value
                    .validate_for_policy(command.descendants)
                    .map_err(ConversationRepositoryError::Invalid)?;
                let mut events = vec![kernel::StagedEvent {
                    conversation_revision: revision,
                    at: context.now,
                    event: ConversationOutboxEvent::MessageTombstoned {
                        conversation_id: context.conversation_id,
                        branch_id: state.branch_id,
                        message_id: command.message_id,
                        descendants: command.descendants,
                        affected_message_ids: descendants,
                        affected_revision_ids,
                        asset_reference_deltas,
                        at: context.now,
                    },
                }];
                if let Some(branch) = &forked_branch {
                    events.push(kernel::StagedEvent {
                        conversation_revision: revision,
                        at: context.now,
                        event: ConversationOutboxEvent::BranchForked {
                            conversation_id: context.conversation_id,
                            branch_id: branch.id,
                            at: context.now,
                        },
                    });
                }
                Ok(kernel::Staged {
                    value,
                    result: OperationResultRef::Message(command.message_id),
                    events,
                })
            },
            |transaction, operation| {
                replayed_message(operation, command.message_id)?;
                let message =
                    load_message(transaction, command.conversation_id, command.message_id)?;
                if message.visibility != lettuce_conversations::MessageVisibility::Tombstoned {
                    return Err(ConversationRepositoryError::Conflict);
                }
                let mut forked_branch = None;
                let mut descendant_count = 0;
                for event in recorded_events(transaction, command.conversation_id, operation)? {
                    match event {
                        ConversationOutboxEvent::MessageTombstoned {
                            affected_message_ids,
                            ..
                        } => {
                            descendant_count = u32::try_from(affected_message_ids.len())
                                .map_err(|_| ConversationRepositoryError::Storage)?;
                        }
                        ConversationOutboxEvent::BranchForked { branch_id, .. } => {
                            forked_branch = Some(load_branch(
                                transaction,
                                command.conversation_id,
                                branch_id,
                            )?);
                        }
                        _ => {}
                    }
                }
                Ok(TombstoneResult {
                    conversation: conversation_value(transaction, command.conversation_id)?,
                    message,
                    descendant_count,
                    asset_reference_deltas: recorded_deltas(
                        transaction,
                        command.conversation_id,
                        operation,
                    )?,
                    forked_branch,
                })
            },
        )
    }

    /// Archiving is metadata only: an in-flight generation keeps running and
    /// settles normally, while new work waits for a restore.
    fn archive(
        &self,
        command: &ArchiveConversation,
        now: TimestampMillis,
    ) -> Result<ArchiveConversationResult, ConversationRepositoryError> {
        lettuce_conversations::ConversationMutation::Archive(command.clone())
            .validate()
            .map_err(ConversationRepositoryError::Invalid)?;
        self.change_lifecycle(
            command.conversation_id,
            command.expected_revision,
            &command.operation,
            OperationKind::Archive,
            ConversationLifecycle::Archived,
            now,
        )
    }

    fn restore(
        &self,
        command: &RestoreConversation,
        now: TimestampMillis,
    ) -> Result<RestoreConversationResult, ConversationRepositoryError> {
        lettuce_conversations::ConversationMutation::Restore(command.clone())
            .validate()
            .map_err(ConversationRepositoryError::Invalid)?;
        self.change_lifecycle(
            command.conversation_id,
            command.expected_revision,
            &command.operation,
            OperationKind::Restore,
            ConversationLifecycle::Active,
            now,
        )
    }

    fn update_participant_policy(
        &self,
        command: &UpdateParticipantPolicy,
        now: TimestampMillis,
    ) -> Result<ParticipantPolicyResult, ConversationRepositoryError> {
        lettuce_conversations::ConversationMutation::ParticipantPolicy(command.clone())
            .validate()
            .map_err(ConversationRepositoryError::Invalid)?;
        kernel::run_mutation(
            self,
            command.conversation_id,
            OperationKind::ParticipantPolicy,
            &command.operation,
            now,
            |transaction, context| {
                let conversation = kernel::cas_conversation(
                    transaction,
                    context.conversation_id,
                    command.expected_revision,
                )?;
                kernel::require_active(&conversation)?;
                let current = conversation_value(transaction, context.conversation_id)?;
                command
                    .validate_against_participants(&current.participants)
                    .map_err(ConversationRepositoryError::Invalid)?;
                let model_selection_json = match &command.model_override {
                    Some(selection) => {
                        if let SnapshotSelection::Inherited(model)
                        | SnapshotSelection::Explicit(model) = selection
                        {
                            verify_current_snapshot(transaction, &model.snapshot_ref)?;
                        }
                        Some(slice::encode(selection)?)
                    }
                    None => None,
                };
                transaction
                    .execute(
                        "UPDATE conversation_participants SET enabled = COALESCE(?3, enabled), muted = COALESCE(?4, muted), model_selection_json = COALESCE(?5, model_selection_json), revision = revision + 1, updated_at = ?6 WHERE conversation_id = ?1 AND id = ?2",
                        params![
                            context.conversation_id.to_string(),
                            command.participant_id.to_string(),
                            command.enabled.map(i64::from),
                            command.muted.map(i64::from),
                            model_selection_json,
                            context.now.get(),
                        ],
                    )
                    .map_err(kernel::map_constraint)?;
                let revision =
                    kernel::bump_conversation(transaction, context.conversation_id, context.now)?;
                Ok(kernel::Staged {
                    value: conversation_value(transaction, context.conversation_id)?,
                    result: OperationResultRef::Conversation(context.conversation_id),
                    events: vec![kernel::StagedEvent {
                        conversation_revision: revision,
                        at: context.now,
                        event: ConversationOutboxEvent::ParticipantPolicyChanged {
                            conversation_id: context.conversation_id,
                            participant_id: command.participant_id,
                            at: context.now,
                        },
                    }],
                })
            },
            |transaction, operation| {
                let changed = recorded_events(transaction, command.conversation_id, operation)?
                    .into_iter()
                    .any(|event| {
                        matches!(
                            event,
                            ConversationOutboxEvent::ParticipantPolicyChanged { participant_id, .. }
                                if participant_id == command.participant_id
                        )
                    });
                if !changed {
                    return Err(ConversationRepositoryError::Conflict);
                }
                conversation_value(transaction, command.conversation_id)
            },
        )
    }

    /// Settings carry their own compare-and-swap and no conversation revision,
    /// so this mutation reads the lifecycle instead of swapping the aggregate.
    fn update_settings(
        &self,
        command: &UpdateConversationSettings,
        now: TimestampMillis,
    ) -> Result<SettingsResult, ConversationRepositoryError> {
        lettuce_conversations::ConversationMutation::Settings(command.clone())
            .validate()
            .map_err(ConversationRepositoryError::Invalid)?;
        kernel::run_mutation(
            self,
            command.conversation_id,
            OperationKind::Settings,
            &command.operation,
            now,
            |transaction, context| {
                require_active_conversation(transaction, context.conversation_id)?;
                let current = read_current_settings(transaction, context.conversation_id)?;
                match (command.cas_requirement(), &current) {
                    (SettingsCasRequirement::CreateOnly, Some(_)) => {
                        return Err(ConversationRepositoryError::Conflict);
                    }
                    (SettingsCasRequirement::Exact(expected), None) => {
                        return Err(ConversationRepositoryError::StaleRevision {
                            expected,
                            actual: Revision::INITIAL,
                        });
                    }
                    (SettingsCasRequirement::Exact(expected), Some(settings))
                        if settings.revision != expected =>
                    {
                        return Err(ConversationRepositoryError::StaleRevision {
                            expected,
                            actual: settings.revision,
                        });
                    }
                    _ => {}
                }
                let next = command
                    .patch
                    .apply(current.as_ref(), command.expected_settings_revision)
                    .map_err(ConversationRepositoryError::Invalid)?;
                if let Some(model) = &next.model_override {
                    verify_current_snapshot(transaction, &model.snapshot_ref)?;
                }
                if let Some(voice) = &next.voice {
                    verify_current_snapshot(transaction, &voice.snapshot_ref)?;
                }
                if let Some(policy) = next
                    .memory
                    .as_ref()
                    .and_then(|memory| memory.policy_ref.as_ref())
                {
                    verify_current_snapshot(transaction, policy)?;
                }
                write_settings(
                    transaction,
                    context.conversation_id,
                    &next,
                    current.is_none(),
                    context.now,
                )?;
                let revision =
                    kernel::bump_conversation(transaction, context.conversation_id, context.now)?;
                Ok(kernel::Staged {
                    value: conversation_value(transaction, context.conversation_id)?,
                    result: OperationResultRef::Conversation(context.conversation_id),
                    events: vec![kernel::StagedEvent {
                        conversation_revision: revision,
                        at: context.now,
                        event: ConversationOutboxEvent::SettingsChanged {
                            conversation_id: context.conversation_id,
                            settings_revision: next.revision,
                            at: context.now,
                        },
                    }],
                })
            },
            |transaction, operation| {
                let recorded = recorded_events(transaction, command.conversation_id, operation)?
                    .into_iter()
                    .any(|event| matches!(event, ConversationOutboxEvent::SettingsChanged { .. }));
                if !recorded {
                    return Err(ConversationRepositoryError::Conflict);
                }
                conversation_value(transaction, command.conversation_id)
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lettuce_conversations::{
        ArtifactCodec, ArtifactRetention, CharacterLaunchSnapshot, ContinueConversation,
        ConversationCreator, ConversationKind, ConversationParticipantDraft, ConversationReader,
        CreateConversationPlan, DirectConversationDetails, GroupChatModeSnapshot,
        GroupConversationDetails, GroupLaunchSnapshot, GroupMemberLaunchSnapshot,
        GroupParticipantPolicyDocument, GroupParticipantPolicySnapshot,
        GroupSpeakerSelectionSnapshot, InitialTimelineDraft, MessageDraft, MessageRole,
        MessageVisibility, ModelProviderKind, ModelSelectionSnapshot, ParticipantRole,
        ParticipantSource, PreparedConversationLaunch, ProtectedArtifactBytes,
        ProtectedSnapshotRef, SnapshotArtifactDraft, SnapshotSelection, SnapshotSource,
    };
    use lettuce_conversations::{
        CurrentConversationSettingsPatch, PatchValue, RestoreConversation, SelectBranch,
        TombstoneMessage, UpdateConversationSettings, UpdateMessageFlags, UpdateParticipantPolicy,
    };
    use lettuce_types::{
        AssetId, CharacterId, ContentHash, GroupId, JobId, MediaBlobId, MessageCandidateId,
        ModelProfileId, PageLimit, PageRequest, SnapshotArtifactId, UsageEventId,
    };

    struct Fixture {
        database: std::rc::Rc<Database>,
        conversation_id: ConversationId,
        branch_id: ConversationBranchId,
        user_participant: ConversationParticipantId,
        characters: Vec<ConversationParticipantId>,
        revision: Revision,
    }

    fn token(key: &str, digest: &str) -> OperationToken {
        OperationToken {
            key: lettuce_jobs::IdempotencyKey::new(key).expect("key"),
            request_digest: ContentHash::parse(digest.repeat(32)).expect("digest"),
        }
    }

    fn artifact(
        source: SnapshotSource,
        payload: &[u8],
    ) -> (ProtectedSnapshotRef, SnapshotArtifactDraft) {
        let bytes = ProtectedArtifactBytes::new(payload.to_vec()).expect("bytes");
        let reference = ProtectedSnapshotRef {
            source,
            source_revision: Revision::INITIAL,
            artifact_id: SnapshotArtifactId::new(),
            digest: bytes.digest(),
            schema_version: 1,
            byte_size: bytes.len() as u64,
        };
        let draft = SnapshotArtifactDraft {
            source,
            source_revision: reference.source_revision,
            artifact_id: reference.artifact_id,
            digest: reference.digest.clone(),
            schema_version: reference.schema_version,
            byte_size: reference.byte_size,
            codec: ArtifactCodec::Json,
            retention: ArtifactRetention::Conversation,
            bytes,
        };
        (reference, draft)
    }

    fn participant(
        id: ConversationParticipantId,
        role: ParticipantRole,
        ordinal: u32,
        source: ParticipantSource,
        name: &str,
    ) -> ConversationParticipantDraft {
        ConversationParticipantDraft {
            id,
            role,
            ordinal,
            source,
            enabled: true,
            muted: false,
            display_name: name.into(),
            authored_description: None,
            model_selection: SnapshotSelection::Disabled,
        }
    }

    fn model_snapshot() -> ModelSelectionSnapshot {
        let source_id = ModelProfileId::new();
        let (reference, _) = artifact(SnapshotSource::Model(source_id), b"model");
        ModelSelectionSnapshot {
            snapshot_ref: reference,
            source_id,
            source_revision: Revision::INITIAL,
            provider_kind: ModelProviderKind::OpenAiCompatible,
            external_model_id: "test-model".into(),
            display_name: "Test Model".into(),
            context_length: None,
            max_output_tokens: None,
        }
    }

    fn launch(
        database: &Database,
        plan: CreateConversationPlan,
        drafts: Vec<SnapshotArtifactDraft>,
    ) -> Revision {
        let launch = PreparedConversationLaunch::new(plan, drafts).expect("prepared launch");
        ConversationCreator::create(database, launch, TimestampMillis::new(10))
            .expect("create")
            .value
            .conversation
            .revision
    }

    fn direct_fixture() -> Fixture {
        direct_fixture_on(std::rc::Rc::new(
            Database::open_in_memory().expect("database"),
        ))
    }

    fn direct_fixture_on(database: std::rc::Rc<Database>) -> Fixture {
        let conversation_id = ConversationId::new();
        let character_id = CharacterId::new();
        let (character_ref, character_draft) =
            artifact(SnapshotSource::Character(character_id), b"character");
        let user_participant = ConversationParticipantId::new();
        let character_participant = ConversationParticipantId::new();
        let plan = CreateConversationPlan {
            conversation_id,
            title: "Direct".into(),
            kind: ConversationKind::Direct(DirectConversationDetails {
                format_version: 1,
                character: CharacterLaunchSnapshot {
                    snapshot_ref: character_ref,
                    source_id: character_id,
                    source_revision: Revision::INITIAL,
                    name: "Ada".into(),
                    nickname: None,
                },
                persona: SnapshotSelection::Disabled,
                scene: SnapshotSelection::Disabled,
                starter: SnapshotSelection::Disabled,
                prompt: SnapshotSelection::Disabled,
                lorebooks: SnapshotSelection::Explicit(Vec::new()),
                model: SnapshotSelection::Disabled,
                memory: SnapshotSelection::Disabled,
                voice: SnapshotSelection::Disabled,
            }),
            participants: vec![
                participant(
                    user_participant,
                    ParticipantRole::User,
                    0,
                    ParticipantSource::User,
                    "User",
                ),
                participant(
                    character_participant,
                    ParticipantRole::Character,
                    1,
                    ParticipantSource::Character(character_id),
                    "Ada",
                ),
            ],
            initial_timeline: InitialTimelineDraft {
                format_version: 1,
                entries: Vec::new(),
            },
            operation: token("create-direct", "ab"),
        };
        let revision = launch(&database, plan, vec![character_draft]);
        let branch_id = ConversationReader::get(database.as_ref(), conversation_id)
            .expect("aggregate")
            .conversation
            .active_branch_id;
        Fixture {
            database,
            conversation_id,
            branch_id,
            user_participant,
            characters: vec![character_participant],
            revision,
        }
    }

    fn group_fixture() -> Fixture {
        let database = std::rc::Rc::new(Database::open_in_memory().expect("database"));
        let conversation_id = ConversationId::new();
        let group_id = GroupId::new();
        let first_character = CharacterId::new();
        let second_character = CharacterId::new();
        let (group_ref, group_draft) = artifact(SnapshotSource::Group(group_id), b"group");
        let (first_ref, first_draft) =
            artifact(SnapshotSource::Character(first_character), b"first");
        let (second_ref, second_draft) =
            artifact(SnapshotSource::Character(second_character), b"second");
        let user_participant = ConversationParticipantId::new();
        let first_participant = ConversationParticipantId::new();
        let second_participant = ConversationParticipantId::new();
        let member = |snapshot_ref: ProtectedSnapshotRef,
                      source_id: CharacterId,
                      name: &str,
                      ordinal: u32| GroupMemberLaunchSnapshot {
            character: CharacterLaunchSnapshot {
                snapshot_ref,
                source_id,
                source_revision: Revision::INITIAL,
                name: name.into(),
                nickname: None,
            },
            ordinal,
            enabled: true,
            muted: false,
            model_override: SnapshotSelection::Disabled,
            lorebooks: SnapshotSelection::Explicit(Vec::new()),
            prompt: SnapshotSelection::Disabled,
        };
        let policy = |participant_id: ConversationParticipantId| GroupParticipantPolicySnapshot {
            participant_id,
            enabled: true,
            muted: false,
            model_override: SnapshotSelection::Disabled,
        };
        let plan = CreateConversationPlan {
            conversation_id,
            title: "Group".into(),
            kind: ConversationKind::Group(GroupConversationDetails {
                format_version: 1,
                group: GroupLaunchSnapshot {
                    snapshot_ref: group_ref,
                    source_id: group_id,
                    source_revision: Revision::INITIAL,
                    name: "Cast".into(),
                    members: vec![
                        member(first_ref, first_character, "First", 0),
                        member(second_ref, second_character, "Second", 1),
                    ],
                    chat_mode: GroupChatModeSnapshot::Conversation,
                    speaker_selection: GroupSpeakerSelectionSnapshot::RoundRobin,
                    memory: SnapshotSelection::Disabled,
                    disable_character_lorebook: false,
                    persona: SnapshotSelection::Disabled,
                    scene: SnapshotSelection::Disabled,
                    prompt: SnapshotSelection::Disabled,
                    lorebooks: SnapshotSelection::Explicit(Vec::new()),
                    model: SnapshotSelection::Disabled,
                },
                initial_participant_policy: GroupParticipantPolicyDocument {
                    members: vec![policy(first_participant), policy(second_participant)],
                    revision: Revision::INITIAL,
                    created_at: TimestampMillis::new(10),
                    updated_at: TimestampMillis::new(10),
                },
            }),
            participants: vec![
                participant(
                    user_participant,
                    ParticipantRole::User,
                    0,
                    ParticipantSource::User,
                    "User",
                ),
                participant(
                    first_participant,
                    ParticipantRole::Character,
                    1,
                    ParticipantSource::Character(first_character),
                    "First",
                ),
                participant(
                    second_participant,
                    ParticipantRole::Character,
                    2,
                    ParticipantSource::Character(second_character),
                    "Second",
                ),
            ],
            initial_timeline: InitialTimelineDraft {
                format_version: 1,
                entries: Vec::new(),
            },
            operation: token("create-group", "ab"),
        };
        let revision = launch(
            &database,
            plan,
            vec![group_draft, first_draft, second_draft],
        );
        let branch_id = ConversationReader::get(database.as_ref(), conversation_id)
            .expect("aggregate")
            .conversation
            .active_branch_id;
        Fixture {
            database,
            conversation_id,
            branch_id,
            user_participant,
            characters: vec![first_participant, second_participant],
            revision,
        }
    }

    fn send_command(
        fixture: &Fixture,
        key: &str,
        digest: &str,
        parts: Vec<MessagePart>,
    ) -> SendConversation {
        SendConversation {
            conversation_id: fixture.conversation_id,
            branch_id: fixture.branch_id,
            expected_revision: fixture.revision,
            operation: token(key, digest),
            message: MessageDraft {
                role: MessageRole::User,
                author_participant_id: Some(fixture.user_participant),
                parts,
                visibility: MessageVisibility::Visible,
                pinned: false,
                scene_edited: false,
            },
            swap_roles: false,
        }
    }

    fn text(value: &str) -> Vec<MessagePart> {
        vec![MessagePart::Text { text: value.into() }]
    }

    fn stage_media_asset(database: &Database, seed: &str) -> AssetId {
        let asset_id = AssetId::new();
        let blob_id = MediaBlobId::new();
        let connection = database.connection().expect("connection");
        connection
            .execute(
                "INSERT INTO media_blobs (id, content_hash, kind, mime_type, byte_size, width, height, duration_ms, validation_version, state, created_at, updated_at) VALUES (?1, ?2, 'image', 'image/png', 4, NULL, NULL, NULL, 1, 'ready', 1, 1)",
                params![blob_id.to_string(), seed.repeat(32)],
            )
            .expect("blob");
        connection
            .execute(
                "INSERT INTO media_assets (id, blob_id, blob_kind, kind, origin, retention, expires_at, provenance_json, revision, created_at, updated_at) VALUES (?1, ?2, 'image', 'message_image', 'upload', 'persistent', NULL, '{}', 1, 1, 1)",
                params![asset_id.to_string(), blob_id.to_string()],
            )
            .expect("asset");
        asset_id
    }

    fn next_checkpoint_sequence(
        fixture: &Fixture,
        turn_id: GenerationTurnId,
        attempt_id: GenerationAttemptId,
    ) -> u64 {
        let value: i64 = fixture
            .database
            .connection()
            .expect("connection")
            .query_row(
                "SELECT COALESCE(max(sequence), 0) + 1 FROM generation_checkpoints WHERE conversation_id = ?1 AND turn_id = ?2 AND attempt_id = ?3",
                params![
                    fixture.conversation_id.to_string(),
                    turn_id.to_string(),
                    attempt_id.to_string(),
                ],
                |row| row.get(0),
            )
            .expect("sequence");
        u64::try_from(value).expect("sequence")
    }

    fn attempt_job(
        fixture: &Fixture,
        turn_id: GenerationTurnId,
        attempt_id: GenerationAttemptId,
    ) -> Option<JobId> {
        fixture
            .database
            .connection()
            .expect("connection")
            .query_row(
                "SELECT job_id FROM generation_attempts WHERE conversation_id = ?1 AND turn_id = ?2 AND id = ?3",
                params![
                    fixture.conversation_id.to_string(),
                    turn_id.to_string(),
                    attempt_id.to_string(),
                ],
                |row| row.get::<_, Option<String>>(0),
            )
            .expect("attempt")
            .map(|value| value.parse().expect("job id"))
    }

    /// Walks a live turn through real stage checkpoints, continuing the
    /// attempt's own sequence and carrying whatever job it has attached.
    fn drive(
        fixture: &Fixture,
        turn_id: GenerationTurnId,
        attempt_id: GenerationAttemptId,
        stages: &[GenerationTurnStatus],
        key: &str,
        now: i64,
    ) -> Revision {
        let mut revision = turn_revision(fixture, turn_id);
        let job_id = attempt_job(fixture, turn_id, attempt_id);
        for status in stages {
            let sequence = next_checkpoint_sequence(fixture, turn_id, attempt_id);
            revision = fixture
                .database
                .append_event(
                    turn_id,
                    revision,
                    &token(&format!("{key}-{sequence}"), "cd"),
                    GenerationCheckpointEnvelope {
                        turn_id,
                        attempt_id,
                        job_id,
                        correlation_id: None,
                        sequence,
                        event: GenerationCheckpointEvent::Stage { status: *status },
                    },
                    TimestampMillis::new(now + i64::try_from(sequence).expect("sequence")),
                )
                .expect("stage checkpoint")
                .value
                .revision;
        }
        revision
    }

    fn settle_failed(fixture: &Fixture, turn: &GenerationTurn, now: i64) {
        let attempt_id = turn.attempts.first().expect("attempt").id;
        let revision = drive(
            fixture,
            turn.id,
            attempt_id,
            &[GenerationTurnStatus::Preparing],
            &format!("drive-fail-{}", turn.id),
            now,
        );
        fixture
            .database
            .fail_generation(
                turn.id,
                attempt_id,
                conversation_revision(fixture),
                revision,
                &token(&format!("fail-{}", turn.id), "cd"),
                GenerationFailureCode::Internal,
                UsageEventId::new(),
                TimestampMillis::new(now + 50),
            )
            .expect("fail");
    }

    fn settle_cancelled(fixture: &Fixture, turn: &GenerationTurn, now: i64) {
        let attempt_id = turn.attempts.first().expect("attempt").id;
        let requested = fixture
            .database
            .request_cancellation(
                &CancelGeneration {
                    conversation_id: fixture.conversation_id,
                    turn_id: turn.id,
                    attempt_id,
                    expected_revision: conversation_revision(fixture),
                    expected_turn_revision: turn_revision(fixture, turn.id),
                    operation: token(&format!("cancel-{}", turn.id), "cd"),
                },
                TimestampMillis::new(now),
            )
            .expect("request cancellation");
        fixture
            .database
            .settle_cancellation(
                &SettleCancellation {
                    conversation_id: fixture.conversation_id,
                    turn_id: turn.id,
                    attempt_id,
                    expected_revision: conversation_revision(fixture),
                    expected_turn_revision: requested.value.revision,
                    operation: token(&format!("settle-{}", turn.id), "cd"),
                    usage_event_id: UsageEventId::new(),
                },
                TimestampMillis::new(now + 1),
            )
            .expect("settle cancellation");
    }

    fn finalization_draft(parts: Vec<MessagePart>, ordinal: u16) -> FinalizationDraft {
        FinalizationDraft {
            parts,
            ordinal,
            model: model_snapshot(),
            replay: None,
            outcome: GenerationCheckpointEvent::Completed,
        }
    }

    /// Drives a live turn to running and finalizes it through the real port.
    fn settle_succeeded(
        fixture: &Fixture,
        turn: &GenerationTurn,
        now: i64,
    ) -> (MessageId, MessageCandidateId) {
        let attempt_id = turn.attempts.first().expect("attempt").id;
        let revision = drive(
            fixture,
            turn.id,
            attempt_id,
            &[
                GenerationTurnStatus::Preparing,
                GenerationTurnStatus::ContextPrepared,
                GenerationTurnStatus::Running,
            ],
            &format!("drive-ok-{}", turn.id),
            now,
        );
        let finalized = fixture
            .database
            .finalize_generation(
                turn.id,
                attempt_id,
                conversation_revision(fixture),
                revision,
                &token(&format!("finalize-{}", turn.id), "cd"),
                finalization_draft(text("generated"), 0),
                UsageEventId::new(),
                TimestampMillis::new(now + 50),
            )
            .expect("finalize");
        (
            finalized.value.assistant_message.id,
            finalized.value.candidate.id,
        )
    }

    fn scalar<T: rusqlite::types::FromSql>(database: &Database, sql: &str, id: &str) -> T {
        database
            .connection()
            .expect("connection")
            .query_row(sql, [id], |row| row.get(0))
            .expect("scalar")
    }

    fn conversation_revision(fixture: &Fixture) -> Revision {
        ConversationReader::get(fixture.database.as_ref(), fixture.conversation_id)
            .expect("aggregate")
            .conversation
            .revision
    }

    fn turn_status(fixture: &Fixture, turn_id: GenerationTurnId) -> GenerationTurnStatus {
        let value: String = fixture
            .database
            .connection()
            .expect("connection")
            .query_row(
                "SELECT status FROM conversation_turns WHERE conversation_id = ?1 AND id = ?2",
                params![fixture.conversation_id.to_string(), turn_id.to_string()],
                |row| row.get(0),
            )
            .expect("turn status");
        conversation_query::generation_status(&value).expect("status")
    }

    fn turn_revision(fixture: &Fixture, turn_id: GenerationTurnId) -> Revision {
        let value: i64 = fixture
            .database
            .connection()
            .expect("connection")
            .query_row(
                "SELECT revision FROM conversation_turns WHERE conversation_id = ?1 AND id = ?2",
                params![fixture.conversation_id.to_string(), turn_id.to_string()],
                |row| row.get(0),
            )
            .expect("turn revision");
        Revision::new(u64::try_from(value).expect("revision"))
    }

    fn turn_intent(fixture: &Fixture, turn_id: GenerationTurnId) -> Vec<Option<String>> {
        fixture
            .database
            .connection()
            .expect("connection")
            .query_row(
                "SELECT branch_id, operation, input_kind, user_message_id, head_message_id, candidate_message_id, candidate_id, target_kind, target_message_id, target_parent_message_id, target_prior_candidate_id, guidance, requested_model_override_json, forced_speaker_participant_id, CAST(swap_roles AS TEXT) FROM conversation_turns WHERE conversation_id = ?1 AND id = ?2",
                params![fixture.conversation_id.to_string(), turn_id.to_string()],
                |row| (0..15).map(|index| row.get(index)).collect(),
            )
            .expect("turn intent")
    }

    #[test]
    fn send_commits_the_user_message_and_opens_one_attempt() {
        let fixture = direct_fixture();
        let asset_id = stage_media_asset(&fixture.database, "ab");
        let parts = vec![
            MessagePart::Text {
                text: "hello".into(),
            },
            MessagePart::MediaAsset {
                asset_id,
                role: lettuce_conversations::MediaAssetRole::Inline,
            },
        ];
        let command = send_command(&fixture, "send-one", "cd", parts);
        let result = fixture
            .database
            .begin_send(&command, TimestampMillis::new(20))
            .expect("send");

        assert_eq!(result.value.turn.operation, GenerationOperation::Send);
        assert_eq!(result.value.turn.status, GenerationTurnStatus::Created);
        assert_eq!(result.value.turn.revision, Revision::INITIAL);
        assert_eq!(result.value.turn.attempts.len(), 1);
        assert_eq!(result.value.attempt.ordinal, 0);
        assert_eq!(result.value.attempt.parent_attempt_id, None);
        assert_eq!(result.value.attempt.job_id, None);
        assert_eq!(
            result.value.attempt.job_idempotency_key,
            attempt_job_idempotency_key(result.value.turn.id, result.value.attempt.id)
        );
        assert_eq!(result.value.conversation.revision, Revision::new(2));
        assert_eq!(result.operation.kind, OperationKind::Send);

        let GenerationInput::UserMessage { message_id } = result.value.turn.input else {
            panic!("expected a user-message input");
        };
        let GenerationTarget::NewAssistant {
            message_id: target_message_id,
            parent_message_id,
        } = result.value.turn.target
        else {
            panic!("expected a new assistant target");
        };
        assert_eq!(parent_message_id, Some(message_id));
        assert_ne!(target_message_id, message_id);
        let target_exists: bool = scalar(
            &fixture.database,
            "SELECT EXISTS(SELECT 1 FROM conversation_messages WHERE id = ?1)",
            &target_message_id.to_string(),
        );
        assert!(!target_exists);

        assert_eq!(result.outbox.len(), 1);
        assert_eq!(result.outbox[0].sequence, 2);
        assert_eq!(result.outbox[0].conversation_revision, Revision::new(2));
        let lettuce_conversations::ConversationOutboxEvent::MessageCommitted {
            message_id: committed,
            revision_id,
            candidate_id,
            ..
        } = &result.outbox[0].event
        else {
            panic!("expected a message-committed event");
        };
        assert_eq!(*committed, message_id);
        assert!(revision_id.is_some());
        assert_eq!(*candidate_id, None);

        let stored_key: String = scalar(
            &fixture.database,
            "SELECT idempotency_key FROM conversation_turns WHERE id = ?1",
            &result.value.turn.id.to_string(),
        );
        assert_eq!(stored_key, "send.send-one");
        let ordinal: i64 = scalar(
            &fixture.database,
            "SELECT timeline_ordinal FROM conversation_messages WHERE id = ?1",
            &message_id.to_string(),
        );
        assert_eq!(ordinal, 1);
        let next_ordinal: i64 = scalar(
            &fixture.database,
            "SELECT next_timeline_ordinal FROM conversations WHERE id = ?1",
            &fixture.conversation_id.to_string(),
        );
        assert_eq!(next_ordinal, 2);
        let head: Option<String> = scalar(
            &fixture.database,
            "SELECT head_message_id FROM conversation_branches WHERE id = ?1",
            &fixture.branch_id.to_string(),
        );
        assert_eq!(head, Some(message_id.to_string()));
        let media_refs: i64 = scalar(
            &fixture.database,
            "SELECT count(*) FROM revision_media_refs WHERE conversation_id = ?1 AND part_ordinal = 1 AND state = 'active'",
            &fixture.conversation_id.to_string(),
        );
        assert_eq!(media_refs, 1);

        let timeline = ConversationReader::timeline_page(
            fixture.database.as_ref(),
            fixture.conversation_id,
            fixture.branch_id,
            &PageRequest {
                cursor: None,
                limit: PageLimit::new(20),
            },
        )
        .expect("timeline");
        assert_eq!(timeline.items.len(), 1);
        assert_eq!(timeline.items[0].message.id, message_id);
        assert_eq!(
            timeline.items[0]
                .active_revision
                .as_ref()
                .expect("revision")
                .parts,
            command.message.parts
        );
    }

    #[test]
    fn send_replays_on_its_token_and_conflicts_on_a_changed_digest() {
        let fixture = direct_fixture();
        let command = send_command(&fixture, "send-replay", "cd", text("hello"));
        let first = fixture
            .database
            .begin_send(&command, TimestampMillis::new(20))
            .expect("send");
        let replay = fixture
            .database
            .begin_send(&command, TimestampMillis::new(30))
            .expect("replay");
        assert_eq!(replay.operation, first.operation);
        assert_eq!(replay.outbox, first.outbox);
        assert_eq!(replay.value.turn.id, first.value.turn.id);
        let messages: i64 = scalar(
            &fixture.database,
            "SELECT count(*) FROM conversation_messages WHERE conversation_id = ?1",
            &fixture.conversation_id.to_string(),
        );
        assert_eq!(messages, 1);
        assert_eq!(conversation_revision(&fixture), Revision::new(2));

        let mut changed = send_command(&fixture, "send-replay", "ef", text("hello"));
        changed.expected_revision = Revision::new(2);
        assert_eq!(
            fixture
                .database
                .begin_send(&changed, TimestampMillis::new(40)),
            Err(ConversationRepositoryError::Conflict)
        );
    }

    #[test]
    fn send_rejects_stale_revisions_foreign_branches_and_archived_conversations() {
        let fixture = direct_fixture();
        let mut stale = send_command(&fixture, "send-stale", "cd", text("hello"));
        stale.expected_revision = Revision::new(9);
        assert_eq!(
            fixture
                .database
                .begin_send(&stale, TimestampMillis::new(20)),
            Err(ConversationRepositoryError::StaleRevision {
                expected: Revision::new(9),
                actual: Revision::INITIAL,
            })
        );

        let mut foreign = send_command(&fixture, "send-branch", "cd", text("hello"));
        foreign.branch_id = ConversationBranchId::new();
        assert_eq!(
            fixture
                .database
                .begin_send(&foreign, TimestampMillis::new(21)),
            Err(ConversationRepositoryError::Conflict)
        );

        fixture
            .database
            .connection()
            .expect("connection")
            .execute(
                "UPDATE conversations SET lifecycle = 'archived' WHERE id = ?1",
                [fixture.conversation_id.to_string()],
            )
            .expect("archive");
        let archived = send_command(&fixture, "send-archived", "cd", text("hello"));
        assert_eq!(
            fixture
                .database
                .begin_send(&archived, TimestampMillis::new(22)),
            Err(ConversationRepositoryError::Conflict)
        );
        let turns: i64 = scalar(
            &fixture.database,
            "SELECT count(*) FROM conversation_turns WHERE conversation_id = ?1",
            &fixture.conversation_id.to_string(),
        );
        assert_eq!(turns, 0);
    }

    #[test]
    fn a_conversation_holds_at_most_one_live_turn() {
        let mut fixture = direct_fixture();
        let first = fixture
            .database
            .begin_send(
                &send_command(&fixture, "send-live-one", "cd", text("first")),
                TimestampMillis::new(20),
            )
            .expect("first send");
        fixture.revision = first.value.conversation.revision;

        assert_eq!(
            fixture.database.begin_send(
                &send_command(&fixture, "send-live-two", "cd", text("second")),
                TimestampMillis::new(21)
            ),
            Err(ConversationRepositoryError::Conflict)
        );

        settle_failed(&fixture, &first.value.turn, 22);
        fixture.revision = conversation_revision(&fixture);
        let second = fixture
            .database
            .begin_send(
                &send_command(&fixture, "send-live-two", "cd", text("second")),
                TimestampMillis::new(23),
            )
            .expect("second send");
        assert_ne!(second.value.turn.id, first.value.turn.id);
        let ordinals: Vec<i64> = fixture
            .database
            .connection()
            .expect("connection")
            .prepare("SELECT timeline_ordinal FROM conversation_messages WHERE conversation_id = ?1 ORDER BY timeline_ordinal")
            .expect("statement")
            .query_map([fixture.conversation_id.to_string()], |row| row.get(0))
            .expect("ordinals")
            .collect::<rusqlite::Result<_>>()
            .expect("ordinals");
        assert_eq!(ordinals, vec![1, 2]);
    }

    #[test]
    fn continue_requires_a_branch_head() {
        let mut fixture = direct_fixture();
        let empty = ContinueConversation {
            conversation_id: fixture.conversation_id,
            branch_id: fixture.branch_id,
            expected_revision: fixture.revision,
            forced_speaker: None,
            swap_roles: false,
            operation: token("continue-empty", "cd"),
        };
        assert_eq!(
            fixture
                .database
                .begin_continue(&empty, TimestampMillis::new(20)),
            Err(ConversationRepositoryError::Conflict)
        );

        let send = fixture
            .database
            .begin_send(
                &send_command(&fixture, "send-before-continue", "cd", text("hello")),
                TimestampMillis::new(21),
            )
            .expect("send");
        settle_failed(&fixture, &send.value.turn, 22);
        fixture.revision = conversation_revision(&fixture);
        let command = ContinueConversation {
            conversation_id: fixture.conversation_id,
            branch_id: fixture.branch_id,
            expected_revision: fixture.revision,
            forced_speaker: None,
            swap_roles: false,
            operation: token("continue-head", "cd"),
        };
        let result = fixture
            .database
            .begin_continue(&command, TimestampMillis::new(23))
            .expect("continue");
        let GenerationInput::UserMessage { message_id } = send.value.turn.input else {
            panic!("expected a user-message input");
        };
        assert_eq!(
            result.value.turn.input,
            GenerationInput::ExistingHead {
                head_message_id: message_id
            }
        );
        assert_eq!(result.value.turn.operation, GenerationOperation::Continue);
        assert_eq!(result.value.turn.guidance, None);
        assert_eq!(result.value.turn.requested_model_override, None);
        assert!(result.outbox.is_empty());
        let messages: i64 = scalar(
            &fixture.database,
            "SELECT count(*) FROM conversation_messages WHERE conversation_id = ?1",
            &fixture.conversation_id.to_string(),
        );
        assert_eq!(messages, 1);
    }

    #[test]
    fn regenerate_enforces_the_target_candidate_context() {
        let mut fixture = direct_fixture();
        let send = fixture
            .database
            .begin_send(
                &send_command(&fixture, "send-before-regenerate", "cd", text("hello")),
                TimestampMillis::new(20),
            )
            .expect("send");
        let (message_id, candidate_id) = settle_succeeded(&fixture, &send.value.turn, 21);
        fixture.revision = conversation_revision(&fixture);
        let source_revision = turn_revision(&fixture, send.value.turn.id);

        let mut command = RegenerateCandidate {
            conversation_id: fixture.conversation_id,
            branch_id: fixture.branch_id,
            message_id,
            turn_id: send.value.turn.id,
            expected_revision: fixture.revision,
            expected_turn_revision: source_revision,
            operation: token("regenerate-wrong", "cd"),
            active_candidate_id: MessageCandidateId::new(),
            guidance: Some("more detail".into()),
            model_override: Some(model_snapshot()),
            forced_speaker: None,
            swap_roles: false,
        };
        assert_eq!(
            fixture
                .database
                .begin_regenerate(&command, TimestampMillis::new(22)),
            Err(ConversationRepositoryError::Conflict),
            "an unknown candidate is not owned by the source turn"
        );

        command.operation = token("regenerate-blank", "cd");
        command.active_candidate_id = candidate_id;
        command.guidance = Some("   ".into());
        assert!(matches!(
            fixture
                .database
                .begin_regenerate(&command, TimestampMillis::new(22)),
            Err(ConversationRepositoryError::Invalid(_))
        ));
        command.guidance = Some("more detail".into());

        command.operation = token("regenerate-speaker", "cd");
        command.forced_speaker = Some(fixture.characters[0]);
        assert_eq!(
            fixture
                .database
                .begin_regenerate(&command, TimestampMillis::new(22)),
            Err(invalid("generation_turn.direct_speaker"))
        );
        command.forced_speaker = None;

        command.operation = token("regenerate-ok", "cd");
        let result = fixture
            .database
            .begin_regenerate(&command, TimestampMillis::new(23))
            .expect("regenerate");
        assert_eq!(result.value.turn.operation, GenerationOperation::Regenerate);
        assert_eq!(
            result.value.turn.input,
            GenerationInput::ExistingCandidate {
                message_id,
                candidate_id
            }
        );
        assert_eq!(
            result.value.turn.target,
            GenerationTarget::ExistingCandidate {
                message_id,
                prior_candidate_id: candidate_id
            }
        );
        assert_eq!(result.value.turn.guidance.as_deref(), Some("more detail"));
        assert_eq!(
            result.value.turn.requested_model_override,
            command.model_override
        );
        assert!(result.outbox.is_empty());
        assert_eq!(
            turn_revision(&fixture, send.value.turn.id),
            source_revision,
            "the source turn is untouched"
        );

        let stale = RegenerateCandidate {
            expected_revision: result.value.conversation.revision,
            operation: token("regenerate-stale", "cd"),
            ..command
        };
        assert_eq!(
            fixture
                .database
                .begin_regenerate(&stale, TimestampMillis::new(24)),
            Err(ConversationRepositoryError::Conflict),
            "the regenerate turn is still live"
        );
    }

    #[test]
    fn retry_copies_the_source_intent_and_refuses_a_settled_success() {
        let mut fixture = direct_fixture();
        let send = fixture
            .database
            .begin_send(
                &send_command(&fixture, "shared-key", "cd", text("hello")),
                TimestampMillis::new(20),
            )
            .expect("send");
        settle_failed(&fixture, &send.value.turn, 21);
        fixture.revision = conversation_revision(&fixture);
        let command = RetryGeneration {
            conversation_id: fixture.conversation_id,
            branch_id: fixture.branch_id,
            turn_id: send.value.turn.id,
            expected_revision: fixture.revision,
            expected_turn_revision: turn_revision(&fixture, send.value.turn.id),
            operation: token("shared-key", "cd"),
        };
        let result = fixture
            .database
            .begin_retry(&command, TimestampMillis::new(22))
            .expect("retry");
        assert_eq!(result.value.turn.retry_of_turn_id, Some(send.value.turn.id));
        assert_eq!(result.value.turn.status, GenerationTurnStatus::Created);
        assert_eq!(result.value.turn.attempts.len(), 1);
        assert_eq!(
            turn_intent(&fixture, result.value.turn.id),
            turn_intent(&fixture, send.value.turn.id),
            "retry copies every intent column verbatim"
        );
        let keys: Vec<String> = fixture
            .database
            .connection()
            .expect("connection")
            .prepare("SELECT idempotency_key FROM conversation_turns WHERE conversation_id = ?1 ORDER BY idempotency_key")
            .expect("statement")
            .query_map([fixture.conversation_id.to_string()], |row| row.get(0))
            .expect("keys")
            .collect::<rusqlite::Result<_>>()
            .expect("keys");
        assert_eq!(
            keys,
            vec!["retry.shared-key".to_owned(), "send.shared-key".to_owned()]
        );

        settle_failed(&fixture, &result.value.turn, 23);
        let succeeded_send = fixture
            .database
            .begin_send(
                &SendConversation {
                    expected_revision: conversation_revision(&fixture),
                    operation: token("send-after-retry", "cd"),
                    ..send_command(&fixture, "unused", "cd", text("again"))
                },
                TimestampMillis::new(24),
            )
            .expect("second send");
        settle_succeeded(&fixture, &succeeded_send.value.turn, 25);
        let refused = RetryGeneration {
            conversation_id: fixture.conversation_id,
            branch_id: fixture.branch_id,
            turn_id: succeeded_send.value.turn.id,
            expected_revision: conversation_revision(&fixture),
            expected_turn_revision: turn_revision(&fixture, succeeded_send.value.turn.id),
            operation: token("retry-succeeded", "cd"),
        };
        assert_eq!(
            fixture
                .database
                .begin_retry(&refused, TimestampMillis::new(26)),
            Err(ConversationRepositoryError::Conflict)
        );
    }

    #[test]
    fn checkpoints_are_contiguous_and_drive_legal_transitions() {
        let fixture = direct_fixture();
        let send = fixture
            .database
            .begin_send(
                &send_command(&fixture, "send-checkpoints", "cd", text("hello")),
                TimestampMillis::new(20),
            )
            .expect("send");
        let turn_id = send.value.turn.id;
        let attempt_id = send.value.attempt.id;
        let envelope =
            |sequence: u64, event: GenerationCheckpointEvent| GenerationCheckpointEnvelope {
                turn_id,
                attempt_id,
                job_id: None,
                correlation_id: None,
                sequence,
                event,
            };

        let first = fixture
            .database
            .append_event(
                turn_id,
                Revision::INITIAL,
                &token("checkpoint-one", "cd"),
                envelope(
                    1,
                    GenerationCheckpointEvent::Stage {
                        status: GenerationTurnStatus::Preparing,
                    },
                ),
                TimestampMillis::new(21),
            )
            .expect("first checkpoint");
        assert_eq!(first.value.status, GenerationTurnStatus::Preparing);
        assert_eq!(first.value.revision, Revision::new(2));
        assert!(first.outbox.is_empty());
        assert_eq!(
            conversation_revision(&fixture),
            Revision::new(2),
            "checkpoints never bump the conversation revision"
        );

        let second = fixture
            .database
            .append_event(
                turn_id,
                Revision::new(2),
                &token("checkpoint-two", "cd"),
                envelope(2, GenerationCheckpointEvent::Progress { emitted_parts: 3 }),
                TimestampMillis::new(22),
            )
            .expect("second checkpoint");
        assert_eq!(second.value.status, GenerationTurnStatus::Preparing);
        assert_eq!(second.value.revision, Revision::new(3));

        assert_eq!(
            fixture.database.append_event(
                turn_id,
                Revision::new(3),
                &token("checkpoint-gap", "cd"),
                envelope(4, GenerationCheckpointEvent::Progress { emitted_parts: 4 }),
                TimestampMillis::new(23),
            ),
            Err(ConversationRepositoryError::Conflict)
        );
        assert_eq!(
            fixture.database.append_event(
                turn_id,
                Revision::new(3),
                &token("checkpoint-illegal", "cd"),
                envelope(
                    3,
                    GenerationCheckpointEvent::Stage {
                        status: GenerationTurnStatus::Running,
                    },
                ),
                TimestampMillis::new(24),
            ),
            Err(ConversationRepositoryError::Conflict)
        );
        assert_eq!(turn_revision(&fixture, turn_id), Revision::new(3));
        assert_eq!(
            fixture.database.append_event(
                turn_id,
                Revision::new(3),
                &token("checkpoint-cancel", "cd"),
                envelope(
                    3,
                    GenerationCheckpointEvent::Stage {
                        status: GenerationTurnStatus::CancellationRequested,
                    },
                ),
                TimestampMillis::new(24),
            ),
            Err(ConversationRepositoryError::Conflict),
            "cancellation belongs to the cancellation mutations"
        );
        let checkpoints: i64 = scalar(
            &fixture.database,
            "SELECT count(*) FROM generation_checkpoints WHERE turn_id = ?1",
            &turn_id.to_string(),
        );
        assert_eq!(checkpoints, 2);

        let mut sequence = 2;
        let mut revision = 3;
        for status in [
            GenerationTurnStatus::SelectingSpeaker,
            GenerationTurnStatus::ContextPrepared,
            GenerationTurnStatus::Running,
            GenerationTurnStatus::Finalizing,
        ] {
            sequence += 1;
            revision += 1;
            let step = fixture
                .database
                .append_event(
                    turn_id,
                    Revision::new(revision - 1),
                    &token(&format!("checkpoint-stage-{sequence}"), "cd"),
                    envelope(sequence, GenerationCheckpointEvent::Stage { status }),
                    TimestampMillis::new(24 + i64::try_from(sequence).expect("sequence")),
                )
                .expect("stage checkpoint");
            assert_eq!(step.value.status, status);
            assert_eq!(step.value.revision, Revision::new(revision));
        }

        assert_eq!(
            fixture.database.append_event(
                turn_id,
                Revision::new(99),
                &token("checkpoint-stale", "cd"),
                envelope(7, GenerationCheckpointEvent::Completed),
                TimestampMillis::new(35),
            ),
            Err(ConversationRepositoryError::StaleRevision {
                expected: Revision::new(99),
                actual: Revision::new(7),
            })
        );
        let unknown_turn = GenerationTurnId::new();
        assert_eq!(
            fixture.database.append_event(
                unknown_turn,
                Revision::INITIAL,
                &token("checkpoint-missing", "cd"),
                GenerationCheckpointEnvelope {
                    turn_id: unknown_turn,
                    attempt_id,
                    job_id: None,
                    correlation_id: None,
                    sequence: 1,
                    event: GenerationCheckpointEvent::Completed,
                },
                TimestampMillis::new(26),
            ),
            Err(ConversationRepositoryError::NotFound)
        );
        assert_eq!(
            fixture.database.append_event(
                turn_id,
                Revision::new(7),
                &token("checkpoint-foreign", "cd"),
                GenerationCheckpointEnvelope {
                    turn_id: unknown_turn,
                    attempt_id,
                    job_id: None,
                    correlation_id: None,
                    sequence: 7,
                    event: GenerationCheckpointEvent::Completed,
                },
                TimestampMillis::new(37),
            ),
            Err(invalid("checkpoint.turn_id"))
        );
    }

    #[test]
    fn attach_job_is_exclusive_per_attempt_and_per_job() {
        let mut fixture = direct_fixture();
        let send = fixture
            .database
            .begin_send(
                &send_command(&fixture, "send-attach", "cd", text("hello")),
                TimestampMillis::new(20),
            )
            .expect("send");
        fixture.revision = send.value.conversation.revision;
        let job_id = JobId::new();
        let attach = |key: &str, job: JobId, revision: Revision| AttachAttemptJob {
            conversation_id: fixture.conversation_id,
            turn_id: send.value.turn.id,
            attempt_id: send.value.attempt.id,
            expected_revision: revision,
            expected_turn_revision: Revision::INITIAL,
            operation: token(key, "cd"),
            job_id: job,
        };

        let attached = fixture
            .database
            .attach_attempt_job(
                &attach("attach-one", job_id, fixture.revision),
                TimestampMillis::new(21),
            )
            .expect("attach");
        assert_eq!(attached.value.job_id, Some(job_id));
        assert_eq!(attached.value.id, send.value.attempt.id);
        assert!(attached.outbox.is_empty());
        assert_eq!(conversation_revision(&fixture), Revision::new(3));
        assert_eq!(
            turn_revision(&fixture, send.value.turn.id),
            Revision::INITIAL,
            "attaching a job leaves the turn revision alone"
        );

        assert_eq!(
            fixture.database.attach_attempt_job(
                &attach("attach-two", JobId::new(), Revision::new(3)),
                TimestampMillis::new(22)
            ),
            Err(ConversationRepositoryError::JobAlreadyAttached)
        );

        let mut other = direct_fixture_on(fixture.database.clone());
        let other_send = other
            .database
            .begin_send(
                &send_command(&other, "send-other", "cd", text("hello")),
                TimestampMillis::new(20),
            )
            .expect("other send");
        other.revision = other_send.value.conversation.revision;

        let taken = AttachAttemptJob {
            conversation_id: other.conversation_id,
            turn_id: other_send.value.turn.id,
            attempt_id: other_send.value.attempt.id,
            expected_revision: other.revision,
            expected_turn_revision: Revision::INITIAL,
            operation: token("attach-taken", "cd"),
            job_id,
        };
        assert_eq!(
            other
                .database
                .attach_attempt_job(&taken, TimestampMillis::new(23)),
            Err(ConversationRepositoryError::JobInUse)
        );

        other
            .database
            .connection()
            .expect("connection")
            .execute(
                "UPDATE generation_attempts SET status = 'running', started_at = 24 WHERE conversation_id = ?1 AND id = ?2",
                params![
                    other.conversation_id.to_string(),
                    other_send.value.attempt.id.to_string(),
                ],
            )
            .expect("running attempt");
        assert_eq!(
            other.database.attach_attempt_job(
                &AttachAttemptJob {
                    operation: token("attach-running", "cd"),
                    job_id: JobId::new(),
                    ..taken
                },
                TimestampMillis::new(25)
            ),
            Err(ConversationRepositoryError::Conflict)
        );
    }

    #[test]
    fn group_turns_honor_the_speaker_and_swap_role_contracts() {
        let mut fixture = group_fixture();
        let mut swapped = send_command(&fixture, "group-swap", "cd", text("hello"));
        swapped.swap_roles = true;
        assert_eq!(
            fixture
                .database
                .begin_send(&swapped, TimestampMillis::new(20)),
            Err(invalid("generation_turn.swap_roles"))
        );

        let send = fixture
            .database
            .begin_send(
                &send_command(&fixture, "group-send", "cd", text("hello")),
                TimestampMillis::new(21),
            )
            .expect("group send");
        settle_cancelled(&fixture, &send.value.turn, 22);
        fixture.revision = conversation_revision(&fixture);
        let forced = fixture
            .database
            .begin_continue(
                &ContinueConversation {
                    conversation_id: fixture.conversation_id,
                    branch_id: fixture.branch_id,
                    expected_revision: fixture.revision,
                    forced_speaker: Some(fixture.characters[1]),
                    swap_roles: false,
                    operation: token("group-continue", "cd"),
                },
                TimestampMillis::new(23),
            )
            .expect("group continue");
        assert_eq!(
            forced.value.turn.forced_speaker,
            Some(fixture.characters[1])
        );

        let direct = direct_fixture();
        let direct_send = direct
            .database
            .begin_send(
                &send_command(&direct, "direct-send", "cd", text("hello")),
                TimestampMillis::new(20),
            )
            .expect("direct send");
        settle_failed(&direct, &direct_send.value.turn, 21);
        assert_eq!(
            direct.database.begin_continue(
                &ContinueConversation {
                    conversation_id: direct.conversation_id,
                    branch_id: direct.branch_id,
                    expected_revision: conversation_revision(&direct),
                    forced_speaker: Some(direct.characters[0]),
                    swap_roles: false,
                    operation: token("direct-forced", "cd"),
                },
                TimestampMillis::new(22)
            ),
            Err(invalid("generation_turn.direct_speaker"))
        );
    }

    #[test]
    fn checkpoints_bind_to_the_named_attempt_and_its_job() {
        let mut fixture = direct_fixture();
        let send = fixture
            .database
            .begin_send(
                &send_command(&fixture, "send-attempt-binding", "cd", text("hello")),
                TimestampMillis::new(20),
            )
            .expect("send");
        fixture.revision = send.value.conversation.revision;
        let turn_id = send.value.turn.id;
        let attempt_id = send.value.attempt.id;
        let job_id = JobId::new();
        let envelope = |attempt: GenerationAttemptId, job: Option<JobId>, sequence: u64| {
            GenerationCheckpointEnvelope {
                turn_id,
                attempt_id: attempt,
                job_id: job,
                correlation_id: None,
                sequence,
                event: GenerationCheckpointEvent::Progress { emitted_parts: 1 },
            }
        };

        let other = direct_fixture_on(fixture.database.clone());
        let foreign = other
            .database
            .begin_send(
                &send_command(&other, "send-foreign-attempt", "cd", text("hello")),
                TimestampMillis::new(20),
            )
            .expect("foreign send");
        assert_eq!(
            fixture.database.append_event(
                turn_id,
                Revision::INITIAL,
                &token("checkpoint-foreign-attempt", "cd"),
                envelope(foreign.value.attempt.id, None, 1),
                TimestampMillis::new(21),
            ),
            Err(ConversationRepositoryError::NotFound)
        );
        assert_eq!(
            fixture.database.append_event(
                turn_id,
                Revision::INITIAL,
                &token("checkpoint-early-job", "cd"),
                envelope(attempt_id, Some(job_id), 1),
                TimestampMillis::new(22),
            ),
            Err(ConversationRepositoryError::Conflict),
            "an unattached attempt cannot carry a job"
        );

        fixture
            .database
            .attach_attempt_job(
                &AttachAttemptJob {
                    conversation_id: fixture.conversation_id,
                    turn_id,
                    attempt_id,
                    expected_revision: fixture.revision,
                    expected_turn_revision: Revision::INITIAL,
                    operation: token("attach-binding", "cd"),
                    job_id,
                },
                TimestampMillis::new(23),
            )
            .expect("attach");
        assert_eq!(
            fixture.database.append_event(
                turn_id,
                Revision::INITIAL,
                &token("checkpoint-no-job", "cd"),
                envelope(attempt_id, None, 1),
                TimestampMillis::new(24),
            ),
            Err(ConversationRepositoryError::Conflict),
            "an attached attempt must be named by its job"
        );
        let matched = fixture
            .database
            .append_event(
                turn_id,
                Revision::INITIAL,
                &token("checkpoint-matched-job", "cd"),
                envelope(attempt_id, Some(job_id), 1),
                TimestampMillis::new(25),
            )
            .expect("matching job");
        assert_eq!(matched.value.revision, Revision::new(2));

        settle_failed(&fixture, &matched.value, 26);
        assert_eq!(
            fixture.database.append_event(
                turn_id,
                turn_revision(&fixture, turn_id),
                &token("checkpoint-settled", "cd"),
                envelope(attempt_id, Some(job_id), 2),
                TimestampMillis::new(27),
            ),
            Err(ConversationRepositoryError::Conflict),
            "a settled attempt takes no more checkpoints"
        );
    }

    #[test]
    fn archived_conversations_settle_in_flight_work_but_admit_none() {
        let mut fixture = direct_fixture();
        let send = fixture
            .database
            .begin_send(
                &send_command(&fixture, "send-archive-midturn", "cd", text("hello")),
                TimestampMillis::new(20),
            )
            .expect("send");
        fixture.revision = send.value.conversation.revision;
        fixture
            .database
            .archive(
                &ArchiveConversation {
                    conversation_id: fixture.conversation_id,
                    expected_revision: fixture.revision,
                    operation: token("archive-midturn", "cd"),
                },
                TimestampMillis::new(21),
            )
            .expect("archive");
        fixture.revision = conversation_revision(&fixture);

        let job_id = JobId::new();
        let attached = fixture
            .database
            .attach_attempt_job(
                &AttachAttemptJob {
                    conversation_id: fixture.conversation_id,
                    turn_id: send.value.turn.id,
                    attempt_id: send.value.attempt.id,
                    expected_revision: fixture.revision,
                    expected_turn_revision: Revision::INITIAL,
                    operation: token("attach-archived", "cd"),
                    job_id,
                },
                TimestampMillis::new(22),
            )
            .expect("an archived conversation still attaches its job");
        assert_eq!(attached.value.job_id, Some(job_id));
        fixture.revision = conversation_revision(&fixture);
        let revision = drive(
            &fixture,
            send.value.turn.id,
            send.value.attempt.id,
            &[
                GenerationTurnStatus::Preparing,
                GenerationTurnStatus::ContextPrepared,
                GenerationTurnStatus::Running,
            ],
            "drive-archived",
            23,
        );
        let finalized = fixture
            .database
            .finalize_generation(
                send.value.turn.id,
                send.value.attempt.id,
                conversation_revision(&fixture),
                revision,
                &token("finalize-archived", "cd"),
                finalization_draft(text("generated"), 0),
                UsageEventId::new(),
                TimestampMillis::new(30),
            )
            .expect("an archived conversation still settles its live turn");
        assert_eq!(finalized.value.turn.status, GenerationTurnStatus::Succeeded);
        assert_eq!(
            ConversationReader::get(fixture.database.as_ref(), fixture.conversation_id)
                .expect("aggregate")
                .conversation
                .lifecycle,
            ConversationLifecycle::Archived
        );
        fixture.revision = conversation_revision(&fixture);

        assert_eq!(
            fixture.database.begin_send(
                &send_command(&fixture, "send-while-archived", "cd", text("again")),
                TimestampMillis::new(31)
            ),
            Err(ConversationRepositoryError::Conflict),
            "new work waits for a restore"
        );
        fixture
            .database
            .restore(
                &RestoreConversation {
                    conversation_id: fixture.conversation_id,
                    expected_revision: fixture.revision,
                    operation: token("restore-midturn", "cd"),
                },
                TimestampMillis::new(32),
            )
            .expect("restore");
        fixture.revision = conversation_revision(&fixture);
        let resumed = fixture
            .database
            .begin_send(
                &send_command(&fixture, "send-after-restore", "cd", text("again")),
                TimestampMillis::new(33),
            )
            .expect("restored conversations admit work");

        fixture
            .database
            .connection()
            .expect("connection")
            .execute(
                "UPDATE conversations SET lifecycle = 'tombstoned' WHERE id = ?1",
                [fixture.conversation_id.to_string()],
            )
            .expect("raw tombstone: the purge port arrives in a later region");
        assert_eq!(
            fixture.database.append_event(
                resumed.value.turn.id,
                Revision::INITIAL,
                &token("checkpoint-tombstoned", "cd"),
                GenerationCheckpointEnvelope {
                    turn_id: resumed.value.turn.id,
                    attempt_id: resumed.value.attempt.id,
                    job_id: None,
                    correlation_id: None,
                    sequence: 1,
                    event: GenerationCheckpointEvent::Completed,
                },
                TimestampMillis::new(34),
            ),
            Err(ConversationRepositoryError::Conflict),
            "a tombstoned conversation settles nothing"
        );
    }
    #[test]
    fn an_over_long_composed_turn_key_is_rejected_before_any_write() {
        let fixture = direct_fixture();
        let too_long = "k".repeat(125);
        assert!(matches!(
            fixture.database.begin_send(
                &send_command(&fixture, &too_long, "cd", text("hello")),
                TimestampMillis::new(20)
            ),
            Err(ConversationRepositoryError::Invalid(
                lettuce_conversations::ValidationError::TooLarge { .. }
            ))
        ));
        let turns: i64 = scalar(
            &fixture.database,
            "SELECT count(*) FROM conversation_turns WHERE conversation_id = ?1",
            &fixture.conversation_id.to_string(),
        );
        assert_eq!(turns, 0);

        let longest = "k".repeat(123);
        let result = fixture
            .database
            .begin_send(
                &send_command(&fixture, &longest, "cd", text("hello")),
                TimestampMillis::new(21),
            )
            .expect("longest legal key");
        assert_eq!(
            result.value.turn.idempotency_key.as_str(),
            format!("send.{longest}")
        );
    }

    #[test]
    fn regenerate_refuses_a_candidate_from_another_turn() {
        let mut fixture = direct_fixture();
        let first = fixture
            .database
            .begin_send(
                &send_command(&fixture, "send-owner", "cd", text("hello")),
                TimestampMillis::new(20),
            )
            .expect("send");
        let (message_id, candidate_id) = settle_succeeded(&fixture, &first.value.turn, 21);
        fixture.revision = conversation_revision(&fixture);
        let second = fixture
            .database
            .begin_send(
                &send_command(&fixture, "send-unrelated", "cd", text("again")),
                TimestampMillis::new(22),
            )
            .expect("second send");
        settle_failed(&fixture, &second.value.turn, 23);
        fixture.revision = conversation_revision(&fixture);

        assert_eq!(
            fixture.database.begin_regenerate(
                &RegenerateCandidate {
                    conversation_id: fixture.conversation_id,
                    branch_id: fixture.branch_id,
                    message_id,
                    turn_id: second.value.turn.id,
                    expected_revision: fixture.revision,
                    expected_turn_revision: turn_revision(&fixture, second.value.turn.id),
                    operation: token("regenerate-unrelated", "cd"),
                    active_candidate_id: candidate_id,
                    guidance: None,
                    model_override: None,
                    forced_speaker: None,
                    swap_roles: false,
                },
                TimestampMillis::new(24)
            ),
            Err(ConversationRepositoryError::Conflict)
        );

        assert_eq!(
            fixture.database.begin_regenerate(
                &RegenerateCandidate {
                    conversation_id: fixture.conversation_id,
                    branch_id: fixture.branch_id,
                    message_id,
                    turn_id: first.value.turn.id,
                    expected_revision: fixture.revision,
                    expected_turn_revision: turn_revision(&fixture, first.value.turn.id),
                    operation: token("regenerate-stale-head", "cd"),
                    active_candidate_id: candidate_id,
                    guidance: None,
                    model_override: None,
                    forced_speaker: None,
                    swap_roles: false,
                },
                TimestampMillis::new(25)
            ),
            Err(invalid("regenerate.target_message")),
            "the owning turn's candidate is no longer the branch head"
        );
    }

    #[test]
    fn replays_verify_the_requested_identity() {
        let mut fixture = direct_fixture();
        let first = fixture
            .database
            .begin_send(
                &send_command(&fixture, "send-identity-one", "cd", text("hello")),
                TimestampMillis::new(20),
            )
            .expect("first send");
        let checkpoint = token("checkpoint-shared", "cd");
        fixture
            .database
            .append_event(
                first.value.turn.id,
                Revision::INITIAL,
                &checkpoint,
                GenerationCheckpointEnvelope {
                    turn_id: first.value.turn.id,
                    attempt_id: first.value.attempt.id,
                    job_id: None,
                    correlation_id: None,
                    sequence: 1,
                    event: GenerationCheckpointEvent::Completed,
                },
                TimestampMillis::new(21),
            )
            .expect("checkpoint");
        settle_failed(&fixture, &first.value.turn, 22);
        fixture.revision = conversation_revision(&fixture);
        fixture.revision = conversation_revision(&fixture);
        let second = fixture
            .database
            .begin_send(
                &send_command(&fixture, "send-identity-two", "cd", text("again")),
                TimestampMillis::new(23),
            )
            .expect("second send");
        fixture.revision = second.value.conversation.revision;

        assert_eq!(
            fixture.database.append_event(
                second.value.turn.id,
                Revision::INITIAL,
                &checkpoint,
                GenerationCheckpointEnvelope {
                    turn_id: second.value.turn.id,
                    attempt_id: second.value.attempt.id,
                    job_id: None,
                    correlation_id: None,
                    sequence: 1,
                    event: GenerationCheckpointEvent::Completed,
                },
                TimestampMillis::new(24),
            ),
            Err(ConversationRepositoryError::Conflict),
            "a replayed checkpoint must belong to the requested turn"
        );

        let attach = token("attach-identity", "cd");
        let job_id = JobId::new();
        fixture
            .database
            .attach_attempt_job(
                &AttachAttemptJob {
                    conversation_id: fixture.conversation_id,
                    turn_id: second.value.turn.id,
                    attempt_id: second.value.attempt.id,
                    expected_revision: fixture.revision,
                    expected_turn_revision: Revision::INITIAL,
                    operation: attach.clone(),
                    job_id,
                },
                TimestampMillis::new(25),
            )
            .expect("attach");
        assert_eq!(
            fixture.database.attach_attempt_job(
                &AttachAttemptJob {
                    conversation_id: fixture.conversation_id,
                    turn_id: second.value.turn.id,
                    attempt_id: second.value.attempt.id,
                    expected_revision: conversation_revision(&fixture),
                    expected_turn_revision: Revision::INITIAL,
                    operation: attach,
                    job_id: JobId::new(),
                },
                TimestampMillis::new(26)
            ),
            Err(ConversationRepositoryError::Conflict),
            "a replayed attach must name the job it attached"
        );
    }

    #[test]
    fn a_replay_returns_current_state_with_the_original_records() {
        let fixture = direct_fixture();
        let command = send_command(&fixture, "send-live-replay", "cd", text("hello"));
        let first = fixture
            .database
            .begin_send(&command, TimestampMillis::new(20))
            .expect("send");
        fixture
            .database
            .append_event(
                first.value.turn.id,
                Revision::INITIAL,
                &token("checkpoint-live-replay", "cd"),
                GenerationCheckpointEnvelope {
                    turn_id: first.value.turn.id,
                    attempt_id: first.value.attempt.id,
                    job_id: None,
                    correlation_id: None,
                    sequence: 1,
                    event: GenerationCheckpointEvent::Stage {
                        status: GenerationTurnStatus::Preparing,
                    },
                },
                TimestampMillis::new(21),
            )
            .expect("checkpoint");

        let replay = fixture
            .database
            .begin_send(&command, TimestampMillis::new(22))
            .expect("replay");
        assert_eq!(replay.operation, first.operation);
        assert_eq!(replay.outbox, first.outbox);
        assert_eq!(replay.value.turn.id, first.value.turn.id);
        assert_eq!(replay.value.turn.status, GenerationTurnStatus::Preparing);
        assert_eq!(replay.value.turn.revision, Revision::new(2));
        assert_eq!(first.value.turn.status, GenerationTurnStatus::Created);
    }

    #[test]
    fn finalize_commits_the_reserved_assistant_message() {
        let mut fixture = direct_fixture();
        let asset_id = stage_media_asset(&fixture.database, "ab");
        let send = fixture
            .database
            .begin_send(
                &send_command(&fixture, "send-finalize", "cd", text("hello")),
                TimestampMillis::new(20),
            )
            .expect("send");
        let turn_id = send.value.turn.id;
        let attempt_id = send.value.attempt.id;
        let GenerationTarget::NewAssistant {
            message_id: reserved,
            ..
        } = send.value.turn.target
        else {
            panic!("expected a new assistant target");
        };
        let turn_revision_before = drive(
            &fixture,
            turn_id,
            attempt_id,
            &[
                GenerationTurnStatus::Preparing,
                GenerationTurnStatus::ContextPrepared,
                GenerationTurnStatus::Running,
            ],
            "drive-finalize",
            21,
        );
        let usage_event_id = UsageEventId::new();
        let result = fixture
            .database
            .finalize_generation(
                turn_id,
                attempt_id,
                conversation_revision(&fixture),
                turn_revision_before,
                &token("finalize-happy", "cd"),
                finalization_draft(
                    vec![
                        MessagePart::Text {
                            text: "generated".into(),
                        },
                        MessagePart::MediaAsset {
                            asset_id,
                            role: lettuce_conversations::MediaAssetRole::Inline,
                        },
                    ],
                    0,
                ),
                usage_event_id,
                TimestampMillis::new(30),
            )
            .expect("finalize");

        assert_eq!(result.value.assistant_message.id, reserved);
        assert_eq!(result.value.candidate.message_id, reserved);
        assert_eq!(result.value.candidate.turn_id, turn_id);
        assert_eq!(result.value.candidate.attempt_id, attempt_id);
        assert_eq!(
            result.value.candidate.author_participant_id,
            fixture.characters[0]
        );
        assert_eq!(result.value.turn.status, GenerationTurnStatus::Succeeded);
        assert_eq!(
            result.value.turn.selected_candidate_id,
            Some(result.value.candidate.id)
        );
        assert_eq!(result.value.revision, None);
        assert_eq!(result.value.usage_event_id, usage_event_id);
        assert_eq!(result.value.asset_reference_deltas.len(), 1);
        assert_eq!(
            result.value.asset_reference_deltas[0].state,
            AssetReferenceState::Active
        );
        assert_eq!(
            result.value.turn.attempts[0].status,
            GenerationAttemptStatus::Succeeded
        );
        assert_eq!(
            result.value.turn.attempts[0].usage_event_id,
            Some(usage_event_id)
        );

        assert!(matches!(
            result.outbox[0].event,
            ConversationOutboxEvent::TurnFinalized {
                revision_id: None,
                ..
            }
        ));
        assert!(matches!(
            result.outbox[1].event,
            ConversationOutboxEvent::MessageCommitted {
                candidate_id: Some(_),
                revision_id: None,
                ..
            }
        ));
        assert!(matches!(
            result.outbox[2].event,
            ConversationOutboxEvent::AssetReferencesChanged { .. }
        ));
        assert_eq!(result.outbox.len(), 3);

        let head: Option<String> = scalar(
            &fixture.database,
            "SELECT head_message_id FROM conversation_branches WHERE id = ?1",
            &fixture.branch_id.to_string(),
        );
        assert_eq!(head, Some(reserved.to_string()));
        let usage_rows: i64 = scalar(
            &fixture.database,
            "SELECT count(*) FROM conversation_usage_refs WHERE usage_event_id = ?1",
            &usage_event_id.to_string(),
        );
        assert_eq!(usage_rows, 1);
        let ordinals: Vec<i64> = fixture
            .database
            .connection()
            .expect("connection")
            .prepare("SELECT timeline_ordinal FROM conversation_messages WHERE conversation_id = ?1 ORDER BY timeline_ordinal")
            .expect("statement")
            .query_map([fixture.conversation_id.to_string()], |row| row.get(0))
            .expect("ordinals")
            .collect::<rusqlite::Result<_>>()
            .expect("ordinals");
        assert_eq!(ordinals, vec![1, 2]);

        let timeline = ConversationReader::timeline_page(
            fixture.database.as_ref(),
            fixture.conversation_id,
            fixture.branch_id,
            &PageRequest {
                cursor: None,
                limit: PageLimit::new(20),
            },
        )
        .expect("timeline");
        assert_eq!(timeline.items.len(), 2);
        assert_eq!(timeline.items[0].message.id, reserved);
        assert!(timeline.items[0].active_candidate.is_some());
        assert!(timeline.items[0].active_revision.is_none());
        ConversationReader::get(fixture.database.as_ref(), fixture.conversation_id)
            .expect("aggregate");

        fixture.revision = conversation_revision(&fixture);
        fixture
            .database
            .begin_send(
                &send_command(&fixture, "send-after-finalize", "cd", text("again")),
                TimestampMillis::new(40),
            )
            .expect("the finalized turn is no longer live");
    }

    #[test]
    fn regenerate_finalize_swaps_the_active_candidate() {
        let mut fixture = direct_fixture();
        let send = fixture
            .database
            .begin_send(
                &send_command(&fixture, "send-regen-finalize", "cd", text("hello")),
                TimestampMillis::new(20),
            )
            .expect("send");
        let (message_id, first_candidate) = settle_succeeded(&fixture, &send.value.turn, 21);
        fixture.revision = conversation_revision(&fixture);
        let regenerate = fixture
            .database
            .begin_regenerate(
                &RegenerateCandidate {
                    conversation_id: fixture.conversation_id,
                    branch_id: fixture.branch_id,
                    message_id,
                    turn_id: send.value.turn.id,
                    expected_revision: fixture.revision,
                    expected_turn_revision: turn_revision(&fixture, send.value.turn.id),
                    operation: token("regen-finalize", "cd"),
                    active_candidate_id: first_candidate,
                    guidance: None,
                    model_override: None,
                    forced_speaker: None,
                    swap_roles: false,
                },
                TimestampMillis::new(80),
            )
            .expect("regenerate");
        let attempt_id = regenerate.value.attempt.id;
        let revision = drive(
            &fixture,
            regenerate.value.turn.id,
            attempt_id,
            &[
                GenerationTurnStatus::Preparing,
                GenerationTurnStatus::ContextPrepared,
                GenerationTurnStatus::Running,
            ],
            "drive-regen",
            81,
        );
        let result = fixture
            .database
            .finalize_generation(
                regenerate.value.turn.id,
                attempt_id,
                conversation_revision(&fixture),
                revision,
                &token("finalize-regen", "cd"),
                finalization_draft(text("second take"), 1),
                UsageEventId::new(),
                TimestampMillis::new(90),
            )
            .expect("finalize regenerate");

        assert_eq!(result.value.assistant_message.id, message_id);
        assert_ne!(result.value.candidate.id, first_candidate);
        assert_eq!(
            result.value.assistant_message.active_render_source,
            lettuce_conversations::MessageRenderSource::Candidate(result.value.candidate.id)
        );
        assert_eq!(result.outbox.len(), 1);
        assert!(matches!(
            result.outbox[0].event,
            ConversationOutboxEvent::TurnFinalized { .. }
        ));
        let candidates: i64 = scalar(
            &fixture.database,
            "SELECT count(*) FROM conversation_message_candidates WHERE message_id = ?1",
            &message_id.to_string(),
        );
        assert_eq!(candidates, 2);
        let head: Option<String> = scalar(
            &fixture.database,
            "SELECT head_message_id FROM conversation_branches WHERE id = ?1",
            &fixture.branch_id.to_string(),
        );
        assert_eq!(head, Some(message_id.to_string()), "the head is unchanged");
        let messages: i64 = scalar(
            &fixture.database,
            "SELECT count(*) FROM conversation_messages WHERE conversation_id = ?1",
            &fixture.conversation_id.to_string(),
        );
        assert_eq!(messages, 2);
    }

    #[test]
    fn group_finalization_resolves_the_forced_speaker() {
        let mut fixture = group_fixture();
        let send = fixture
            .database
            .begin_send(
                &send_command(&fixture, "group-finalize-send", "cd", text("hello")),
                TimestampMillis::new(20),
            )
            .expect("group send");
        settle_cancelled(&fixture, &send.value.turn, 21);
        fixture.revision = conversation_revision(&fixture);

        let first_speaker = fixture.characters[1];
        let continued = fixture
            .database
            .begin_continue(
                &ContinueConversation {
                    conversation_id: fixture.conversation_id,
                    branch_id: fixture.branch_id,
                    expected_revision: fixture.revision,
                    forced_speaker: Some(first_speaker),
                    swap_roles: false,
                    operation: token("group-finalize-continue", "cd"),
                },
                TimestampMillis::new(30),
            )
            .expect("group continue");
        let revision = drive(
            &fixture,
            continued.value.turn.id,
            continued.value.attempt.id,
            &[
                GenerationTurnStatus::Preparing,
                GenerationTurnStatus::ContextPrepared,
                GenerationTurnStatus::Running,
            ],
            "drive-group",
            31,
        );
        let finalized = fixture
            .database
            .finalize_generation(
                continued.value.turn.id,
                continued.value.attempt.id,
                conversation_revision(&fixture),
                revision,
                &token("group-finalize", "cd"),
                finalization_draft(text("first speaker"), 0),
                UsageEventId::new(),
                TimestampMillis::new(40),
            )
            .expect("group finalize");
        assert_eq!(
            finalized.value.candidate.author_participant_id,
            first_speaker
        );
        assert_eq!(
            finalized.value.assistant_message.author_participant_id,
            Some(first_speaker)
        );

        let second_speaker = fixture.characters[0];
        fixture.revision = conversation_revision(&fixture);
        let regenerate = fixture
            .database
            .begin_regenerate(
                &RegenerateCandidate {
                    conversation_id: fixture.conversation_id,
                    branch_id: fixture.branch_id,
                    message_id: finalized.value.assistant_message.id,
                    turn_id: continued.value.turn.id,
                    expected_revision: fixture.revision,
                    expected_turn_revision: turn_revision(&fixture, continued.value.turn.id),
                    operation: token("group-regen", "cd"),
                    active_candidate_id: finalized.value.candidate.id,
                    guidance: None,
                    model_override: None,
                    forced_speaker: Some(second_speaker),
                    swap_roles: false,
                },
                TimestampMillis::new(50),
            )
            .expect("group regenerate");
        let revision = drive(
            &fixture,
            regenerate.value.turn.id,
            regenerate.value.attempt.id,
            &[
                GenerationTurnStatus::Preparing,
                GenerationTurnStatus::ContextPrepared,
                GenerationTurnStatus::Running,
            ],
            "drive-group-regen",
            51,
        );
        let reauthored = fixture
            .database
            .finalize_generation(
                regenerate.value.turn.id,
                regenerate.value.attempt.id,
                conversation_revision(&fixture),
                revision,
                &token("group-regen-finalize", "cd"),
                finalization_draft(text("second speaker"), 1),
                UsageEventId::new(),
                TimestampMillis::new(60),
            )
            .expect("group regenerate finalize");
        assert_eq!(
            reauthored.value.candidate.author_participant_id,
            second_speaker
        );
        assert_eq!(
            reauthored.value.assistant_message.author_participant_id,
            Some(second_speaker),
            "the message author follows its active candidate"
        );
    }

    #[test]
    fn failure_settles_from_running_and_from_a_cancellation_request() {
        let mut fixture = direct_fixture();
        let send = fixture
            .database
            .begin_send(
                &send_command(&fixture, "send-fail-running", "cd", text("hello")),
                TimestampMillis::new(20),
            )
            .expect("send");
        let attempt_id = send.value.attempt.id;
        let revision = drive(
            &fixture,
            send.value.turn.id,
            attempt_id,
            &[
                GenerationTurnStatus::Preparing,
                GenerationTurnStatus::ContextPrepared,
                GenerationTurnStatus::Running,
            ],
            "drive-fail-running",
            21,
        );
        let usage_event_id = UsageEventId::new();
        let failed = fixture
            .database
            .fail_generation(
                send.value.turn.id,
                attempt_id,
                conversation_revision(&fixture),
                revision,
                &token("fail-running", "cd"),
                GenerationFailureCode::ProviderRejected,
                usage_event_id,
                TimestampMillis::new(30),
            )
            .expect("fail");
        assert_eq!(failed.value.turn.status, GenerationTurnStatus::Failed);
        assert_eq!(
            failed.value.turn.failure,
            Some(GenerationFailureCode::ProviderRejected)
        );
        assert_eq!(
            failed.value.turn.attempts[0].failure,
            Some(GenerationFailureCode::ProviderRejected)
        );
        assert_eq!(failed.value.usage_event_id, usage_event_id);
        assert_eq!(failed.outbox.len(), 1);
        assert!(matches!(
            failed.outbox[0].event,
            ConversationOutboxEvent::TurnFailed { .. }
        ));

        fixture.revision = conversation_revision(&fixture);
        let second = fixture
            .database
            .begin_send(
                &send_command(&fixture, "send-fail-cancelling", "cd", text("again")),
                TimestampMillis::new(40),
            )
            .expect("second send");
        let requested = fixture
            .database
            .request_cancellation(
                &CancelGeneration {
                    conversation_id: fixture.conversation_id,
                    turn_id: second.value.turn.id,
                    attempt_id: second.value.attempt.id,
                    expected_revision: conversation_revision(&fixture),
                    expected_turn_revision: Revision::INITIAL,
                    operation: token("cancel-then-fail", "cd"),
                },
                TimestampMillis::new(41),
            )
            .expect("request cancellation");
        assert_eq!(
            requested.value.status,
            GenerationTurnStatus::CancellationRequested
        );
        let late = fixture
            .database
            .fail_generation(
                second.value.turn.id,
                second.value.attempt.id,
                conversation_revision(&fixture),
                requested.value.revision,
                &token("fail-cancelling", "cd"),
                GenerationFailureCode::Internal,
                UsageEventId::new(),
                TimestampMillis::new(42),
            )
            .expect("fail after a cancellation request");
        assert_eq!(late.value.turn.status, GenerationTurnStatus::Failed);
    }

    #[test]
    fn an_interrupted_turn_recovers_into_a_child_attempt_and_finalizes() {
        let mut fixture = direct_fixture();
        let send = fixture
            .database
            .begin_send(
                &send_command(&fixture, "send-recovery", "cd", text("hello")),
                TimestampMillis::new(20),
            )
            .expect("send");
        let turn_id = send.value.turn.id;
        let first_attempt = send.value.attempt.id;
        let revision = drive(
            &fixture,
            turn_id,
            first_attempt,
            &[
                GenerationTurnStatus::Preparing,
                GenerationTurnStatus::ContextPrepared,
                GenerationTurnStatus::Running,
            ],
            "drive-recovery",
            21,
        );
        let interrupted = fixture
            .database
            .interrupt_generation(
                turn_id,
                first_attempt,
                conversation_revision(&fixture),
                revision,
                &token("interrupt", "cd"),
                UsageEventId::new(),
                TimestampMillis::new(30),
            )
            .expect("interrupt");
        assert_eq!(interrupted.value.status, GenerationTurnStatus::Interrupted);
        assert_eq!(
            interrupted.value.attempts[0].status,
            GenerationAttemptStatus::Interrupted
        );
        assert!(interrupted.outbox.is_empty());

        let recovered = fixture
            .database
            .recover_generation(
                turn_id,
                first_attempt,
                conversation_revision(&fixture),
                interrupted.value.revision,
                &token("recover", "cd"),
                TimestampMillis::new(31),
            )
            .expect("recover");
        assert_eq!(
            recovered.value.turn.status,
            GenerationTurnStatus::Recovering
        );
        assert_eq!(recovered.value.turn.attempts.len(), 2);
        assert_eq!(recovered.value.attempt.ordinal, 1);
        assert_eq!(
            recovered.value.attempt.parent_attempt_id,
            Some(first_attempt)
        );
        assert_eq!(
            recovered.value.attempt.status,
            GenerationAttemptStatus::Created
        );
        assert_eq!(
            recovered.value.attempt.job_idempotency_key,
            attempt_job_idempotency_key(turn_id, recovered.value.attempt.id)
        );
        assert!(recovered.outbox.is_empty());

        let child = recovered.value.attempt.id;
        let revision = drive(
            &fixture,
            turn_id,
            child,
            &[GenerationTurnStatus::Running],
            "drive-child",
            32,
        );
        let finalized = fixture
            .database
            .finalize_generation(
                turn_id,
                child,
                conversation_revision(&fixture),
                revision,
                &token("finalize-recovered", "cd"),
                finalization_draft(text("recovered output"), 0),
                UsageEventId::new(),
                TimestampMillis::new(40),
            )
            .expect("finalize the recovered attempt");
        assert_eq!(finalized.value.turn.status, GenerationTurnStatus::Succeeded);
        assert_eq!(finalized.value.candidate.attempt_id, child);
        assert_eq!(finalized.value.turn.attempts.len(), 2);
        fixture.revision = conversation_revision(&fixture);
        ConversationReader::get(fixture.database.as_ref(), fixture.conversation_id)
            .expect("aggregate");
    }

    #[test]
    fn cancellation_settles_with_or_without_a_request() {
        let mut fixture = direct_fixture();
        let send = fixture
            .database
            .begin_send(
                &send_command(&fixture, "send-cancel-requested", "cd", text("hello")),
                TimestampMillis::new(20),
            )
            .expect("send");
        let requested = fixture
            .database
            .request_cancellation(
                &CancelGeneration {
                    conversation_id: fixture.conversation_id,
                    turn_id: send.value.turn.id,
                    attempt_id: send.value.attempt.id,
                    expected_revision: conversation_revision(&fixture),
                    expected_turn_revision: Revision::INITIAL,
                    operation: token("cancel-request", "cd"),
                },
                TimestampMillis::new(21),
            )
            .expect("request");
        assert_eq!(requested.outbox.len(), 1);
        assert!(matches!(
            requested.outbox[0].event,
            ConversationOutboxEvent::TurnCancellationRequested { .. }
        ));

        let wrong_attempt = SettleCancellation {
            conversation_id: fixture.conversation_id,
            turn_id: send.value.turn.id,
            attempt_id: GenerationAttemptId::new(),
            expected_revision: conversation_revision(&fixture),
            expected_turn_revision: requested.value.revision,
            operation: token("settle-wrong-attempt", "cd"),
            usage_event_id: UsageEventId::new(),
        };
        assert_eq!(
            fixture
                .database
                .settle_cancellation(&wrong_attempt, TimestampMillis::new(22)),
            Err(ConversationRepositoryError::NotFound)
        );

        let usage_event_id = UsageEventId::new();
        let settled = fixture
            .database
            .settle_cancellation(
                &SettleCancellation {
                    attempt_id: send.value.attempt.id,
                    operation: token("settle-request", "cd"),
                    usage_event_id,
                    ..wrong_attempt
                },
                TimestampMillis::new(23),
            )
            .expect("settle");
        assert_eq!(settled.value.turn.status, GenerationTurnStatus::Cancelled);
        assert_eq!(settled.value.usage_event_id, usage_event_id);
        assert_eq!(
            settled.value.turn.attempts[0].status,
            GenerationAttemptStatus::Cancelled
        );
        assert!(matches!(
            settled.outbox[0].event,
            ConversationOutboxEvent::TurnCancelled { .. }
        ));
        let replay = fixture
            .database
            .settle_cancellation(
                &SettleCancellation {
                    attempt_id: send.value.attempt.id,
                    operation: token("settle-request", "cd"),
                    usage_event_id,
                    expected_revision: Revision::new(99),
                    ..wrong_attempt
                },
                TimestampMillis::new(24),
            )
            .expect("settle replay");
        assert_eq!(replay.operation, settled.operation);
        assert_eq!(replay.outbox, settled.outbox);
        assert_eq!(replay.value.turn.status, GenerationTurnStatus::Cancelled);

        fixture.revision = conversation_revision(&fixture);
        let direct = fixture
            .database
            .begin_send(
                &send_command(&fixture, "send-cancel-direct", "cd", text("again")),
                TimestampMillis::new(30),
            )
            .expect("second send");
        let settled_from_created = fixture
            .database
            .settle_cancellation(
                &SettleCancellation {
                    conversation_id: fixture.conversation_id,
                    turn_id: direct.value.turn.id,
                    attempt_id: direct.value.attempt.id,
                    expected_revision: conversation_revision(&fixture),
                    expected_turn_revision: Revision::INITIAL,
                    operation: token("settle-created", "cd"),
                    usage_event_id: UsageEventId::new(),
                },
                TimestampMillis::new(31),
            )
            .expect("settle from created");
        assert_eq!(
            settled_from_created.value.turn.status,
            GenerationTurnStatus::Cancelled
        );
    }

    #[test]
    fn settlement_rejects_ineligible_statuses_and_stale_revisions() {
        let fixture = direct_fixture();
        let send = fixture
            .database
            .begin_send(
                &send_command(&fixture, "send-settle-guards", "cd", text("hello")),
                TimestampMillis::new(20),
            )
            .expect("send");
        let turn_id = send.value.turn.id;
        let attempt_id = send.value.attempt.id;
        let draft = || finalization_draft(text("generated"), 0);

        assert_eq!(
            fixture.database.finalize_generation(
                turn_id,
                attempt_id,
                conversation_revision(&fixture),
                Revision::INITIAL,
                &token("finalize-created", "cd"),
                draft(),
                UsageEventId::new(),
                TimestampMillis::new(21),
            ),
            Err(ConversationRepositoryError::Conflict),
            "a created turn has produced nothing to finalize"
        );
        assert_eq!(
            fixture.database.fail_generation(
                turn_id,
                attempt_id,
                conversation_revision(&fixture),
                Revision::INITIAL,
                &token("fail-created", "cd"),
                GenerationFailureCode::Internal,
                UsageEventId::new(),
                TimestampMillis::new(22),
            ),
            Err(ConversationRepositoryError::Conflict),
            "created has no legal edge to failed"
        );
        assert_eq!(
            fixture.database.recover_generation(
                turn_id,
                attempt_id,
                conversation_revision(&fixture),
                Revision::INITIAL,
                &token("recover-created", "cd"),
                TimestampMillis::new(23),
            ),
            Err(ConversationRepositoryError::Conflict)
        );

        let revision = drive(
            &fixture,
            turn_id,
            attempt_id,
            &[
                GenerationTurnStatus::Preparing,
                GenerationTurnStatus::ContextPrepared,
                GenerationTurnStatus::Running,
            ],
            "drive-guards",
            24,
        );
        assert_eq!(
            fixture.database.finalize_generation(
                turn_id,
                attempt_id,
                Revision::new(99),
                revision,
                &token("finalize-stale-conversation", "cd"),
                draft(),
                UsageEventId::new(),
                TimestampMillis::new(30),
            ),
            Err(ConversationRepositoryError::StaleRevision {
                expected: Revision::new(99),
                actual: conversation_revision(&fixture),
            })
        );
        assert_eq!(
            fixture.database.finalize_generation(
                turn_id,
                attempt_id,
                conversation_revision(&fixture),
                Revision::new(99),
                &token("finalize-stale-turn", "cd"),
                draft(),
                UsageEventId::new(),
                TimestampMillis::new(31),
            ),
            Err(ConversationRepositoryError::StaleRevision {
                expected: Revision::new(99),
                actual: revision,
            })
        );
    }

    #[test]
    fn a_usage_event_is_recorded_once_per_conversation() {
        let mut fixture = direct_fixture();
        let first = fixture
            .database
            .begin_send(
                &send_command(&fixture, "send-usage-one", "cd", text("hello")),
                TimestampMillis::new(20),
            )
            .expect("send");
        let usage_event_id = UsageEventId::new();
        let revision = drive(
            &fixture,
            first.value.turn.id,
            first.value.attempt.id,
            &[GenerationTurnStatus::Preparing],
            "drive-usage-one",
            21,
        );
        fixture
            .database
            .fail_generation(
                first.value.turn.id,
                first.value.attempt.id,
                conversation_revision(&fixture),
                revision,
                &token("fail-usage-one", "cd"),
                GenerationFailureCode::Internal,
                usage_event_id,
                TimestampMillis::new(30),
            )
            .expect("first failure");

        fixture.revision = conversation_revision(&fixture);
        let second = fixture
            .database
            .begin_send(
                &send_command(&fixture, "send-usage-two", "cd", text("again")),
                TimestampMillis::new(40),
            )
            .expect("second send");
        let revision = drive(
            &fixture,
            second.value.turn.id,
            second.value.attempt.id,
            &[GenerationTurnStatus::Preparing],
            "drive-usage-two",
            41,
        );
        assert_eq!(
            fixture.database.fail_generation(
                second.value.turn.id,
                second.value.attempt.id,
                conversation_revision(&fixture),
                revision,
                &token("fail-usage-two", "cd"),
                GenerationFailureCode::Internal,
                usage_event_id,
                TimestampMillis::new(50),
            ),
            Err(ConversationRepositoryError::Conflict)
        );
    }

    #[test]
    fn finalize_replays_its_records_and_rolls_back_a_failed_write() {
        let fixture = direct_fixture();
        let send = fixture
            .database
            .begin_send(
                &send_command(&fixture, "send-finalize-replay", "cd", text("hello")),
                TimestampMillis::new(20),
            )
            .expect("send");
        let turn_id = send.value.turn.id;
        let attempt_id = send.value.attempt.id;
        let revision = drive(
            &fixture,
            turn_id,
            attempt_id,
            &[
                GenerationTurnStatus::Preparing,
                GenerationTurnStatus::ContextPrepared,
                GenerationTurnStatus::Running,
            ],
            "drive-finalize-replay",
            21,
        );

        fixture
            .database
            .connection()
            .expect("connection")
            .execute_batch(
                "CREATE TRIGGER fail_test_usage BEFORE INSERT ON conversation_usage_refs BEGIN SELECT RAISE(ABORT, 'test rollback'); END;",
            )
            .expect("failure trigger");
        let usage_event_id = UsageEventId::new();
        assert_eq!(
            fixture.database.finalize_generation(
                turn_id,
                attempt_id,
                conversation_revision(&fixture),
                revision,
                &token("finalize-rollback", "cd"),
                finalization_draft(text("generated"), 0),
                usage_event_id,
                TimestampMillis::new(30),
            ),
            Err(ConversationRepositoryError::Storage)
        );
        for table in [
            "conversation_message_candidates",
            "conversation_usage_refs",
            "candidate_media_refs",
        ] {
            let count: i64 = scalar(
                &fixture.database,
                &format!("SELECT count(*) FROM {table} WHERE conversation_id = ?1"),
                &fixture.conversation_id.to_string(),
            );
            assert_eq!(count, 0, "rows leaked in {table}");
        }
        let messages: i64 = scalar(
            &fixture.database,
            "SELECT count(*) FROM conversation_messages WHERE conversation_id = ?1",
            &fixture.conversation_id.to_string(),
        );
        assert_eq!(messages, 1, "the assistant message was rolled back");
        assert_eq!(turn_revision(&fixture, turn_id), revision);

        fixture
            .database
            .connection()
            .expect("connection")
            .execute_batch("DROP TRIGGER fail_test_usage")
            .expect("drop trigger");
        let finalize_token = token("finalize-replayed", "cd");
        let first = fixture
            .database
            .finalize_generation(
                turn_id,
                attempt_id,
                conversation_revision(&fixture),
                revision,
                &finalize_token,
                finalization_draft(text("generated"), 0),
                usage_event_id,
                TimestampMillis::new(40),
            )
            .expect("finalize");
        let replay = fixture
            .database
            .finalize_generation(
                turn_id,
                attempt_id,
                Revision::new(99),
                Revision::new(99),
                &finalize_token,
                finalization_draft(text("generated"), 0),
                usage_event_id,
                TimestampMillis::new(41),
            )
            .expect("replay ignores the stale expectations it never re-checks");
        assert_eq!(replay.operation, first.operation);
        assert_eq!(replay.outbox, first.outbox);
        assert_eq!(replay.value.candidate.id, first.value.candidate.id);
        assert_eq!(replay.value.turn.status, GenerationTurnStatus::Succeeded);

        let candidates: i64 = scalar(
            &fixture.database,
            "SELECT count(*) FROM conversation_message_candidates WHERE conversation_id = ?1",
            &fixture.conversation_id.to_string(),
        );
        assert_eq!(candidates, 1);
    }

    #[test]
    fn a_stale_head_blocks_retry_at_begin_and_finalize() {
        let mut fixture = direct_fixture();
        let first = fixture
            .database
            .begin_send(
                &send_command(&fixture, "send-orphan-one", "cd", text("first")),
                TimestampMillis::new(20),
            )
            .expect("first send");
        settle_failed(&fixture, &first.value.turn, 21);
        fixture.revision = conversation_revision(&fixture);
        let second = fixture
            .database
            .begin_send(
                &send_command(&fixture, "send-orphan-two", "cd", text("second")),
                TimestampMillis::new(30),
            )
            .expect("second send");
        settle_succeeded(&fixture, &second.value.turn, 31);
        fixture.revision = conversation_revision(&fixture);

        assert_eq!(
            fixture.database.begin_retry(
                &RetryGeneration {
                    conversation_id: fixture.conversation_id,
                    branch_id: fixture.branch_id,
                    turn_id: first.value.turn.id,
                    expected_revision: fixture.revision,
                    expected_turn_revision: turn_revision(&fixture, first.value.turn.id),
                    operation: token("retry-orphan", "cd"),
                },
                TimestampMillis::new(40)
            ),
            Err(ConversationRepositoryError::Conflict),
            "the failed turn's parent is no longer the branch head"
        );

        let live = fixture
            .database
            .begin_send(
                &send_command(&fixture, "send-orphan-three", "cd", text("third")),
                TimestampMillis::new(50),
            )
            .expect("third send");
        let revision = drive(
            &fixture,
            live.value.turn.id,
            live.value.attempt.id,
            &[
                GenerationTurnStatus::Preparing,
                GenerationTurnStatus::ContextPrepared,
                GenerationTurnStatus::Running,
            ],
            "drive-orphan",
            51,
        );
        fixture
            .database
            .connection()
            .expect("connection")
            .execute(
                "UPDATE conversation_branches SET head_message_id = NULL WHERE conversation_id = ?1 AND id = ?2",
                params![
                    fixture.conversation_id.to_string(),
                    fixture.branch_id.to_string(),
                ],
            )
            .expect("raw head move: no port retracts a head yet");
        assert_eq!(
            fixture.database.finalize_generation(
                live.value.turn.id,
                live.value.attempt.id,
                conversation_revision(&fixture),
                revision,
                &token("finalize-orphan", "cd"),
                finalization_draft(text("generated"), 0),
                UsageEventId::new(),
                TimestampMillis::new(60),
            ),
            Err(ConversationRepositoryError::Conflict),
            "the head moved under the running turn"
        );
        let messages: i64 = scalar(
            &fixture.database,
            "SELECT count(*) FROM conversation_messages WHERE conversation_id = ?1",
            &fixture.conversation_id.to_string(),
        );
        assert_eq!(messages, 4, "the rolled-back finalize wrote no message");
    }

    #[test]
    fn finalize_confirms_the_derived_ordinal_and_the_settled_usage_event() {
        let fixture = direct_fixture();
        let send = fixture
            .database
            .begin_send(
                &send_command(&fixture, "send-ordinal", "cd", text("hello")),
                TimestampMillis::new(20),
            )
            .expect("send");
        let turn_id = send.value.turn.id;
        let attempt_id = send.value.attempt.id;
        let revision = drive(
            &fixture,
            turn_id,
            attempt_id,
            &[
                GenerationTurnStatus::Preparing,
                GenerationTurnStatus::ContextPrepared,
                GenerationTurnStatus::Running,
            ],
            "drive-ordinal",
            21,
        );
        assert_eq!(
            fixture.database.finalize_generation(
                turn_id,
                attempt_id,
                conversation_revision(&fixture),
                revision,
                &token("finalize-bad-ordinal", "cd"),
                finalization_draft(text("generated"), 3),
                UsageEventId::new(),
                TimestampMillis::new(30),
            ),
            Err(invalid("finalization.ordinal"))
        );
        assert_eq!(
            fixture.database.finalize_generation(
                turn_id,
                attempt_id,
                conversation_revision(&fixture),
                revision,
                &token("finalize-bad-outcome", "cd"),
                FinalizationDraft {
                    outcome: GenerationCheckpointEvent::CandidateReady {
                        candidate_id: MessageCandidateId::new()
                    },
                    ..finalization_draft(text("generated"), 0)
                },
                UsageEventId::new(),
                TimestampMillis::new(31),
            ),
            Err(invalid("finalization.outcome"))
        );

        let usage_event_id = UsageEventId::new();
        let finalize_token = token("finalize-usage-check", "cd");
        fixture
            .database
            .finalize_generation(
                turn_id,
                attempt_id,
                conversation_revision(&fixture),
                revision,
                &finalize_token,
                finalization_draft(text("generated"), 0),
                usage_event_id,
                TimestampMillis::new(40),
            )
            .expect("finalize");
        assert_eq!(
            fixture.database.finalize_generation(
                turn_id,
                attempt_id,
                conversation_revision(&fixture),
                turn_revision(&fixture, turn_id),
                &finalize_token,
                finalization_draft(text("generated"), 0),
                UsageEventId::new(),
                TimestampMillis::new(41),
            ),
            Err(ConversationRepositoryError::Conflict),
            "the settled attempt's usage event is authoritative on replay"
        );
    }

    #[test]
    fn retention_deltas_are_attributed_to_their_owning_candidate() {
        let mut fixture = direct_fixture();
        let first_asset = stage_media_asset(&fixture.database, "11");
        let second_asset = stage_media_asset(&fixture.database, "22");
        let media = |asset_id| {
            vec![MessagePart::MediaAsset {
                asset_id,
                role: lettuce_conversations::MediaAssetRole::Inline,
            }]
        };
        let send = fixture
            .database
            .begin_send(
                &send_command(&fixture, "send-deltas", "cd", text("hello")),
                TimestampMillis::new(20),
            )
            .expect("send");
        let revision = drive(
            &fixture,
            send.value.turn.id,
            send.value.attempt.id,
            &[
                GenerationTurnStatus::Preparing,
                GenerationTurnStatus::ContextPrepared,
                GenerationTurnStatus::Running,
            ],
            "drive-deltas-one",
            21,
        );
        let first = fixture
            .database
            .finalize_generation(
                send.value.turn.id,
                send.value.attempt.id,
                conversation_revision(&fixture),
                revision,
                &token("finalize-deltas-one", "cd"),
                finalization_draft(media(first_asset), 0),
                UsageEventId::new(),
                TimestampMillis::new(30),
            )
            .expect("first finalize");
        fixture.revision = conversation_revision(&fixture);

        let regenerate = fixture
            .database
            .begin_regenerate(
                &RegenerateCandidate {
                    conversation_id: fixture.conversation_id,
                    branch_id: fixture.branch_id,
                    message_id: first.value.assistant_message.id,
                    turn_id: send.value.turn.id,
                    expected_revision: fixture.revision,
                    expected_turn_revision: turn_revision(&fixture, send.value.turn.id),
                    operation: token("regen-deltas", "cd"),
                    active_candidate_id: first.value.candidate.id,
                    guidance: None,
                    model_override: None,
                    forced_speaker: None,
                    swap_roles: false,
                },
                TimestampMillis::new(40),
            )
            .expect("regenerate");
        let revision = drive(
            &fixture,
            regenerate.value.turn.id,
            regenerate.value.attempt.id,
            &[
                GenerationTurnStatus::Preparing,
                GenerationTurnStatus::ContextPrepared,
                GenerationTurnStatus::Running,
            ],
            "drive-deltas-two",
            41,
        );
        let second = fixture
            .database
            .finalize_generation(
                regenerate.value.turn.id,
                regenerate.value.attempt.id,
                conversation_revision(&fixture),
                revision,
                &token("finalize-deltas-two", "cd"),
                finalization_draft(media(second_asset), 1),
                UsageEventId::new(),
                TimestampMillis::new(50),
            )
            .expect("second finalize");

        assert_eq!(second.outbox.len(), 3);
        let ConversationOutboxEvent::AssetReferencesChanged {
            candidate_id: Some(new_owner),
            changes: new_changes,
            ..
        } = &second.outbox[1].event
        else {
            panic!("expected the new candidate's asset event");
        };
        assert_eq!(*new_owner, second.value.candidate.id);
        assert_eq!(new_changes.len(), 1);
        assert_eq!(new_changes[0].asset_id, second_asset);
        assert_eq!(new_changes[0].state, AssetReferenceState::Active);
        assert_eq!(
            new_changes[0].retainer,
            lettuce_media::AssetRetainer::MessageCandidate(second.value.candidate.id)
        );
        let ConversationOutboxEvent::AssetReferencesChanged {
            candidate_id: Some(prior_owner),
            changes: prior_changes,
            ..
        } = &second.outbox[2].event
        else {
            panic!("expected the prior candidate's asset event");
        };
        assert_eq!(*prior_owner, first.value.candidate.id);
        assert_eq!(prior_changes.len(), 1);
        assert_eq!(prior_changes[0].asset_id, first_asset);
        assert_eq!(prior_changes[0].state, AssetReferenceState::Historical);
        assert_eq!(
            prior_changes[0].retainer,
            lettuce_media::AssetRetainer::MessageCandidate(first.value.candidate.id)
        );
        assert_eq!(second.value.asset_reference_deltas.len(), 2);

        let replay = fixture
            .database
            .finalize_generation(
                regenerate.value.turn.id,
                regenerate.value.attempt.id,
                Revision::new(99),
                Revision::new(99),
                &token("finalize-deltas-two", "cd"),
                finalization_draft(media(second_asset), 1),
                second.value.usage_event_id,
                TimestampMillis::new(60),
            )
            .expect("replay");
        assert_eq!(
            replay.value.asset_reference_deltas, second.value.asset_reference_deltas,
            "replayed deltas come from the recorded events"
        );

        let outbox = ConversationReader::page_outbox(
            fixture.database.as_ref(),
            fixture.conversation_id,
            &PageRequest {
                cursor: None,
                limit: PageLimit::new(50),
            },
        )
        .expect("outbox page");
        let sequences: Vec<u64> = outbox.items.iter().map(|record| record.sequence).collect();
        assert_eq!(
            sequences,
            (1..=u64::try_from(outbox.items.len()).expect("len")).collect::<Vec<_>>()
        );
        for record in &second.outbox {
            assert!(
                outbox.items.contains(record),
                "the finalize records read back unchanged"
            );
        }
    }

    #[test]
    fn a_regenerated_message_flip_requires_the_expected_active_candidate() {
        let mut fixture = direct_fixture();
        let send = fixture
            .database
            .begin_send(
                &send_command(&fixture, "send-flip", "cd", text("hello")),
                TimestampMillis::new(20),
            )
            .expect("send");
        let (message_id, first_candidate) = settle_succeeded(&fixture, &send.value.turn, 21);
        fixture.revision = conversation_revision(&fixture);
        let regenerate = fixture
            .database
            .begin_regenerate(
                &RegenerateCandidate {
                    conversation_id: fixture.conversation_id,
                    branch_id: fixture.branch_id,
                    message_id,
                    turn_id: send.value.turn.id,
                    expected_revision: fixture.revision,
                    expected_turn_revision: turn_revision(&fixture, send.value.turn.id),
                    operation: token("regen-flip", "cd"),
                    active_candidate_id: first_candidate,
                    guidance: None,
                    model_override: None,
                    forced_speaker: None,
                    swap_roles: false,
                },
                TimestampMillis::new(80),
            )
            .expect("regenerate");
        let revision = drive(
            &fixture,
            regenerate.value.turn.id,
            regenerate.value.attempt.id,
            &[
                GenerationTurnStatus::Preparing,
                GenerationTurnStatus::ContextPrepared,
                GenerationTurnStatus::Running,
            ],
            "drive-flip",
            81,
        );
        let intruder = MessageCandidateId::new();
        fixture
            .database
            .connection()
            .expect("connection")
            .execute(
                "INSERT INTO conversation_message_candidates (conversation_id, id, message_id, branch_id, turn_id, attempt_id, author_participant_id, ordinal, parts_json, model_json, created_at, provider_replay_artifact_id, provider_replay_retention) SELECT conversation_id, ?3, message_id, branch_id, turn_id, attempt_id, author_participant_id, 1, parts_json, model_json, created_at, NULL, NULL FROM conversation_message_candidates WHERE conversation_id = ?1 AND id = ?2",
                params![
                    fixture.conversation_id.to_string(),
                    first_candidate.to_string(),
                    intruder.to_string(),
                ],
            )
            .expect("raw sibling candidate: choose_candidate arrives in a later slice");
        fixture
            .database
            .connection()
            .expect("connection")
            .execute(
                "UPDATE conversation_messages SET active_candidate_id = ?3 WHERE conversation_id = ?1 AND id = ?2",
                params![
                    fixture.conversation_id.to_string(),
                    message_id.to_string(),
                    intruder.to_string(),
                ],
            )
            .expect("raw active candidate move");
        assert_eq!(
            fixture.database.finalize_generation(
                regenerate.value.turn.id,
                regenerate.value.attempt.id,
                conversation_revision(&fixture),
                revision,
                &token("finalize-flip", "cd"),
                finalization_draft(text("second take"), 2),
                UsageEventId::new(),
                TimestampMillis::new(90),
            ),
            Err(ConversationRepositoryError::Conflict),
            "the active candidate moved under the regeneration"
        );
    }

    #[test]
    fn cancellation_tokens_do_not_impersonate_each_other() {
        let mut fixture = direct_fixture();
        let send = fixture
            .database
            .begin_send(
                &send_command(&fixture, "send-cancel-tokens", "cd", text("hello")),
                TimestampMillis::new(20),
            )
            .expect("send");
        let request_token = token("cancel-token-request", "cd");
        let requested = fixture
            .database
            .request_cancellation(
                &CancelGeneration {
                    conversation_id: fixture.conversation_id,
                    turn_id: send.value.turn.id,
                    attempt_id: send.value.attempt.id,
                    expected_revision: conversation_revision(&fixture),
                    expected_turn_revision: Revision::INITIAL,
                    operation: request_token.clone(),
                },
                TimestampMillis::new(21),
            )
            .expect("request");
        fixture
            .database
            .settle_cancellation(
                &SettleCancellation {
                    conversation_id: fixture.conversation_id,
                    turn_id: send.value.turn.id,
                    attempt_id: send.value.attempt.id,
                    expected_revision: conversation_revision(&fixture),
                    expected_turn_revision: requested.value.revision,
                    operation: token("cancel-token-settle", "cd"),
                    usage_event_id: UsageEventId::new(),
                },
                TimestampMillis::new(22),
            )
            .expect("settle");

        assert_eq!(
            fixture.database.settle_cancellation(
                &SettleCancellation {
                    conversation_id: fixture.conversation_id,
                    turn_id: send.value.turn.id,
                    attempt_id: send.value.attempt.id,
                    expected_revision: conversation_revision(&fixture),
                    expected_turn_revision: Revision::INITIAL,
                    operation: request_token.clone(),
                    usage_event_id: UsageEventId::new(),
                },
                TimestampMillis::new(23),
            ),
            Err(ConversationRepositoryError::Conflict),
            "the request record does not describe a settled usage event"
        );

        fixture.revision = conversation_revision(&fixture);
        let other = fixture
            .database
            .begin_send(
                &send_command(&fixture, "send-cancel-tokens-two", "cd", text("again")),
                TimestampMillis::new(30),
            )
            .expect("second send");
        assert_eq!(
            fixture.database.request_cancellation(
                &CancelGeneration {
                    conversation_id: fixture.conversation_id,
                    turn_id: other.value.turn.id,
                    attempt_id: other.value.attempt.id,
                    expected_revision: conversation_revision(&fixture),
                    expected_turn_revision: Revision::INITIAL,
                    operation: request_token,
                },
                TimestampMillis::new(31),
            ),
            Err(ConversationRepositoryError::Conflict),
            "a cancellation token belongs to the turn it was minted for"
        );
        assert_eq!(
            fixture.database.request_cancellation(
                &CancelGeneration {
                    conversation_id: fixture.conversation_id,
                    turn_id: other.value.turn.id,
                    attempt_id: other.value.attempt.id,
                    expected_revision: Revision::new(0),
                    expected_turn_revision: Revision::INITIAL,
                    operation: token("cancel-zero", "cd"),
                },
                TimestampMillis::new(32),
            ),
            Err(ConversationRepositoryError::Invalid(
                lettuce_conversations::ValidationError::ZeroRevision
            ))
        );
    }

    fn staged_model(database: &Database) -> ModelSelectionSnapshot {
        let source_id = ModelProfileId::new();
        let (reference, draft) = artifact(SnapshotSource::Model(source_id), b"policy model");
        lettuce_conversations::ConversationArtifactStore::put_snapshot(database, draft)
            .expect("stage model artifact");
        ModelSelectionSnapshot {
            snapshot_ref: reference,
            source_id,
            source_revision: Revision::INITIAL,
            provider_kind: ModelProviderKind::OpenAiCompatible,
            external_model_id: "policy-model".into(),
            display_name: "Policy Model".into(),
            context_length: None,
            max_output_tokens: None,
        }
    }

    /// send → finalize → regenerate → finalize, leaving one assistant message
    /// with two candidates and the newer one active.
    fn two_candidates(
        fixture: &mut Fixture,
        key: &str,
    ) -> (MessageId, MessageCandidateId, MessageCandidateId) {
        let send = fixture
            .database
            .begin_send(
                &send_command(fixture, &format!("{key}-send"), "cd", text("hello")),
                TimestampMillis::new(20),
            )
            .expect("send");
        let (message_id, first) = settle_succeeded(fixture, &send.value.turn, 21);
        fixture.revision = conversation_revision(fixture);
        let regenerate = fixture
            .database
            .begin_regenerate(
                &RegenerateCandidate {
                    conversation_id: fixture.conversation_id,
                    branch_id: fixture.branch_id,
                    message_id,
                    turn_id: send.value.turn.id,
                    expected_revision: fixture.revision,
                    expected_turn_revision: turn_revision(fixture, send.value.turn.id),
                    operation: token(&format!("{key}-regen"), "cd"),
                    active_candidate_id: first,
                    guidance: None,
                    model_override: None,
                    forced_speaker: None,
                    swap_roles: false,
                },
                TimestampMillis::new(80),
            )
            .expect("regenerate");
        let revision = drive(
            fixture,
            regenerate.value.turn.id,
            regenerate.value.attempt.id,
            &[
                GenerationTurnStatus::Preparing,
                GenerationTurnStatus::ContextPrepared,
                GenerationTurnStatus::Running,
            ],
            &format!("{key}-drive"),
            81,
        );
        let second = fixture
            .database
            .finalize_generation(
                regenerate.value.turn.id,
                regenerate.value.attempt.id,
                conversation_revision(fixture),
                revision,
                &token(&format!("{key}-finalize"), "cd"),
                finalization_draft(text("second take"), 1),
                UsageEventId::new(),
                TimestampMillis::new(90),
            )
            .expect("second finalize")
            .value
            .candidate
            .id;
        fixture.revision = conversation_revision(fixture);
        (message_id, first, second)
    }

    fn outbox_reads_back(
        fixture: &Fixture,
        expected: &[lettuce_conversations::ConversationOutboxRecord],
    ) {
        let page = ConversationReader::page_outbox(
            fixture.database.as_ref(),
            fixture.conversation_id,
            &PageRequest {
                cursor: None,
                limit: PageLimit::new(100),
            },
        )
        .expect("outbox page");
        let sequences: Vec<u64> = page.items.iter().map(|record| record.sequence).collect();
        assert_eq!(
            sequences,
            (1..=u64::try_from(page.items.len()).expect("len")).collect::<Vec<_>>()
        );
        for record in expected {
            assert!(
                page.items.contains(record),
                "outbox record read back changed"
            );
        }
    }

    #[test]
    fn choosing_an_older_candidate_moves_the_render_source() {
        let mut fixture = direct_fixture();
        let (message_id, first, second) = two_candidates(&mut fixture, "choose");
        let chosen = fixture
            .database
            .choose_candidate(
                &ChooseCandidate {
                    conversation_id: fixture.conversation_id,
                    message_id,
                    candidate_id: first,
                    expected_revision: fixture.revision,
                    operation: token("choose-first", "cd"),
                },
                TimestampMillis::new(100),
            )
            .expect("choose");
        assert_eq!(
            chosen.value.active_render_source,
            lettuce_conversations::MessageRenderSource::Candidate(first)
        );
        assert!(matches!(
            chosen.outbox[0].event,
            ConversationOutboxEvent::CandidateChosen { .. }
        ));
        outbox_reads_back(&fixture, &chosen.outbox);
        fixture.revision = conversation_revision(&fixture);

        let replay = fixture
            .database
            .choose_candidate(
                &ChooseCandidate {
                    conversation_id: fixture.conversation_id,
                    message_id,
                    candidate_id: first,
                    expected_revision: Revision::new(99),
                    operation: token("choose-first", "cd"),
                },
                TimestampMillis::new(101),
            )
            .expect("replay");
        assert_eq!(replay.operation, chosen.operation);
        assert_eq!(replay.outbox, chosen.outbox);

        assert_eq!(
            fixture.database.choose_candidate(
                &ChooseCandidate {
                    conversation_id: fixture.conversation_id,
                    message_id,
                    candidate_id: MessageCandidateId::new(),
                    expected_revision: fixture.revision,
                    operation: token("choose-unknown", "cd"),
                },
                TimestampMillis::new(102),
            ),
            Err(ConversationRepositoryError::NotFound)
        );

        let regenerate = fixture
            .database
            .begin_regenerate(
                &RegenerateCandidate {
                    conversation_id: fixture.conversation_id,
                    branch_id: fixture.branch_id,
                    message_id,
                    turn_id: chosen_turn(&fixture, first),
                    expected_revision: fixture.revision,
                    expected_turn_revision: turn_revision(&fixture, chosen_turn(&fixture, first)),
                    operation: token("choose-then-regen", "cd"),
                    active_candidate_id: first,
                    guidance: None,
                    model_override: None,
                    forced_speaker: None,
                    swap_roles: false,
                },
                TimestampMillis::new(110),
            )
            .expect("regenerate follows the chosen candidate");
        assert_eq!(
            regenerate.value.turn.target,
            GenerationTarget::ExistingCandidate {
                message_id,
                prior_candidate_id: first
            }
        );
        fixture.revision = conversation_revision(&fixture);
        assert_eq!(
            fixture.database.choose_candidate(
                &ChooseCandidate {
                    conversation_id: fixture.conversation_id,
                    message_id,
                    candidate_id: second,
                    expected_revision: fixture.revision,
                    operation: token("choose-during-generation", "cd"),
                },
                TimestampMillis::new(111),
            ),
            Err(ConversationRepositoryError::Conflict),
            "a live turn owns the render source"
        );
    }

    fn chosen_turn(fixture: &Fixture, candidate_id: MessageCandidateId) -> GenerationTurnId {
        let value: String = scalar(
            &fixture.database,
            "SELECT turn_id FROM conversation_message_candidates WHERE id = ?1",
            &candidate_id.to_string(),
        );
        value.parse().expect("turn id")
    }

    #[test]
    fn editing_a_message_supersedes_its_render_source() {
        let mut fixture = direct_fixture();
        let (message_id, first, _) = two_candidates(&mut fixture, "edit");
        let asset_id = stage_media_asset(&fixture.database, "33");
        let edited = fixture
            .database
            .edit_message(
                &EditMessage {
                    conversation_id: fixture.conversation_id,
                    message_id,
                    expected_revision: fixture.revision,
                    operation: token("edit-one", "cd"),
                    draft: lettuce_conversations::MessageEditDraft {
                        parts: vec![
                            MessagePart::Text {
                                text: "edited".into(),
                            },
                            MessagePart::MediaAsset {
                                asset_id,
                                role: lettuce_conversations::MediaAssetRole::Inline,
                            },
                        ],
                        visibility: MessageVisibility::Visible,
                        pinned: true,
                        scene_edited: false,
                    },
                },
                TimestampMillis::new(100),
            )
            .expect("edit");
        assert_eq!(edited.value.revision.sequence, Revision::INITIAL);
        assert_eq!(
            edited.value.message.active_render_source,
            lettuce_conversations::MessageRenderSource::Revision(edited.value.revision.id)
        );
        assert!(edited.value.message.pinned);
        assert_eq!(edited.value.asset_reference_deltas.len(), 1);
        assert!(matches!(
            edited.outbox[0].event,
            ConversationOutboxEvent::MessageRevised { .. }
        ));
        outbox_reads_back(&fixture, &edited.outbox);
        fixture.revision = conversation_revision(&fixture);

        let replay = fixture
            .database
            .edit_message(
                &EditMessage {
                    conversation_id: fixture.conversation_id,
                    message_id,
                    expected_revision: Revision::new(99),
                    operation: token("edit-one", "cd"),
                    draft: lettuce_conversations::MessageEditDraft {
                        parts: text("edited"),
                        visibility: MessageVisibility::Visible,
                        pinned: true,
                        scene_edited: false,
                    },
                },
                TimestampMillis::new(101),
            )
            .expect("replay");
        assert_eq!(replay.value.revision.id, edited.value.revision.id);
        assert_eq!(replay.outbox, edited.outbox);

        let restored = fixture
            .database
            .choose_candidate(
                &ChooseCandidate {
                    conversation_id: fixture.conversation_id,
                    message_id,
                    candidate_id: first,
                    expected_revision: fixture.revision,
                    operation: token("choose-after-edit", "cd"),
                },
                TimestampMillis::new(102),
            )
            .expect("choose after edit");
        assert_eq!(
            restored.value.active_render_source,
            lettuce_conversations::MessageRenderSource::Candidate(first)
        );
        let historical: i64 = scalar(
            &fixture.database,
            "SELECT count(*) FROM revision_media_refs WHERE conversation_id = ?1 AND state = 'historical'",
            &fixture.conversation_id.to_string(),
        );
        assert_eq!(historical, 1, "the edited revision's media retired");
    }

    #[test]
    fn message_flags_apply_only_the_requested_patch() {
        let mut fixture = direct_fixture();
        let send = fixture
            .database
            .begin_send(
                &send_command(&fixture, "send-flags", "cd", text("hello")),
                TimestampMillis::new(20),
            )
            .expect("send");
        let GenerationInput::UserMessage { message_id } = send.value.turn.input else {
            panic!("expected a user message");
        };
        fixture.revision = send.value.conversation.revision;
        let flagged = fixture
            .database
            .update_message_flags(
                &UpdateMessageFlags {
                    conversation_id: fixture.conversation_id,
                    message_id,
                    expected_revision: fixture.revision,
                    operation: token("flags-pin", "cd"),
                    pinned: Some(true),
                    visibility: None,
                },
                TimestampMillis::new(21),
            )
            .expect("flags");
        assert!(flagged.value.pinned);
        assert_eq!(flagged.value.visibility, MessageVisibility::Visible);
        assert!(matches!(
            flagged.outbox[0].event,
            ConversationOutboxEvent::MessageFlagsChanged {
                pinned: true,
                visibility: MessageVisibility::Visible,
                ..
            }
        ));
        fixture.revision = conversation_revision(&fixture);

        let hidden = fixture
            .database
            .update_message_flags(
                &UpdateMessageFlags {
                    conversation_id: fixture.conversation_id,
                    message_id,
                    expected_revision: fixture.revision,
                    operation: token("flags-hide", "cd"),
                    pinned: None,
                    visibility: Some(MessageVisibility::Hidden),
                },
                TimestampMillis::new(22),
            )
            .expect("hide");
        assert!(hidden.value.pinned, "the untouched flag survives");
        assert_eq!(hidden.value.visibility, MessageVisibility::Hidden);
        fixture.revision = conversation_revision(&fixture);

        let replay = fixture
            .database
            .update_message_flags(
                &UpdateMessageFlags {
                    conversation_id: fixture.conversation_id,
                    message_id,
                    expected_revision: Revision::new(99),
                    operation: token("flags-hide", "cd"),
                    pinned: None,
                    visibility: Some(MessageVisibility::Hidden),
                },
                TimestampMillis::new(23),
            )
            .expect("replay");
        assert_eq!(replay.outbox, hidden.outbox);
        assert_eq!(
            fixture.database.update_message_flags(
                &UpdateMessageFlags {
                    conversation_id: fixture.conversation_id,
                    message_id: MessageId::new(),
                    expected_revision: fixture.revision,
                    operation: token("flags-missing", "cd"),
                    pinned: Some(false),
                    visibility: None,
                },
                TimestampMillis::new(24),
            ),
            Err(ConversationRepositoryError::NotFound)
        );
    }

    /// send → finalize, twice, leaving user/assistant/user/assistant on the
    /// root branch.
    fn conversation_with_two_exchanges(fixture: &mut Fixture, key: &str) -> Vec<MessageId> {
        let mut ids = Vec::new();
        for round in 0..2 {
            let send = fixture
                .database
                .begin_send(
                    &send_command(fixture, &format!("{key}-send-{round}"), "cd", text("hello")),
                    TimestampMillis::new(20 + round * 100),
                )
                .expect("send");
            let GenerationInput::UserMessage { message_id } = send.value.turn.input else {
                panic!("expected a user message");
            };
            ids.push(message_id);
            let (assistant, _) = settle_succeeded(fixture, &send.value.turn, 21 + round * 100);
            ids.push(assistant);
            fixture.revision = conversation_revision(fixture);
        }
        ids
    }

    #[test]
    fn forking_selects_the_new_branch_and_selecting_returns() {
        let mut fixture = direct_fixture();
        let messages = conversation_with_two_exchanges(&mut fixture, "fork");
        let root_branch = fixture.branch_id;
        let forked = fixture
            .database
            .fork_branch(
                &ForkBranch {
                    conversation_id: fixture.conversation_id,
                    source_branch_id: root_branch,
                    at_message_id: None,
                    expected_revision: fixture.revision,
                    operation: token("fork-head", "cd"),
                },
                TimestampMillis::new(200),
            )
            .expect("fork");
        assert_eq!(forked.value.branch.parent_branch_id, Some(root_branch));
        assert_eq!(forked.value.branch.fork_message_id, Some(messages[3]));
        assert_eq!(forked.value.branch.head_message_id, None);
        assert_eq!(
            forked.value.conversation.active_branch_id,
            forked.value.branch.id
        );
        assert!(matches!(
            forked.outbox[0].event,
            ConversationOutboxEvent::BranchForked { .. }
        ));
        outbox_reads_back(&fixture, &forked.outbox);
        fixture.revision = conversation_revision(&fixture);

        let page = PageRequest {
            cursor: None,
            limit: PageLimit::new(20),
        };
        let inherited = ConversationReader::timeline_page(
            fixture.database.as_ref(),
            fixture.conversation_id,
            forked.value.branch.id,
            &page,
        )
        .expect("forked timeline");
        assert_eq!(inherited.items.len(), 4);
        assert_eq!(inherited.branch_path.len(), 2);

        let selected = fixture
            .database
            .select_branch(
                &SelectBranch {
                    conversation_id: fixture.conversation_id,
                    branch_id: root_branch,
                    expected_revision: fixture.revision,
                    operation: token("select-root", "cd"),
                },
                TimestampMillis::new(210),
            )
            .expect("select");
        assert_eq!(selected.value.active_branch_id, root_branch);
        assert!(matches!(
            selected.outbox[0].event,
            ConversationOutboxEvent::BranchSelected { .. }
        ));
        fixture.revision = conversation_revision(&fixture);
        assert_eq!(
            ConversationReader::timeline_page(
                fixture.database.as_ref(),
                fixture.conversation_id,
                root_branch,
                &page,
            )
            .expect("root timeline")
            .items
            .len(),
            4
        );

        let replay = fixture
            .database
            .fork_branch(
                &ForkBranch {
                    conversation_id: fixture.conversation_id,
                    source_branch_id: root_branch,
                    at_message_id: None,
                    expected_revision: Revision::new(99),
                    operation: token("fork-head", "cd"),
                },
                TimestampMillis::new(220),
            )
            .expect("fork replay");
        assert_eq!(replay.value.branch.id, forked.value.branch.id);
        assert_eq!(
            replay.value.conversation.active_branch_id, root_branch,
            "a replay reports the live selection"
        );

        let at_message = fixture
            .database
            .fork_branch(
                &ForkBranch {
                    conversation_id: fixture.conversation_id,
                    source_branch_id: root_branch,
                    at_message_id: Some(messages[1]),
                    expected_revision: fixture.revision,
                    operation: token("fork-at", "cd"),
                },
                TimestampMillis::new(230),
            )
            .expect("fork at a message");
        assert_eq!(at_message.value.branch.fork_message_id, Some(messages[1]));
        fixture.revision = conversation_revision(&fixture);

        assert_eq!(
            fixture.database.fork_branch(
                &ForkBranch {
                    conversation_id: fixture.conversation_id,
                    source_branch_id: ConversationBranchId::new(),
                    at_message_id: None,
                    expected_revision: fixture.revision,
                    operation: token("fork-missing", "cd"),
                },
                TimestampMillis::new(240),
            ),
            Err(ConversationRepositoryError::NotFound)
        );
        assert_eq!(
            fixture.database.select_branch(
                &SelectBranch {
                    conversation_id: fixture.conversation_id,
                    branch_id: ConversationBranchId::new(),
                    expected_revision: fixture.revision,
                    operation: token("select-missing", "cd"),
                },
                TimestampMillis::new(241),
            ),
            Err(ConversationRepositoryError::NotFound)
        );
        assert_eq!(
            fixture.database.fork_branch(
                &ForkBranch {
                    conversation_id: fixture.conversation_id,
                    source_branch_id: at_message.value.branch.id,
                    at_message_id: Some(messages[0]),
                    expected_revision: fixture.revision,
                    operation: token("fork-foreign-message", "cd"),
                },
                TimestampMillis::new(242),
            ),
            Err(ConversationRepositoryError::Conflict),
            "the fork point must live on the source branch"
        );
    }

    #[test]
    fn a_send_on_a_headless_fork_keeps_the_inherited_ancestry() {
        let mut fixture = direct_fixture();
        let messages = conversation_with_two_exchanges(&mut fixture, "fork-send");
        let forked = fixture
            .database
            .fork_branch(
                &ForkBranch {
                    conversation_id: fixture.conversation_id,
                    source_branch_id: fixture.branch_id,
                    at_message_id: Some(messages[1]),
                    expected_revision: fixture.revision,
                    operation: token("fork-send-point", "cd"),
                },
                TimestampMillis::new(200),
            )
            .expect("fork");
        fixture.branch_id = forked.value.branch.id;
        fixture.revision = conversation_revision(&fixture);
        assert_eq!(forked.value.branch.head_message_id, None);

        let send = fixture
            .database
            .begin_send(
                &send_command(&fixture, "fork-send-user", "cd", text("on the fork")),
                TimestampMillis::new(201),
            )
            .expect("send on the fork");
        let GenerationInput::UserMessage { message_id } = send.value.turn.input else {
            panic!("expected a user message");
        };
        let user = load_message_for_test(&fixture, message_id);
        assert_eq!(
            user.parent_message_id,
            Some(messages[1]),
            "the first message on a fork hangs off the fork point"
        );
        let (assistant, _) = settle_succeeded(&fixture, &send.value.turn, 202);
        fixture.revision = conversation_revision(&fixture);

        let timeline = ConversationReader::timeline_page(
            fixture.database.as_ref(),
            fixture.conversation_id,
            fixture.branch_id,
            &PageRequest {
                cursor: None,
                limit: PageLimit::new(20),
            },
        )
        .expect("forked timeline");
        let rendered: Vec<MessageId> = timeline.items.iter().map(|item| item.message.id).collect();
        assert_eq!(
            rendered,
            vec![assistant, message_id, messages[1], messages[0]],
            "the inherited ancestry survives the first send"
        );
        for pair in timeline.items.windows(2) {
            assert_eq!(
                pair[0].message.parent_message_id,
                Some(pair[1].message.id),
                "the parent chain is unbroken"
            );
        }
    }

    #[test]
    fn tombstone_policies_preserve_cascade_and_fork() {
        let mut fixture = direct_fixture();
        let messages = conversation_with_two_exchanges(&mut fixture, "tomb-preserve");
        let preserved = fixture
            .database
            .tombstone_message(
                &TombstoneMessage {
                    conversation_id: fixture.conversation_id,
                    message_id: messages[3],
                    expected_revision: fixture.revision,
                    operation: token("tombstone-preserve", "cd"),
                    descendants: DescendantPolicy::Preserve,
                },
                TimestampMillis::new(200),
            )
            .expect("preserve");
        assert_eq!(preserved.value.descendant_count, 0);
        assert_eq!(
            preserved.value.message.visibility,
            MessageVisibility::Tombstoned
        );
        assert_eq!(preserved.value.forked_branch, None);
        assert!(matches!(
            preserved.outbox[0].event,
            ConversationOutboxEvent::MessageTombstoned { .. }
        ));
        outbox_reads_back(&fixture, &preserved.outbox);
        fixture.revision = conversation_revision(&fixture);
        assert_eq!(
            fixture.database.tombstone_message(
                &TombstoneMessage {
                    conversation_id: fixture.conversation_id,
                    message_id: messages[3],
                    expected_revision: fixture.revision,
                    operation: token("tombstone-again", "cd"),
                    descendants: DescendantPolicy::Preserve,
                },
                TimestampMillis::new(201),
            ),
            Err(ConversationRepositoryError::Conflict)
        );

        let mut cascade = direct_fixture();
        let messages = conversation_with_two_exchanges(&mut cascade, "tomb-cascade");
        let tombstoned = cascade
            .database
            .tombstone_message(
                &TombstoneMessage {
                    conversation_id: cascade.conversation_id,
                    message_id: messages[2],
                    expected_revision: cascade.revision,
                    operation: token("tombstone-cascade", "cd"),
                    descendants: DescendantPolicy::Tombstone,
                },
                TimestampMillis::new(200),
            )
            .expect("cascade");
        assert_eq!(tombstoned.value.descendant_count, 1);
        let ConversationOutboxEvent::MessageTombstoned {
            affected_message_ids,
            ..
        } = &tombstoned.outbox[0].event
        else {
            panic!("expected a tombstone event");
        };
        assert_eq!(affected_message_ids, &vec![messages[3]]);
        let hidden: i64 = scalar(
            &cascade.database,
            "SELECT count(*) FROM conversation_messages WHERE conversation_id = ?1 AND visibility = 'tombstoned'",
            &cascade.conversation_id.to_string(),
        );
        assert_eq!(hidden, 2);

        let mut partial = direct_fixture();
        let messages = conversation_with_two_exchanges(&mut partial, "tomb-partial");
        partial
            .database
            .tombstone_message(
                &TombstoneMessage {
                    conversation_id: partial.conversation_id,
                    message_id: messages[3],
                    expected_revision: partial.revision,
                    operation: token("tombstone-leaf", "cd"),
                    descendants: DescendantPolicy::Preserve,
                },
                TimestampMillis::new(200),
            )
            .expect("leaf");
        partial.revision = conversation_revision(&partial);
        let ancestor = partial
            .database
            .tombstone_message(
                &TombstoneMessage {
                    conversation_id: partial.conversation_id,
                    message_id: messages[2],
                    expected_revision: partial.revision,
                    operation: token("tombstone-ancestor", "cd"),
                    descendants: DescendantPolicy::Tombstone,
                },
                TimestampMillis::new(201),
            )
            .expect("ancestor");
        assert_eq!(
            ancestor.value.descendant_count, 0,
            "an already tombstoned descendant is not retired twice"
        );

        let mut forked = direct_fixture();
        let messages = conversation_with_two_exchanges(&mut forked, "tomb-fork");
        assert_eq!(
            forked.database.tombstone_message(
                &TombstoneMessage {
                    conversation_id: forked.conversation_id,
                    message_id: messages[0],
                    expected_revision: forked.revision,
                    operation: token("tombstone-root-fork", "cd"),
                    descendants: DescendantPolicy::Fork,
                },
                TimestampMillis::new(200),
            ),
            Err(ConversationRepositoryError::Conflict),
            "a root message has no parent to fork from"
        );
        let result = forked
            .database
            .tombstone_message(
                &TombstoneMessage {
                    conversation_id: forked.conversation_id,
                    message_id: messages[1],
                    expected_revision: forked.revision,
                    operation: token("tombstone-fork", "cd"),
                    descendants: DescendantPolicy::Fork,
                },
                TimestampMillis::new(201),
            )
            .expect("fork tombstone");
        let branch = result.value.forked_branch.clone().expect("forked branch");
        assert_eq!(branch.fork_message_id, Some(messages[0]));
        assert_eq!(branch.parent_branch_id, Some(forked.branch_id));
        assert_eq!(result.value.conversation.active_branch_id, branch.id);
        assert_eq!(result.value.descendant_count, 0);
        assert_eq!(result.outbox.len(), 2);
        assert!(matches!(
            result.outbox[1].event,
            ConversationOutboxEvent::BranchForked { .. }
        ));
        forked.revision = conversation_revision(&forked);

        let replay = forked
            .database
            .tombstone_message(
                &TombstoneMessage {
                    conversation_id: forked.conversation_id,
                    message_id: messages[1],
                    expected_revision: Revision::new(99),
                    operation: token("tombstone-fork", "cd"),
                    descendants: DescendantPolicy::Fork,
                },
                TimestampMillis::new(202),
            )
            .expect("replay");
        assert_eq!(replay.outbox, result.outbox);
        assert_eq!(
            replay.value.forked_branch.map(|branch| branch.id),
            Some(branch.id)
        );
    }

    #[test]
    fn a_tombstoned_message_takes_no_further_content_changes() {
        let mut fixture = direct_fixture();
        let messages = conversation_with_two_exchanges(&mut fixture, "tomb-guard");
        fixture
            .database
            .tombstone_message(
                &TombstoneMessage {
                    conversation_id: fixture.conversation_id,
                    message_id: messages[3],
                    expected_revision: fixture.revision,
                    operation: token("tombstone-guard", "cd"),
                    descendants: DescendantPolicy::Preserve,
                },
                TimestampMillis::new(200),
            )
            .expect("tombstone");
        fixture.revision = conversation_revision(&fixture);
        let draft = |scene_edited: bool| lettuce_conversations::MessageEditDraft {
            parts: text("edited"),
            visibility: MessageVisibility::Visible,
            pinned: false,
            scene_edited,
        };
        assert_eq!(
            fixture.database.edit_message(
                &EditMessage {
                    conversation_id: fixture.conversation_id,
                    message_id: messages[3],
                    expected_revision: fixture.revision,
                    operation: token("edit-tombstoned", "cd"),
                    draft: draft(false),
                },
                TimestampMillis::new(201),
            ),
            Err(ConversationRepositoryError::Conflict)
        );
        assert_eq!(
            fixture.database.update_message_flags(
                &UpdateMessageFlags {
                    conversation_id: fixture.conversation_id,
                    message_id: messages[3],
                    expected_revision: fixture.revision,
                    operation: token("flags-tombstoned", "cd"),
                    pinned: Some(true),
                    visibility: None,
                },
                TimestampMillis::new(202),
            ),
            Err(ConversationRepositoryError::Conflict)
        );
        assert_eq!(
            fixture.database.edit_message(
                &EditMessage {
                    conversation_id: fixture.conversation_id,
                    message_id: messages[2],
                    expected_revision: fixture.revision,
                    operation: token("edit-scene-flag", "cd"),
                    draft: draft(true),
                },
                TimestampMillis::new(203),
            ),
            Err(invalid("message_edit.scene_edited")),
            "scene editing belongs to scene messages"
        );
    }

    fn load_message_for_test(fixture: &Fixture, message_id: MessageId) -> Message {
        let mut connection = fixture.database.connection().expect("connection");
        let transaction = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Deferred)
            .expect("transaction");
        let message =
            load_message(&transaction, fixture.conversation_id, message_id).expect("message");
        transaction.commit().expect("commit");
        message
    }

    fn active_candidate(fixture: &Fixture, message_id: MessageId) -> MessageCandidateId {
        let value: String = scalar(
            &fixture.database,
            "SELECT active_candidate_id FROM conversation_messages WHERE id = ?1",
            &message_id.to_string(),
        );
        value.parse().expect("candidate id")
    }

    fn media_state(fixture: &Fixture, candidate_id: MessageCandidateId) -> String {
        scalar(
            &fixture.database,
            "SELECT state FROM candidate_media_refs WHERE candidate_id = ?1",
            &candidate_id.to_string(),
        )
    }

    #[test]
    fn a_tombstoned_message_refuses_a_candidate_choice() {
        let mut fixture = direct_fixture();
        let asset_id = stage_media_asset(&fixture.database, "44");
        let send = fixture
            .database
            .begin_send(
                &send_command(&fixture, "send-tomb-choose", "cd", text("hello")),
                TimestampMillis::new(20),
            )
            .expect("send");
        let revision = drive(
            &fixture,
            send.value.turn.id,
            send.value.attempt.id,
            &[
                GenerationTurnStatus::Preparing,
                GenerationTurnStatus::ContextPrepared,
                GenerationTurnStatus::Running,
            ],
            "drive-tomb-choose",
            21,
        );
        let finalized = fixture
            .database
            .finalize_generation(
                send.value.turn.id,
                send.value.attempt.id,
                conversation_revision(&fixture),
                revision,
                &token("finalize-tomb-choose", "cd"),
                finalization_draft(
                    vec![MessagePart::MediaAsset {
                        asset_id,
                        role: lettuce_conversations::MediaAssetRole::Inline,
                    }],
                    0,
                ),
                UsageEventId::new(),
                TimestampMillis::new(30),
            )
            .expect("finalize");
        let message_id = finalized.value.assistant_message.id;
        let candidate_id = finalized.value.candidate.id;
        assert_eq!(media_state(&fixture, candidate_id), "active");
        fixture.revision = conversation_revision(&fixture);

        fixture
            .database
            .tombstone_message(
                &TombstoneMessage {
                    conversation_id: fixture.conversation_id,
                    message_id,
                    expected_revision: fixture.revision,
                    operation: token("tombstone-before-choose", "cd"),
                    descendants: DescendantPolicy::Preserve,
                },
                TimestampMillis::new(31),
            )
            .expect("tombstone");
        assert_eq!(media_state(&fixture, candidate_id), "historical");
        fixture.revision = conversation_revision(&fixture);

        assert_eq!(
            fixture.database.choose_candidate(
                &ChooseCandidate {
                    conversation_id: fixture.conversation_id,
                    message_id,
                    candidate_id,
                    expected_revision: fixture.revision,
                    operation: token("choose-tombstoned", "cd"),
                },
                TimestampMillis::new(32),
            ),
            Err(ConversationRepositoryError::Conflict)
        );
        assert_eq!(
            media_state(&fixture, candidate_id),
            "historical",
            "a refused choice reinstates nothing"
        );
        assert_eq!(conversation_revision(&fixture), fixture.revision);
    }

    #[test]
    fn a_live_turn_blocks_content_and_branch_admission() {
        let mut fixture = direct_fixture();
        let messages = conversation_with_two_exchanges(&mut fixture, "live-guard");
        let candidate_id = active_candidate(&fixture, messages[3]);
        let turn_id = chosen_turn(&fixture, candidate_id);
        let regenerate = fixture
            .database
            .begin_regenerate(
                &RegenerateCandidate {
                    conversation_id: fixture.conversation_id,
                    branch_id: fixture.branch_id,
                    message_id: messages[3],
                    turn_id,
                    expected_revision: fixture.revision,
                    expected_turn_revision: turn_revision(&fixture, turn_id),
                    operation: token("live-guard-regen", "cd"),
                    active_candidate_id: candidate_id,
                    guidance: None,
                    model_override: None,
                    forced_speaker: None,
                    swap_roles: false,
                },
                TimestampMillis::new(200),
            )
            .expect("regenerate");
        fixture.revision = conversation_revision(&fixture);

        assert_eq!(
            fixture.database.tombstone_message(
                &TombstoneMessage {
                    conversation_id: fixture.conversation_id,
                    message_id: messages[0],
                    expected_revision: fixture.revision,
                    operation: token("live-guard-tombstone", "cd"),
                    descendants: DescendantPolicy::Tombstone,
                },
                TimestampMillis::new(201),
            ),
            Err(ConversationRepositoryError::Conflict),
            "a descendant is still being regenerated"
        );
        assert_eq!(
            fixture.database.fork_branch(
                &ForkBranch {
                    conversation_id: fixture.conversation_id,
                    source_branch_id: fixture.branch_id,
                    at_message_id: None,
                    expected_revision: fixture.revision,
                    operation: token("live-guard-fork", "cd"),
                },
                TimestampMillis::new(202),
            ),
            Err(ConversationRepositoryError::Conflict)
        );
        assert_eq!(
            fixture.database.select_branch(
                &SelectBranch {
                    conversation_id: fixture.conversation_id,
                    branch_id: fixture.branch_id,
                    expected_revision: fixture.revision,
                    operation: token("live-guard-select", "cd"),
                },
                TimestampMillis::new(203),
            ),
            Err(ConversationRepositoryError::Conflict)
        );

        settle_failed(&fixture, &regenerate.value.turn, 210);
        fixture.revision = conversation_revision(&fixture);
        let forked = fixture
            .database
            .fork_branch(
                &ForkBranch {
                    conversation_id: fixture.conversation_id,
                    source_branch_id: fixture.branch_id,
                    at_message_id: None,
                    expected_revision: fixture.revision,
                    operation: token("live-guard-fork-after", "cd"),
                },
                TimestampMillis::new(220),
            )
            .expect("a settled conversation forks");
        fixture.branch_id = forked.value.branch.id;
        fixture.revision = conversation_revision(&fixture);

        let send = fixture
            .database
            .begin_send(
                &send_command(&fixture, "live-guard-send", "cd", text("pending")),
                TimestampMillis::new(230),
            )
            .expect("send");
        let GenerationInput::UserMessage { message_id } = send.value.turn.input else {
            panic!("expected a user message");
        };
        fixture.revision = conversation_revision(&fixture);
        assert_eq!(
            fixture.database.tombstone_message(
                &TombstoneMessage {
                    conversation_id: fixture.conversation_id,
                    message_id,
                    expected_revision: fixture.revision,
                    operation: token("live-guard-tombstone-input", "cd"),
                    descendants: DescendantPolicy::Preserve,
                },
                TimestampMillis::new(231),
            ),
            Err(ConversationRepositoryError::Conflict),
            "the live turn reads this user message"
        );
        assert_eq!(
            fixture.database.edit_message(
                &EditMessage {
                    conversation_id: fixture.conversation_id,
                    message_id,
                    expected_revision: fixture.revision,
                    operation: token("live-guard-edit-input", "cd"),
                    draft: lettuce_conversations::MessageEditDraft {
                        parts: text("rewritten"),
                        visibility: MessageVisibility::Visible,
                        pinned: false,
                        scene_edited: false,
                    },
                },
                TimestampMillis::new(232),
            ),
            Err(ConversationRepositoryError::Conflict)
        );
    }

    #[test]
    fn a_replayed_choice_reports_the_current_render_source() {
        let mut fixture = direct_fixture();
        let (message_id, first, second) = two_candidates(&mut fixture, "choose-replay");
        let choice = |candidate_id, key: &str, revision| ChooseCandidate {
            conversation_id: fixture.conversation_id,
            message_id,
            candidate_id,
            expected_revision: revision,
            operation: token(key, "cd"),
        };
        let earlier = fixture
            .database
            .choose_candidate(
                &choice(first, "choose-earlier", fixture.revision),
                TimestampMillis::new(100),
            )
            .expect("choose the first candidate");
        fixture.revision = conversation_revision(&fixture);
        fixture
            .database
            .choose_candidate(
                &choice(second, "choose-later", fixture.revision),
                TimestampMillis::new(101),
            )
            .expect("choose the second candidate");
        fixture.revision = conversation_revision(&fixture);

        let replay = fixture
            .database
            .choose_candidate(
                &choice(first, "choose-earlier", Revision::new(99)),
                TimestampMillis::new(102),
            )
            .expect("the earlier choice still replays");
        assert_eq!(replay.operation, earlier.operation);
        assert_eq!(replay.outbox, earlier.outbox);
        assert_eq!(
            replay.value.active_render_source,
            lettuce_conversations::MessageRenderSource::Candidate(second),
            "a replay reports current state"
        );
    }

    #[test]
    fn archiving_blocks_mutations_until_restore() {
        let mut fixture = direct_fixture();
        let archived = fixture
            .database
            .archive(
                &ArchiveConversation {
                    conversation_id: fixture.conversation_id,
                    expected_revision: fixture.revision,
                    operation: token("archive-one", "cd"),
                },
                TimestampMillis::new(20),
            )
            .expect("archive");
        assert_eq!(archived.value.lifecycle, ConversationLifecycle::Archived);
        assert!(matches!(
            archived.outbox[0].event,
            ConversationOutboxEvent::ConversationLifecycleChanged {
                lifecycle: ConversationLifecycle::Archived,
                ..
            }
        ));
        fixture.revision = conversation_revision(&fixture);

        assert_eq!(
            fixture.database.begin_send(
                &send_command(&fixture, "send-archived", "cd", text("hello")),
                TimestampMillis::new(21)
            ),
            Err(ConversationRepositoryError::Conflict)
        );
        assert_eq!(
            fixture.database.archive(
                &ArchiveConversation {
                    conversation_id: fixture.conversation_id,
                    expected_revision: fixture.revision,
                    operation: token("archive-two", "cd"),
                },
                TimestampMillis::new(22),
            ),
            Err(ConversationRepositoryError::Conflict)
        );

        let restored = fixture
            .database
            .restore(
                &RestoreConversation {
                    conversation_id: fixture.conversation_id,
                    expected_revision: fixture.revision,
                    operation: token("restore-one", "cd"),
                },
                TimestampMillis::new(23),
            )
            .expect("restore");
        assert_eq!(restored.value.lifecycle, ConversationLifecycle::Active);
        fixture.revision = conversation_revision(&fixture);
        assert_eq!(
            fixture.database.restore(
                &RestoreConversation {
                    conversation_id: fixture.conversation_id,
                    expected_revision: fixture.revision,
                    operation: token("restore-two", "cd"),
                },
                TimestampMillis::new(24),
            ),
            Err(ConversationRepositoryError::Conflict)
        );
        fixture
            .database
            .begin_send(
                &send_command(&fixture, "send-restored", "cd", text("hello")),
                TimestampMillis::new(25),
            )
            .expect("the restored conversation accepts work again");

        let replay = fixture.database.archive(
            &ArchiveConversation {
                conversation_id: fixture.conversation_id,
                expected_revision: Revision::new(99),
                operation: token("archive-one", "cd"),
            },
            TimestampMillis::new(26),
        );
        assert_eq!(
            replay,
            Err(ConversationRepositoryError::Conflict),
            "the archive replay no longer describes an archived conversation"
        );
    }

    #[test]
    fn participant_policy_accepts_a_newly_staged_model() {
        let mut fixture = group_fixture();
        let participant_id = fixture.characters[1];
        let muted = fixture
            .database
            .update_participant_policy(
                &UpdateParticipantPolicy {
                    conversation_id: fixture.conversation_id,
                    participant_id,
                    expected_revision: fixture.revision,
                    operation: token("policy-mute", "cd"),
                    enabled: None,
                    muted: Some(true),
                    model_override: None,
                },
                TimestampMillis::new(20),
            )
            .expect("mute");
        let participant = |conversation: &lettuce_conversations::Conversation| {
            conversation
                .participants
                .iter()
                .find(|participant| participant.id == participant_id)
                .expect("participant")
                .clone()
        };
        assert!(participant(&muted.value).muted);
        assert!(participant(&muted.value).enabled);
        assert!(matches!(
            muted.outbox[0].event,
            ConversationOutboxEvent::ParticipantPolicyChanged { .. }
        ));
        fixture.revision = conversation_revision(&fixture);

        let model = staged_model(fixture.database.as_ref());
        let updated = fixture
            .database
            .update_participant_policy(
                &UpdateParticipantPolicy {
                    conversation_id: fixture.conversation_id,
                    participant_id,
                    expected_revision: fixture.revision,
                    operation: token("policy-model", "cd"),
                    enabled: Some(false),
                    muted: None,
                    model_override: Some(SnapshotSelection::Explicit(model.clone())),
                },
                TimestampMillis::new(21),
            )
            .expect("model override");
        assert!(!participant(&updated.value).enabled);
        assert!(participant(&updated.value).muted);
        assert_eq!(
            participant(&updated.value).model_selection,
            SnapshotSelection::Explicit(model)
        );
        ConversationReader::get(fixture.database.as_ref(), fixture.conversation_id)
            .expect("a policy model outside the launch set still hydrates");
        fixture.revision = conversation_revision(&fixture);

        assert!(matches!(
            fixture.database.update_participant_policy(
                &UpdateParticipantPolicy {
                    conversation_id: fixture.conversation_id,
                    participant_id,
                    expected_revision: fixture.revision,
                    operation: token("policy-unstaged", "cd"),
                    enabled: None,
                    muted: Some(false),
                    model_override: Some(SnapshotSelection::Explicit(model_snapshot())),
                },
                TimestampMillis::new(22),
            ),
            Err(ConversationRepositoryError::ArtifactReference(_))
        ));
        assert!(matches!(
            fixture.database.update_participant_policy(
                &UpdateParticipantPolicy {
                    conversation_id: fixture.conversation_id,
                    participant_id: fixture.user_participant,
                    expected_revision: fixture.revision,
                    operation: token("policy-user", "cd"),
                    enabled: Some(false),
                    muted: None,
                    model_override: None,
                },
                TimestampMillis::new(23),
            ),
            Err(ConversationRepositoryError::Invalid(_))
        ));
    }

    #[test]
    fn settings_are_created_once_then_compare_and_swapped() {
        let fixture = direct_fixture();
        let patch = |author_note: PatchValue<String>, model: PatchValue<ModelSelectionSnapshot>| {
            CurrentConversationSettingsPatch {
                author_note,
                memory: PatchValue::Keep,
                model_override: model,
                voice: PatchValue::Keep,
            }
        };
        let created = fixture
            .database
            .update_settings(
                &UpdateConversationSettings {
                    conversation_id: fixture.conversation_id,
                    expected_settings_revision: None,
                    operation: token("settings-create", "cd"),
                    patch: patch(PatchValue::Set("be brief".into()), PatchValue::Keep),
                },
                TimestampMillis::new(20),
            )
            .expect("create settings");
        let settings = created
            .value
            .current_settings
            .clone()
            .expect("settings materialized");
        assert_eq!(settings.revision, Revision::INITIAL);
        assert_eq!(settings.author_note.as_deref(), Some("be brief"));
        assert_eq!(
            settings.author_note_provenance,
            lettuce_conversations::SettingProvenance::CurrentOverride
        );
        assert_eq!(
            settings.model_provenance,
            lettuce_conversations::SettingProvenance::LaunchInherited
        );
        assert!(matches!(
            created.outbox[0].event,
            ConversationOutboxEvent::SettingsChanged { .. }
        ));
        outbox_reads_back(&fixture, &created.outbox);

        assert_eq!(
            fixture.database.update_settings(
                &UpdateConversationSettings {
                    conversation_id: fixture.conversation_id,
                    expected_settings_revision: None,
                    operation: token("settings-create-again", "cd"),
                    patch: patch(PatchValue::Keep, PatchValue::Keep),
                },
                TimestampMillis::new(21),
            ),
            Err(ConversationRepositoryError::Conflict)
        );
        assert_eq!(
            fixture.database.update_settings(
                &UpdateConversationSettings {
                    conversation_id: fixture.conversation_id,
                    expected_settings_revision: Some(Revision::new(7)),
                    operation: token("settings-stale", "cd"),
                    patch: patch(PatchValue::Keep, PatchValue::Keep),
                },
                TimestampMillis::new(22),
            ),
            Err(ConversationRepositoryError::StaleRevision {
                expected: Revision::new(7),
                actual: Revision::INITIAL,
            })
        );
        assert!(matches!(
            fixture.database.update_settings(
                &UpdateConversationSettings {
                    conversation_id: fixture.conversation_id,
                    expected_settings_revision: Some(Revision::INITIAL),
                    operation: token("settings-unstaged-model", "cd"),
                    patch: patch(PatchValue::Keep, PatchValue::Set(model_snapshot())),
                },
                TimestampMillis::new(23),
            ),
            Err(ConversationRepositoryError::ArtifactReference(_))
        ));

        let model = staged_model(fixture.database.as_ref());
        let updated = fixture
            .database
            .update_settings(
                &UpdateConversationSettings {
                    conversation_id: fixture.conversation_id,
                    expected_settings_revision: Some(Revision::INITIAL),
                    operation: token("settings-update", "cd"),
                    patch: patch(PatchValue::Clear, PatchValue::Set(model.clone())),
                },
                TimestampMillis::new(24),
            )
            .expect("update settings");
        let settings = updated
            .value
            .current_settings
            .clone()
            .expect("settings present");
        assert_eq!(settings.revision, Revision::new(2));
        assert_eq!(settings.author_note, None);
        assert_eq!(
            settings.author_note_provenance,
            lettuce_conversations::SettingProvenance::Disabled
        );
        assert_eq!(settings.model_override, Some(model));
        ConversationReader::get(fixture.database.as_ref(), fixture.conversation_id)
            .expect("settings hydrate with a freshly staged model");

        let replay = fixture
            .database
            .update_settings(
                &UpdateConversationSettings {
                    conversation_id: fixture.conversation_id,
                    expected_settings_revision: Some(Revision::INITIAL),
                    operation: token("settings-update", "cd"),
                    patch: patch(PatchValue::Clear, PatchValue::Keep),
                },
                TimestampMillis::new(25),
            )
            .expect("replay");
        assert_eq!(replay.outbox, updated.outbox);
        assert_eq!(
            replay.value.current_settings.expect("settings").revision,
            Revision::new(2)
        );
    }

    #[test]
    fn a_group_turn_without_a_speaker_cannot_finalize_but_still_settles() {
        let mut fixture = group_fixture();
        let send = fixture
            .database
            .begin_send(
                &send_command(&fixture, "group-no-speaker", "cd", text("hello")),
                TimestampMillis::new(20),
            )
            .expect("group send");
        let revision = drive(
            &fixture,
            send.value.turn.id,
            send.value.attempt.id,
            &[
                GenerationTurnStatus::Preparing,
                GenerationTurnStatus::ContextPrepared,
                GenerationTurnStatus::Running,
            ],
            "drive-group-no-speaker",
            21,
        );
        assert_eq!(
            fixture.database.finalize_generation(
                send.value.turn.id,
                send.value.attempt.id,
                conversation_revision(&fixture),
                revision,
                &token("group-no-speaker-finalize", "cd"),
                finalization_draft(text("generated"), 0),
                UsageEventId::new(),
                TimestampMillis::new(30),
            ),
            Err(ConversationRepositoryError::Conflict),
            "a group cast has no sole character to author the candidate"
        );
        let failed = fixture
            .database
            .fail_generation(
                send.value.turn.id,
                send.value.attempt.id,
                conversation_revision(&fixture),
                revision,
                &token("group-no-speaker-fail", "cd"),
                GenerationFailureCode::SpeakerUnavailable,
                UsageEventId::new(),
                TimestampMillis::new(31),
            )
            .expect("fail recovers the stuck turn");
        assert_eq!(failed.value.turn.status, GenerationTurnStatus::Failed);

        fixture.revision = conversation_revision(&fixture);
        let second = fixture
            .database
            .begin_send(
                &send_command(&fixture, "group-no-speaker-two", "cd", text("again")),
                TimestampMillis::new(40),
            )
            .expect("second group send");
        settle_cancelled(&fixture, &second.value.turn, 41);
        assert_eq!(
            turn_status(&fixture, second.value.turn.id),
            GenerationTurnStatus::Cancelled
        );
    }
}
