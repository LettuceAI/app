//! Shared transaction kernel for conversation mutations.
//!
//! Every mutation adapter runs the same invariant order: an Immediate
//! transaction, an idempotency replay check, the caller's staged writes, the
//! operation record, and the outbox events allocated in staged order. This
//! module owns that order plus the enum writers, the conversation
//! compare-and-swap, and the trigger-message mapping the adapters share.

#![allow(dead_code)]

use lettuce_conversations::{
    ConversationLifecycle, ConversationOutboxEvent, ConversationOutboxRecord,
    ConversationRepositoryError, GenerationAttemptStatus, GenerationFailureCode,
    GenerationOperation, GenerationTurnStatus, MessageRole, MessageVisibility, OperationKind,
    OperationRecord, OperationResultRef, OperationToken, ReplayCodec,
};
use lettuce_types::{ConversationId, OperationRecordId, OutboxEventId, Revision, TimestampMillis};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use super::{Database, conversation_query, conversation_vertical_slice as slice};

const SQLITE_CONSTRAINT_TRIGGER: i32 = 1811;

/// Trigger `RAISE(ABORT, …)` strings declared by migration 0008. A firing
/// trigger means the caller asked for a state the aggregate forbids, so it is
/// a domain conflict rather than a storage fault.
const TRIGGER_CONFLICT_MESSAGES: [&str; 36] = [
    "invalid generation turn status transition",
    "terminal turn requires settled attempts",
    "terminal generation attempt cannot regress",
    "generation attempt identity is immutable",
    "generation attempt parent must be immediate predecessor",
    "generation turn request intent is immutable",
    "generation checkpoint sequence must be contiguous",
    "candidate does not match turn target",
    "candidate target must be assistant on turn branch",
    "candidate author must be a character participant",
    "candidate author is immutable",
    "final generation turn contract violation",
    "retry source is protected",
    "selected speaker must be a character",
    "selected speaker is immutable once resolved",
    "turn input does not match branch semantics",
    "branch head must belong to branch",
    "branch topology is immutable",
    "fork message must belong to parent branch",
    "message topology is immutable",
    "message parent violates branch topology",
    "tombstoned message cannot be restored",
    "message author role does not match participant",
    "send turn user message role is immutable",
    "revision ownership is immutable",
    "snapshot artifact is immutable",
    "snapshot reference is immutable",
    "initial message origin is immutable",
    "initial message origin cannot be added after create",
    "conversation initial timeline exceeds 512 origins",
    "conversation operation is immutable",
    "conversation outbox event is immutable",
    "create operation result must be its conversation",
    "create outbox event has invalid shape",
    "conversation_created event requires create operation",
    "create operation already has an outbox event",
];

/// Map a rusqlite failure onto the repository error a caller can act on. A
/// known trigger abort is a conflict; everything else stays a storage fault.
pub(crate) fn map_constraint(error: rusqlite::Error) -> ConversationRepositoryError {
    if let rusqlite::Error::SqliteFailure(code, Some(message)) = &error
        && code.code == rusqlite::ErrorCode::ConstraintViolation
        && code.extended_code == SQLITE_CONSTRAINT_TRIGGER
        && TRIGGER_CONFLICT_MESSAGES.contains(&message.as_str())
    {
        return ConversationRepositoryError::Conflict;
    }
    slice::db(error)
}

pub(crate) fn operation_kind_name(value: OperationKind) -> &'static str {
    match value {
        OperationKind::Create => "create",
        OperationKind::Send => "send",
        OperationKind::Continue => "continue",
        OperationKind::Regenerate => "regenerate",
        OperationKind::Retry => "retry",
        OperationKind::Checkpoint => "checkpoint",
        OperationKind::Cancel => "cancel",
        OperationKind::Finalize => "finalize",
        OperationKind::Fail => "fail",
        OperationKind::Interrupt => "interrupt",
        OperationKind::Recover => "recover",
        OperationKind::ChooseCandidate => "choose_candidate",
        OperationKind::Edit => "edit",
        OperationKind::Flags => "flags",
        OperationKind::Fork => "fork",
        OperationKind::SelectBranch => "select_branch",
        OperationKind::Tombstone => "tombstone",
        OperationKind::Archive => "archive",
        OperationKind::Restore => "restore",
        OperationKind::Rename => "rename",
        OperationKind::ParticipantPolicy => "participant_policy",
        OperationKind::Settings => "settings",
        OperationKind::AttachJob => "attach_job",
        OperationKind::PrepareGeneration => "prepare_generation",
        OperationKind::ResolveSpeaker => "resolve_speaker",
    }
}

pub(crate) fn generation_operation_name(value: GenerationOperation) -> &'static str {
    match value {
        GenerationOperation::Send => "send",
        GenerationOperation::Continue => "continue",
        GenerationOperation::Regenerate => "regenerate",
    }
}

pub(crate) fn generation_status_name(value: GenerationTurnStatus) -> &'static str {
    match value {
        GenerationTurnStatus::Created => "created",
        GenerationTurnStatus::Preparing => "preparing",
        GenerationTurnStatus::SelectingSpeaker => "selecting_speaker",
        GenerationTurnStatus::ContextPrepared => "context_prepared",
        GenerationTurnStatus::Running => "running",
        GenerationTurnStatus::CancellationRequested => "cancellation_requested",
        GenerationTurnStatus::Finalizing => "finalizing",
        GenerationTurnStatus::Succeeded => "succeeded",
        GenerationTurnStatus::Failed => "failed",
        GenerationTurnStatus::Cancelled => "cancelled",
        GenerationTurnStatus::Interrupted => "interrupted",
        GenerationTurnStatus::Recovering => "recovering",
    }
}

pub(crate) fn attempt_status_name(value: GenerationAttemptStatus) -> &'static str {
    match value {
        GenerationAttemptStatus::Created => "created",
        GenerationAttemptStatus::Preparing => "preparing",
        GenerationAttemptStatus::Running => "running",
        GenerationAttemptStatus::Succeeded => "succeeded",
        GenerationAttemptStatus::Failed => "failed",
        GenerationAttemptStatus::Cancelled => "cancelled",
        GenerationAttemptStatus::Interrupted => "interrupted",
    }
}

pub(crate) fn failure_name(value: GenerationFailureCode) -> &'static str {
    match value {
        GenerationFailureCode::InvalidConversation => "invalid_conversation",
        GenerationFailureCode::MissingModel => "missing_model",
        GenerationFailureCode::ContextUnavailable => "context_unavailable",
        GenerationFailureCode::SpeakerUnavailable => "speaker_unavailable",
        GenerationFailureCode::ProviderUnavailable => "provider_unavailable",
        GenerationFailureCode::ProviderRejected => "provider_rejected",
        GenerationFailureCode::EmptyOutput => "empty_output",
        GenerationFailureCode::Cancelled => "cancelled",
        GenerationFailureCode::TimedOut => "timed_out",
        GenerationFailureCode::RecoveryUnavailable => "recovery_unavailable",
        GenerationFailureCode::Internal => "internal",
    }
}

pub(crate) fn message_role_name(role: MessageRole) -> &'static str {
    match role {
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
        MessageRole::System => "system",
        MessageRole::Scene => "scene",
    }
}

pub(crate) fn message_visibility_name(value: MessageVisibility) -> &'static str {
    match value {
        MessageVisibility::Visible => "visible",
        MessageVisibility::Hidden => "hidden",
        MessageVisibility::Tombstoned => "tombstoned",
    }
}

pub(crate) fn replay_codec_name(value: ReplayCodec) -> &'static str {
    match value {
        ReplayCodec::Json => "json",
        ReplayCodec::Cbor => "cbor",
        ReplayCodec::Binary => "binary",
    }
}

/// Prefix a turn's stored idempotency key with its operation kind. Turns share
/// one `UNIQUE(conversation_id, idempotency_key)` index across send, continue,
/// regenerate and retry, so the same caller key on two kinds must not collide.
/// Generation attempts keep their own derived job keys and do not use this.
pub(crate) fn turn_idempotency_key(kind: OperationKind, token: &OperationToken) -> String {
    format!("{}.{}", operation_kind_name(kind), token.key.as_str())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CasConversation {
    pub(crate) conversation_id: ConversationId,
    pub(crate) lifecycle: ConversationLifecycle,
    pub(crate) revision: Revision,
}

/// Read the conversation row and compare its revision against the caller's
/// expectation before any write touches the aggregate.
pub(crate) fn cas_conversation(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    expected: Revision,
) -> Result<CasConversation, ConversationRepositoryError> {
    let row: Option<(String, i64)> = transaction
        .query_row(
            "SELECT lifecycle, revision FROM conversations WHERE id = ?1",
            [conversation_id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(slice::db)?;
    let Some((lifecycle, revision)) = row else {
        return Err(ConversationRepositoryError::NotFound);
    };
    let actual = slice::rev(revision)?;
    if actual != expected {
        return Err(ConversationRepositoryError::StaleRevision { expected, actual });
    }
    Ok(CasConversation {
        conversation_id,
        lifecycle: slice::lifecycle_from_name(&lifecycle)?,
        revision: actual,
    })
}

/// Every mutation except restore requires an Active conversation, per the
/// `ConversationRepository` contract.
pub(crate) fn require_active(
    conversation: &CasConversation,
) -> Result<(), ConversationRepositoryError> {
    match conversation.lifecycle {
        ConversationLifecycle::Active => Ok(()),
        ConversationLifecycle::Archived | ConversationLifecycle::Tombstoned => {
            Err(ConversationRepositoryError::Conflict)
        }
    }
}

/// Advance the conversation revision and stamp `updated_at` in one statement.
pub(crate) fn bump_conversation(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    now: TimestampMillis,
) -> Result<Revision, ConversationRepositoryError> {
    let revision: Option<i64> = transaction
        .query_row(
            "UPDATE conversations SET revision = revision + 1, updated_at = ?2 WHERE id = ?1 RETURNING revision",
            params![conversation_id.to_string(), now.get()],
            |row| row.get(0),
        )
        .optional()
        .map_err(map_constraint)?;
    let revision = revision.ok_or(ConversationRepositoryError::NotFound)?;
    slice::rev(revision)
}

fn result_projection(result: &OperationResultRef) -> (&'static str, String) {
    match result {
        OperationResultRef::Conversation(id) => ("conversation", id.to_string()),
        OperationResultRef::Turn(id) => ("turn", id.to_string()),
        OperationResultRef::Message(id) => ("message", id.to_string()),
        OperationResultRef::Candidate(id) => ("candidate", id.to_string()),
        OperationResultRef::Branch(id) => ("branch", id.to_string()),
    }
}

/// Write the idempotency record for one mutation. The result projection
/// columns are derived from the reference so a reader can filter without
/// decoding the document.
pub(crate) fn insert_operation(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    kind: OperationKind,
    token: &OperationToken,
    result: &OperationResultRef,
    now: TimestampMillis,
) -> Result<OperationRecord, ConversationRepositoryError> {
    let operation_id = OperationRecordId::new();
    let (result_kind, result_id) = result_projection(result);
    let result_json = slice::encode(result)?;
    transaction
        .execute(
            "INSERT INTO conversation_operations (id, conversation_id, kind, operation_key, request_digest, result_kind, result_id, result_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                operation_id.to_string(),
                conversation_id.to_string(),
                operation_kind_name(kind),
                token.key.as_str(),
                token.request_digest.as_str(),
                result_kind,
                result_id,
                result_json,
                now.get(),
            ],
        )
        .map_err(map_constraint)?;
    Ok(OperationRecord {
        id: operation_id,
        conversation_id,
        kind,
        operation: token.clone(),
        result: result.clone(),
        created_at: now,
    })
}

/// Allocate the next per-conversation outbox sequence. This is only safe under
/// an Immediate transaction, which serializes the read against concurrent
/// writers. A conversation that already holds its create row can never
/// allocate sequence 1 again, so a non-create operation seeing 1 means the
/// create row is missing and the conversation is not writable.
pub(crate) fn next_outbox_sequence(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    kind: OperationKind,
) -> Result<u64, ConversationRepositoryError> {
    let highest: i64 = transaction
        .query_row(
            "SELECT COALESCE(MAX(sequence), 0) FROM conversation_outbox WHERE conversation_id = ?1",
            [conversation_id.to_string()],
            |row| row.get(0),
        )
        .map_err(slice::db)?;
    let next = u64::try_from(highest)
        .map_err(|_| ConversationRepositoryError::Storage)?
        .checked_add(1)
        .ok_or(ConversationRepositoryError::Storage)?;
    if next == 1 && kind != OperationKind::Create {
        return Err(ConversationRepositoryError::Storage);
    }
    Ok(next)
}

/// Validate an outbox record against the domain rules and the transaction's
/// own rows, then append it.
pub(crate) fn insert_outbox(
    transaction: &Transaction<'_>,
    record: &ConversationOutboxRecord,
) -> Result<(), ConversationRepositoryError> {
    record
        .validate()
        .map_err(ConversationRepositoryError::Invalid)?;
    conversation_query::validate_outbox_event_timestamp(record)?;
    conversation_query::validate_outbox_event(transaction, record)?;
    let sequence =
        i64::try_from(record.sequence).map_err(|_| ConversationRepositoryError::Storage)?;
    let revision = slice::sql_revision(record.conversation_revision)?;
    transaction
        .execute(
            "INSERT INTO conversation_outbox (conversation_id, id, sequence, conversation_revision, operation_record_id, at, event_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                record.conversation_id.to_string(),
                record.id.to_string(),
                sequence,
                revision,
                record.operation_record_id.to_string(),
                record.at.get(),
                slice::encode(&record.event)?,
            ],
        )
        .map_err(map_constraint)?;
    Ok(())
}

/// Look up the idempotency record for one conversation, kind and key. The
/// stored projection columns must agree with the decoded reference, and the
/// reference must still be owned by this conversation.
pub(crate) fn read_operation_any(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    kind: OperationKind,
    token: &OperationToken,
) -> Result<Option<OperationRecord>, ConversationRepositoryError> {
    let row: Option<(String, String, String, String, String, String, i64)> = transaction
        .query_row(
            "SELECT id, operation_key, request_digest, result_kind, result_id, result_json, created_at FROM conversation_operations WHERE conversation_id = ?1 AND kind = ?2 AND operation_key = ?3",
            params![
                conversation_id.to_string(),
                operation_kind_name(kind),
                token.key.as_str(),
            ],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                ))
            },
        )
        .optional()
        .map_err(slice::db)?;
    let Some((id, stored_key, digest, result_kind, result_id, result_json, created_at)) = row
    else {
        return Ok(None);
    };
    let stored_key = stored_key
        .parse()
        .map_err(|_| ConversationRepositoryError::Storage)?;
    let digest = digest
        .parse()
        .map_err(|_| ConversationRepositoryError::Storage)?;
    let result: OperationResultRef = slice::decode(&result_json)?;
    let (expected_kind, expected_id) = result_projection(&result);
    if result_kind != expected_kind || result_id != expected_id {
        return Err(ConversationRepositoryError::Storage);
    }
    conversation_query::validate_operation_result_ownership(transaction, conversation_id, &result)?;
    Ok(Some(OperationRecord {
        id: slice::parse_id(id)?,
        conversation_id,
        kind,
        operation: OperationToken {
            key: stored_key,
            request_digest: digest,
        },
        result,
        created_at: TimestampMillis::new(created_at),
    }))
}

/// Read every outbox row an operation produced, in sequence order.
pub(crate) fn read_outbox_for_operation(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    operation_id: OperationRecordId,
) -> Result<Vec<ConversationOutboxRecord>, ConversationRepositoryError> {
    let records: Vec<ConversationOutboxRecord> = transaction
        .prepare("SELECT id, sequence, conversation_revision, operation_record_id, at, event_json FROM conversation_outbox WHERE conversation_id = ?1 AND operation_record_id = ?2 ORDER BY sequence, id")
        .map_err(slice::db)?
        .query_map(
            params![conversation_id.to_string(), operation_id.to_string()],
            |row| {
                let event: ConversationOutboxEvent =
                    slice::decode(&row.get::<_, String>(5)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?;
                Ok(ConversationOutboxRecord {
                    format_version: 1,
                    id: slice::parse_id(row.get(0)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    conversation_id,
                    sequence: u64::try_from(row.get::<_, i64>(1)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    conversation_revision: slice::rev(row.get(2)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    operation_record_id: slice::parse_id(row.get(3)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    at: TimestampMillis::new(row.get(4)?),
                    event,
                })
            },
        )
        .map_err(slice::db)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(slice::db)?;
    for record in &records {
        record
            .validate()
            .map_err(ConversationRepositoryError::Invalid)?;
    }
    Ok(records)
}

#[derive(Debug, Clone)]
pub(crate) struct MutationCtx {
    pub(crate) conversation_id: ConversationId,
    pub(crate) kind: OperationKind,
    pub(crate) token: OperationToken,
    pub(crate) now: TimestampMillis,
}

/// One outbox event a mutation body produced. The kernel owns the event id and
/// the sequence; the body owns the revision the event describes and its time.
#[derive(Debug, Clone)]
pub(crate) struct StagedEvent {
    pub(crate) conversation_revision: Revision,
    pub(crate) at: TimestampMillis,
    pub(crate) event: ConversationOutboxEvent,
}

/// What a mutation body hands back: the caller-visible value, the reference
/// the idempotency record points at, and the events to append in order.
#[derive(Debug, Clone)]
pub(crate) struct Staged<T> {
    pub(crate) value: T,
    pub(crate) result: OperationResultRef,
    pub(crate) events: Vec<StagedEvent>,
}

/// Run one mutation under the shared invariant order.
///
/// An Immediate transaction opens, the idempotency record is consulted, and a
/// hit with the same digest replays through `rehydrate` without running
/// `body`. A hit with a different digest is a conflict. Otherwise `body`
/// stages its writes, the operation record lands, and the staged events are
/// appended with contiguous sequences before the commit.
pub(crate) fn run_mutation<T, B, R>(
    database: &Database,
    conversation_id: ConversationId,
    kind: OperationKind,
    token: &OperationToken,
    now: TimestampMillis,
    body: B,
    rehydrate: R,
) -> Result<lettuce_conversations::MutationCommit<T>, ConversationRepositoryError>
where
    B: FnOnce(&Transaction<'_>, &MutationCtx) -> Result<Staged<T>, ConversationRepositoryError>,
    R: FnOnce(&Transaction<'_>, &OperationRecord) -> Result<T, ConversationRepositoryError>,
{
    let mut connection = database
        .connection()
        .map_err(|_| ConversationRepositoryError::Storage)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(slice::db)?;

    if let Some(operation) = read_operation_any(&transaction, conversation_id, kind, token)? {
        operation.replay_or_conflict(conversation_id, kind, token)?;
        let value = rehydrate(&transaction, &operation)?;
        let outbox = read_outbox_for_operation(&transaction, conversation_id, operation.id)?;
        transaction.commit().map_err(slice::db)?;
        return Ok(lettuce_conversations::MutationCommit {
            value,
            operation,
            outbox,
        });
    }

    let context = MutationCtx {
        conversation_id,
        kind,
        token: token.clone(),
        now,
    };
    let staged = body(&transaction, &context)?;
    let operation = insert_operation(
        &transaction,
        conversation_id,
        kind,
        token,
        &staged.result,
        now,
    )?;
    let base = next_outbox_sequence(&transaction, conversation_id, kind)?;
    let mut outbox = Vec::with_capacity(staged.events.len());
    for (offset, staged_event) in staged.events.into_iter().enumerate() {
        let offset = u64::try_from(offset).map_err(|_| ConversationRepositoryError::Storage)?;
        let sequence = base
            .checked_add(offset)
            .ok_or(ConversationRepositoryError::Storage)?;
        let record = ConversationOutboxRecord {
            format_version: 1,
            id: OutboxEventId::new(),
            conversation_id,
            conversation_revision: staged_event.conversation_revision,
            sequence,
            operation_record_id: operation.id,
            at: staged_event.at,
            event: staged_event.event,
        };
        insert_outbox(&transaction, &record)?;
        outbox.push(record);
    }
    transaction.commit().map_err(slice::db)?;
    Ok(lettuce_conversations::MutationCommit {
        value: staged.value,
        operation,
        outbox,
    })
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;
    use lettuce_conversations::{
        ArtifactCodec, ArtifactRetention, CharacterLaunchSnapshot, ConversationCreator,
        ConversationKind, ConversationParticipantDraft, DirectConversationDetails, ParticipantRole,
        ParticipantSource, PreparedConversationLaunch, ProtectedArtifactBytes,
        ProtectedSnapshotRef, SnapshotArtifactDraft, SnapshotSelection, SnapshotSource,
    };
    use lettuce_types::{
        CharacterId, ContentHash, ConversationParticipantId, MessageId, SnapshotArtifactId,
    };

    fn token(key: &str, digest: &str) -> OperationToken {
        OperationToken {
            key: lettuce_jobs::IdempotencyKey::new(key).expect("key"),
            request_digest: ContentHash::parse(digest.repeat(32)).expect("digest"),
        }
    }

    fn launch(conversation_id: ConversationId) -> PreparedConversationLaunch {
        let character_id = CharacterId::new();
        let bytes = ProtectedArtifactBytes::new(b"kernel launch".to_vec()).expect("bytes");
        let reference = ProtectedSnapshotRef {
            source: SnapshotSource::Character(character_id),
            source_revision: Revision::INITIAL,
            artifact_id: SnapshotArtifactId::new(),
            digest: bytes.digest(),
            schema_version: 1,
            byte_size: bytes.len() as u64,
        };
        let plan = lettuce_conversations::CreateConversationPlan {
            conversation_id,
            title: "Kernel".into(),
            kind: ConversationKind::Direct(DirectConversationDetails {
                format_version: 1,
                character: CharacterLaunchSnapshot {
                    snapshot_ref: reference.clone(),
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
                ConversationParticipantDraft {
                    id: ConversationParticipantId::new(),
                    role: ParticipantRole::User,
                    ordinal: 0,
                    source: ParticipantSource::User,
                    enabled: true,
                    muted: false,
                    display_name: "User".into(),
                    authored_description: None,
                    model_selection: SnapshotSelection::Disabled,
                },
                ConversationParticipantDraft {
                    id: ConversationParticipantId::new(),
                    role: ParticipantRole::Character,
                    ordinal: 1,
                    source: ParticipantSource::Character(character_id),
                    enabled: true,
                    muted: false,
                    display_name: "Ada".into(),
                    authored_description: None,
                    model_selection: SnapshotSelection::Disabled,
                },
            ],
            initial_timeline: lettuce_conversations::InitialTimelineDraft {
                format_version: 1,
                entries: Vec::new(),
            },
            operation: token("kernel-create", "ab"),
        };
        PreparedConversationLaunch::new(
            plan,
            vec![SnapshotArtifactDraft {
                source: reference.source,
                source_revision: reference.source_revision,
                artifact_id: reference.artifact_id,
                digest: reference.digest,
                schema_version: reference.schema_version,
                byte_size: reference.byte_size,
                codec: ArtifactCodec::Json,
                retention: ArtifactRetention::Conversation,
                bytes,
            }],
        )
        .expect("prepared launch")
    }

    fn created() -> (
        Database,
        ConversationId,
        lettuce_types::ConversationBranchId,
    ) {
        let database = Database::open_in_memory().expect("database");
        let conversation_id = ConversationId::new();
        let commit = ConversationCreator::create(
            &database,
            launch(conversation_id),
            TimestampMillis::new(10),
        )
        .expect("create");
        let branch_id = commit.value.conversation.active_branch_id;
        (database, conversation_id, branch_id)
    }

    const OPERATION_KINDS: [OperationKind; 24] = [
        OperationKind::Create,
        OperationKind::Send,
        OperationKind::Continue,
        OperationKind::Regenerate,
        OperationKind::Retry,
        OperationKind::Checkpoint,
        OperationKind::Cancel,
        OperationKind::Finalize,
        OperationKind::Fail,
        OperationKind::Interrupt,
        OperationKind::Recover,
        OperationKind::ChooseCandidate,
        OperationKind::Edit,
        OperationKind::Flags,
        OperationKind::Fork,
        OperationKind::SelectBranch,
        OperationKind::Tombstone,
        OperationKind::Archive,
        OperationKind::Restore,
        OperationKind::Rename,
        OperationKind::ParticipantPolicy,
        OperationKind::Settings,
        OperationKind::AttachJob,
        OperationKind::ResolveSpeaker,
    ];

    #[test]
    fn enum_writers_round_trip_through_their_readers() {
        for value in OPERATION_KINDS {
            assert_eq!(
                conversation_query::operation_kind(operation_kind_name(value)),
                Ok(value)
            );
        }
        for value in [
            GenerationOperation::Send,
            GenerationOperation::Continue,
            GenerationOperation::Regenerate,
        ] {
            assert_eq!(
                conversation_query::generation_operation(generation_operation_name(value)),
                Ok(value)
            );
        }
        for value in [
            GenerationTurnStatus::Created,
            GenerationTurnStatus::Preparing,
            GenerationTurnStatus::SelectingSpeaker,
            GenerationTurnStatus::ContextPrepared,
            GenerationTurnStatus::Running,
            GenerationTurnStatus::CancellationRequested,
            GenerationTurnStatus::Finalizing,
            GenerationTurnStatus::Succeeded,
            GenerationTurnStatus::Failed,
            GenerationTurnStatus::Cancelled,
            GenerationTurnStatus::Interrupted,
            GenerationTurnStatus::Recovering,
        ] {
            assert_eq!(
                conversation_query::generation_status(generation_status_name(value)),
                Ok(value)
            );
        }
        for value in [
            GenerationAttemptStatus::Created,
            GenerationAttemptStatus::Preparing,
            GenerationAttemptStatus::Running,
            GenerationAttemptStatus::Succeeded,
            GenerationAttemptStatus::Failed,
            GenerationAttemptStatus::Cancelled,
            GenerationAttemptStatus::Interrupted,
        ] {
            assert_eq!(
                conversation_query::attempt_status(attempt_status_name(value)),
                Ok(value)
            );
        }
        for value in [
            GenerationFailureCode::InvalidConversation,
            GenerationFailureCode::MissingModel,
            GenerationFailureCode::ContextUnavailable,
            GenerationFailureCode::SpeakerUnavailable,
            GenerationFailureCode::ProviderUnavailable,
            GenerationFailureCode::ProviderRejected,
            GenerationFailureCode::EmptyOutput,
            GenerationFailureCode::Cancelled,
            GenerationFailureCode::TimedOut,
            GenerationFailureCode::RecoveryUnavailable,
            GenerationFailureCode::Internal,
        ] {
            assert_eq!(
                conversation_query::failure(Some(failure_name(value).to_owned())),
                Ok(Some(value))
            );
        }
        for value in [
            MessageRole::User,
            MessageRole::Assistant,
            MessageRole::System,
            MessageRole::Scene,
        ] {
            assert_eq!(
                conversation_query::message_role(message_role_name(value)),
                Ok(value)
            );
        }
        for value in [
            MessageVisibility::Visible,
            MessageVisibility::Hidden,
            MessageVisibility::Tombstoned,
        ] {
            assert_eq!(
                conversation_query::message_visibility(message_visibility_name(value)),
                Ok(value)
            );
        }
        for value in [ReplayCodec::Json, ReplayCodec::Cbor, ReplayCodec::Binary] {
            assert_eq!(
                conversation_query::replay_codec(replay_codec_name(value)),
                Ok(value)
            );
        }
        assert_eq!(conversation_query::failure(None), Ok(None));
    }

    #[test]
    fn turn_keys_are_namespaced_by_kind() {
        let token = token("abc", "cd");
        assert_eq!(
            turn_idempotency_key(OperationKind::Send, &token),
            "send.abc"
        );
        assert_eq!(
            turn_idempotency_key(OperationKind::Regenerate, &token),
            "regenerate.abc"
        );
        assert_ne!(
            turn_idempotency_key(OperationKind::Continue, &token),
            turn_idempotency_key(OperationKind::Retry, &token)
        );
    }

    #[test]
    fn cas_reports_missing_stale_and_lifecycle() {
        let (database, conversation_id, _) = created();
        let mut connection = database.connection().expect("connection");
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("transaction");
        assert_eq!(
            cas_conversation(&transaction, ConversationId::new(), Revision::INITIAL),
            Err(ConversationRepositoryError::NotFound)
        );
        assert_eq!(
            cas_conversation(&transaction, conversation_id, Revision::new(7)),
            Err(ConversationRepositoryError::StaleRevision {
                expected: Revision::new(7),
                actual: Revision::INITIAL,
            })
        );
        let current =
            cas_conversation(&transaction, conversation_id, Revision::INITIAL).expect("cas");
        assert_eq!(current.lifecycle, ConversationLifecycle::Active);
        assert_eq!(require_active(&current), Ok(()));

        let bumped = bump_conversation(&transaction, conversation_id, TimestampMillis::new(40))
            .expect("bump");
        assert_eq!(bumped, Revision::new(2));
        let after = cas_conversation(&transaction, conversation_id, Revision::new(2)).expect("cas");
        assert_eq!(after.revision, Revision::new(2));
        let updated_at: i64 = transaction
            .query_row(
                "SELECT updated_at FROM conversations WHERE id = ?1",
                [conversation_id.to_string()],
                |row| row.get(0),
            )
            .expect("updated_at");
        assert_eq!(updated_at, 40);

        transaction
            .execute(
                "UPDATE conversations SET lifecycle = 'archived' WHERE id = ?1",
                [conversation_id.to_string()],
            )
            .expect("archive");
        let archived =
            cas_conversation(&transaction, conversation_id, Revision::new(2)).expect("cas");
        assert_eq!(archived.lifecycle, ConversationLifecycle::Archived);
        assert_eq!(
            require_active(&archived),
            Err(ConversationRepositoryError::Conflict)
        );
        assert_eq!(
            bump_conversation(
                &transaction,
                ConversationId::new(),
                TimestampMillis::new(41)
            ),
            Err(ConversationRepositoryError::NotFound)
        );
    }

    #[test]
    fn constraint_mapping_separates_trigger_conflicts_from_storage_faults() {
        let (database, conversation_id, _) = created();
        let mut connection = database.connection().expect("connection");
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("transaction");

        let immutable_operation = transaction
            .execute(
                "UPDATE conversation_operations SET request_digest = ?1 WHERE conversation_id = ?2",
                params!["cd".repeat(32), conversation_id.to_string()],
            )
            .expect_err("operation immutable");
        assert_eq!(
            map_constraint(immutable_operation),
            ConversationRepositoryError::Conflict
        );

        let immutable_outbox = transaction
            .execute(
                "DELETE FROM conversation_outbox WHERE conversation_id = ?1",
                [conversation_id.to_string()],
            )
            .expect_err("outbox immutable");
        assert_eq!(
            map_constraint(immutable_outbox),
            ConversationRepositoryError::Conflict
        );

        let late_origin = transaction
            .execute(
                "INSERT INTO conversation_initial_message_origins (conversation_id, message_id, snapshot_artifact_id, source_kind, starter_message_id) VALUES (?1, ?2, ?3, 'scene', NULL)",
                params![
                    conversation_id.to_string(),
                    MessageId::new().to_string(),
                    SnapshotArtifactId::new().to_string(),
                ],
            )
            .expect_err("origin after create");
        assert_eq!(
            map_constraint(late_origin),
            ConversationRepositoryError::Conflict
        );

        let duplicate = transaction
            .execute(
                "INSERT INTO conversations (id, kind, lifecycle, title, active_branch_id, kind_json, revision, created_at, updated_at) SELECT id, kind, lifecycle, title, active_branch_id, kind_json, revision, created_at, updated_at FROM conversations WHERE id = ?1",
                [conversation_id.to_string()],
            )
            .expect_err("duplicate conversation");
        assert_eq!(
            map_constraint(duplicate),
            ConversationRepositoryError::Storage
        );

        transaction
            .execute_batch(
                "CREATE TRIGGER kernel_unlisted_guard BEFORE INSERT ON conversation_snapshot_refs BEGIN SELECT RAISE(ABORT, 'unlisted kernel guard'); END;",
            )
            .expect("guard trigger");
        let unlisted = transaction
            .execute(
                "INSERT INTO conversation_snapshot_refs (conversation_id, artifact_id) VALUES (?1, ?2)",
                params![
                    conversation_id.to_string(),
                    SnapshotArtifactId::new().to_string(),
                ],
            )
            .expect_err("unlisted trigger");
        assert_eq!(
            map_constraint(unlisted),
            ConversationRepositoryError::Storage
        );
    }

    #[test]
    fn sequence_allocator_refuses_a_non_create_first_event() {
        let database = Database::open_in_memory().expect("database");
        let mut connection = database.connection().expect("connection");
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("transaction");
        let empty = ConversationId::new();
        assert_eq!(
            next_outbox_sequence(&transaction, empty, OperationKind::Create),
            Ok(1)
        );
        assert_eq!(
            next_outbox_sequence(&transaction, empty, OperationKind::Settings),
            Err(ConversationRepositoryError::Storage)
        );
    }

    #[test]
    fn run_mutation_appends_after_creation_and_replays_without_rerunning() {
        let (database, conversation_id, branch_id) = created();
        let mutation = token("kernel-select", "cd");
        let ran = Cell::new(0_u32);

        let first = run_mutation(
            &database,
            conversation_id,
            OperationKind::SelectBranch,
            &mutation,
            TimestampMillis::new(50),
            |transaction, context| {
                ran.set(ran.get() + 1);
                let current =
                    cas_conversation(transaction, context.conversation_id, Revision::INITIAL)?;
                require_active(&current)?;
                let revision =
                    bump_conversation(transaction, context.conversation_id, context.now)?;
                Ok(Staged {
                    value: revision,
                    result: OperationResultRef::Conversation(context.conversation_id),
                    events: vec![
                        StagedEvent {
                            conversation_revision: revision,
                            at: context.now,
                            event: ConversationOutboxEvent::BranchSelected {
                                conversation_id: context.conversation_id,
                                branch_id,
                                at: context.now,
                            },
                        },
                        StagedEvent {
                            conversation_revision: revision,
                            at: context.now,
                            event: ConversationOutboxEvent::ConversationLifecycleChanged {
                                conversation_id: context.conversation_id,
                                lifecycle: ConversationLifecycle::Active,
                                at: context.now,
                            },
                        },
                    ],
                })
            },
            |_, _| Err(ConversationRepositoryError::Storage),
        )
        .expect("mutation");

        assert_eq!(ran.get(), 1);
        assert_eq!(first.value, Revision::new(2));
        assert_eq!(first.operation.kind, OperationKind::SelectBranch);
        assert_eq!(first.operation.created_at, TimestampMillis::new(50));
        assert_eq!(
            first
                .outbox
                .iter()
                .map(|record| record.sequence)
                .collect::<Vec<_>>(),
            vec![2, 3]
        );
        assert!(
            first
                .outbox
                .iter()
                .all(|record| record.operation_record_id == first.operation.id
                    && record.conversation_revision == Revision::new(2))
        );

        let stored: i64 = database
            .connection()
            .expect("connection")
            .query_row(
                "SELECT count(*) FROM conversation_operations WHERE conversation_id = ?1 AND kind = 'select_branch' AND operation_key = 'kernel-select'",
                [conversation_id.to_string()],
                |row| row.get(0),
            )
            .expect("operation row");
        assert_eq!(stored, 1);

        let replay = run_mutation(
            &database,
            conversation_id,
            OperationKind::SelectBranch,
            &mutation,
            TimestampMillis::new(90),
            |_, _| {
                ran.set(ran.get() + 1);
                Err(ConversationRepositoryError::Storage)
            },
            |_, operation| {
                assert_eq!(
                    operation.result,
                    OperationResultRef::Conversation(conversation_id)
                );
                Ok(Revision::new(2))
            },
        )
        .expect("replay");
        assert_eq!(ran.get(), 1);
        assert_eq!(replay.operation, first.operation);
        assert_eq!(replay.outbox, first.outbox);

        let conflicting = token("kernel-select", "ef");
        assert_eq!(
            run_mutation(
                &database,
                conversation_id,
                OperationKind::SelectBranch,
                &conflicting,
                TimestampMillis::new(91),
                |_, _| -> Result<Staged<Revision>, ConversationRepositoryError> {
                    ran.set(ran.get() + 1);
                    Err(ConversationRepositoryError::Storage)
                },
                |_, _| Ok(Revision::new(2)),
            ),
            Err(ConversationRepositoryError::Conflict)
        );
        assert_eq!(ran.get(), 1);

        let total: i64 = database
            .connection()
            .expect("connection")
            .query_row(
                "SELECT count(*) FROM conversation_outbox WHERE conversation_id = ?1",
                [conversation_id.to_string()],
                |row| row.get(0),
            )
            .expect("outbox count");
        assert_eq!(total, 3);
    }
}
