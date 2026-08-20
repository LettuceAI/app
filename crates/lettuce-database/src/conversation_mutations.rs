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
    AttachAttemptJob, AttachAttemptJobResult, BeginGeneration, ContinueConversation,
    ContinueConversationResult, ConversationRepository, ConversationRepositoryError,
    GenerationAttempt, GenerationCheckpointEnvelope, GenerationCheckpointEvent, GenerationInput,
    GenerationOperation, GenerationTarget, GenerationTurn, GenerationTurnStatus, MessagePart,
    OperationKind, OperationResultRef, OperationToken, RegenerateCandidate,
    RegenerateCandidateResult, RetryGeneration, RetryGenerationResult, SendConversation,
    SendConversationResult, attempt_job_idempotency_key,
};
use lettuce_types::{
    ConversationBranchId, ConversationId, ConversationParticipantId, GenerationAttemptId,
    GenerationTurnId, MessageId, MessageRevisionId, Revision, TimestampMillis,
};
use rusqlite::{OptionalExtension, Transaction, params};

use super::{
    Database, conversation_creator, conversation_mutation_kernel as kernel, conversation_query,
    conversation_vertical_slice as slice,
};

const SQLITE_CONSTRAINT_UNIQUE: i32 = 2067;

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

/// `append_event` has no expected revision to compare, so it reads the
/// lifecycle directly to keep the active-conversation contract.
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
        lettuce_conversations::ConversationLifecycle::Active => Ok(()),
        _ => Err(ConversationRepositoryError::Conflict),
    }
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
fn insert_first_attempt(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    turn_id: GenerationTurnId,
) -> Result<GenerationAttemptId, ConversationRepositoryError> {
    let attempt_id = GenerationAttemptId::new();
    transaction
        .execute(
            "INSERT INTO generation_attempts (conversation_id, turn_id, id, ordinal, parent_attempt_id, status, job_idempotency_key, job_id, started_at, finished_at, usage_event_id, usage_outcome, failure) VALUES (?1, ?2, ?3, 0, NULL, 'created', ?4, NULL, NULL, NULL, NULL, NULL, NULL)",
            params![
                conversation_id.to_string(),
                turn_id.to_string(),
                attempt_id.to_string(),
                attempt_job_idempotency_key(turn_id, attempt_id).as_str(),
            ],
        )
        .map_err(kernel::map_constraint)?;
    Ok(attempt_id)
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

/// The remaining methods arrive with the settlement and content slices.
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
                    branch_head(transaction, context.conversation_id, command.branch_id)?;
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
                let (status, branch_id): (String, String) = transaction
                    .query_row(
                        "SELECT status, branch_id FROM conversation_turns WHERE conversation_id = ?1 AND id = ?2",
                        params![
                            context.conversation_id.to_string(),
                            command.turn_id.to_string(),
                        ],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .optional()
                    .map_err(slice::db)?
                    .ok_or(ConversationRepositoryError::NotFound)?;
                if branch_id != command.branch_id.to_string()
                    || !matches!(status.as_str(), "failed" | "cancelled")
                {
                    return Err(ConversationRepositoryError::Conflict);
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
                require_active_conversation(transaction, context.conversation_id)?;
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
                kernel::require_active(&conversation)?;
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
        _turn_id: GenerationTurnId,
        _attempt_id: GenerationAttemptId,
        _expected_conversation_revision: Revision,
        _expected_turn_revision: Revision,
        _operation: &OperationToken,
        _draft: lettuce_conversations::FinalizationDraft,
        _usage_event_id: lettuce_types::UsageEventId,
        _now: TimestampMillis,
    ) -> Result<lettuce_conversations::GenerationFinalizationResult, ConversationRepositoryError>
    {
        Err(ConversationRepositoryError::Unsupported)
    }

    fn fail_generation(
        &self,
        _turn_id: GenerationTurnId,
        _attempt_id: GenerationAttemptId,
        _expected_conversation_revision: Revision,
        _expected_turn_revision: Revision,
        _operation: &OperationToken,
        _failure: lettuce_conversations::GenerationFailureCode,
        _usage_event_id: lettuce_types::UsageEventId,
        _now: TimestampMillis,
    ) -> Result<lettuce_conversations::GenerationFailureResult, ConversationRepositoryError> {
        Err(ConversationRepositoryError::Unsupported)
    }

    fn interrupt_generation(
        &self,
        _turn_id: GenerationTurnId,
        _attempt_id: GenerationAttemptId,
        _expected_conversation_revision: Revision,
        _expected_turn_revision: Revision,
        _operation: &OperationToken,
        _usage_event_id: lettuce_types::UsageEventId,
        _now: TimestampMillis,
    ) -> Result<lettuce_conversations::GenerationInterruptionResult, ConversationRepositoryError>
    {
        Err(ConversationRepositoryError::Unsupported)
    }

    fn request_cancellation(
        &self,
        _command: &lettuce_conversations::CancelGeneration,
        _now: TimestampMillis,
    ) -> Result<lettuce_conversations::RequestCancellationResult, ConversationRepositoryError> {
        Err(ConversationRepositoryError::Unsupported)
    }

    fn settle_cancellation(
        &self,
        _command: &lettuce_conversations::SettleCancellation,
        _now: TimestampMillis,
    ) -> Result<lettuce_conversations::SettleCancellationResult, ConversationRepositoryError> {
        Err(ConversationRepositoryError::Unsupported)
    }

    fn recover_generation(
        &self,
        _turn_id: GenerationTurnId,
        _attempt_id: GenerationAttemptId,
        _expected_conversation_revision: Revision,
        _expected_turn_revision: Revision,
        _operation: &OperationToken,
        _now: TimestampMillis,
    ) -> Result<lettuce_conversations::GenerationRecoveryResult, ConversationRepositoryError> {
        Err(ConversationRepositoryError::Unsupported)
    }

    fn choose_candidate(
        &self,
        _command: &lettuce_conversations::ChooseCandidate,
        _now: TimestampMillis,
    ) -> Result<lettuce_conversations::ChooseCandidateResult, ConversationRepositoryError> {
        Err(ConversationRepositoryError::Unsupported)
    }

    fn fork_branch(
        &self,
        _command: &lettuce_conversations::ForkBranch,
        _now: TimestampMillis,
    ) -> Result<lettuce_conversations::ForkBranchResult, ConversationRepositoryError> {
        Err(ConversationRepositoryError::Unsupported)
    }

    fn select_branch(
        &self,
        _command: &lettuce_conversations::SelectBranch,
        _now: TimestampMillis,
    ) -> Result<lettuce_conversations::SelectBranchResult, ConversationRepositoryError> {
        Err(ConversationRepositoryError::Unsupported)
    }

    fn edit_message(
        &self,
        _command: &lettuce_conversations::EditMessage,
        _now: TimestampMillis,
    ) -> Result<lettuce_conversations::EditMessageResult, ConversationRepositoryError> {
        Err(ConversationRepositoryError::Unsupported)
    }

    fn update_message_flags(
        &self,
        _command: &lettuce_conversations::UpdateMessageFlags,
        _now: TimestampMillis,
    ) -> Result<lettuce_conversations::UpdateMessageFlagsResult, ConversationRepositoryError> {
        Err(ConversationRepositoryError::Unsupported)
    }

    fn tombstone_message(
        &self,
        _command: &lettuce_conversations::TombstoneMessage,
        _now: TimestampMillis,
    ) -> Result<lettuce_conversations::TombstoneMessageResult, ConversationRepositoryError> {
        Err(ConversationRepositoryError::Unsupported)
    }

    fn archive(
        &self,
        _command: &lettuce_conversations::ArchiveConversation,
        _now: TimestampMillis,
    ) -> Result<lettuce_conversations::ArchiveConversationResult, ConversationRepositoryError> {
        Err(ConversationRepositoryError::Unsupported)
    }

    fn restore(
        &self,
        _command: &lettuce_conversations::RestoreConversation,
        _now: TimestampMillis,
    ) -> Result<lettuce_conversations::RestoreConversationResult, ConversationRepositoryError> {
        Err(ConversationRepositoryError::Unsupported)
    }

    fn update_participant_policy(
        &self,
        _command: &lettuce_conversations::UpdateParticipantPolicy,
        _now: TimestampMillis,
    ) -> Result<lettuce_conversations::ParticipantPolicyResult, ConversationRepositoryError> {
        Err(ConversationRepositoryError::Unsupported)
    }

    fn update_settings(
        &self,
        _command: &lettuce_conversations::UpdateConversationSettings,
        _now: TimestampMillis,
    ) -> Result<lettuce_conversations::SettingsResult, ConversationRepositoryError> {
        Err(ConversationRepositoryError::Unsupported)
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

    fn stage_media_asset(database: &Database) -> AssetId {
        let asset_id = AssetId::new();
        let blob_id = MediaBlobId::new();
        let connection = database.connection().expect("connection");
        connection
            .execute(
                "INSERT INTO media_blobs (id, content_hash, kind, mime_type, byte_size, width, height, duration_ms, validation_version, state, created_at, updated_at) VALUES (?1, ?2, 'image', 'image/png', 4, NULL, NULL, NULL, 1, 'ready', 1, 1)",
                params![blob_id.to_string(), "ab".repeat(32)],
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

    fn settle_terminal(fixture: &Fixture, turn: &GenerationTurn, outcome: &str, now: i64) {
        let attempt = turn.attempts.first().expect("attempt");
        let mut connection = fixture.database.connection().expect("connection");
        let transaction = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .expect("transaction");
        settle_attempt(
            &transaction,
            fixture.conversation_id,
            turn.id,
            attempt.id,
            outcome,
            now,
        );
        let intermediate = if outcome == "cancelled" {
            "cancellation_requested"
        } else {
            "preparing"
        };
        transaction
            .execute(
                "UPDATE conversation_turns SET status = ?3, revision = revision + 1, updated_at = ?4 WHERE conversation_id = ?1 AND id = ?2",
                params![
                    fixture.conversation_id.to_string(),
                    turn.id.to_string(),
                    intermediate,
                    now,
                ],
            )
            .expect("intermediate status");
        transaction
            .execute(
                "UPDATE conversation_turns SET status = ?3, failure = ?4, revision = revision + 1, updated_at = ?5 WHERE conversation_id = ?1 AND id = ?2",
                params![
                    fixture.conversation_id.to_string(),
                    turn.id.to_string(),
                    outcome,
                    if outcome == "failed" { Some("internal") } else { None },
                    now,
                ],
            )
            .expect("terminal status");
        transaction.commit().expect("commit");
    }

    fn settle_attempt(
        transaction: &Transaction<'_>,
        conversation_id: ConversationId,
        turn_id: GenerationTurnId,
        attempt_id: GenerationAttemptId,
        outcome: &str,
        now: i64,
    ) {
        let usage_event_id = UsageEventId::new();
        transaction
            .execute(
                "INSERT INTO conversation_usage_refs (conversation_id, turn_id, attempt_id, usage_event_id, outcome, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    conversation_id.to_string(),
                    turn_id.to_string(),
                    attempt_id.to_string(),
                    usage_event_id.to_string(),
                    outcome,
                    now,
                ],
            )
            .expect("usage ref");
        transaction
            .execute(
                "UPDATE generation_attempts SET status = ?4, started_at = ?5, finished_at = ?5, usage_event_id = ?6, usage_outcome = ?4, failure = ?7 WHERE conversation_id = ?1 AND turn_id = ?2 AND id = ?3",
                params![
                    conversation_id.to_string(),
                    turn_id.to_string(),
                    attempt_id.to_string(),
                    outcome,
                    now,
                    usage_event_id.to_string(),
                    if outcome == "failed" { Some("internal") } else { None },
                ],
            )
            .expect("attempt outcome");
    }

    /// Drives a live turn all the way to `succeeded` with a materialized
    /// assistant message and candidate, using the trigger-legal status path.
    fn settle_succeeded(
        fixture: &Fixture,
        turn: &GenerationTurn,
        author: ConversationParticipantId,
        now: i64,
    ) -> (MessageId, MessageCandidateId) {
        let GenerationTarget::NewAssistant {
            message_id,
            parent_message_id,
        } = turn.target
        else {
            panic!("expected a new assistant target");
        };
        let attempt = turn.attempts.first().expect("attempt");
        let candidate_id = MessageCandidateId::new();
        let mut connection = fixture.database.connection().expect("connection");
        let transaction = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .expect("transaction");
        let ordinal: i64 = transaction
            .query_row(
                "UPDATE conversations SET next_timeline_ordinal = next_timeline_ordinal + 1 WHERE id = ?1 RETURNING next_timeline_ordinal - 1",
                [fixture.conversation_id.to_string()],
                |row| row.get(0),
            )
            .expect("ordinal");
        transaction
            .execute(
                "INSERT INTO conversation_messages (conversation_id, id, branch_id, parent_message_id, author_participant_id, role, timeline_ordinal, logical_time, effective_time, visibility, pinned, scene_edited, active_revision_id, active_candidate_id, revision, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, 'assistant', ?6, ?7, ?7, 'visible', 0, 0, NULL, ?8, 1, ?7, ?7)",
                params![
                    fixture.conversation_id.to_string(),
                    message_id.to_string(),
                    fixture.branch_id.to_string(),
                    parent_message_id.map(|id| id.to_string()),
                    author.to_string(),
                    ordinal,
                    now,
                    candidate_id.to_string(),
                ],
            )
            .expect("assistant message");
        transaction
            .execute(
                "INSERT INTO conversation_message_candidates (conversation_id, id, message_id, branch_id, turn_id, attempt_id, author_participant_id, ordinal, parts_json, model_json, created_at, provider_replay_artifact_id, provider_replay_retention) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8, ?9, ?10, NULL, NULL)",
                params![
                    fixture.conversation_id.to_string(),
                    candidate_id.to_string(),
                    message_id.to_string(),
                    fixture.branch_id.to_string(),
                    turn.id.to_string(),
                    attempt.id.to_string(),
                    author.to_string(),
                    slice::encode(&text("generated")).expect("parts"),
                    slice::encode(&model_snapshot()).expect("model"),
                    now,
                ],
            )
            .expect("candidate");
        transaction
            .execute(
                "UPDATE conversation_branches SET head_message_id = ?1, updated_at = ?2 WHERE conversation_id = ?3 AND id = ?4",
                params![
                    message_id.to_string(),
                    now,
                    fixture.conversation_id.to_string(),
                    fixture.branch_id.to_string(),
                ],
            )
            .expect("advance head");
        settle_attempt(
            &transaction,
            fixture.conversation_id,
            turn.id,
            attempt.id,
            "succeeded",
            now,
        );
        for status in ["preparing", "context_prepared", "running", "finalizing"] {
            transaction
                .execute(
                    "UPDATE conversation_turns SET status = ?3, revision = revision + 1, updated_at = ?4 WHERE conversation_id = ?1 AND id = ?2",
                    params![
                        fixture.conversation_id.to_string(),
                        turn.id.to_string(),
                        status,
                        now,
                    ],
                )
                .expect("turn status");
        }
        transaction
            .execute(
                "UPDATE conversation_turns SET status = 'succeeded', selected_candidate_id = ?3, revision = revision + 1, updated_at = ?4 WHERE conversation_id = ?1 AND id = ?2",
                params![
                    fixture.conversation_id.to_string(),
                    turn.id.to_string(),
                    candidate_id.to_string(),
                    now,
                ],
            )
            .expect("succeeded");
        transaction.commit().expect("commit");
        (message_id, candidate_id)
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
        let asset_id = stage_media_asset(&fixture.database);
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

        settle_terminal(&fixture, &first.value.turn, "failed", 22);
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
        settle_terminal(&fixture, &send.value.turn, "failed", 22);
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
        let author = fixture.characters[0];
        let (message_id, candidate_id) = settle_succeeded(&fixture, &send.value.turn, author, 21);
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
        settle_terminal(&fixture, &send.value.turn, "failed", 21);
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

        settle_terminal(&fixture, &result.value.turn, "failed", 23);
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
        settle_succeeded(
            &fixture,
            &succeeded_send.value.turn,
            fixture.characters[0],
            25,
        );
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
        settle_terminal(&fixture, &send.value.turn, "cancelled", 22);
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
        settle_terminal(&direct, &direct_send.value.turn, "failed", 21);
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

        settle_terminal(&fixture, &matched.value, "failed", 26);
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
    fn checkpoints_require_an_active_conversation() {
        let fixture = direct_fixture();
        let send = fixture
            .database
            .begin_send(
                &send_command(&fixture, "send-archived-checkpoint", "cd", text("hello")),
                TimestampMillis::new(20),
            )
            .expect("send");
        fixture
            .database
            .connection()
            .expect("connection")
            .execute(
                "UPDATE conversations SET lifecycle = 'archived' WHERE id = ?1",
                [fixture.conversation_id.to_string()],
            )
            .expect("archive");
        assert_eq!(
            fixture.database.append_event(
                send.value.turn.id,
                Revision::INITIAL,
                &token("checkpoint-archived", "cd"),
                GenerationCheckpointEnvelope {
                    turn_id: send.value.turn.id,
                    attempt_id: send.value.attempt.id,
                    job_id: None,
                    correlation_id: None,
                    sequence: 1,
                    event: GenerationCheckpointEvent::Completed,
                },
                TimestampMillis::new(21),
            ),
            Err(ConversationRepositoryError::Conflict)
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
        let (message_id, candidate_id) =
            settle_succeeded(&fixture, &first.value.turn, fixture.characters[0], 21);
        fixture.revision = conversation_revision(&fixture);
        let second = fixture
            .database
            .begin_send(
                &send_command(&fixture, "send-unrelated", "cd", text("again")),
                TimestampMillis::new(22),
            )
            .expect("second send");
        settle_terminal(&fixture, &second.value.turn, "failed", 23);
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
        settle_terminal(&fixture, &first.value.turn, "failed", 22);
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
    fn unimplemented_mutations_report_unsupported() {
        let fixture = direct_fixture();
        assert_eq!(
            fixture.database.archive(
                &lettuce_conversations::ArchiveConversation {
                    conversation_id: fixture.conversation_id,
                    expected_revision: fixture.revision,
                    operation: token("archive", "cd"),
                },
                TimestampMillis::new(20)
            ),
            Err(ConversationRepositoryError::Unsupported)
        );
        assert_eq!(
            fixture.database.select_branch(
                &lettuce_conversations::SelectBranch {
                    conversation_id: fixture.conversation_id,
                    branch_id: fixture.branch_id,
                    expected_revision: fixture.revision,
                    operation: token("select", "cd"),
                },
                TimestampMillis::new(20)
            ),
            Err(ConversationRepositoryError::Unsupported)
        );
    }
}
