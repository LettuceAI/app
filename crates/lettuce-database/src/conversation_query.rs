//! Snapshot-consistent read adapter for the conversation aggregate.
//!
//! This module is intentionally read-only. It owns SQL projections, cursor
//! decoding, and the conversion from normalized rows to validated domain
//! values; callers never receive a rusqlite connection or storage DTO.

use std::{collections::HashSet, str::FromStr};

use lettuce_conversations::{
    ConversationAggregate, ConversationBranch, ConversationKind, ConversationKindTag,
    ConversationLifecycle, ConversationOutboxEvent, ConversationOutboxRecord, ConversationQuery,
    ConversationReader, ConversationRepositoryError, ConversationSummary, GenerationAttempt,
    GenerationAttemptStatus, GenerationFailureCode, GenerationInput, GenerationOperation,
    GenerationTarget, GenerationTurn, GenerationTurnStatus, InitialMessageOrigin, KeysetPage,
    LorebookAttribution, MemoryAttribution, Message, MessageCandidate, MessagePart,
    MessageRenderSource, MessageRevision, MessageRole, MessageVisibility, OperationKind,
    OperationRecord, OperationResultRef, PromptAttribution, ProtectedSnapshotRef,
    ReplayArtifactRef, ReplayCodec, ReplayRetention, SelectedSpeakerDecision,
    SpeakerDecisionMethod, SpeakerDecisionReference, SpeakerFallback, TimelineItem, TimelinePage,
};
use lettuce_media::AssetRetainer;
use lettuce_types::{
    ConversationBranchId, ConversationId, ConversationParticipantId, GenerationAttemptId,
    GenerationTurnId, MessageCandidateId, MessageId, MessageRevisionId, Page, PageRequest,
    TimestampMillis,
};
use rusqlite::{OptionalExtension, Row, Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize};

use super::{Database, conversation_artifact_adapter, conversation_vertical_slice as slice};

const CURSOR_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    format_version: u32,
    scope: String,
    text: String,
    number: i64,
}

fn cursor_error() -> ConversationRepositoryError {
    ConversationRepositoryError::Invalid(lettuce_conversations::ValidationError::InvalidValue {
        field: "page.cursor",
    })
}

// A cursor is deliberately encoded as hex rather than exposing a JSON object
// to transport callers. It is still deterministic and has no secret state.
fn encode_cursor(
    scope: &str,
    text: &str,
    number: i64,
) -> Result<String, ConversationRepositoryError> {
    let bytes = serde_json::to_vec(&Cursor {
        format_version: CURSOR_VERSION,
        scope: scope.to_owned(),
        text: text.to_owned(),
        number,
    })
    .map_err(|_| ConversationRepositoryError::Storage)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn decode_cursor(
    value: Option<&String>,
    scope: &str,
) -> Result<Option<Cursor>, ConversationRepositoryError> {
    let Some(value) = value else { return Ok(None) };
    if value.is_empty() || value.len() > 4096 || value.len() % 2 != 0 {
        return Err(cursor_error());
    }
    let mut bytes = Vec::with_capacity(value.len() / 2);
    for pair in value.as_bytes().chunks_exact(2) {
        let hex = std::str::from_utf8(pair).map_err(|_| cursor_error())?;
        bytes.push(u8::from_str_radix(hex, 16).map_err(|_| cursor_error())?);
    }
    let cursor: Cursor = serde_json::from_slice(&bytes).map_err(|_| cursor_error())?;
    if cursor.format_version != CURSOR_VERSION || cursor.scope != scope {
        return Err(cursor_error());
    }
    Ok(Some(cursor))
}

fn validate_cursor_id<T: FromStr>(cursor: &Cursor) -> Result<T, ConversationRepositoryError> {
    cursor.text.parse().map_err(|_| cursor_error())
}

fn validate_cursor_number(
    _cursor: &Cursor,
    valid: bool,
) -> Result<(), ConversationRepositoryError> {
    if valid { Ok(()) } else { Err(cursor_error()) }
}

fn parse<T: FromStr>(value: String) -> Result<T, ConversationRepositoryError> {
    value
        .parse()
        .map_err(|_| ConversationRepositoryError::Storage)
}

fn parse_opt<T: FromStr>(value: Option<String>) -> Result<Option<T>, ConversationRepositoryError> {
    value.map(parse).transpose()
}

fn timestamp(value: i64) -> TimestampMillis {
    TimestampMillis::new(value)
}

fn open_read<'a>(
    database: &'a Database,
) -> Result<std::sync::MutexGuard<'a, rusqlite::Connection>, ConversationRepositoryError> {
    database
        .connection()
        .map_err(|_| ConversationRepositoryError::Storage)
}

fn operation_kind(value: &str) -> Result<OperationKind, ConversationRepositoryError> {
    Ok(match value {
        "create" => OperationKind::Create,
        "send" => OperationKind::Send,
        "continue" => OperationKind::Continue,
        "regenerate" => OperationKind::Regenerate,
        "retry" => OperationKind::Retry,
        "checkpoint" => OperationKind::Checkpoint,
        "cancel" => OperationKind::Cancel,
        "finalize" => OperationKind::Finalize,
        "fail" => OperationKind::Fail,
        "recover" => OperationKind::Recover,
        "choose_candidate" => OperationKind::ChooseCandidate,
        "edit" => OperationKind::Edit,
        "fork" => OperationKind::Fork,
        "select_branch" => OperationKind::SelectBranch,
        "tombstone" => OperationKind::Tombstone,
        "archive" => OperationKind::Archive,
        "restore" => OperationKind::Restore,
        "participant_policy" => OperationKind::ParticipantPolicy,
        "settings" => OperationKind::Settings,
        "attach_job" => OperationKind::AttachJob,
        _ => return Err(ConversationRepositoryError::Storage),
    })
}

fn generation_operation(value: &str) -> Result<GenerationOperation, ConversationRepositoryError> {
    match value {
        "send" => Ok(GenerationOperation::Send),
        "continue" => Ok(GenerationOperation::Continue),
        "regenerate" => Ok(GenerationOperation::Regenerate),
        _ => Err(ConversationRepositoryError::Storage),
    }
}

fn generation_status(value: &str) -> Result<GenerationTurnStatus, ConversationRepositoryError> {
    use GenerationTurnStatus::*;
    Ok(match value {
        "created" => Created,
        "preparing" => Preparing,
        "selecting_speaker" => SelectingSpeaker,
        "context_prepared" => ContextPrepared,
        "running" => Running,
        "cancellation_requested" => CancellationRequested,
        "finalizing" => Finalizing,
        "succeeded" => Succeeded,
        "failed" => Failed,
        "cancelled" => Cancelled,
        "interrupted" => Interrupted,
        "recovering" => Recovering,
        _ => return Err(ConversationRepositoryError::Storage),
    })
}

fn attempt_status(value: &str) -> Result<GenerationAttemptStatus, ConversationRepositoryError> {
    use GenerationAttemptStatus::*;
    Ok(match value {
        "created" => Created,
        "preparing" => Preparing,
        "running" => Running,
        "succeeded" => Succeeded,
        "failed" => Failed,
        "cancelled" => Cancelled,
        "interrupted" => Interrupted,
        _ => return Err(ConversationRepositoryError::Storage),
    })
}

fn failure(
    value: Option<String>,
) -> Result<Option<GenerationFailureCode>, ConversationRepositoryError> {
    value
        .map(|value| match value.as_str() {
            "invalid_conversation" => Ok(GenerationFailureCode::InvalidConversation),
            "missing_model" => Ok(GenerationFailureCode::MissingModel),
            "context_unavailable" => Ok(GenerationFailureCode::ContextUnavailable),
            "speaker_unavailable" => Ok(GenerationFailureCode::SpeakerUnavailable),
            "provider_unavailable" => Ok(GenerationFailureCode::ProviderUnavailable),
            "provider_rejected" => Ok(GenerationFailureCode::ProviderRejected),
            "empty_output" => Ok(GenerationFailureCode::EmptyOutput),
            "cancelled" => Ok(GenerationFailureCode::Cancelled),
            "timed_out" => Ok(GenerationFailureCode::TimedOut),
            "recovery_unavailable" => Ok(GenerationFailureCode::RecoveryUnavailable),
            "internal" => Ok(GenerationFailureCode::Internal),
            _ => Err(ConversationRepositoryError::Storage),
        })
        .transpose()
}

fn message_role(value: &str) -> Result<MessageRole, ConversationRepositoryError> {
    match value {
        "user" => Ok(MessageRole::User),
        "assistant" => Ok(MessageRole::Assistant),
        "system" => Ok(MessageRole::System),
        "scene" => Ok(MessageRole::Scene),
        _ => Err(ConversationRepositoryError::Storage),
    }
}

fn message_visibility(value: &str) -> Result<MessageVisibility, ConversationRepositoryError> {
    match value {
        "visible" => Ok(MessageVisibility::Visible),
        "hidden" => Ok(MessageVisibility::Hidden),
        "tombstoned" => Ok(MessageVisibility::Tombstoned),
        _ => Err(ConversationRepositoryError::Storage),
    }
}

fn replay_codec(value: &str) -> Result<ReplayCodec, ConversationRepositoryError> {
    match value {
        "json" => Ok(ReplayCodec::Json),
        "cbor" => Ok(ReplayCodec::Cbor),
        "binary" => Ok(ReplayCodec::Binary),
        _ => Err(ConversationRepositoryError::Storage),
    }
}

fn replay_ref(
    transaction: &Transaction<'_>,
    artifact_id: Option<String>,
    retention: Option<String>,
) -> Result<Option<ReplayArtifactRef>, ConversationRepositoryError> {
    if artifact_id.is_none() && retention.is_none() {
        return Ok(None);
    }
    let (Some(artifact_id), Some(retention)) = (artifact_id, retention) else {
        return Err(ConversationRepositoryError::Storage);
    };
    if retention != "conversation" {
        return Err(ConversationRepositoryError::Storage);
    }
    let (digest, schema, size, codec, stored_retention): (String, i64, i64, String, String) = transaction
        .query_row(
            "SELECT digest, schema_version, byte_size, codec, retention FROM conversation_replay_artifacts WHERE artifact_id = ?1",
            [artifact_id.clone()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .optional()
        .map_err(slice::db)?
        .ok_or(ConversationRepositoryError::Storage)?;
    if stored_retention != "conversation" {
        return Err(ConversationRepositoryError::Storage);
    }
    let reference = ReplayArtifactRef {
        artifact_id: parse(artifact_id)?,
        digest: digest
            .parse()
            .map_err(|_| ConversationRepositoryError::Storage)?,
        schema_version: u32::try_from(schema).map_err(|_| ConversationRepositoryError::Storage)?,
        byte_size: u64::try_from(size).map_err(|_| ConversationRepositoryError::Storage)?,
        retention: ReplayRetention::Conversation,
        codec: replay_codec(&codec)?,
    };
    conversation_artifact_adapter::verify_replay_in_transaction(transaction, &reference)
        .map_err(|_| ConversationRepositoryError::Storage)?;
    Ok(Some(reference))
}

fn verify_media_projection(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    owner_table: &str,
    owner_column: &str,
    owner_id: &str,
    parts: &[MessagePart],
) -> Result<(), ConversationRepositoryError> {
    let expected: Vec<(i64, String, String)> = parts
        .iter()
        .enumerate()
        .filter_map(|(ordinal, part)| match part {
            MessagePart::MediaAsset { asset_id, role } => Some((
                i64::try_from(ordinal).ok()?,
                asset_id.to_string(),
                format!("{role:?}").to_lowercase(),
            )),
            _ => None,
        })
        .collect();
    let sql = format!(
        "SELECT part_ordinal, asset_id, media_role, state FROM {owner_table} WHERE conversation_id = ?1 AND {owner_column} = ?2 ORDER BY part_ordinal"
    );
    let actual: Vec<(i64, String, String, String)> = transaction
        .prepare(&sql)
        .map_err(slice::db)?
        .query_map(params![conversation_id.to_string(), owner_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .map_err(slice::db)?
        .collect::<rusqlite::Result<_>>()
        .map_err(slice::db)?;
    if actual.len() != expected.len()
        || actual
            .iter()
            .zip(expected.iter())
            .any(|(actual, expected)| {
                actual.0 != expected.0
                    || actual.1 != expected.1
                    || actual.2 != expected.2
                    || !matches!(actual.3.as_str(), "active" | "historical")
            })
    {
        return Err(ConversationRepositoryError::Storage);
    }
    Ok(())
}

fn has_terminal_root(chain: &[(String, Option<String>, i64)]) -> bool {
    chain
        .last()
        .is_some_and(|(_, parent_message_id, _)| parent_message_id.is_none())
}

fn validate_outbox_event_exact(
    transaction: &Transaction<'_>,
    record: &ConversationOutboxRecord,
) -> Result<(), ConversationRepositoryError> {
    let conversation_id = record.conversation_id;
    match &record.event {
        ConversationOutboxEvent::ConversationCreated {
            root_branch_id,
            head_message_id,
            initial_message_count,
            ..
        } => {
            require_exists(
                transaction,
                "SELECT EXISTS(SELECT 1 FROM conversation_branches WHERE conversation_id = ?1 AND id = ?2 AND parent_branch_id IS NULL)",
                params![conversation_id.to_string(), root_branch_id.to_string()],
            )?;
            if *initial_message_count == 0 {
                let origins: i64 = transaction
                    .query_row(
                        "SELECT count(*) FROM conversation_initial_message_origins WHERE conversation_id = ?1",
                        [conversation_id.to_string()],
                        |row| row.get(0),
                    )
                    .map_err(slice::db)?;
                if origins != 0 {
                    return Err(ConversationRepositoryError::Storage);
                }
                return Ok(());
            }
            let head = head_message_id
                .as_ref()
                .ok_or(ConversationRepositoryError::Storage)?;
            let chain: Vec<(String, Option<String>, i64)> = transaction
                .prepare(
                    "WITH RECURSIVE chain(id, parent_message_id, depth) AS (
                         SELECT id, parent_message_id, 1 FROM conversation_messages
                         WHERE conversation_id = ?1 AND branch_id = ?2 AND id = ?3
                         UNION ALL
                         SELECT message.id, message.parent_message_id, chain.depth + 1
                         FROM conversation_messages AS message
                         JOIN chain ON chain.parent_message_id = message.id
                         WHERE message.conversation_id = ?1 AND message.branch_id = ?2 AND chain.depth < 513
                     )
                     SELECT id, parent_message_id, depth FROM chain ORDER BY depth",
                )
                .map_err(slice::db)?
                .query_map(
                    params![conversation_id.to_string(), root_branch_id.to_string(), head.to_string()],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .map_err(slice::db)?
                .collect::<rusqlite::Result<_>>()
                .map_err(slice::db)?;
            let total_origins: i64 = transaction
                .query_row(
                    "SELECT count(*) FROM conversation_initial_message_origins WHERE conversation_id = ?1",
                    [conversation_id.to_string()],
                    |row| row.get(0),
                )
                .map_err(slice::db)?;
            if chain.len() != usize::from(*initial_message_count)
                || total_origins != i64::from(*initial_message_count)
                || !has_terminal_root(&chain)
            {
                return Err(ConversationRepositoryError::Storage);
            }
            for (index, (message_id, parent_message_id, _depth)) in chain.iter().enumerate() {
                let message_id: MessageId = slice::parse_id(message_id.clone())?;
                let origin = hydrate_initial_origin(transaction, conversation_id, message_id)?
                    .ok_or(ConversationRepositoryError::Storage)?;
                validate_initial_origin_message(
                    transaction,
                    conversation_id,
                    message_id,
                    Some(&origin),
                )?;
                if matches!(origin, InitialMessageOrigin::SelectedScene { .. })
                    && (index + 1 != chain.len() || parent_message_id.is_some())
                {
                    return Err(ConversationRepositoryError::Storage);
                }
            }
        }
        ConversationOutboxEvent::MessageCommitted {
            branch_id,
            message_id,
            revision_id,
            candidate_id,
            ..
        } => {
            require_exists(
                transaction,
                "SELECT EXISTS(SELECT 1 FROM conversation_messages WHERE conversation_id = ?1 AND id = ?2 AND branch_id = ?3)",
                params![
                    conversation_id.to_string(),
                    message_id.to_string(),
                    branch_id.to_string()
                ],
            )?;
            if let Some(id) = revision_id {
                require_exists(
                    transaction,
                    "SELECT EXISTS(SELECT 1 FROM conversation_message_revisions WHERE conversation_id = ?1 AND id = ?2 AND message_id = ?3 AND branch_id = ?4)",
                    params![
                        conversation_id.to_string(),
                        id.to_string(),
                        message_id.to_string(),
                        branch_id.to_string()
                    ],
                )?;
            }
            if let Some(id) = candidate_id {
                require_exists(
                    transaction,
                    "SELECT EXISTS(SELECT 1 FROM conversation_message_candidates WHERE conversation_id = ?1 AND id = ?2 AND message_id = ?3 AND branch_id = ?4)",
                    params![
                        conversation_id.to_string(),
                        id.to_string(),
                        message_id.to_string(),
                        branch_id.to_string()
                    ],
                )?;
            }
        }
        ConversationOutboxEvent::MessageRevised {
            branch_id,
            message_id,
            revision_id,
            ..
        } => {
            require_exists(
                transaction,
                "SELECT EXISTS(SELECT 1 FROM conversation_message_revisions WHERE conversation_id = ?1 AND id = ?2 AND message_id = ?3 AND branch_id = ?4)",
                params![
                    conversation_id.to_string(),
                    revision_id.to_string(),
                    message_id.to_string(),
                    branch_id.to_string()
                ],
            )?;
        }
        ConversationOutboxEvent::MessageTombstoned {
            branch_id,
            message_id,
            affected_message_ids,
            affected_revision_ids,
            asset_reference_deltas,
            ..
        } => {
            require_exists(
                transaction,
                "SELECT EXISTS(SELECT 1 FROM conversation_messages WHERE conversation_id = ?1 AND id = ?2 AND branch_id = ?3)",
                params![
                    conversation_id.to_string(),
                    message_id.to_string(),
                    branch_id.to_string()
                ],
            )?;
            for id in affected_message_ids {
                require_exists(
                    transaction,
                    "SELECT EXISTS(SELECT 1 FROM conversation_messages WHERE conversation_id = ?1 AND id = ?2 AND branch_id = ?3)",
                    params![
                        conversation_id.to_string(),
                        id.to_string(),
                        branch_id.to_string()
                    ],
                )?;
            }
            for id in affected_revision_ids {
                require_exists(
                    transaction,
                    "SELECT EXISTS(SELECT 1 FROM conversation_message_revisions WHERE conversation_id = ?1 AND id = ?2 AND branch_id = ?3)",
                    params![
                        conversation_id.to_string(),
                        id.to_string(),
                        branch_id.to_string()
                    ],
                )?;
            }
            for delta in asset_reference_deltas {
                validate_asset_delta(transaction, conversation_id, delta)?;
            }
        }
        ConversationOutboxEvent::TurnFinalized {
            branch_id,
            turn_id,
            attempt_id,
            message_id,
            candidate_id,
            revision_id,
            ..
        } => {
            require_exists(
                transaction,
                "SELECT EXISTS(SELECT 1 FROM conversation_turns WHERE conversation_id = ?1 AND id = ?2 AND branch_id = ?3)",
                params![
                    conversation_id.to_string(),
                    turn_id.to_string(),
                    branch_id.to_string()
                ],
            )?;
            require_exists(
                transaction,
                "SELECT EXISTS(SELECT 1 FROM generation_attempts AS attempt JOIN conversation_turns AS turn ON turn.conversation_id = attempt.conversation_id AND turn.id = attempt.turn_id WHERE attempt.conversation_id = ?1 AND attempt.id = ?2 AND attempt.turn_id = ?3 AND turn.branch_id = ?4)",
                params![
                    conversation_id.to_string(),
                    attempt_id.to_string(),
                    turn_id.to_string(),
                    branch_id.to_string()
                ],
            )?;
            require_exists(
                transaction,
                "SELECT EXISTS(SELECT 1 FROM conversation_messages WHERE conversation_id = ?1 AND id = ?2 AND branch_id = ?3)",
                params![
                    conversation_id.to_string(),
                    message_id.to_string(),
                    branch_id.to_string()
                ],
            )?;
            require_exists(
                transaction,
                "SELECT EXISTS(SELECT 1 FROM conversation_message_candidates WHERE conversation_id = ?1 AND id = ?2 AND message_id = ?3 AND turn_id = ?4 AND attempt_id = ?5 AND branch_id = ?6)",
                params![
                    conversation_id.to_string(),
                    candidate_id.to_string(),
                    message_id.to_string(),
                    turn_id.to_string(),
                    attempt_id.to_string(),
                    branch_id.to_string()
                ],
            )?;
            if let Some(id) = revision_id {
                require_exists(
                    transaction,
                    "SELECT EXISTS(SELECT 1 FROM conversation_message_revisions WHERE conversation_id = ?1 AND id = ?2 AND message_id = ?3 AND branch_id = ?4)",
                    params![
                        conversation_id.to_string(),
                        id.to_string(),
                        message_id.to_string(),
                        branch_id.to_string()
                    ],
                )?;
            }
        }
        ConversationOutboxEvent::TurnFailed {
            branch_id,
            turn_id,
            attempt_id,
            ..
        }
        | ConversationOutboxEvent::TurnCancellationRequested {
            branch_id,
            turn_id,
            attempt_id,
            ..
        }
        | ConversationOutboxEvent::TurnCancelled {
            branch_id,
            turn_id,
            attempt_id,
            ..
        } => {
            require_exists(
                transaction,
                "SELECT EXISTS(SELECT 1 FROM conversation_turns WHERE conversation_id = ?1 AND id = ?2 AND branch_id = ?3)",
                params![
                    conversation_id.to_string(),
                    turn_id.to_string(),
                    branch_id.to_string()
                ],
            )?;
            require_exists(
                transaction,
                "SELECT EXISTS(SELECT 1 FROM generation_attempts AS attempt JOIN conversation_turns AS turn ON turn.conversation_id = attempt.conversation_id AND turn.id = attempt.turn_id WHERE attempt.conversation_id = ?1 AND attempt.id = ?2 AND attempt.turn_id = ?3 AND turn.branch_id = ?4)",
                params![
                    conversation_id.to_string(),
                    attempt_id.to_string(),
                    turn_id.to_string(),
                    branch_id.to_string()
                ],
            )?;
        }
        ConversationOutboxEvent::BranchForked { branch_id, .. } => {
            require_exists(
                transaction,
                "SELECT EXISTS(SELECT 1 FROM conversation_branches WHERE conversation_id = ?1 AND id = ?2)",
                params![conversation_id.to_string(), branch_id.to_string()],
            )?;
        }
        ConversationOutboxEvent::ConversationTombstoned { .. } => {}
        ConversationOutboxEvent::AssetReferencesChanged {
            message_revision_id,
            candidate_id,
            changes,
            ..
        } => {
            if let Some(id) = message_revision_id {
                require_exists(
                    transaction,
                    "SELECT EXISTS(SELECT 1 FROM conversation_message_revisions WHERE conversation_id = ?1 AND id = ?2)",
                    params![conversation_id.to_string(), id.to_string()],
                )?;
            }
            if let Some(id) = candidate_id {
                require_exists(
                    transaction,
                    "SELECT EXISTS(SELECT 1 FROM conversation_message_candidates WHERE conversation_id = ?1 AND id = ?2)",
                    params![conversation_id.to_string(), id.to_string()],
                )?;
            }
            if message_revision_id.is_none() == candidate_id.is_none() {
                return Err(ConversationRepositoryError::Storage);
            }
            for delta in changes {
                validate_asset_delta(transaction, conversation_id, delta)?;
            }
        }
    }
    Ok(())
}

fn hydrate_revision_row(
    transaction: &Transaction<'_>,
    row: &Row<'_>,
) -> Result<MessageRevision, ConversationRepositoryError> {
    let conversation_id: ConversationId = parse(row.get::<_, String>(0).map_err(slice::db)?)?;
    let id: MessageRevisionId = parse(row.get::<_, String>(1).map_err(slice::db)?)?;
    let message_id: MessageId = parse(row.get::<_, String>(2).map_err(slice::db)?)?;
    let branch_id: ConversationBranchId = parse(row.get::<_, String>(3).map_err(slice::db)?)?;
    let parts: Vec<MessagePart> = slice::decode(&row.get::<_, String>(5).map_err(slice::db)?)?;
    let replay = replay_ref(
        transaction,
        row.get(7).map_err(slice::db)?,
        row.get(8).map_err(slice::db)?,
    )?;
    verify_media_projection(
        transaction,
        conversation_id,
        "revision_media_refs",
        "message_revision_id",
        &id.to_string(),
        &parts,
    )?;
    let value = MessageRevision {
        id,
        message_id,
        sequence: slice::rev(row.get(4).map_err(slice::db)?)?,
        parts,
        authored_at: timestamp(row.get(6).map_err(slice::db)?),
        source_turn_id: parse_opt(row.get(9).map_err(slice::db)?)?,
        provider_replay: replay,
    };
    value
        .validate()
        .map_err(|_| ConversationRepositoryError::Storage)?;
    let owner_ok: bool = transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM conversation_messages WHERE conversation_id = ?1 AND id = ?2 AND branch_id = ?3)",
            params![conversation_id.to_string(), message_id.to_string(), branch_id.to_string()],
            |row| row.get(0),
        )
        .map_err(slice::db)?;
    if !owner_ok {
        return Err(ConversationRepositoryError::Storage);
    }
    Ok(value)
}

fn hydrate_candidate_row(
    transaction: &Transaction<'_>,
    row: &Row<'_>,
) -> Result<MessageCandidate, ConversationRepositoryError> {
    let conversation_id: ConversationId = parse(row.get::<_, String>(0).map_err(slice::db)?)?;
    let id: MessageCandidateId = parse(row.get::<_, String>(1).map_err(slice::db)?)?;
    let message_id: MessageId = parse(row.get::<_, String>(2).map_err(slice::db)?)?;
    let branch_id: ConversationBranchId = parse(row.get::<_, String>(3).map_err(slice::db)?)?;
    let turn_id: GenerationTurnId = parse(row.get::<_, String>(4).map_err(slice::db)?)?;
    let attempt_id: GenerationAttemptId = parse(row.get::<_, String>(5).map_err(slice::db)?)?;
    let parts: Vec<MessagePart> = slice::decode(&row.get::<_, String>(7).map_err(slice::db)?)?;
    let model = slice::decode(&row.get::<_, String>(8).map_err(slice::db)?)?;
    let replay = replay_ref(
        transaction,
        row.get(10).map_err(slice::db)?,
        row.get(11).map_err(slice::db)?,
    )?;
    verify_media_projection(
        transaction,
        conversation_id,
        "candidate_media_refs",
        "candidate_id",
        &id.to_string(),
        &parts,
    )?;
    let value = MessageCandidate {
        id,
        message_id,
        turn_id,
        attempt_id,
        ordinal: u16::try_from(row.get::<_, i64>(6).map_err(slice::db)?)
            .map_err(|_| ConversationRepositoryError::Storage)?,
        parts,
        model,
        created_at: timestamp(row.get(9).map_err(slice::db)?),
        provider_replay: replay,
    };
    value
        .validate()
        .map_err(|_| ConversationRepositoryError::Storage)?;
    let owner_ok: bool = transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM conversation_messages WHERE conversation_id = ?1 AND id = ?2 AND branch_id = ?3) AND EXISTS(SELECT 1 FROM conversation_turns WHERE conversation_id = ?1 AND id = ?4 AND branch_id = ?3)",
            params![conversation_id.to_string(), message_id.to_string(), branch_id.to_string(), turn_id.to_string()],
            |row| row.get(0),
        )
        .map_err(slice::db)?;
    if !owner_ok {
        return Err(ConversationRepositoryError::Storage);
    }
    Ok(value)
}

fn message_row(
    transaction: &Transaction<'_>,
    row: &Row<'_>,
) -> Result<(TimelineItem, i64), ConversationRepositoryError> {
    let conversation_id: ConversationId = parse(row.get::<_, String>(0).map_err(slice::db)?)?;
    let id: MessageId = parse(row.get::<_, String>(1).map_err(slice::db)?)?;
    let active_revision_id: Option<MessageRevisionId> = parse_opt(row.get(12).map_err(slice::db)?)?;
    let active_candidate_id: Option<MessageCandidateId> =
        parse_opt(row.get(13).map_err(slice::db)?)?;
    let active_render_source = match (active_revision_id, active_candidate_id) {
        (Some(id), None) => MessageRenderSource::Revision(id),
        (None, Some(id)) => MessageRenderSource::Candidate(id),
        _ => return Err(ConversationRepositoryError::Storage),
    };
    let message = Message {
        id,
        conversation_id,
        branch_id: parse(row.get::<_, String>(2).map_err(slice::db)?)?,
        parent_message_id: parse_opt(row.get(3).map_err(slice::db)?)?,
        author_participant_id: parse_opt(row.get(4).map_err(slice::db)?)?,
        role: message_role(&row.get::<_, String>(5).map_err(slice::db)?)?,
        logical_time: timestamp(row.get(6).map_err(slice::db)?),
        effective_time: timestamp(row.get(7).map_err(slice::db)?),
        visibility: message_visibility(&row.get::<_, String>(8).map_err(slice::db)?)?,
        pinned: row.get::<_, i64>(9).map_err(slice::db)? != 0,
        scene_edited: row.get::<_, i64>(10).map_err(slice::db)? != 0,
        active_render_source,
        revision: slice::rev(row.get(14).map_err(slice::db)?)?,
        created_at: timestamp(row.get(15).map_err(slice::db)?),
        updated_at: timestamp(row.get(16).map_err(slice::db)?),
    };
    message
        .validate()
        .map_err(|_| ConversationRepositoryError::Storage)?;
    let active_revision = if let Some(revision_id) = active_revision_id {
        let mut statement = transaction.prepare("SELECT conversation_id, id, message_id, branch_id, sequence, parts_json, authored_at, provider_replay_artifact_id, provider_replay_retention, source_turn_id FROM conversation_message_revisions WHERE conversation_id = ?1 AND id = ?2").map_err(slice::db)?;
        Some(
            statement
                .query_row(
                    params![conversation_id.to_string(), revision_id.to_string()],
                    |row| {
                        hydrate_revision_row(transaction, row)
                            .map_err(|_| rusqlite::Error::InvalidQuery)
                    },
                )
                .optional()
                .map_err(slice::db)?
                .ok_or(ConversationRepositoryError::Storage)?,
        )
    } else {
        return Err(ConversationRepositoryError::Storage);
    };
    let active_candidate = if let Some(candidate_id) = active_candidate_id {
        let mut statement = transaction.prepare("SELECT conversation_id, id, message_id, branch_id, turn_id, attempt_id, ordinal, parts_json, model_json, created_at, provider_replay_artifact_id, provider_replay_retention FROM conversation_message_candidates WHERE conversation_id = ?1 AND id = ?2").map_err(slice::db)?;
        Some(
            statement
                .query_row(
                    params![conversation_id.to_string(), candidate_id.to_string()],
                    |row| {
                        hydrate_candidate_row(transaction, row)
                            .map_err(|_| rusqlite::Error::InvalidQuery)
                    },
                )
                .optional()
                .map_err(slice::db)?
                .ok_or(ConversationRepositoryError::Storage)?,
        )
    } else {
        None
    };
    if active_revision
        .as_ref()
        .is_some_and(|value| value.message_id != message.id)
        || active_candidate
            .as_ref()
            .is_some_and(|value| value.message_id != message.id)
    {
        return Err(ConversationRepositoryError::Storage);
    }
    let initial_origin = hydrate_initial_origin(transaction, conversation_id, id)?;
    if message.role == MessageRole::Scene && initial_origin.is_none() {
        return Err(ConversationRepositoryError::Storage);
    }
    validate_initial_origin_message(transaction, conversation_id, id, initial_origin.as_ref())?;
    Ok((
        TimelineItem {
            message,
            active_revision,
            active_candidate,
            initial_origin,
        },
        row.get(11).map_err(slice::db)?,
    ))
}

fn validate_initial_origin_message(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    message_id: MessageId,
    origin: Option<&InitialMessageOrigin>,
) -> Result<(), ConversationRepositoryError> {
    let Some(origin) = origin else { return Ok(()) };
    let (role, author, revision_id): (String, Option<String>, String) = transaction
        .query_row(
            "SELECT role, author_participant_id, active_revision_id FROM conversation_messages WHERE conversation_id = ?1 AND id = ?2",
            params![conversation_id.to_string(), message_id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(slice::db)?;
    let parts_json: String = transaction
        .query_row(
            "SELECT parts_json FROM conversation_message_revisions WHERE conversation_id = ?1 AND id = ?2 AND message_id = ?3",
            params![conversation_id.to_string(), revision_id, message_id.to_string()],
            |row| row.get(0),
        )
        .map_err(slice::db)?;
    let parts: Vec<MessagePart> = slice::decode(&parts_json)?;
    for part in &parts {
        part.validate()
            .map_err(|_| ConversationRepositoryError::Storage)?;
    }
    match origin {
        InitialMessageOrigin::SelectedScene { .. } => {
            if role != "scene"
                || author.is_some()
                || !parts.iter().any(
                    |part| matches!(part, MessagePart::Text { text } if !text.trim().is_empty()),
                )
            {
                return Err(ConversationRepositoryError::Storage);
            }
        }
        InitialMessageOrigin::StarterMessage { .. } => {
            let Some(author) = author else {
                return Err(ConversationRepositoryError::Storage);
            };
            let expected_role = match role.as_str() {
                "user" => "user",
                "assistant" => "character",
                _ => return Err(ConversationRepositoryError::Storage),
            };
            let participant_role: String = transaction
                .query_row(
                    "SELECT role FROM conversation_participants WHERE conversation_id = ?1 AND id = ?2",
                    params![conversation_id.to_string(), author],
                    |row| row.get(0),
                )
                .map_err(slice::db)?;
            if participant_role != expected_role {
                return Err(ConversationRepositoryError::Storage);
            }
        }
    }
    Ok(())
}

fn hydrate_initial_origin(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    message_id: MessageId,
) -> Result<Option<InitialMessageOrigin>, ConversationRepositoryError> {
    let row = transaction
        .query_row(
            "SELECT origin.source_kind, origin.starter_message_id, artifact.artifact_id, artifact.source_kind, artifact.source_id, artifact.source_revision, artifact.digest, artifact.schema_version, artifact.byte_size FROM conversation_initial_message_origins AS origin JOIN conversation_snapshot_artifacts AS artifact ON artifact.artifact_id = origin.snapshot_artifact_id AND artifact.source_kind = origin.source_kind WHERE origin.conversation_id = ?1 AND origin.message_id = ?2",
            params![conversation_id.to_string(), message_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, i64>(8)?,
                ))
            },
        )
        .optional()
        .map_err(slice::db)?;
    let Some((
        kind,
        starter_message_id,
        artifact_id,
        artifact_kind,
        source_id,
        source_revision,
        digest,
        schema_version,
        byte_size,
    )) = row
    else {
        return Ok(None);
    };
    if kind != artifact_kind {
        return Err(ConversationRepositoryError::Storage);
    }
    let source_revision = slice::rev(source_revision)?;
    let source = match kind.as_str() {
        "scene" => lettuce_conversations::SnapshotSource::Scene(parse(source_id)?),
        "starter" => lettuce_conversations::SnapshotSource::Starter(parse(source_id)?),
        _ => return Err(ConversationRepositoryError::Storage),
    };
    let reference = ProtectedSnapshotRef {
        source,
        source_revision,
        artifact_id: parse(artifact_id)?,
        digest: digest
            .parse()
            .map_err(|_| ConversationRepositoryError::Storage)?,
        schema_version: u32::try_from(schema_version)
            .map_err(|_| ConversationRepositoryError::Storage)?,
        byte_size: u64::try_from(byte_size).map_err(|_| ConversationRepositoryError::Storage)?,
    };
    reference
        .validate()
        .map_err(|_| ConversationRepositoryError::Storage)?;
    let origin = match kind.as_str() {
        "scene" if starter_message_id.is_none() => InitialMessageOrigin::SelectedScene {
            snapshot_ref: reference,
        },
        "starter" if let Some(starter_message_id) = starter_message_id => {
            InitialMessageOrigin::StarterMessage {
                snapshot_ref: reference,
                starter_message_id: parse(starter_message_id)?,
            }
        }
        _ => return Err(ConversationRepositoryError::Storage),
    };

    // The normalized origin row is only an index into the immutable artifact
    // tables.  It must also agree with the launch snapshot stored on the
    // conversation; otherwise a corrupt row could make an unrelated scene or
    // starter appear to be part of the initial timeline.
    let kind_json: String = transaction
        .query_row(
            "SELECT kind_json FROM conversations WHERE id = ?1",
            [conversation_id.to_string()],
            |row| row.get(0),
        )
        .map_err(slice::db)?;
    let conversation_kind: ConversationKind = slice::decode(&kind_json)?;
    let selected_ref = match &conversation_kind {
        ConversationKind::Direct(details) => match &origin {
            InitialMessageOrigin::SelectedScene { .. } => match &details.scene {
                lettuce_conversations::SnapshotSelection::Inherited(value)
                | lettuce_conversations::SnapshotSelection::Explicit(value) => {
                    Some((&value.snapshot_ref, false))
                }
                lettuce_conversations::SnapshotSelection::Disabled => None,
            },
            InitialMessageOrigin::StarterMessage { .. } => match &details.starter {
                lettuce_conversations::SnapshotSelection::Inherited(value)
                | lettuce_conversations::SnapshotSelection::Explicit(value) => {
                    Some((&value.snapshot_ref, false))
                }
                lettuce_conversations::SnapshotSelection::Disabled => None,
            },
        },
        ConversationKind::Group(details) => match &origin {
            InitialMessageOrigin::SelectedScene { .. }
                if matches!(
                    details.group.chat_mode,
                    lettuce_conversations::GroupChatModeSnapshot::Roleplay
                ) =>
            {
                match &details.group.scene {
                    lettuce_conversations::SnapshotSelection::Inherited(value)
                    | lettuce_conversations::SnapshotSelection::Explicit(value) => {
                        Some((&value.snapshot_ref, false))
                    }
                    lettuce_conversations::SnapshotSelection::Disabled => None,
                }
            }
            _ => None,
        },
    };
    let Some((selected_ref, _)) = selected_ref else {
        return Err(ConversationRepositoryError::Storage);
    };
    let origin_ref = match &origin {
        InitialMessageOrigin::SelectedScene { snapshot_ref }
        | InitialMessageOrigin::StarterMessage { snapshot_ref, .. } => snapshot_ref,
    };
    if origin_ref != selected_ref {
        return Err(ConversationRepositoryError::Storage);
    }
    Ok(Some(origin))
}

fn branch_path(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    selected: ConversationBranchId,
) -> Result<Vec<ConversationBranch>, ConversationRepositoryError> {
    let mut path = Vec::new();
    let mut visited = HashSet::new();
    let mut current = Some(selected);
    while let Some(id) = current {
        if !visited.insert(id) {
            return Err(ConversationRepositoryError::Storage);
        }
        let mut statement = transaction.prepare("SELECT id, parent_branch_id, fork_message_id, head_message_id, status, revision, created_at, updated_at FROM conversation_branches WHERE conversation_id = ?1 AND id = ?2").map_err(slice::db)?;
        let branch = statement
            .query_row(
                params![conversation_id.to_string(), id.to_string()],
                |row| slice::read_branch(row, conversation_id),
            )
            .optional()
            .map_err(slice::db)?
            .ok_or(if id == selected {
                ConversationRepositoryError::NotFound
            } else {
                ConversationRepositoryError::Storage
            })?;
        current = branch.parent_branch_id;
        path.push(branch);
        if path.len() > 10_000 {
            return Err(ConversationRepositoryError::Storage);
        }
    }
    path.reverse();
    Ok(path)
}

fn hydrate_timeline(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    branch_id: ConversationBranchId,
    page: &PageRequest,
) -> Result<TimelinePage, ConversationRepositoryError> {
    let branches = branch_path(transaction, conversation_id, branch_id)?;
    let selected = branches
        .last()
        .ok_or(ConversationRepositoryError::Storage)?;
    if selected.status != lettuce_conversations::BranchStatus::Active {
        return Err(ConversationRepositoryError::Invalid(
            lettuce_conversations::ValidationError::InvalidReference {
                field: "timeline_page.selected_branch",
            },
        ));
    }
    let scope = format!("timeline:{conversation_id}:{branch_id}");
    let cursor = decode_cursor(page.cursor.as_ref(), &scope)?;
    if let Some(cursor) = cursor.as_ref() {
        validate_cursor_number(cursor, cursor.number > 0)?;
        let _: MessageId = validate_cursor_id(cursor)?;
    }
    let cursor_number = cursor.as_ref().map(|c| c.number).unwrap_or(i64::MAX);
    let cursor_id = cursor.as_ref().map(|c| c.text.as_str()).unwrap_or("");
    let sql = "WITH RECURSIVE ancestry(id) AS (SELECT head_message_id FROM conversation_branches WHERE conversation_id = ?1 AND id = ?2 AND head_message_id IS NOT NULL UNION ALL SELECT m.parent_message_id FROM conversation_messages AS m JOIN ancestry ON m.id = ancestry.id WHERE m.conversation_id = ?1 AND m.parent_message_id IS NOT NULL) SELECT m.conversation_id, m.id, m.branch_id, m.parent_message_id, m.author_participant_id, m.role, m.logical_time, m.effective_time, m.visibility, m.pinned, m.scene_edited, m.timeline_ordinal, m.active_revision_id, m.active_candidate_id, m.revision, m.created_at, m.updated_at FROM conversation_messages AS m JOIN ancestry ON ancestry.id = m.id WHERE m.conversation_id = ?1 AND (m.timeline_ordinal < ?3 OR (m.timeline_ordinal = ?3 AND m.id > ?4)) ORDER BY m.timeline_ordinal DESC, m.id LIMIT ?5";
    let mut statement = transaction.prepare(sql).map_err(slice::db)?;
    let mut items = Vec::new();
    let mut ordinals = Vec::new();
    for row in statement
        .query_map(
            params![
                conversation_id.to_string(),
                branch_id.to_string(),
                cursor_number,
                cursor_id,
                i64::from(page.limit.get())
            ],
            |row| message_row(transaction, row).map_err(|_| rusqlite::Error::InvalidQuery),
        )
        .map_err(slice::db)?
    {
        let (item, ordinal) = row.map_err(slice::db)?;
        items.push(item);
        ordinals.push(ordinal);
    }
    let next_cursor = if let (Some(last), Some(&ordinal)) = (items.last(), ordinals.last()) {
        let has_more: bool = transaction.query_row("WITH RECURSIVE ancestry(id) AS (SELECT head_message_id FROM conversation_branches WHERE conversation_id = ?1 AND id = ?2 AND head_message_id IS NOT NULL UNION ALL SELECT m.parent_message_id FROM conversation_messages AS m JOIN ancestry ON m.id = ancestry.id WHERE m.conversation_id = ?1 AND m.parent_message_id IS NOT NULL) SELECT EXISTS(SELECT 1 FROM conversation_messages AS m JOIN ancestry ON ancestry.id = m.id WHERE m.conversation_id = ?1 AND (m.timeline_ordinal < ?3 OR (m.timeline_ordinal = ?3 AND m.id > ?4)))", params![conversation_id.to_string(), branch_id.to_string(), ordinal, last.message.id.to_string()], |row| row.get(0)).map_err(slice::db)?;
        if has_more {
            Some(encode_cursor(
                &scope,
                &last.message.id.to_string(),
                ordinal,
            )?)
        } else {
            None
        }
    } else {
        None
    };
    let boundary_parent_id = items.last().and_then(|item| item.message.parent_message_id);
    let result = TimelinePage {
        conversation_id,
        selected_branch_id: branch_id,
        branch_path: branches,
        items,
        boundary_parent_id,
        next_cursor,
    };
    result
        .validate_page()
        .map_err(|_| ConversationRepositoryError::Storage)?;
    Ok(result)
}

fn stored_speaker(
    participant_id: Option<String>,
    payload: Option<String>,
) -> Result<Option<SelectedSpeakerDecision>, ConversationRepositoryError> {
    if participant_id.is_none() && payload.is_none() {
        return Ok(None);
    }
    let (Some(participant_id), Some(payload)) = (participant_id, payload) else {
        return Err(ConversationRepositoryError::Storage);
    };
    let participant_id: ConversationParticipantId = parse(participant_id)?;
    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct IdentityFree {
        method: SpeakerDecisionMethod,
        fallback: SpeakerFallback,
        reference: Option<SpeakerDecisionReference>,
        rationale_summary: Option<String>,
        decision_model: Option<lettuce_conversations::ModelSelectionSnapshot>,
        usage_event_id: Option<lettuce_types::UsageEventId>,
    }
    let details: IdentityFree = slice::decode(&payload)?;
    Ok(Some(SelectedSpeakerDecision {
        participant_id,
        method: details.method,
        fallback: details.fallback,
        reference: details.reference,
        rationale_summary: details.rationale_summary,
        decision_model: details.decision_model,
        usage_event_id: details.usage_event_id,
    }))
}

fn hydrate_attempts(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    turn_id: GenerationTurnId,
) -> Result<(Vec<GenerationAttempt>, Vec<MessageCandidateId>), ConversationRepositoryError> {
    let mut statement = transaction.prepare("SELECT id, ordinal, parent_attempt_id, status, job_idempotency_key, job_id, started_at, finished_at, usage_event_id, failure FROM generation_attempts WHERE conversation_id = ?1 AND turn_id = ?2 ORDER BY ordinal, id").map_err(slice::db)?;
    let rows = statement
        .query_map(
            params![conversation_id.to_string(), turn_id.to_string()],
            |row| {
                let id: GenerationAttemptId =
                    parse(row.get::<_, String>(0)?).map_err(|_| rusqlite::Error::InvalidQuery)?;
                let candidate_ids = Vec::<MessageCandidateId>::new();
                Ok((
                    id,
                    row.get::<_, i64>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<i64>>(6)?,
                    row.get::<_, Option<i64>>(7)?,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, Option<String>>(9)?,
                    candidate_ids,
                ))
            },
        )
        .map_err(slice::db)?;
    let mut attempts = Vec::new();
    let mut all_candidates = Vec::new();
    for row in rows {
        let (id, ordinal, parent, status, key, job, started, finished, usage, failure_value, _) =
            row.map_err(slice::db)?;
        let mut candidates_stmt = transaction.prepare("SELECT id FROM conversation_message_candidates WHERE conversation_id = ?1 AND turn_id = ?2 AND attempt_id = ?3 ORDER BY ordinal, id").map_err(slice::db)?;
        let candidates: Vec<MessageCandidateId> = candidates_stmt
            .query_map(
                params![
                    conversation_id.to_string(),
                    turn_id.to_string(),
                    id.to_string()
                ],
                |row| parse(row.get(0)?).map_err(|_| rusqlite::Error::InvalidQuery),
            )
            .map_err(slice::db)?
            .collect::<rusqlite::Result<_>>()
            .map_err(slice::db)?;
        all_candidates.extend(candidates.iter().copied());
        let status_value = attempt_status(&status)?;
        let usage_event_id = parse_opt(usage)?;
        let value = GenerationAttempt {
            id,
            turn_id,
            ordinal: u16::try_from(ordinal).map_err(|_| ConversationRepositoryError::Storage)?,
            parent_attempt_id: parse_opt(parent)?,
            status: status_value,
            job_idempotency_key: key
                .parse()
                .map_err(|_| ConversationRepositoryError::Storage)?,
            job_id: parse_opt(job)?,
            started_at: started.map(timestamp),
            finished_at: finished.map(timestamp),
            candidate_ids: candidates,
            usage_event_id,
            failure: failure(failure_value)?,
        };
        value
            .validate()
            .map_err(|_| ConversationRepositoryError::Storage)?;
        attempts.push(value);
    }
    Ok((attempts, all_candidates))
}

fn hydrate_lorebooks(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    turn_id: GenerationTurnId,
) -> Result<Vec<LorebookAttribution>, ConversationRepositoryError> {
    let mut statement = transaction.prepare("SELECT lorebook_id, revision FROM turn_lorebooks WHERE conversation_id = ?1 AND turn_id = ?2 ORDER BY ordinal, lorebook_id").map_err(slice::db)?;
    let mut values = Vec::new();
    for row in statement
        .query_map(
            params![conversation_id.to_string(), turn_id.to_string()],
            |row| {
                Ok(LorebookAttribution {
                    lorebook_id: row
                        .get::<_, String>(0)?
                        .parse()
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    revision: slice::rev(row.get(1)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
                })
            },
        )
        .map_err(slice::db)?
    {
        values.push(row.map_err(slice::db)?);
    }
    if values.len() > 64 {
        return Err(ConversationRepositoryError::Storage);
    }
    Ok(values)
}

fn hydrate_turn_row(
    transaction: &Transaction<'_>,
    row: &Row<'_>,
) -> Result<GenerationTurn, ConversationRepositoryError> {
    let mut visited = HashSet::new();
    hydrate_turn_row_inner(transaction, row, &mut visited)
}

fn hydrate_turn_row_inner(
    transaction: &Transaction<'_>,
    row: &Row<'_>,
    visited: &mut HashSet<GenerationTurnId>,
) -> Result<GenerationTurn, ConversationRepositoryError> {
    let conversation_id: ConversationId = parse(row.get::<_, String>(0).map_err(slice::db)?)?;
    let id: GenerationTurnId = parse(row.get::<_, String>(1).map_err(slice::db)?)?;
    if !visited.insert(id) || visited.len() > 10_000 {
        return Err(ConversationRepositoryError::Storage);
    }
    let branch_id: ConversationBranchId = parse(row.get::<_, String>(2).map_err(slice::db)?)?;
    let operation = generation_operation(&row.get::<_, String>(3).map_err(slice::db)?)?;
    let input = match row.get::<_, String>(4).map_err(slice::db)?.as_str() {
        "user_message" => GenerationInput::UserMessage {
            message_id: parse(row.get::<_, String>(5).map_err(slice::db)?)?,
        },
        "existing_head" => GenerationInput::ExistingHead {
            head_message_id: parse(row.get::<_, String>(6).map_err(slice::db)?)?,
        },
        "existing_candidate" => GenerationInput::ExistingCandidate {
            message_id: parse(row.get::<_, String>(7).map_err(slice::db)?)?,
            candidate_id: parse(row.get::<_, String>(8).map_err(slice::db)?)?,
        },
        _ => return Err(ConversationRepositoryError::Storage),
    };
    let status = generation_status(&row.get::<_, String>(10).map_err(slice::db)?)?;
    let selected_speaker = stored_speaker(
        row.get(11).map_err(slice::db)?,
        row.get(12).map_err(slice::db)?,
    )?;
    let resolved_model = row
        .get::<_, Option<String>>(13)
        .map_err(slice::db)?
        .map(|value| slice::decode(&value))
        .transpose()?;
    let prompt_id: Option<lettuce_types::PromptDocumentId> =
        parse_opt(row.get(14).map_err(slice::db)?)?;
    let prompt_revision = row
        .get::<_, Option<i64>>(15)
        .map_err(slice::db)?
        .map(slice::rev)
        .transpose()?;
    if prompt_id.is_some() != prompt_revision.is_some() {
        return Err(ConversationRepositoryError::Storage);
    }
    let prompt = prompt_id
        .zip(prompt_revision)
        .map(|(document_id, revision)| PromptAttribution {
            document_id,
            revision,
        });
    let memory = parse_opt(row.get(16).map_err(slice::db)?)?
        .map(|revision_id| MemoryAttribution { revision_id });
    let selected_candidate_id = parse_opt(row.get(17).map_err(slice::db)?)?;
    let failure = failure(row.get(18).map_err(slice::db)?)?;
    let revision = slice::rev(row.get(19).map_err(slice::db)?)?;
    let created_at = timestamp(row.get(20).map_err(slice::db)?);
    let updated_at = timestamp(row.get(21).map_err(slice::db)?);
    let target_kind: String = row.get(23).map_err(slice::db)?;
    let target_message_id: MessageId = parse(row.get::<_, String>(24).map_err(slice::db)?)?;
    let target_parent_message_id: Option<MessageId> = parse_opt(row.get(25).map_err(slice::db)?)?;
    let target_prior_candidate_id: Option<MessageCandidateId> =
        parse_opt(row.get(26).map_err(slice::db)?)?;
    let target = match (
        target_kind.as_str(),
        target_parent_message_id,
        target_prior_candidate_id,
    ) {
        ("new_assistant", parent_message_id, None) => GenerationTarget::NewAssistant {
            message_id: target_message_id,
            parent_message_id,
        },
        ("existing_candidate", None, Some(prior_candidate_id)) => {
            GenerationTarget::ExistingCandidate {
                message_id: target_message_id,
                prior_candidate_id,
            }
        }
        _ => return Err(ConversationRepositoryError::Storage),
    };
    let guidance = row.get::<_, Option<String>>(27).map_err(slice::db)?;
    let requested_model_override = row
        .get::<_, Option<String>>(28)
        .map_err(slice::db)?
        .map(|value| slice::decode(&value))
        .transpose()?;
    let forced_speaker = parse_opt(row.get(29).map_err(slice::db)?)?;
    if let Some(participant_id) = forced_speaker {
        validate_character_participant(transaction, conversation_id, participant_id)?;
    }
    let swap_roles = row.get::<_, i64>(30).map_err(slice::db)? != 0;
    let retry_of_turn_id = parse_opt(row.get(31).map_err(slice::db)?)?;
    let idempotency_key: lettuce_jobs::IdempotencyKey = row
        .get::<_, String>(9)
        .map_err(slice::db)?
        .parse()
        .map_err(|_| ConversationRepositoryError::Storage)?;
    let correlation_id = row
        .get::<_, Option<String>>(22)
        .map_err(slice::db)?
        .map(|value| {
            value
                .parse()
                .map_err(|_| ConversationRepositoryError::Storage)
        })
        .transpose()?;
    let (attempts, candidate_ids) = hydrate_attempts(transaction, conversation_id, id)?;
    let lorebooks = hydrate_lorebooks(transaction, conversation_id, id)?;
    let group: bool = transaction
        .query_row(
            "SELECT kind = 'group' FROM conversations WHERE id = ?1",
            [conversation_id.to_string()],
            |row| row.get(0),
        )
        .map_err(slice::db)?;
    if let Some(speaker) = selected_speaker.as_ref() {
        validate_character_participant(transaction, conversation_id, speaker.participant_id)?;
    }
    let value = GenerationTurn {
        id,
        conversation_id,
        branch_id,
        operation,
        input,
        target,
        swap_roles,
        retry_of_turn_id,
        idempotency_key,
        correlation_id,
        status,
        selected_speaker,
        guidance,
        requested_model_override,
        forced_speaker,
        resolved_model,
        prompt,
        lorebooks,
        memory,
        candidate_ids,
        selected_candidate_id,
        attempts,
        failure,
        revision,
        created_at,
        updated_at,
    };
    value
        .validate(group)
        .map_err(|_| ConversationRepositoryError::Storage)?;
    if let Some(source_id) = value.retry_of_turn_id {
        let source_sql = format!(
            "{} WHERE conversation_id = ?1 AND id = ?2",
            turn_select_sql()
        );
        let source = transaction
            .query_row(
                &source_sql,
                params![conversation_id.to_string(), source_id.to_string()],
                |row| {
                    hydrate_turn_row_inner(transaction, row, visited)
                        .map_err(|_| rusqlite::Error::InvalidQuery)
                },
            )
            .optional()
            .map_err(slice::db)?
            .ok_or(ConversationRepositoryError::Storage)?;
        value
            .validate_retry_against(&source, group)
            .map_err(|_| ConversationRepositoryError::Storage)?;
    }
    Ok(value)
}

fn owned_ref(
    transaction: &Transaction<'_>,
    table: &str,
    column: &str,
    conversation_id: ConversationId,
    value: impl ToString,
) -> Result<(), ConversationRepositoryError> {
    let sql = format!(
        "SELECT EXISTS(SELECT 1 FROM {table} WHERE conversation_id = ?1 AND {column} = ?2)"
    );
    let present: bool = transaction
        .query_row(
            &sql,
            params![conversation_id.to_string(), value.to_string()],
            |row| row.get(0),
        )
        .map_err(slice::db)?;
    if present {
        Ok(())
    } else {
        Err(ConversationRepositoryError::Storage)
    }
}

fn validate_character_participant(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    participant_id: ConversationParticipantId,
) -> Result<(), ConversationRepositoryError> {
    let valid: bool = transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM conversation_participants WHERE conversation_id = ?1 AND id = ?2 AND role = 'character')",
            params![conversation_id.to_string(), participant_id.to_string()],
            |row| row.get(0),
        )
        .map_err(slice::db)?;
    if valid {
        Ok(())
    } else {
        Err(ConversationRepositoryError::Storage)
    }
}

fn validate_operation_result_ownership(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    result: &OperationResultRef,
) -> Result<(), ConversationRepositoryError> {
    match result {
        OperationResultRef::Conversation(id) => {
            if *id != conversation_id {
                return Err(ConversationRepositoryError::Storage);
            }
        }
        OperationResultRef::Turn(id) => {
            owned_ref(transaction, "conversation_turns", "id", conversation_id, id)?
        }
        OperationResultRef::Message(id) => owned_ref(
            transaction,
            "conversation_messages",
            "id",
            conversation_id,
            id,
        )?,
        OperationResultRef::Candidate(id) => owned_ref(
            transaction,
            "conversation_message_candidates",
            "id",
            conversation_id,
            id,
        )?,
        OperationResultRef::Branch(id) => owned_ref(
            transaction,
            "conversation_branches",
            "id",
            conversation_id,
            id,
        )?,
    }
    Ok(())
}

fn require_exists<P: rusqlite::Params>(
    transaction: &Transaction<'_>,
    sql: &str,
    parameters: P,
) -> Result<(), ConversationRepositoryError> {
    let exists: bool = transaction
        .query_row(sql, parameters, |row| row.get(0))
        .map_err(slice::db)?;
    if exists {
        Ok(())
    } else {
        Err(ConversationRepositoryError::Storage)
    }
}

fn validate_asset_delta(
    transaction: &Transaction<'_>,
    conversation_id: ConversationId,
    delta: &lettuce_conversations::AssetReferenceDelta,
) -> Result<(), ConversationRepositoryError> {
    require_exists(
        transaction,
        "SELECT EXISTS(SELECT 1 FROM media_assets WHERE id = ?1)",
        params![delta.asset_id.to_string()],
    )?;
    match delta.retainer {
        AssetRetainer::Conversation(id) => {
            if id != conversation_id {
                return Err(ConversationRepositoryError::Storage);
            }
        }
        AssetRetainer::Message(id) => owned_ref(
            transaction,
            "conversation_messages",
            "id",
            conversation_id,
            id,
        )?,
        AssetRetainer::MessageRevision(id) => owned_ref(
            transaction,
            "conversation_message_revisions",
            "id",
            conversation_id,
            id,
        )?,
        AssetRetainer::MessageCandidate(id) => owned_ref(
            transaction,
            "conversation_message_candidates",
            "id",
            conversation_id,
            id,
        )?,
        AssetRetainer::Character(id) => require_exists(
            transaction,
            "SELECT EXISTS(SELECT 1 FROM characters WHERE id = ?1)",
            params![id.to_string()],
        )?,
        AssetRetainer::Persona(id) => require_exists(
            transaction,
            "SELECT EXISTS(SELECT 1 FROM personas WHERE id = ?1)",
            params![id.to_string()],
        )?,
        AssetRetainer::Group(id) => require_exists(
            transaction,
            "SELECT EXISTS(SELECT 1 FROM groups WHERE id = ?1)",
            params![id.to_string()],
        )?,
        AssetRetainer::Scene(id) => require_exists(
            transaction,
            "SELECT EXISTS(SELECT 1 FROM scenes WHERE id = ?1)",
            params![id.to_string()],
        )?,
        AssetRetainer::SceneVariant(id) => require_exists(
            transaction,
            "SELECT EXISTS(SELECT 1 FROM scene_variants WHERE id = ?1)",
            params![id.to_string()],
        )?,
        AssetRetainer::SceneAssetLink(id) => require_exists(
            transaction,
            "SELECT EXISTS(SELECT 1 FROM scene_assets WHERE id = ?1)",
            params![id.to_string()],
        )?,
        AssetRetainer::PromptDocument(id) => require_exists(
            transaction,
            "SELECT EXISTS(SELECT 1 FROM prompt_documents WHERE id = ?1)",
            params![id.to_string()],
        )?,
        AssetRetainer::Lorebook(id) => require_exists(
            transaction,
            "SELECT EXISTS(SELECT 1 FROM lorebooks WHERE id = ?1)",
            params![id.to_string()],
        )?,
        AssetRetainer::LorebookEntry(id) => require_exists(
            transaction,
            "SELECT EXISTS(SELECT 1 FROM lorebook_entries WHERE id = ?1)",
            params![id.to_string()],
        )?,
    }
    Ok(())
}

fn validate_outbox_event_timestamp(
    record: &ConversationOutboxRecord,
) -> Result<(), ConversationRepositoryError> {
    let event_at = match &record.event {
        ConversationOutboxEvent::ConversationCreated { at, .. }
        | ConversationOutboxEvent::MessageCommitted { at, .. }
        | ConversationOutboxEvent::MessageRevised { at, .. }
        | ConversationOutboxEvent::MessageTombstoned { at, .. }
        | ConversationOutboxEvent::TurnFailed { at, .. }
        | ConversationOutboxEvent::TurnCancellationRequested { at, .. }
        | ConversationOutboxEvent::TurnCancelled { at, .. }
        | ConversationOutboxEvent::BranchForked { at, .. }
        | ConversationOutboxEvent::ConversationTombstoned { at, .. }
        | ConversationOutboxEvent::AssetReferencesChanged { at, .. } => *at,
        ConversationOutboxEvent::TurnFinalized { effective_time, .. } => *effective_time,
    };
    if event_at == record.at {
        Ok(())
    } else {
        Err(ConversationRepositoryError::Storage)
    }
}

fn validate_outbox_event(
    transaction: &Transaction<'_>,
    record: &ConversationOutboxRecord,
) -> Result<(), ConversationRepositoryError> {
    let conversation_id = record.conversation_id;
    match &record.event {
        ConversationOutboxEvent::ConversationCreated {
            root_branch_id,
            head_message_id,
            ..
        } => {
            owned_ref(
                transaction,
                "conversation_branches",
                "id",
                conversation_id,
                root_branch_id,
            )?;
            if let Some(message_id) = head_message_id {
                owned_ref(
                    transaction,
                    "conversation_messages",
                    "id",
                    conversation_id,
                    message_id,
                )?;
            }
        }
        ConversationOutboxEvent::MessageCommitted {
            branch_id,
            message_id,
            revision_id,
            candidate_id,
            ..
        } => {
            owned_ref(
                transaction,
                "conversation_branches",
                "id",
                conversation_id,
                branch_id,
            )?;
            owned_ref(
                transaction,
                "conversation_messages",
                "id",
                conversation_id,
                message_id,
            )?;
            if let Some(id) = revision_id {
                owned_ref(
                    transaction,
                    "conversation_message_revisions",
                    "id",
                    conversation_id,
                    id,
                )?;
            }
            if let Some(id) = candidate_id {
                owned_ref(
                    transaction,
                    "conversation_message_candidates",
                    "id",
                    conversation_id,
                    id,
                )?;
            }
        }
        ConversationOutboxEvent::MessageRevised {
            branch_id,
            message_id,
            revision_id,
            ..
        } => {
            owned_ref(
                transaction,
                "conversation_branches",
                "id",
                conversation_id,
                branch_id,
            )?;
            owned_ref(
                transaction,
                "conversation_messages",
                "id",
                conversation_id,
                message_id,
            )?;
            owned_ref(
                transaction,
                "conversation_message_revisions",
                "id",
                conversation_id,
                revision_id,
            )?;
        }
        ConversationOutboxEvent::MessageTombstoned {
            branch_id,
            message_id,
            affected_message_ids,
            affected_revision_ids,
            ..
        } => {
            owned_ref(
                transaction,
                "conversation_branches",
                "id",
                conversation_id,
                branch_id,
            )?;
            owned_ref(
                transaction,
                "conversation_messages",
                "id",
                conversation_id,
                message_id,
            )?;
            for id in affected_message_ids {
                owned_ref(
                    transaction,
                    "conversation_messages",
                    "id",
                    conversation_id,
                    id,
                )?;
            }
            for id in affected_revision_ids {
                owned_ref(
                    transaction,
                    "conversation_message_revisions",
                    "id",
                    conversation_id,
                    id,
                )?;
            }
        }
        ConversationOutboxEvent::TurnFinalized {
            branch_id,
            turn_id,
            attempt_id,
            message_id,
            candidate_id,
            revision_id,
            ..
        } => {
            owned_ref(
                transaction,
                "conversation_branches",
                "id",
                conversation_id,
                branch_id,
            )?;
            owned_ref(
                transaction,
                "conversation_turns",
                "id",
                conversation_id,
                turn_id,
            )?;
            owned_ref(
                transaction,
                "generation_attempts",
                "id",
                conversation_id,
                attempt_id,
            )?;
            owned_ref(
                transaction,
                "conversation_messages",
                "id",
                conversation_id,
                message_id,
            )?;
            owned_ref(
                transaction,
                "conversation_message_candidates",
                "id",
                conversation_id,
                candidate_id,
            )?;
            if let Some(id) = revision_id {
                owned_ref(
                    transaction,
                    "conversation_message_revisions",
                    "id",
                    conversation_id,
                    id,
                )?;
            }
        }
        ConversationOutboxEvent::TurnFailed {
            branch_id,
            turn_id,
            attempt_id,
            ..
        }
        | ConversationOutboxEvent::TurnCancellationRequested {
            branch_id,
            turn_id,
            attempt_id,
            ..
        }
        | ConversationOutboxEvent::TurnCancelled {
            branch_id,
            turn_id,
            attempt_id,
            ..
        } => {
            owned_ref(
                transaction,
                "conversation_branches",
                "id",
                conversation_id,
                branch_id,
            )?;
            owned_ref(
                transaction,
                "conversation_turns",
                "id",
                conversation_id,
                turn_id,
            )?;
            owned_ref(
                transaction,
                "generation_attempts",
                "id",
                conversation_id,
                attempt_id,
            )?;
        }
        ConversationOutboxEvent::BranchForked { branch_id, .. } => owned_ref(
            transaction,
            "conversation_branches",
            "id",
            conversation_id,
            branch_id,
        )?,
        ConversationOutboxEvent::ConversationTombstoned { .. } => {}
        ConversationOutboxEvent::AssetReferencesChanged {
            message_revision_id,
            candidate_id,
            ..
        } => {
            if let Some(id) = message_revision_id {
                owned_ref(
                    transaction,
                    "conversation_message_revisions",
                    "id",
                    conversation_id,
                    id,
                )?;
            }
            if let Some(id) = candidate_id {
                owned_ref(
                    transaction,
                    "conversation_message_candidates",
                    "id",
                    conversation_id,
                    id,
                )?;
            }
        }
    }
    validate_outbox_event_exact(transaction, record)?;
    Ok(())
}

fn turn_select_sql() -> &'static str {
    "SELECT conversation_id, id, branch_id, operation, input_kind, user_message_id, head_message_id, candidate_message_id, candidate_id, idempotency_key, status, selected_speaker_participant_id, selected_speaker_details_json, resolved_model_json, prompt_document_id, prompt_revision, memory_revision_id, selected_candidate_id, failure, revision, created_at, updated_at, correlation_id, target_kind, target_message_id, target_parent_message_id, target_prior_candidate_id, guidance, requested_model_override_json, forced_speaker_participant_id, swap_roles, retry_of_turn_id FROM conversation_turns"
}

fn unique_message_conversation(
    transaction: &Transaction<'_>,
    message_id: MessageId,
) -> Result<ConversationId, ConversationRepositoryError> {
    let mut statement = transaction
        .prepare("SELECT conversation_id FROM conversation_messages WHERE id = ?1 LIMIT 2")
        .map_err(slice::db)?;
    let mut values = Vec::new();
    for row in statement
        .query_map([message_id.to_string()], |row| row.get::<_, String>(0))
        .map_err(slice::db)?
        .take(2)
    {
        values.push(row.map_err(slice::db)?);
    }
    drop(statement);
    if values.len() > 1 {
        return Err(ConversationRepositoryError::Storage);
    }
    values
        .pop()
        .ok_or(ConversationRepositoryError::NotFound)?
        .parse()
        .map_err(|_| ConversationRepositoryError::Storage)
}

fn read_revision_page(
    transaction: &Transaction<'_>,
    message_id: MessageId,
    page: &PageRequest,
) -> Result<KeysetPage<MessageRevision>, ConversationRepositoryError> {
    let conversation_id = unique_message_conversation(transaction, message_id)?;
    let scope = format!("revisions:{message_id}");
    let cursor = decode_cursor(page.cursor.as_ref(), &scope)?;
    if let Some(cursor) = cursor.as_ref() {
        validate_cursor_number(cursor, cursor.number > 0)?;
        let _: MessageRevisionId = validate_cursor_id(cursor)?;
    }
    let number = cursor.as_ref().map(|value| value.number).unwrap_or(0);
    let text = cursor
        .as_ref()
        .map(|value| value.text.as_str())
        .unwrap_or("");
    let sql = "SELECT conversation_id, id, message_id, branch_id, sequence, parts_json, authored_at, provider_replay_artifact_id, provider_replay_retention, source_turn_id FROM conversation_message_revisions WHERE conversation_id = ?1 AND message_id = ?2 AND (sequence > ?3 OR (sequence = ?3 AND id > ?4)) ORDER BY sequence, id LIMIT ?5";
    let mut statement = transaction.prepare(sql).map_err(slice::db)?;
    let mut values = Vec::new();
    for row in statement
        .query_map(
            params![
                conversation_id.to_string(),
                message_id.to_string(),
                number,
                text,
                i64::from(page.limit.get())
            ],
            |row| hydrate_revision_row(transaction, row).map_err(|_| rusqlite::Error::InvalidQuery),
        )
        .map_err(slice::db)?
    {
        values.push(row.map_err(slice::db)?);
    }
    let next = if let Some(last) = values.last() {
        drop(statement);
        let has_more: bool = transaction.query_row("SELECT EXISTS(SELECT 1 FROM conversation_message_revisions WHERE conversation_id = ?1 AND message_id = ?2 AND (sequence > ?3 OR (sequence = ?3 AND id > ?4)))", params![conversation_id.to_string(), message_id.to_string(), last.sequence.get() as i64, last.id.to_string()], |row| row.get(0)).map_err(slice::db)?;
        if has_more {
            Some(encode_cursor(
                &scope,
                &last.id.to_string(),
                last.sequence.get() as i64,
            )?)
        } else {
            None
        }
    } else {
        None
    };
    Ok(Page {
        items: values,
        next_cursor: next,
    })
}

fn read_candidate_page(
    transaction: &Transaction<'_>,
    message_id: MessageId,
    page: &PageRequest,
) -> Result<KeysetPage<MessageCandidate>, ConversationRepositoryError> {
    let conversation_id = unique_message_conversation(transaction, message_id)?;
    let scope = format!("candidates:{message_id}");
    let cursor = decode_cursor(page.cursor.as_ref(), &scope)?;
    if let Some(cursor) = cursor.as_ref() {
        validate_cursor_number(cursor, cursor.number >= 0)?;
        let _: MessageCandidateId = validate_cursor_id(cursor)?;
    }
    let number = cursor.as_ref().map(|value| value.number).unwrap_or(-1);
    let text = cursor
        .as_ref()
        .map(|value| value.text.as_str())
        .unwrap_or("");
    let sql = "SELECT conversation_id, id, message_id, branch_id, turn_id, attempt_id, ordinal, parts_json, model_json, created_at, provider_replay_artifact_id, provider_replay_retention FROM conversation_message_candidates WHERE conversation_id = ?1 AND message_id = ?2 AND (ordinal > ?3 OR (ordinal = ?3 AND id > ?4)) ORDER BY ordinal, id LIMIT ?5";
    let mut statement = transaction.prepare(sql).map_err(slice::db)?;
    let mut values = Vec::new();
    for row in statement
        .query_map(
            params![
                conversation_id.to_string(),
                message_id.to_string(),
                number,
                text,
                i64::from(page.limit.get())
            ],
            |row| {
                hydrate_candidate_row(transaction, row).map_err(|_| rusqlite::Error::InvalidQuery)
            },
        )
        .map_err(slice::db)?
    {
        values.push(row.map_err(slice::db)?);
    }
    let next = if let Some(last) = values.last() {
        drop(statement);
        let has_more: bool = transaction.query_row("SELECT EXISTS(SELECT 1 FROM conversation_message_candidates WHERE conversation_id = ?1 AND message_id = ?2 AND (ordinal > ?3 OR (ordinal = ?3 AND id > ?4)))", params![conversation_id.to_string(), message_id.to_string(), i64::from(last.ordinal), last.id.to_string()], |row| row.get(0)).map_err(slice::db)?;
        if has_more {
            Some(encode_cursor(
                &scope,
                &last.id.to_string(),
                i64::from(last.ordinal),
            )?)
        } else {
            None
        }
    } else {
        None
    };
    Ok(Page {
        items: values,
        next_cursor: next,
    })
}

impl ConversationReader for Database {
    fn get(
        &self,
        id: ConversationId,
    ) -> Result<ConversationAggregate, ConversationRepositoryError> {
        self.get_conversation_record(id)
    }

    fn page(
        &self,
        query: &ConversationQuery,
    ) -> Result<KeysetPage<ConversationSummary>, ConversationRepositoryError> {
        let mut connection = open_read(self)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(slice::db)?;
        let scope = format!("conversations:{:?}", query.lifecycle);
        let cursor = decode_cursor(query.page.cursor.as_ref(), &scope)?;
        if let Some(cursor) = cursor.as_ref() {
            validate_cursor_number(cursor, true)?;
            let _: ConversationId = validate_cursor_id(cursor)?;
        }
        let updated = cursor
            .as_ref()
            .map(|value| value.number)
            .unwrap_or(i64::MAX);
        let id = cursor
            .as_ref()
            .map(|value| value.text.as_str())
            .unwrap_or("\u{ffff}");
        let limit = i64::from(query.page.limit.get());
        let lifecycle = query.lifecycle.map(|value| match value {
            ConversationLifecycle::Active => "active",
            ConversationLifecycle::Archived => "archived",
            ConversationLifecycle::Tombstoned => "tombstoned",
        });
        let mut statement = transaction.prepare("SELECT id, title, lifecycle, kind, revision, updated_at FROM conversations WHERE (?1 IS NULL OR lifecycle = ?1) AND (updated_at < ?2 OR (updated_at = ?2 AND id > ?3)) ORDER BY updated_at DESC, id LIMIT ?4").map_err(slice::db)?;
        let mut items = Vec::new();
        for row in statement
            .query_map(params![lifecycle, updated, id, limit], |row| {
                let id: ConversationId =
                    parse(row.get(0)?).map_err(|_| rusqlite::Error::InvalidQuery)?;
                let lifecycle = match row.get::<_, String>(2)?.as_str() {
                    "active" => ConversationLifecycle::Active,
                    "archived" => ConversationLifecycle::Archived,
                    "tombstoned" => ConversationLifecycle::Tombstoned,
                    _ => return Err(rusqlite::Error::InvalidQuery),
                };
                let kind = match row.get::<_, String>(3)?.as_str() {
                    "direct" => ConversationKindTag::Direct,
                    "group" => ConversationKindTag::Group,
                    _ => return Err(rusqlite::Error::InvalidQuery),
                };
                Ok(ConversationSummary {
                    id,
                    title: row.get(1)?,
                    lifecycle,
                    kind,
                    revision: slice::rev(row.get(4)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
                    updated_at: timestamp(row.get(5)?),
                })
            })
            .map_err(slice::db)?
        {
            items.push(row.map_err(slice::db)?);
        }
        drop(statement);
        let next_cursor = if let Some(last) = items.last() {
            let has_more: bool = transaction.query_row("SELECT EXISTS(SELECT 1 FROM conversations WHERE (?1 IS NULL OR lifecycle = ?1) AND (updated_at < ?2 OR (updated_at = ?2 AND id > ?3)))", params![lifecycle, last.updated_at.get(), last.id.to_string()], |row| row.get(0)).map_err(slice::db)?;
            if has_more {
                Some(encode_cursor(
                    &scope,
                    &last.id.to_string(),
                    last.updated_at.get(),
                )?)
            } else {
                None
            }
        } else {
            None
        };
        transaction.commit().map_err(slice::db)?;
        Ok(Page { items, next_cursor })
    }

    fn timeline_page(
        &self,
        conversation_id: ConversationId,
        branch_id: ConversationBranchId,
        page: &PageRequest,
    ) -> Result<TimelinePage, ConversationRepositoryError> {
        let mut connection = open_read(self)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(slice::db)?;
        let value = hydrate_timeline(&transaction, conversation_id, branch_id, page)?;
        transaction.commit().map_err(slice::db)?;
        Ok(value)
    }

    fn get_message_revision(
        &self,
        id: MessageRevisionId,
    ) -> Result<MessageRevision, ConversationRepositoryError> {
        let mut connection = open_read(self)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(slice::db)?;
        let mut statement = transaction.prepare("SELECT conversation_id, id, message_id, branch_id, sequence, parts_json, authored_at, provider_replay_artifact_id, provider_replay_retention, source_turn_id FROM conversation_message_revisions WHERE id = ?1").map_err(slice::db)?;
        let mut values = Vec::new();
        for row in statement
            .query_map([id.to_string()], |row| {
                hydrate_revision_row(&transaction, row).map_err(|_| rusqlite::Error::InvalidQuery)
            })
            .map_err(slice::db)?
            .take(2)
        {
            values.push(row.map_err(slice::db)?);
        }
        drop(statement);
        if values.len() > 1 {
            return Err(ConversationRepositoryError::Storage);
        }
        transaction.commit().map_err(slice::db)?;
        values.pop().ok_or(ConversationRepositoryError::NotFound)
    }

    fn page_message_revisions(
        &self,
        message_id: MessageId,
        page: &PageRequest,
    ) -> Result<KeysetPage<MessageRevision>, ConversationRepositoryError> {
        let mut connection = open_read(self)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(slice::db)?;
        let value = read_revision_page(&transaction, message_id, page)?;
        transaction.commit().map_err(slice::db)?;
        Ok(value)
    }

    fn get_candidate(
        &self,
        id: MessageCandidateId,
    ) -> Result<MessageCandidate, ConversationRepositoryError> {
        let mut connection = open_read(self)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(slice::db)?;
        let mut statement = transaction.prepare("SELECT conversation_id, id, message_id, branch_id, turn_id, attempt_id, ordinal, parts_json, model_json, created_at, provider_replay_artifact_id, provider_replay_retention FROM conversation_message_candidates WHERE id = ?1").map_err(slice::db)?;
        let mut values = Vec::new();
        for row in statement
            .query_map([id.to_string()], |row| {
                hydrate_candidate_row(&transaction, row).map_err(|_| rusqlite::Error::InvalidQuery)
            })
            .map_err(slice::db)?
            .take(2)
        {
            values.push(row.map_err(slice::db)?);
        }
        drop(statement);
        if values.len() > 1 {
            return Err(ConversationRepositoryError::Storage);
        }
        transaction.commit().map_err(slice::db)?;
        values.pop().ok_or(ConversationRepositoryError::NotFound)
    }

    fn page_candidates(
        &self,
        message_id: MessageId,
        page: &PageRequest,
    ) -> Result<KeysetPage<MessageCandidate>, ConversationRepositoryError> {
        let mut connection = open_read(self)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(slice::db)?;
        let value = read_candidate_page(&transaction, message_id, page)?;
        transaction.commit().map_err(slice::db)?;
        Ok(value)
    }

    fn get_turn(
        &self,
        id: GenerationTurnId,
    ) -> Result<GenerationTurn, ConversationRepositoryError> {
        let mut connection = open_read(self)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(slice::db)?;
        let sql = format!("{} WHERE id = ?1", turn_select_sql());
        let mut statement = transaction.prepare(&sql).map_err(slice::db)?;
        let mut values = Vec::new();
        for row in statement
            .query_map([id.to_string()], |row| {
                hydrate_turn_row(&transaction, row).map_err(|_| rusqlite::Error::InvalidQuery)
            })
            .map_err(slice::db)?
            .take(2)
        {
            values.push(row.map_err(slice::db)?);
        }
        drop(statement);
        if values.len() > 1 {
            return Err(ConversationRepositoryError::Storage);
        }
        transaction.commit().map_err(slice::db)?;
        values.pop().ok_or(ConversationRepositoryError::NotFound)
    }

    fn page_turns(
        &self,
        conversation_id: ConversationId,
        page: &PageRequest,
    ) -> Result<KeysetPage<GenerationTurn>, ConversationRepositoryError> {
        let mut connection = open_read(self)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(slice::db)?;
        let scope = format!("turns:{conversation_id}");
        let conversation_exists: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM conversations WHERE id = ?1)",
                [conversation_id.to_string()],
                |row| row.get(0),
            )
            .map_err(slice::db)?;
        if !conversation_exists {
            return Err(ConversationRepositoryError::NotFound);
        }
        let cursor = decode_cursor(page.cursor.as_ref(), &scope)?;
        if let Some(cursor) = cursor.as_ref() {
            validate_cursor_number(cursor, true)?;
            let _: GenerationTurnId = validate_cursor_id(cursor)?;
        }
        let number = cursor
            .as_ref()
            .map(|value| value.number)
            .unwrap_or(i64::MIN);
        let text = cursor
            .as_ref()
            .map(|value| value.text.as_str())
            .unwrap_or("");
        let sql = format!(
            "{} WHERE conversation_id = ?1 AND (created_at > ?2 OR (created_at = ?2 AND id > ?3)) ORDER BY created_at, id LIMIT ?4",
            turn_select_sql()
        );
        let mut statement = transaction.prepare(&sql).map_err(slice::db)?;
        let mut values = Vec::new();
        for row in statement
            .query_map(
                params![
                    conversation_id.to_string(),
                    number,
                    text,
                    i64::from(page.limit.get())
                ],
                |row| {
                    hydrate_turn_row(&transaction, row).map_err(|_| rusqlite::Error::InvalidQuery)
                },
            )
            .map_err(slice::db)?
        {
            values.push(row.map_err(slice::db)?);
        }
        drop(statement);
        let next_cursor = if let Some(value) = values.last() {
            let (id, created) = (value.id, value.created_at);
            let has_more: bool = transaction.query_row("SELECT EXISTS(SELECT 1 FROM conversation_turns WHERE conversation_id = ?1 AND (created_at > ?2 OR (created_at = ?2 AND id > ?3)))", params![conversation_id.to_string(), created.get(), id.to_string()], |row| row.get(0)).map_err(slice::db)?;
            if has_more {
                Some(encode_cursor(&scope, &id.to_string(), created.get())?)
            } else {
                None
            }
        } else {
            None
        };
        transaction.commit().map_err(slice::db)?;
        Ok(Page {
            items: values,
            next_cursor,
        })
    }

    fn operation_record(
        &self,
        conversation_id: ConversationId,
        kind: OperationKind,
        token: &lettuce_conversations::OperationToken,
    ) -> Result<Option<OperationRecord>, ConversationRepositoryError> {
        let mut connection = open_read(self)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(slice::db)?;
        let conversation_exists: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM conversations WHERE id = ?1)",
                [conversation_id.to_string()],
                |row| row.get(0),
            )
            .map_err(slice::db)?;
        if !conversation_exists {
            return Err(ConversationRepositoryError::NotFound);
        }
        let kind_name = serde_json::to_value(kind)
            .map_err(|_| ConversationRepositoryError::Storage)?
            .as_str()
            .ok_or(ConversationRepositoryError::Storage)?
            .to_owned();
        if operation_kind(&kind_name)? != kind {
            return Err(ConversationRepositoryError::Storage);
        }
        let row: Option<(String, String, String, String, String, String, i64)> = transaction.query_row("SELECT id, operation_key, request_digest, result_kind, result_id, result_json, created_at FROM conversation_operations WHERE conversation_id = ?1 AND kind = ?2 AND operation_key = ?3", params![conversation_id.to_string(), kind_name, token.key.as_str()], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?))).optional().map_err(slice::db)?;
        let Some((operation_id, key, digest, result_kind, result_id, result_json, created_at)) =
            row
        else {
            transaction.commit().map_err(slice::db)?;
            return Ok(None);
        };
        let stored_key: lettuce_jobs::IdempotencyKey = key
            .parse()
            .map_err(|_| ConversationRepositoryError::Storage)?;
        let stored_digest: lettuce_types::ContentHash = digest
            .parse()
            .map_err(|_| ConversationRepositoryError::Storage)?;
        if stored_digest != token.request_digest {
            return Err(ConversationRepositoryError::Conflict);
        }
        let result: OperationResultRef = slice::decode(&result_json)?;
        let expected = match (result_kind.as_str(), result) {
            ("conversation", OperationResultRef::Conversation(value)) => {
                OperationResultRef::Conversation(value)
            }
            ("turn", OperationResultRef::Turn(value)) => OperationResultRef::Turn(value),
            ("message", OperationResultRef::Message(value)) => OperationResultRef::Message(value),
            ("candidate", OperationResultRef::Candidate(value)) => {
                OperationResultRef::Candidate(value)
            }
            ("branch", OperationResultRef::Branch(value)) => OperationResultRef::Branch(value),
            _ => return Err(ConversationRepositoryError::Storage),
        };
        let projected_id = match &expected {
            OperationResultRef::Conversation(value) => value.to_string(),
            OperationResultRef::Turn(value) => value.to_string(),
            OperationResultRef::Message(value) => value.to_string(),
            OperationResultRef::Candidate(value) => value.to_string(),
            OperationResultRef::Branch(value) => value.to_string(),
        };
        if projected_id != result_id {
            return Err(ConversationRepositoryError::Storage);
        }
        validate_operation_result_ownership(&transaction, conversation_id, &expected)?;
        let value = OperationRecord {
            id: parse(operation_id)?,
            conversation_id,
            kind,
            operation: lettuce_conversations::OperationToken {
                key: stored_key,
                request_digest: stored_digest,
            },
            result: expected,
            created_at: timestamp(created_at),
        };
        transaction.commit().map_err(slice::db)?;
        Ok(Some(value))
    }

    fn page_outbox(
        &self,
        conversation_id: ConversationId,
        page: &PageRequest,
    ) -> Result<KeysetPage<ConversationOutboxRecord>, ConversationRepositoryError> {
        let mut connection = open_read(self)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(slice::db)?;
        let conversation_exists: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM conversations WHERE id = ?1)",
                [conversation_id.to_string()],
                |row| row.get(0),
            )
            .map_err(slice::db)?;
        if !conversation_exists {
            return Err(ConversationRepositoryError::NotFound);
        }
        let scope = format!("outbox:{conversation_id}");
        let cursor = decode_cursor(page.cursor.as_ref(), &scope)?;
        if let Some(cursor) = cursor.as_ref() {
            validate_cursor_number(cursor, cursor.number > 0)?;
            let _: lettuce_types::OutboxEventId = validate_cursor_id(cursor)?;
        }
        let number = cursor.as_ref().map(|value| value.number).unwrap_or(0);
        let text = cursor
            .as_ref()
            .map(|value| value.text.as_str())
            .unwrap_or("");
        let mut statement = transaction.prepare("SELECT id, sequence, conversation_revision, operation_record_id, at, event_json FROM conversation_outbox WHERE conversation_id = ?1 AND (sequence > ?2 OR (sequence = ?2 AND id > ?3)) ORDER BY sequence, id LIMIT ?4").map_err(slice::db)?;
        let mut items = Vec::new();
        for row in statement
            .query_map(
                params![
                    conversation_id.to_string(),
                    number,
                    text,
                    i64::from(page.limit.get())
                ],
                |row| {
                    let event: ConversationOutboxEvent =
                        slice::decode(&row.get::<_, String>(5)?)
                            .map_err(|_| rusqlite::Error::InvalidQuery)?;
                    Ok(ConversationOutboxRecord {
                        format_version: 1,
                        id: parse(row.get(0)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
                        conversation_id,
                        conversation_revision: slice::rev(row.get(2)?)
                            .map_err(|_| rusqlite::Error::InvalidQuery)?,
                        sequence: u64::try_from(row.get::<_, i64>(1)?)
                            .map_err(|_| rusqlite::Error::InvalidQuery)?,
                        operation_record_id: parse(row.get(3)?)
                            .map_err(|_| rusqlite::Error::InvalidQuery)?,
                        at: timestamp(row.get(4)?),
                        event,
                    })
                },
            )
            .map_err(slice::db)?
        {
            let value = row.map_err(slice::db)?;
            value
                .validate()
                .map_err(|_| ConversationRepositoryError::Storage)?;
            validate_outbox_event_timestamp(&value)?;
            let operation_exists: bool = transaction.query_row("SELECT EXISTS(SELECT 1 FROM conversation_operations WHERE conversation_id = ?1 AND id = ?2)", params![conversation_id.to_string(), value.operation_record_id.to_string()], |row| row.get(0)).map_err(slice::db)?;
            if !operation_exists {
                return Err(ConversationRepositoryError::Storage);
            }
            validate_outbox_event(&transaction, &value)?;
            items.push(value);
        }
        drop(statement);
        let next_cursor = if let Some(last) = items.last() {
            let has_more: bool = transaction.query_row("SELECT EXISTS(SELECT 1 FROM conversation_outbox WHERE conversation_id = ?1 AND (sequence > ?2 OR (sequence = ?2 AND id > ?3)))", params![conversation_id.to_string(), last.sequence as i64, last.id.to_string()], |row| row.get(0)).map_err(slice::db)?;
            if has_more {
                Some(encode_cursor(
                    &scope,
                    &last.id.to_string(),
                    i64::try_from(last.sequence)
                        .map_err(|_| ConversationRepositoryError::Storage)?,
                )?)
            } else {
                None
            }
        } else {
            None
        };
        transaction.commit().map_err(slice::db)?;
        Ok(Page { items, next_cursor })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lettuce_conversations::{
        ArtifactCodec, ArtifactRetention, CharacterLaunchSnapshot, ConversationArtifactStore,
        ConversationKind, ConversationOutboxEvent, ConversationParticipantDraft,
        DirectConversationDetails, MediaAssetRole, MessagePart, MessageRole, OperationToken,
        ParticipantRole, ParticipantSource, ProtectedArtifactBytes, ProtectedSnapshotRef,
        ReplayArtifactDraft, SceneLaunchSnapshot, SnapshotArtifactDraft, SnapshotSelection,
        SnapshotSource, StarterLaunchSnapshot,
    };
    use lettuce_media::{
        AssetKind, AssetOrigin, AssetProvenanceV1, BlobState, MediaAsset, MediaAssetRepository,
        MediaBlob, MediaBlobRepository, MediaKind, RetentionClass,
    };
    use lettuce_types::{
        AssetId, CharacterId, ContentHash, ConversationParticipantId, ConversationStarterId,
        MediaBlobId, MessageId, MessageRevisionId, OperationRecordId, OutboxEventId, Revision,
        SceneId, SnapshotArtifactId, StarterMessageId,
    };

    fn create_fixture(database: &Database, title: &str) -> ConversationId {
        let character_id = CharacterId::new();
        let bytes = ProtectedArtifactBytes::new(title.as_bytes().to_vec()).expect("bytes");
        let artifact_id = SnapshotArtifactId::new();
        let reference = ProtectedSnapshotRef {
            source: SnapshotSource::Character(character_id),
            source_revision: Revision::INITIAL,
            artifact_id,
            digest: bytes.digest(),
            schema_version: 1,
            byte_size: bytes.len() as u64,
        };
        database
            .put_snapshot(SnapshotArtifactDraft {
                source: reference.source,
                source_revision: reference.source_revision,
                artifact_id,
                digest: reference.digest.clone(),
                schema_version: 1,
                byte_size: reference.byte_size,
                codec: ArtifactCodec::Json,
                retention: ArtifactRetention::Conversation,
                bytes,
            })
            .expect("snapshot");
        let conversation_id = ConversationId::new();
        let plan = lettuce_conversations::CreateConversationPlan {
            conversation_id,
            title: title.to_owned(),
            kind: ConversationKind::Direct(DirectConversationDetails {
                format_version: 1,
                character: CharacterLaunchSnapshot {
                    snapshot_ref: reference,
                    source_id: character_id,
                    source_revision: Revision::INITIAL,
                    name: title.to_owned(),
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
                    display_name: title.into(),
                    authored_description: None,
                    model_selection: SnapshotSelection::Disabled,
                },
            ],
            initial_timeline: lettuce_conversations::InitialTimelineDraft {
                format_version: 1,
                entries: Vec::new(),
            },
            operation: OperationToken {
                key: lettuce_jobs::IdempotencyKey::new(format!("create-{}", conversation_id))
                    .expect("key"),
                request_digest: ContentHash::parse("ab".repeat(32)).expect("digest"),
            },
        };
        database
            .create_conversation_record(&plan, TimestampMillis::UNIX_EPOCH)
            .expect("create");
        conversation_id
    }

    fn put_launch_snapshot(
        database: &Database,
        source: SnapshotSource,
        label: &str,
    ) -> ProtectedSnapshotRef {
        let bytes = ProtectedArtifactBytes::new(label.as_bytes().to_vec()).expect("snapshot bytes");
        let reference = ProtectedSnapshotRef {
            source,
            source_revision: Revision::INITIAL,
            artifact_id: SnapshotArtifactId::new(),
            digest: bytes.digest(),
            schema_version: 1,
            byte_size: bytes.len() as u64,
        };
        database
            .put_snapshot(SnapshotArtifactDraft {
                source: reference.source,
                source_revision: reference.source_revision,
                artifact_id: reference.artifact_id,
                digest: reference.digest.clone(),
                schema_version: reference.schema_version,
                byte_size: reference.byte_size,
                codec: ArtifactCodec::Json,
                retention: ArtifactRetention::Conversation,
                bytes,
            })
            .expect("launch snapshot");
        reference
    }

    fn install_direct_launch_selection(
        database: &Database,
        conversation_id: ConversationId,
        scene: Option<&ProtectedSnapshotRef>,
        starter: Option<&ProtectedSnapshotRef>,
    ) {
        let mut kind = database
            .get_conversation_record(conversation_id)
            .expect("aggregate")
            .conversation
            .kind;
        let ConversationKind::Direct(details) = &mut kind else {
            panic!("direct fixture");
        };
        details.scene = scene
            .map(|reference| {
                let SnapshotSource::Scene(source_id) = reference.source else {
                    panic!("scene source");
                };
                SnapshotSelection::Explicit(SceneLaunchSnapshot {
                    snapshot_ref: reference.clone(),
                    source_id,
                    source_revision: reference.source_revision,
                    title: "Scene".into(),
                })
            })
            .unwrap_or(SnapshotSelection::Disabled);
        details.starter = starter
            .map(|reference| {
                let SnapshotSource::Starter(source_id) = reference.source else {
                    panic!("starter source");
                };
                SnapshotSelection::Explicit(StarterLaunchSnapshot {
                    snapshot_ref: reference.clone(),
                    source_id,
                    source_revision: reference.source_revision,
                    title: "Starter".into(),
                })
            })
            .unwrap_or(SnapshotSelection::Disabled);
        let kind_json = slice::encode(&kind).expect("kind json");
        let connection = database.connection().expect("connection");
        connection
            .execute(
                "UPDATE conversations SET kind_json = ?1 WHERE id = ?2",
                params![kind_json, conversation_id.to_string()],
            )
            .expect("selected launch snapshots");
        for reference in [scene, starter].into_iter().flatten() {
            connection
                .execute(
                    "INSERT INTO conversation_snapshot_refs (conversation_id, artifact_id) VALUES (?1, ?2)",
                    params![conversation_id.to_string(), reference.artifact_id.to_string()],
                )
                .expect("snapshot reference");
        }
    }

    struct TimelineMessageSpec {
        branch_id: lettuce_types::ConversationBranchId,
        role: MessageRole,
        author: Option<ConversationParticipantId>,
        parent: Option<MessageId>,
        ordinal: i64,
        text: &'static str,
    }

    fn insert_timeline_message(
        database: &Database,
        conversation_id: ConversationId,
        spec: TimelineMessageSpec,
    ) -> (MessageId, MessageRevisionId) {
        let message_id = MessageId::new();
        let revision_id = MessageRevisionId::new();
        let role = match spec.role {
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::System => "system",
            MessageRole::Scene => "scene",
        };
        let parts = slice::encode(&vec![MessagePart::Text {
            text: spec.text.to_owned(),
        }])
        .expect("message parts");
        let mut connection = database.connection().expect("connection");
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("message transaction");
        transaction
            .execute(
                "INSERT INTO conversation_message_revisions (conversation_id, id, message_id, branch_id, sequence, parts_json, authored_at) VALUES (?1, ?2, ?3, ?4, 1, ?5, 0)",
                params![conversation_id.to_string(), revision_id.to_string(), message_id.to_string(), spec.branch_id.to_string(), parts],
            )
            .expect("message revision");
        transaction
            .execute(
                "INSERT INTO conversation_messages (conversation_id, id, branch_id, parent_message_id, role, author_participant_id, timeline_ordinal, logical_time, effective_time, visibility, pinned, scene_edited, active_revision_id, revision, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, 0, 'visible', 0, 0, ?8, 1, 0, 0)",
                params![conversation_id.to_string(), message_id.to_string(), spec.branch_id.to_string(), spec.parent.map(|id| id.to_string()), role, spec.author.map(|id| id.to_string()), spec.ordinal, revision_id.to_string()],
            )
            .expect("message");
        transaction
            .execute(
                "UPDATE conversation_branches SET head_message_id = ?1 WHERE conversation_id = ?2 AND id = ?3",
                params![message_id.to_string(), conversation_id.to_string(), spec.branch_id.to_string()],
            )
            .expect("branch head");
        transaction.commit().expect("message commit");
        (message_id, revision_id)
    }

    fn insert_initial_origin(
        database: &Database,
        conversation_id: ConversationId,
        message_id: MessageId,
        reference: &ProtectedSnapshotRef,
        starter_message_id: Option<StarterMessageId>,
    ) {
        let source_kind = match reference.source {
            SnapshotSource::Scene(_) => "scene",
            SnapshotSource::Starter(_) => "starter",
            _ => panic!("initial origin source"),
        };
        database
            .connection()
            .expect("connection")
            .execute(
                "INSERT INTO conversation_initial_message_origins (conversation_id, message_id, snapshot_artifact_id, source_kind, starter_message_id) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![conversation_id.to_string(), message_id.to_string(), reference.artifact_id.to_string(), source_kind, starter_message_id.map(|id| id.to_string())],
            )
            .expect("initial origin");
    }

    fn create_content_fixture(
        database: &Database,
    ) -> (
        ConversationId,
        MessageRevisionId,
        MessageCandidateId,
        GenerationTurnId,
        GenerationTurnId,
    ) {
        let conversation_id = create_fixture(database, "content");
        let aggregate = database
            .get_conversation_record(conversation_id)
            .expect("aggregate");
        let branch_id = aggregate.conversation.active_branch_id;
        let character_id = match aggregate.conversation.kind {
            ConversationKind::Direct(details) => details.character.source_id,
            ConversationKind::Group(_) => unreachable!(),
        };
        let model_id = lettuce_types::ModelProfileId::new();
        let model_bytes = ProtectedArtifactBytes::new(b"model".to_vec()).expect("model bytes");
        let model_ref = ProtectedSnapshotRef {
            source: SnapshotSource::Model(model_id),
            source_revision: Revision::INITIAL,
            artifact_id: SnapshotArtifactId::new(),
            digest: model_bytes.digest(),
            schema_version: 1,
            byte_size: model_bytes.len() as u64,
        };
        database
            .put_snapshot(SnapshotArtifactDraft {
                source: model_ref.source,
                source_revision: model_ref.source_revision,
                artifact_id: model_ref.artifact_id,
                digest: model_ref.digest.clone(),
                schema_version: 1,
                byte_size: model_ref.byte_size,
                codec: ArtifactCodec::Json,
                retention: ArtifactRetention::Conversation,
                bytes: model_bytes,
            })
            .expect("model snapshot");
        let model = lettuce_conversations::ModelSelectionSnapshot {
            snapshot_ref: model_ref,
            source_id: model_id,
            source_revision: Revision::INITIAL,
            provider_kind: lettuce_conversations::ModelProviderKind::Other,
            external_model_id: "model".into(),
            display_name: "Model".into(),
            context_length: None,
            max_output_tokens: None,
        };
        let replay_bytes = ProtectedArtifactBytes::new(b"replay".to_vec()).expect("replay bytes");
        let replay_ref = database
            .put_replay(ReplayArtifactDraft {
                artifact_id: lettuce_types::ReplayArtifactId::new(),
                digest: replay_bytes.digest(),
                schema_version: 1,
                byte_size: replay_bytes.len() as u64,
                codec: ArtifactCodec::Json,
                retention: ArtifactRetention::Conversation,
                bytes: replay_bytes,
            })
            .expect("replay");
        let user_id = MessageId::new();
        let assistant_id = MessageId::new();
        let user_revision_id = MessageRevisionId::new();
        let assistant_revision_id = MessageRevisionId::new();
        let source_turn_id = GenerationTurnId::new();
        let source_attempt_id = GenerationAttemptId::new();
        let candidate_id = MessageCandidateId::new();
        let current_turn_id = GenerationTurnId::new();
        let current_attempt_id = GenerationAttemptId::new();
        let user_participant = aggregate
            .conversation
            .participants
            .iter()
            .find(|p| p.role == ParticipantRole::User)
            .expect("user")
            .id;
        let character_participant = aggregate
            .conversation
            .participants
            .iter()
            .find(|p| p.role == ParticipantRole::Character)
            .expect("character")
            .id;
        let parts = slice::encode(&vec![MessagePart::Text {
            text: "hello".into(),
        }])
        .expect("parts");
        let model_json = slice::encode(&model).expect("model json");
        let mut connection = database.connection().expect("connection");
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("transaction");
        transaction.execute("INSERT INTO conversation_message_revisions (conversation_id, id, message_id, branch_id, sequence, parts_json, authored_at, provider_replay_artifact_id, provider_replay_retention) VALUES (?1, ?2, ?3, ?4, 1, ?5, 0, ?6, 'conversation')", params![conversation_id.to_string(), user_revision_id.to_string(), user_id.to_string(), branch_id.to_string(), parts, replay_ref.artifact_id.to_string()]).expect("user revision");
        transaction.execute("INSERT INTO conversation_messages (conversation_id, id, branch_id, role, author_participant_id, timeline_ordinal, logical_time, effective_time, visibility, pinned, scene_edited, active_revision_id, revision, created_at, updated_at) VALUES (?1, ?2, ?3, 'user', ?4, 1, 0, 0, 'visible', 0, 0, ?5, 1, 0, 0)", params![conversation_id.to_string(), user_id.to_string(), branch_id.to_string(), user_participant.to_string(), user_revision_id.to_string()]).expect("user message");
        transaction.execute("INSERT INTO conversation_message_revisions (conversation_id, id, message_id, branch_id, sequence, parts_json, authored_at) VALUES (?1, ?2, ?3, ?4, 1, ?5, 0)", params![conversation_id.to_string(), assistant_revision_id.to_string(), assistant_id.to_string(), branch_id.to_string(), parts]).expect("assistant revision");
        transaction.execute("INSERT INTO conversation_messages (conversation_id, id, branch_id, parent_message_id, role, author_participant_id, timeline_ordinal, logical_time, effective_time, visibility, pinned, scene_edited, active_revision_id, revision, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, 'assistant', ?5, 2, 0, 0, 'visible', 0, 0, ?6, 1, 0, 0)", params![conversation_id.to_string(), assistant_id.to_string(), branch_id.to_string(), user_id.to_string(), character_participant.to_string(), assistant_revision_id.to_string()]).expect("assistant message");
        transaction.execute("UPDATE conversation_branches SET head_message_id = ?1 WHERE conversation_id = ?2 AND id = ?3", params![assistant_id.to_string(), conversation_id.to_string(), branch_id.to_string()]).expect("head");
        transaction.execute("INSERT INTO conversation_turns (conversation_id, id, branch_id, operation, input_kind, candidate_message_id, candidate_id, idempotency_key, status, target_kind, target_message_id, target_prior_candidate_id, revision, created_at, updated_at) VALUES (?1, ?2, ?3, 'regenerate', 'existing_candidate', ?4, ?5, ?6, 'created', 'existing_candidate', ?4, ?5, 1, 0, 0)", params![conversation_id.to_string(), source_turn_id.to_string(), branch_id.to_string(), assistant_id.to_string(), candidate_id.to_string(), format!("source-{source_turn_id}")]).expect("source turn");
        transaction.execute("INSERT INTO generation_attempts (conversation_id, turn_id, id, ordinal, status, job_idempotency_key) VALUES (?1, ?2, ?3, 0, 'created', ?4)", params![conversation_id.to_string(), source_turn_id.to_string(), source_attempt_id.to_string(), format!("generation.{source_turn_id}.{source_attempt_id}")]).expect("source attempt");
        transaction.execute("INSERT INTO conversation_message_candidates (conversation_id, id, message_id, branch_id, turn_id, attempt_id, ordinal, parts_json, model_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7, ?8, 0)", params![conversation_id.to_string(), candidate_id.to_string(), assistant_id.to_string(), branch_id.to_string(), source_turn_id.to_string(), source_attempt_id.to_string(), parts, model_json]).expect("candidate");
        transaction.execute("INSERT INTO conversation_turns (conversation_id, id, branch_id, operation, input_kind, candidate_message_id, candidate_id, idempotency_key, status, target_kind, target_message_id, target_prior_candidate_id, revision, created_at, updated_at) VALUES (?1, ?2, ?3, 'regenerate', 'existing_candidate', ?4, ?5, ?6, 'created', 'existing_candidate', ?4, ?5, 1, 0, 0)", params![conversation_id.to_string(), current_turn_id.to_string(), branch_id.to_string(), assistant_id.to_string(), candidate_id.to_string(), format!("current-{current_turn_id}")]).expect("current turn");
        transaction.execute("INSERT INTO generation_attempts (conversation_id, turn_id, id, ordinal, status, job_idempotency_key) VALUES (?1, ?2, ?3, 0, 'created', ?4)", params![conversation_id.to_string(), current_turn_id.to_string(), current_attempt_id.to_string(), format!("generation.{current_turn_id}.{current_attempt_id}")]).expect("current attempt");
        transaction.commit().expect("content commit");
        let _ = character_id;
        (
            conversation_id,
            user_revision_id,
            candidate_id,
            source_turn_id,
            current_turn_id,
        )
    }

    #[test]
    fn conversation_created_validation_is_historical_not_active_head_bound() {
        let database = Database::open_in_memory().expect("database");
        let conversation_id = create_fixture(&database, "historical-created");
        let aggregate = database
            .get_conversation_record(conversation_id)
            .expect("aggregate");
        let root_branch_id = aggregate.conversation.active_branch_id;
        let record = ConversationOutboxRecord {
            format_version: 1,
            id: OutboxEventId::new(),
            conversation_id,
            conversation_revision: Revision::INITIAL,
            sequence: 1,
            operation_record_id: OperationRecordId::new(),
            at: TimestampMillis::UNIX_EPOCH,
            event: ConversationOutboxEvent::ConversationCreated {
                conversation_id,
                root_branch_id,
                head_message_id: None,
                initial_message_count: 0,
                at: TimestampMillis::UNIX_EPOCH,
            },
        };
        let mut connection = database.connection().expect("connection");
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .expect("transaction");
        validate_outbox_event_exact(&transaction, &record).expect("historical root");
        drop(transaction);
    }

    #[test]
    fn conversation_created_validation_keeps_the_historical_initial_chain() {
        let database = Database::open_in_memory().expect("database");
        let conversation_id = create_fixture(&database, "historical-initial-chain");
        let scene_ref = put_launch_snapshot(
            &database,
            SnapshotSource::Scene(SceneId::new()),
            "scene snapshot",
        );
        let starter_ref = put_launch_snapshot(
            &database,
            SnapshotSource::Starter(ConversationStarterId::new()),
            "starter snapshot",
        );
        install_direct_launch_selection(
            &database,
            conversation_id,
            Some(&scene_ref),
            Some(&starter_ref),
        );
        let aggregate = database
            .get_conversation_record(conversation_id)
            .expect("aggregate");
        let root_branch_id = aggregate.conversation.active_branch_id;
        let user = aggregate
            .conversation
            .participants
            .iter()
            .find(|participant| participant.role == ParticipantRole::User)
            .expect("user")
            .id;
        let character = aggregate
            .conversation
            .participants
            .iter()
            .find(|participant| participant.role == ParticipantRole::Character)
            .expect("character")
            .id;
        let (scene, _) = insert_timeline_message(
            &database,
            conversation_id,
            TimelineMessageSpec {
                branch_id: root_branch_id,
                role: MessageRole::Scene,
                author: None,
                parent: None,
                ordinal: 1,
                text: "scene",
            },
        );
        insert_initial_origin(&database, conversation_id, scene, &scene_ref, None);
        let (starter, _) = insert_timeline_message(
            &database,
            conversation_id,
            TimelineMessageSpec {
                branch_id: root_branch_id,
                role: MessageRole::User,
                author: Some(user),
                parent: Some(scene),
                ordinal: 2,
                text: "starter",
            },
        );
        insert_initial_origin(
            &database,
            conversation_id,
            starter,
            &starter_ref,
            Some(StarterMessageId::new()),
        );

        let record = ConversationOutboxRecord {
            format_version: 1,
            id: OutboxEventId::new(),
            conversation_id,
            conversation_revision: Revision::INITIAL,
            sequence: 1,
            operation_record_id: OperationRecordId::new(),
            at: TimestampMillis::UNIX_EPOCH,
            event: ConversationOutboxEvent::ConversationCreated {
                conversation_id,
                root_branch_id,
                head_message_id: Some(starter),
                initial_message_count: 2,
                at: TimestampMillis::UNIX_EPOCH,
            },
        };
        let validate = |database: &Database| {
            let mut connection = database.connection().expect("connection");
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Deferred)
                .expect("transaction");
            validate_outbox_event_exact(&transaction, &record).expect("historical event");
            transaction.commit().expect("validation commit");
        };
        validate(&database);

        // Model a corrupt historical row with the scene in the middle of the
        // chain. The normal topology triggers reject this write; the exact
        // creation-event validator must still reject the raw state.
        database
            .connection()
            .expect("connection")
            .execute_batch(
                "DROP TRIGGER IF EXISTS conversation_message_identity_immutable;
                 DROP TRIGGER IF EXISTS conversation_message_parent_topology_update;",
            )
            .expect("disable topology guards");
        database
            .connection()
            .expect("connection")
            .execute(
                "UPDATE conversation_messages SET parent_message_id = ?1 WHERE conversation_id = ?2 AND id = ?3",
                params![starter.to_string(), conversation_id.to_string(), scene.to_string()],
            )
            .expect("scene child");
        database
            .connection()
            .expect("connection")
            .execute(
                "UPDATE conversation_branches SET head_message_id = ?1 WHERE conversation_id = ?2 AND id = ?3",
                params![scene.to_string(), conversation_id.to_string(), root_branch_id.to_string()],
            )
            .expect("corrupt head");
        let corrupt_record = ConversationOutboxRecord {
            event: ConversationOutboxEvent::ConversationCreated {
                conversation_id,
                root_branch_id,
                head_message_id: Some(scene),
                initial_message_count: 2,
                at: TimestampMillis::UNIX_EPOCH,
            },
            ..record.clone()
        };
        let mut connection = database.connection().expect("connection");
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .expect("transaction");
        assert_eq!(
            validate_outbox_event_exact(&transaction, &corrupt_record),
            Err(ConversationRepositoryError::Storage)
        );
        transaction.rollback().expect("validation rollback");
        drop(connection);
        database
            .connection()
            .expect("connection")
            .execute(
                "UPDATE conversation_messages SET parent_message_id = NULL WHERE conversation_id = ?1 AND id = ?2",
                params![conversation_id.to_string(), scene.to_string()],
            )
            .expect("restore scene root");
        database
            .connection()
            .expect("connection")
            .execute(
                "UPDATE conversation_branches SET head_message_id = ?1 WHERE conversation_id = ?2 AND id = ?3",
                params![starter.to_string(), conversation_id.to_string(), root_branch_id.to_string()],
            )
            .expect("restore head");

        // A later root append changes the current root head but must not
        // invalidate the old creation event's recorded head and count.
        insert_timeline_message(
            &database,
            conversation_id,
            TimelineMessageSpec {
                branch_id: root_branch_id,
                role: MessageRole::Assistant,
                author: Some(character),
                parent: Some(starter),
                ordinal: 3,
                text: "later root message",
            },
        );
        validate(&database);

        // Selecting a child branch also must not change the historical root
        // chain represented by ConversationCreated.
        let child_branch_id = lettuce_types::ConversationBranchId::new();
        let connection = database.connection().expect("connection");
        connection
            .execute(
                "INSERT INTO conversation_branches (conversation_id, id, parent_branch_id, fork_message_id, status, revision, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, 'active', 1, 0, 0)",
                params![conversation_id.to_string(), child_branch_id.to_string(), root_branch_id.to_string(), starter.to_string()],
            )
            .expect("child branch");
        drop(connection);
        insert_timeline_message(
            &database,
            conversation_id,
            TimelineMessageSpec {
                branch_id: child_branch_id,
                role: MessageRole::Assistant,
                author: Some(character),
                parent: Some(starter),
                ordinal: 4,
                text: "child message",
            },
        );
        database
            .connection()
            .expect("connection")
            .execute(
                "UPDATE conversations SET active_branch_id = ?1 WHERE id = ?2",
                params![child_branch_id.to_string(), conversation_id.to_string()],
            )
            .expect("select child branch");
        validate(&database);

        // The normal topology triggers make this impossible through the
        // write path. Simulate a legacy/corrupt file by removing only those
        // guards and forming a two-message cycle; the bounded recursive query
        // must fail closed instead of hanging or accepting a partial chain.
        database
            .connection()
            .expect("connection")
            .execute_batch(
                "DROP TRIGGER IF EXISTS conversation_message_identity_immutable;
                 DROP TRIGGER IF EXISTS conversation_message_parent_topology_update;",
            )
            .expect("disable topology guards");
        database
            .connection()
            .expect("connection")
            .execute(
                "UPDATE conversation_messages SET parent_message_id = ?1 WHERE conversation_id = ?2 AND id = ?3",
                params![starter.to_string(), conversation_id.to_string(), scene.to_string()],
            )
            .expect("cycle first edge");
        database
            .connection()
            .expect("connection")
            .execute(
                "UPDATE conversation_messages SET parent_message_id = ?1 WHERE conversation_id = ?2 AND id = ?3",
                params![scene.to_string(), conversation_id.to_string(), starter.to_string()],
            )
            .expect("cycle second edge");
        let mut connection = database.connection().expect("connection");
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .expect("transaction");
        assert_eq!(
            validate_outbox_event_exact(&transaction, &record),
            Err(ConversationRepositoryError::Storage)
        );
    }

    #[test]
    fn created_chain_terminal_root_check_handles_the_512_row_boundary() {
        let mut cyclic_chain = Vec::with_capacity(512);
        for index in 0..512 {
            cyclic_chain.push((
                format!("message-{index}"),
                Some(format!("message-{}", (index + 1) % 512)),
                i64::from(index + 1),
            ));
        }
        assert!(!super::has_terminal_root(&cyclic_chain));

        cyclic_chain[511].1 = None;
        assert!(super::has_terminal_root(&cyclic_chain));
    }

    fn assert_timeline_is_storage_corruption(
        database: &Database,
        conversation_id: ConversationId,
        branch_id: lettuce_types::ConversationBranchId,
    ) {
        assert_eq!(
            ConversationReader::timeline_page(
                database,
                conversation_id,
                branch_id,
                &PageRequest::default(),
            ),
            Err(ConversationRepositoryError::Storage)
        );
    }

    #[test]
    fn timeline_hydration_rejects_corrupt_initial_origin_shapes() {
        // A scene message is always an initial message and therefore must
        // carry exactly one selected-scene origin.
        let database = Database::open_in_memory().expect("database");
        let conversation_id = create_fixture(&database, "scene-without-origin");
        let scene_ref =
            put_launch_snapshot(&database, SnapshotSource::Scene(SceneId::new()), "scene");
        install_direct_launch_selection(&database, conversation_id, Some(&scene_ref), None);
        let branch_id = database
            .get_conversation_record(conversation_id)
            .expect("aggregate")
            .conversation
            .active_branch_id;
        insert_timeline_message(
            &database,
            conversation_id,
            TimelineMessageSpec {
                branch_id,
                role: MessageRole::Scene,
                author: None,
                parent: None,
                ordinal: 1,
                text: "scene",
            },
        );
        assert_timeline_is_storage_corruption(&database, conversation_id, branch_id);

        // A selected-scene origin cannot be attached to an ordinary chat
        // message, even when SQLite's role/author constraints accept it.
        let database = Database::open_in_memory().expect("database");
        let conversation_id = create_fixture(&database, "scene-origin-on-user");
        let scene_ref =
            put_launch_snapshot(&database, SnapshotSource::Scene(SceneId::new()), "scene");
        install_direct_launch_selection(&database, conversation_id, Some(&scene_ref), None);
        let aggregate = database
            .get_conversation_record(conversation_id)
            .expect("aggregate");
        let branch_id = aggregate.conversation.active_branch_id;
        let user = aggregate
            .conversation
            .participants
            .iter()
            .find(|participant| participant.role == ParticipantRole::User)
            .expect("user")
            .id;
        let (message_id, _) = insert_timeline_message(
            &database,
            conversation_id,
            TimelineMessageSpec {
                branch_id,
                role: MessageRole::User,
                author: Some(user),
                parent: None,
                ordinal: 1,
                text: "not a scene",
            },
        );
        insert_initial_origin(&database, conversation_id, message_id, &scene_ref, None);
        assert_timeline_is_storage_corruption(&database, conversation_id, branch_id);
    }

    #[test]
    fn timeline_hydration_rejects_corrupt_starter_origin_ownership_and_selection() {
        // Starter origins are never valid on scene or system messages.
        for role in [MessageRole::Scene, MessageRole::System] {
            let database = Database::open_in_memory().expect("database");
            let conversation_id = create_fixture(&database, "starter-origin-non-chat");
            let starter_ref = put_launch_snapshot(
                &database,
                SnapshotSource::Starter(ConversationStarterId::new()),
                "starter",
            );
            install_direct_launch_selection(&database, conversation_id, None, Some(&starter_ref));
            let branch_id = database
                .get_conversation_record(conversation_id)
                .expect("aggregate")
                .conversation
                .active_branch_id;
            let (message_id, _) = insert_timeline_message(
                &database,
                conversation_id,
                TimelineMessageSpec {
                    branch_id,
                    role,
                    author: None,
                    parent: None,
                    ordinal: 1,
                    text: "not a starter chat message",
                },
            );
            insert_initial_origin(
                &database,
                conversation_id,
                message_id,
                &starter_ref,
                Some(StarterMessageId::new()),
            );
            assert_timeline_is_storage_corruption(&database, conversation_id, branch_id);
        }

        // Bypass the author-role trigger to model a corrupt legacy row where
        // a user-role starter was assigned to a character participant.
        let database = Database::open_in_memory().expect("database");
        let conversation_id = create_fixture(&database, "starter-origin-wrong-author");
        let starter_ref = put_launch_snapshot(
            &database,
            SnapshotSource::Starter(ConversationStarterId::new()),
            "starter",
        );
        install_direct_launch_selection(&database, conversation_id, None, Some(&starter_ref));
        let aggregate = database
            .get_conversation_record(conversation_id)
            .expect("aggregate");
        let branch_id = aggregate.conversation.active_branch_id;
        let user = aggregate
            .conversation
            .participants
            .iter()
            .find(|participant| participant.role == ParticipantRole::User)
            .expect("user")
            .id;
        let character = aggregate
            .conversation
            .participants
            .iter()
            .find(|participant| participant.role == ParticipantRole::Character)
            .expect("character")
            .id;
        let (message_id, _) = insert_timeline_message(
            &database,
            conversation_id,
            TimelineMessageSpec {
                branch_id,
                role: MessageRole::User,
                author: Some(user),
                parent: None,
                ordinal: 1,
                text: "wrong author",
            },
        );
        insert_initial_origin(
            &database,
            conversation_id,
            message_id,
            &starter_ref,
            Some(StarterMessageId::new()),
        );
        database
            .connection()
            .expect("connection")
            .execute_batch("DROP TRIGGER conversation_message_author_role_update;")
            .expect("disable author guard");
        database
            .connection()
            .expect("connection")
            .execute(
                "UPDATE conversation_messages SET author_participant_id = ?1 WHERE conversation_id = ?2 AND id = ?3",
                params![character.to_string(), conversation_id.to_string(), message_id.to_string()],
            )
            .expect("wrong author");
        assert_timeline_is_storage_corruption(&database, conversation_id, branch_id);

        // The origin artifact must equal the selected launch snapshot, not
        // merely have the same source kind.
        let database = Database::open_in_memory().expect("database");
        let conversation_id = create_fixture(&database, "origin-selection-mismatch");
        let selected_scene = put_launch_snapshot(
            &database,
            SnapshotSource::Scene(SceneId::new()),
            "selected scene",
        );
        let foreign_scene = put_launch_snapshot(
            &database,
            SnapshotSource::Scene(SceneId::new()),
            "foreign scene",
        );
        install_direct_launch_selection(&database, conversation_id, Some(&selected_scene), None);
        let branch_id = database
            .get_conversation_record(conversation_id)
            .expect("aggregate")
            .conversation
            .active_branch_id;
        database
            .connection()
            .expect("connection")
            .execute(
                "INSERT INTO conversation_snapshot_refs (conversation_id, artifact_id) VALUES (?1, ?2)",
                params![conversation_id.to_string(), foreign_scene.artifact_id.to_string()],
            )
            .expect("foreign snapshot reference");
        let (message_id, _) = insert_timeline_message(
            &database,
            conversation_id,
            TimelineMessageSpec {
                branch_id,
                role: MessageRole::Scene,
                author: None,
                parent: None,
                ordinal: 1,
                text: "foreign scene",
            },
        );
        insert_initial_origin(&database, conversation_id, message_id, &foreign_scene, None);
        assert_timeline_is_storage_corruption(&database, conversation_id, branch_id);
    }

    #[test]
    fn reader_aggregate_and_terminal_keyset_page_round_trip() {
        let database = Database::open_in_memory().expect("database");
        let first = create_fixture(&database, "first");
        let second = create_fixture(&database, "second");
        let loaded = ConversationReader::get(&database, first).expect("aggregate");
        assert_eq!(loaded.conversation.id, first);
        let first_page = ConversationReader::page(
            &database,
            &ConversationQuery {
                lifecycle: None,
                page: PageRequest {
                    cursor: None,
                    limit: lettuce_types::PageLimit::new(1),
                },
            },
        )
        .expect("page");
        assert_eq!(first_page.items.len(), 1);
        let cursor = first_page.next_cursor.clone().expect("continuation");
        let second_page = ConversationReader::page(
            &database,
            &ConversationQuery {
                lifecycle: None,
                page: PageRequest {
                    cursor: Some(cursor),
                    limit: lettuce_types::PageLimit::new(1),
                },
            },
        )
        .expect("second page");
        assert_eq!(second_page.items.len(), 1);
        assert!(second_page.next_cursor.is_none());
        assert!(second_page.items[0].id == first || second_page.items[0].id == second);
    }

    #[test]
    fn reader_rejects_scope_mismatched_cursor() {
        let database = Database::open_in_memory().expect("database");
        create_fixture(&database, "cursor");
        create_fixture(&database, "cursor-2");
        let page = ConversationReader::page(
            &database,
            &ConversationQuery {
                lifecycle: None,
                page: PageRequest {
                    cursor: None,
                    limit: lettuce_types::PageLimit::new(1),
                },
            },
        )
        .expect("page");
        let cursor = page.next_cursor.expect("cursor");
        let error = ConversationReader::page(
            &database,
            &ConversationQuery {
                lifecycle: Some(ConversationLifecycle::Active),
                page: PageRequest {
                    cursor: Some(cursor),
                    limit: lettuce_types::PageLimit::new(1),
                },
            },
        )
        .expect_err("scope mismatch");
        assert!(matches!(error, ConversationRepositoryError::Invalid(_)));
        let malformed = ConversationReader::page(
            &database,
            &ConversationQuery {
                lifecycle: None,
                page: PageRequest {
                    cursor: Some("not-hex".into()),
                    limit: lettuce_types::PageLimit::new(1),
                },
            },
        )
        .expect_err("malformed cursor");
        assert!(matches!(malformed, ConversationRepositoryError::Invalid(_)));
    }

    #[test]
    fn reader_missing_rows_are_not_found() {
        let database = Database::open_in_memory().expect("database");
        assert_eq!(
            ConversationReader::get(&database, ConversationId::new()),
            Err(ConversationRepositoryError::NotFound)
        );
        assert_eq!(
            ConversationReader::get_message_revision(&database, MessageRevisionId::new()),
            Err(ConversationRepositoryError::NotFound)
        );
        assert_eq!(
            ConversationReader::get_candidate(&database, MessageCandidateId::new()),
            Err(ConversationRepositoryError::NotFound)
        );
        assert_eq!(
            ConversationReader::get_turn(&database, GenerationTurnId::new()),
            Err(ConversationRepositoryError::NotFound)
        );
    }

    #[test]
    fn reader_hydrates_content_turn_variants_and_replay_fail_closed() {
        let database = Database::open_in_memory().expect("database");
        let (conversation_id, revision_id, candidate_id, source_id, current_id) =
            create_content_fixture(&database);
        let revision =
            ConversationReader::get_message_revision(&database, revision_id).expect("revision");
        assert_eq!(revision.id, revision_id);
        assert!(revision.provider_replay.is_some());
        assert_eq!(
            ConversationReader::page_message_revisions(
                &database,
                revision.message_id,
                &PageRequest::default()
            )
            .expect("revision page")
            .items
            .len(),
            1
        );
        let candidate =
            ConversationReader::get_candidate(&database, candidate_id).expect("candidate");
        assert_eq!(candidate.id, candidate_id);
        assert_eq!(
            ConversationReader::page_candidates(
                &database,
                candidate.message_id,
                &PageRequest::default()
            )
            .expect("candidate page")
            .items
            .len(),
            1
        );
        assert!(matches!(
            ConversationReader::get_turn(&database, source_id).expect("source"),
            GenerationTurn { .. }
        ));
        assert!(matches!(
            ConversationReader::get_turn(&database, current_id).expect("current"),
            GenerationTurn { .. }
        ));
        let timeline = ConversationReader::timeline_page(
            &database,
            conversation_id,
            database
                .get_conversation_record(conversation_id)
                .expect("aggregate")
                .conversation
                .active_branch_id,
            &PageRequest {
                cursor: None,
                limit: lettuce_types::PageLimit::new(1),
            },
        )
        .expect("timeline");
        assert_eq!(timeline.items.len(), 1);
        assert_eq!(timeline.items[0].message.id, candidate.message_id);
        let next = timeline.next_cursor.expect("older cursor");
        let older = ConversationReader::timeline_page(
            &database,
            conversation_id,
            timeline.selected_branch_id,
            &PageRequest {
                cursor: Some(next),
                limit: lettuce_types::PageLimit::new(1),
            },
        )
        .expect("older timeline");
        assert_eq!(older.items.len(), 1);
        assert!(older.next_cursor.is_none());
        database.connection().expect("connection").execute("UPDATE conversation_replay_artifacts SET bytes = ?1 WHERE artifact_id = (SELECT provider_replay_artifact_id FROM conversation_message_revisions WHERE id = ?2)", params![b"broken".to_vec(), revision_id.to_string()]).expect("tamper");
        assert_eq!(
            ConversationReader::get_message_revision(&database, revision_id),
            Err(ConversationRepositoryError::Storage)
        );
    }

    #[test]
    fn reader_rejects_a_corrupt_retry_cycle_without_recursive_overflow() {
        let database = Database::open_in_memory().expect("database");
        let (_conversation_id, _revision_id, _candidate_id, source_turn, current_turn) =
            create_content_fixture(&database);
        let connection = database.connection().expect("connection");
        connection
            .execute_batch("DROP TRIGGER conversation_turn_final_update_contract; DROP TRIGGER conversation_turn_final_intent_immutable")
            .expect("disable mutation guard for corruption fixture");
        connection
            .execute(
                "UPDATE conversation_turns SET retry_of_turn_id = ?1 WHERE id = ?2",
                params![source_turn.to_string(), current_turn.to_string()],
            )
            .expect("first cycle edge");
        connection
            .execute(
                "UPDATE conversation_turns SET retry_of_turn_id = ?1 WHERE id = ?2",
                params![current_turn.to_string(), source_turn.to_string()],
            )
            .expect("second cycle edge");
        drop(connection);
        assert_eq!(
            ConversationReader::get_turn(&database, current_turn),
            Err(ConversationRepositoryError::Storage)
        );
    }

    #[test]
    fn reader_requires_exact_media_reference_projections() {
        let database = Database::open_in_memory().expect("database");
        let (_conversation_id, revision_id, candidate_id, _source_id, _current_id) =
            create_content_fixture(&database);
        let blob_id = MediaBlobId::new();
        let blob = MediaBlob {
            id: blob_id,
            content_hash: ContentHash::parse("c1".repeat(32)).expect("hash"),
            kind: MediaKind::Image,
            mime_type: "image/png".into(),
            byte_size: 4,
            width: Some(1),
            height: Some(1),
            duration_ms: None,
            validation_version: 1,
            state: BlobState::Ready,
            created_at: TimestampMillis::new(1),
            updated_at: TimestampMillis::new(1),
        };
        MediaBlobRepository::register(&database, blob).expect("blob");
        let asset_id = AssetId::new();
        let asset = MediaAsset::new(
            asset_id,
            blob_id,
            AssetKind::MessageImage,
            AssetOrigin::Upload,
            RetentionClass::Persistent,
            AssetProvenanceV1::default(),
            Revision::INITIAL,
            TimestampMillis::new(1),
            TimestampMillis::new(1),
        )
        .expect("asset");
        MediaAssetRepository::create(&database, asset).expect("asset");
        let media_parts = slice::encode(&vec![MessagePart::MediaAsset {
            asset_id,
            role: MediaAssetRole::Inline,
        }])
        .expect("media parts");
        let mut connection = database.connection().expect("connection");
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("transaction");
        transaction
            .execute(
                "UPDATE conversation_message_revisions SET parts_json = ?1 WHERE id = ?2",
                params![media_parts, revision_id.to_string()],
            )
            .expect("revision parts");
        transaction
            .execute(
                "INSERT INTO revision_media_refs (conversation_id, message_revision_id, part_ordinal, asset_id, media_role, state, created_at) SELECT conversation_id, ?1, 0, ?2, 'inline', 'active', 1 FROM conversation_message_revisions WHERE id = ?1",
                params![revision_id.to_string(), asset_id.to_string()],
            )
            .expect("revision ref");
        transaction.commit().expect("commit");
        drop(connection);
        let revision = ConversationReader::get_message_revision(&database, revision_id)
            .expect("media revision");
        assert_eq!(
            revision.parts,
            vec![MessagePart::MediaAsset {
                asset_id,
                role: MediaAssetRole::Inline
            }]
        );

        let candidate_parts = slice::encode(&vec![MessagePart::MediaAsset {
            asset_id,
            role: MediaAssetRole::Reference,
        }])
        .expect("candidate media parts");
        let mut connection = database.connection().expect("connection");
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("transaction");
        transaction
            .execute(
                "UPDATE conversation_message_candidates SET parts_json = ?1 WHERE id = ?2",
                params![candidate_parts, candidate_id.to_string()],
            )
            .expect("candidate parts");
        let conversation_id: String = transaction
            .query_row(
                "SELECT conversation_id FROM conversation_message_candidates WHERE id = ?1",
                [candidate_id.to_string()],
                |row| row.get(0),
            )
            .expect("candidate conversation");
        transaction
            .execute(
                "INSERT INTO candidate_media_refs (conversation_id, candidate_id, part_ordinal, asset_id, media_role, state, created_at) VALUES (?1, ?2, 0, ?3, 'reference', 'active', 1)",
                params![conversation_id, candidate_id.to_string(), asset_id.to_string()],
            )
            .expect("candidate ref");
        transaction.commit().expect("commit");
        drop(connection);
        let candidate =
            ConversationReader::get_candidate(&database, candidate_id).expect("media candidate");
        assert_eq!(
            candidate.parts,
            vec![MessagePart::MediaAsset {
                asset_id,
                role: MediaAssetRole::Reference
            }]
        );

        let connection = database.connection().expect("connection");
        connection
            .execute(
                "UPDATE revision_media_refs SET media_role = 'attachment' WHERE message_revision_id = ?1",
                [revision_id.to_string()],
            )
            .expect("tamper media role");
        drop(connection);
        assert_eq!(
            ConversationReader::get_message_revision(&database, revision_id),
            Err(ConversationRepositoryError::Storage)
        );
    }

    #[test]
    fn reader_rejects_duplicate_ids_from_non_unique_global_indexes() {
        let database = Database::open_in_memory().expect("database");
        let (first_conversation, first_revision, first_candidate, _first_source_turn, first_turn) =
            create_content_fixture(&database);
        let (
            second_conversation,
            _second_revision,
            second_candidate,
            _second_source_turn,
            second_turn,
        ) = create_content_fixture(&database);
        let second_assistant_revision: MessageRevisionId = database
            .connection()
            .expect("connection")
            .query_row(
                "SELECT revision.id FROM conversation_message_revisions AS revision JOIN conversation_messages AS message ON message.conversation_id = revision.conversation_id AND message.id = revision.message_id WHERE revision.conversation_id = ?1 AND message.role = 'assistant' LIMIT 1",
                [second_conversation.to_string()],
                |row| row.get::<_, String>(0),
            )
            .expect("assistant revision")
            .parse()
            .expect("assistant revision id");
        let second_branch = database
            .get_conversation_record(second_conversation)
            .expect("second aggregate")
            .conversation
            .active_branch_id;
        let mut connection = database.connection().expect("connection");
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("transaction");
        let first_message: MessageId = transaction
            .query_row(
                "SELECT id FROM conversation_messages WHERE conversation_id = ?1 AND role = 'user' LIMIT 1",
                [first_conversation.to_string()],
                |row| row.get::<_, String>(0),
            )
            .expect("first message")
            .parse()
            .expect("first message id");
        let second_character: String = transaction
            .query_row(
                "SELECT id FROM conversation_participants WHERE conversation_id = ?1 AND role = 'character' LIMIT 1",
                [second_conversation.to_string()],
                |row| row.get(0),
            )
            .expect("second character");
        let duplicate_parent_revision = MessageRevisionId::new();
        let parts = slice::encode(&vec![MessagePart::Text {
            text: "duplicate parent".into(),
        }])
        .expect("parts");
        transaction
            .execute(
                "INSERT INTO conversation_message_revisions (conversation_id, id, message_id, branch_id, sequence, parts_json, authored_at) VALUES (?1, ?2, ?3, ?4, 1, ?5, 0)",
                params![second_conversation.to_string(), duplicate_parent_revision.to_string(), first_message.to_string(), second_branch.to_string(), parts],
            )
            .expect("duplicate parent revision");
        transaction
            .execute(
                "INSERT INTO conversation_messages (conversation_id, id, branch_id, role, author_participant_id, timeline_ordinal, logical_time, effective_time, visibility, pinned, scene_edited, active_revision_id, revision, created_at, updated_at) VALUES (?1, ?2, ?3, 'assistant', ?4, 3, 0, 0, 'visible', 0, 0, ?5, 1, 0, 0)",
                params![second_conversation.to_string(), first_message.to_string(), second_branch.to_string(), second_character, duplicate_parent_revision.to_string()],
            )
            .expect("duplicate parent message");
        transaction
            .execute(
                "INSERT INTO conversation_message_revisions (conversation_id, id, message_id, branch_id, sequence, parts_json, authored_at, provider_replay_artifact_id, provider_replay_retention, source_turn_id) SELECT conversation_id, ?1, message_id, branch_id, sequence + 1, parts_json, authored_at, provider_replay_artifact_id, provider_replay_retention, source_turn_id FROM conversation_message_revisions WHERE conversation_id = ?2 AND id = ?3",
                params![first_revision.to_string(), second_conversation.to_string(), second_assistant_revision.to_string()],
            )
            .expect("duplicate revision");
        transaction
            .execute(
                "INSERT INTO conversation_message_candidates (conversation_id, id, message_id, branch_id, turn_id, attempt_id, ordinal, parts_json, model_json, created_at, provider_replay_artifact_id, provider_replay_retention) SELECT conversation_id, ?1, message_id, branch_id, turn_id, attempt_id, ordinal + 1, parts_json, model_json, created_at, provider_replay_artifact_id, provider_replay_retention FROM conversation_message_candidates WHERE conversation_id = ?2 AND id = ?3",
                params![first_candidate.to_string(), second_conversation.to_string(), second_candidate.to_string()],
            )
            .expect("duplicate candidate");
        transaction
            .execute(
                "INSERT INTO conversation_turns (conversation_id, id, branch_id, operation, input_kind, candidate_message_id, candidate_id, idempotency_key, status, target_kind, target_message_id, target_prior_candidate_id, revision, created_at, updated_at) SELECT conversation_id, ?1, branch_id, operation, input_kind, candidate_message_id, candidate_id, ?2, status, target_kind, target_message_id, target_prior_candidate_id, revision, created_at, updated_at FROM conversation_turns WHERE conversation_id = ?3 AND id = ?4",
                params![first_turn.to_string(), format!("duplicate-{first_turn}"), second_conversation.to_string(), second_turn.to_string()],
            )
            .expect("duplicate turn");
        transaction.commit().expect("commit");
        drop(connection);
        assert_eq!(
            ConversationReader::page_message_revisions(
                &database,
                first_message,
                &PageRequest::default()
            ),
            Err(ConversationRepositoryError::Storage)
        );
        assert_eq!(
            ConversationReader::page_candidates(&database, first_message, &PageRequest::default()),
            Err(ConversationRepositoryError::Storage)
        );
        assert_eq!(
            ConversationReader::get_message_revision(&database, first_revision),
            Err(ConversationRepositoryError::Storage)
        );
        assert_eq!(
            ConversationReader::get_candidate(&database, first_candidate),
            Err(ConversationRepositoryError::Storage)
        );
        assert_eq!(
            ConversationReader::get_turn(&database, first_turn),
            Err(ConversationRepositoryError::Storage)
        );
    }

    #[test]
    fn reader_validates_operation_digest_projection_and_outbox_pages() {
        let database = Database::open_in_memory().expect("database");
        let conversation_id = create_fixture(&database, "operations");
        let foreign_conversation_id = create_fixture(&database, "foreign-operation");
        let operation_id = OperationRecordId::new();
        let token = OperationToken {
            key: lettuce_jobs::IdempotencyKey::new("query-operation").expect("key"),
            request_digest: ContentHash::parse("d1".repeat(32)).expect("digest"),
        };
        let result_json =
            slice::encode(&OperationResultRef::Conversation(conversation_id)).expect("result");
        let connection = database.connection().expect("connection");
        connection
            .execute(
                "INSERT INTO conversation_operations (id, conversation_id, kind, operation_key, request_digest, result_kind, result_id, result_json, created_at) VALUES (?1, ?2, 'send', ?3, ?4, 'conversation', ?5, ?6, 1)",
                params![operation_id.to_string(), conversation_id.to_string(), token.key.as_str(), token.request_digest.as_str(), conversation_id.to_string(), result_json],
            )
            .expect("operation");
        drop(connection);
        let operation = ConversationReader::operation_record(
            &database,
            conversation_id,
            OperationKind::Send,
            &token,
        )
        .expect("operation read")
        .expect("operation");
        assert_eq!(
            operation.result,
            OperationResultRef::Conversation(conversation_id)
        );
        let conflicting = OperationToken {
            key: token.key.clone(),
            request_digest: ContentHash::parse("d2".repeat(32)).expect("digest"),
        };
        assert_eq!(
            ConversationReader::operation_record(
                &database,
                conversation_id,
                OperationKind::Send,
                &conflicting
            ),
            Err(ConversationRepositoryError::Conflict)
        );
        let foreign_result =
            slice::encode(&OperationResultRef::Conversation(foreign_conversation_id))
                .expect("foreign result");
        let connection = database.connection().expect("connection");
        connection
            .execute(
                "UPDATE conversation_operations SET result_id = ?1, result_json = ?2 WHERE id = ?3",
                params![
                    foreign_conversation_id.to_string(),
                    foreign_result,
                    operation_id.to_string()
                ],
            )
            .expect("foreign result");
        drop(connection);
        assert_eq!(
            ConversationReader::operation_record(
                &database,
                conversation_id,
                OperationKind::Send,
                &token
            ),
            Err(ConversationRepositoryError::Storage)
        );

        let event = ConversationOutboxEvent::ConversationTombstoned {
            conversation_id,
            at: TimestampMillis::new(2),
        };
        let event_json = slice::encode(&event).expect("event");
        let connection = database.connection().expect("connection");
        for sequence in [1_i64, 2_i64] {
            let current_operation_id = if sequence == 1 {
                operation_id
            } else {
                OperationRecordId::new()
            };
            if sequence == 2 {
                connection
                    .execute(
                        "INSERT INTO conversation_operations (id, conversation_id, kind, operation_key, request_digest, result_kind, result_id, result_json, created_at) VALUES (?1, ?2, 'send', ?3, ?4, 'conversation', ?5, ?6, 1)",
                        params![current_operation_id.to_string(), conversation_id.to_string(), format!("query-operation-{sequence}"), "e1".repeat(32), conversation_id.to_string(), slice::encode(&OperationResultRef::Conversation(conversation_id)).expect("result")],
                    )
                    .expect("second operation");
            }
            connection
                .execute(
                    "INSERT INTO conversation_outbox (conversation_id, id, sequence, conversation_revision, operation_record_id, at, event_json) VALUES (?1, ?2, ?3, 1, ?4, ?5, ?6)",
                    params![conversation_id.to_string(), OutboxEventId::new().to_string(), sequence, current_operation_id.to_string(), 2_i64, event_json],
                )
                .expect("outbox");
        }
        drop(connection);
        let first = ConversationReader::page_outbox(
            &database,
            conversation_id,
            &PageRequest {
                cursor: None,
                limit: lettuce_types::PageLimit::new(1),
            },
        )
        .expect("outbox page");
        assert_eq!(first.items.len(), 1);
        let cursor = first.next_cursor.clone().expect("outbox cursor");
        let second = ConversationReader::page_outbox(
            &database,
            conversation_id,
            &PageRequest {
                cursor: Some(cursor),
                limit: lettuce_types::PageLimit::new(1),
            },
        )
        .expect("second outbox page");
        assert_eq!(second.items.len(), 1);
        assert!(second.next_cursor.is_none());
        let connection = database.connection().expect("connection");
        connection
            .execute(
                "UPDATE conversation_outbox SET at = 3 WHERE conversation_id = ?1 AND sequence = 1",
                [conversation_id.to_string()],
            )
            .expect("tamper outbox timestamp");
        drop(connection);
        assert_eq!(
            ConversationReader::page_outbox(&database, conversation_id, &PageRequest::default()),
            Err(ConversationRepositoryError::Storage)
        );
        let connection = database.connection().expect("connection");
        connection
            .execute(
                "UPDATE conversation_operations SET result_id = 'wrong' WHERE id = ?1",
                [operation_id.to_string()],
            )
            .expect("tamper result id");
        drop(connection);
        assert_eq!(
            ConversationReader::operation_record(
                &database,
                conversation_id,
                OperationKind::Send,
                &token
            ),
            Err(ConversationRepositoryError::Storage)
        );
    }

    #[test]
    fn timeline_follows_exact_selected_head_ancestry_across_forks() {
        let database = Database::open_in_memory().expect("database");
        let (conversation_id, _revision_id, _candidate_id, _source_id, _current_id) =
            create_content_fixture(&database);
        let aggregate = database
            .get_conversation_record(conversation_id)
            .expect("aggregate");
        let root_branch_id = aggregate
            .branches
            .iter()
            .find(|branch| branch.parent_branch_id.is_none())
            .expect("root")
            .id;
        let assistant_id: MessageId = database
            .connection()
            .expect("connection")
            .query_row(
                "SELECT id FROM conversation_messages WHERE conversation_id = ?1 AND role = 'assistant' ORDER BY timeline_ordinal LIMIT 1",
                [conversation_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .expect("assistant")
            .parse()
            .expect("assistant id");
        let character_participant: String = database
            .connection()
            .expect("connection")
            .query_row(
                "SELECT id FROM conversation_participants WHERE conversation_id = ?1 AND role = 'character' LIMIT 1",
                [conversation_id.to_string()],
                |row| row.get(0),
            )
            .expect("character");
        let parts = slice::encode(&vec![MessagePart::Text {
            text: "fork".into(),
        }])
        .expect("parts");
        let root_post_revision = MessageRevisionId::new();
        let root_post_message = MessageId::new();
        let child_branch = ConversationBranchId::new();
        let child_revision = MessageRevisionId::new();
        let child_message = MessageId::new();
        let mut connection = database.connection().expect("connection");
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("transaction");
        transaction
            .execute(
                "INSERT INTO conversation_message_revisions (conversation_id, id, message_id, branch_id, sequence, parts_json, authored_at) VALUES (?1, ?2, ?3, ?4, 1, ?5, 3)",
                params![conversation_id.to_string(), root_post_revision.to_string(), root_post_message.to_string(), root_branch_id.to_string(), parts],
            )
            .expect("root post revision");
        transaction
            .execute(
                "INSERT INTO conversation_messages (conversation_id, id, branch_id, parent_message_id, role, author_participant_id, timeline_ordinal, logical_time, effective_time, visibility, pinned, scene_edited, active_revision_id, revision, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, 'assistant', ?5, 3, 3, 3, 'visible', 0, 0, ?6, 1, 3, 3)",
                params![conversation_id.to_string(), root_post_message.to_string(), root_branch_id.to_string(), assistant_id.to_string(), character_participant, root_post_revision.to_string()],
            )
            .expect("root post message");
        transaction
            .execute(
                "INSERT INTO conversation_branches (conversation_id, id, parent_branch_id, fork_message_id, status, revision, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, 'active', 1, 4, 4)",
                params![conversation_id.to_string(), child_branch.to_string(), root_branch_id.to_string(), assistant_id.to_string()],
            )
            .expect("child branch");
        transaction
            .execute(
                "INSERT INTO conversation_message_revisions (conversation_id, id, message_id, branch_id, sequence, parts_json, authored_at) VALUES (?1, ?2, ?3, ?4, 1, ?5, 4)",
                params![conversation_id.to_string(), child_revision.to_string(), child_message.to_string(), child_branch.to_string(), parts],
            )
            .expect("child revision");
        transaction
            .execute(
                "INSERT INTO conversation_messages (conversation_id, id, branch_id, parent_message_id, role, author_participant_id, timeline_ordinal, logical_time, effective_time, visibility, pinned, scene_edited, active_revision_id, revision, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, 'assistant', ?5, 4, 4, 4, 'visible', 0, 0, ?6, 1, 4, 4)",
                params![conversation_id.to_string(), child_message.to_string(), child_branch.to_string(), assistant_id.to_string(), character_participant, child_revision.to_string()],
            )
            .expect("child message");
        transaction
            .execute(
                "UPDATE conversation_branches SET head_message_id = ?1 WHERE conversation_id = ?2 AND id = ?3",
                params![child_message.to_string(), conversation_id.to_string(), child_branch.to_string()],
            )
            .expect("child head");
        transaction.commit().expect("commit");
        drop(connection);
        let page = ConversationReader::timeline_page(
            &database,
            conversation_id,
            child_branch,
            &PageRequest {
                cursor: None,
                limit: lettuce_types::PageLimit::new(2),
            },
        )
        .expect("fork timeline");
        let ids: Vec<MessageId> = page.items.iter().map(|item| item.message.id).collect();
        assert_eq!(ids, vec![child_message, assistant_id]);
        assert!(!ids.contains(&root_post_message));
        let user_id = page.items[1]
            .message
            .parent_message_id
            .expect("user parent");
        assert_eq!(page.boundary_parent_id, Some(user_id));
        let cursor = page.next_cursor.expect("older cursor");
        let older = ConversationReader::timeline_page(
            &database,
            conversation_id,
            child_branch,
            &PageRequest {
                cursor: Some(cursor),
                limit: lettuce_types::PageLimit::new(2),
            },
        )
        .expect("older fork timeline");
        assert_eq!(older.items.len(), 1);
        assert_eq!(older.items[0].message.id, user_id);
        assert!(older.next_cursor.is_none());
    }

    #[test]
    fn reader_uses_an_old_snapshot_while_another_handle_writes() {
        let path = std::env::temp_dir().join(format!(
            "lettuce-query-snapshot-{}.db",
            ConversationId::new()
        ));
        let writer = Database::open(&path).expect("writer");
        let conversation_id = create_fixture(&writer, "before");
        let reader = Database::open(&path).expect("reader");
        let mut writer_connection = writer.connection().expect("writer connection");
        let writer_transaction = writer_connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("writer transaction");
        writer_transaction
            .execute(
                "UPDATE conversations SET title = 'after', updated_at = 10 WHERE id = ?1",
                [conversation_id.to_string()],
            )
            .expect("uncommitted update");
        let page = ConversationReader::page(
            &reader,
            &ConversationQuery {
                lifecycle: None,
                page: PageRequest::default(),
            },
        )
        .expect("snapshot page");
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].title, "before");
        writer_transaction.commit().expect("writer commit");
        let after = ConversationReader::get(&reader, conversation_id).expect("after read");
        assert_eq!(after.conversation.title, "after");
        drop(writer_connection);
        drop(reader);
        drop(writer);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(format!("{}-wal", path.display()));
        let _ = std::fs::remove_file(format!("{}-shm", path.display()));
    }

    #[test]
    fn reader_accepts_pre_epoch_conversation_and_turn_cursors() {
        let database = Database::open_in_memory().expect("database");
        let first = create_fixture(&database, "pre-epoch-first");
        let second = create_fixture(&database, "pre-epoch-second");
        let connection = database.connection().expect("connection");
        connection
            .execute(
                "UPDATE conversations SET created_at = -20, updated_at = -20 WHERE id = ?1",
                [first.to_string()],
            )
            .expect("first pre-epoch conversation");
        connection
            .execute(
                "UPDATE conversations SET created_at = -10, updated_at = -10 WHERE id = ?1",
                [second.to_string()],
            )
            .expect("second pre-epoch conversation");
        drop(connection);
        let page = ConversationReader::page(
            &database,
            &ConversationQuery {
                lifecycle: None,
                page: PageRequest {
                    cursor: None,
                    limit: lettuce_types::PageLimit::new(1),
                },
            },
        )
        .expect("first conversation page");
        let cursor = page.next_cursor.expect("conversation cursor");
        let next = ConversationReader::page(
            &database,
            &ConversationQuery {
                lifecycle: None,
                page: PageRequest {
                    cursor: Some(cursor),
                    limit: lettuce_types::PageLimit::new(1),
                },
            },
        )
        .expect("second conversation page");
        assert_eq!(next.items.len(), 1);

        let (conversation_id, _revision_id, _candidate_id, source_id, current_id) =
            create_content_fixture(&database);
        let connection = database.connection().expect("connection");
        connection
            .execute(
                "UPDATE conversation_turns SET created_at = -20, updated_at = -20 WHERE id = ?1",
                [source_id.to_string()],
            )
            .expect("source pre-epoch turn");
        connection
            .execute(
                "UPDATE conversation_turns SET created_at = -10, updated_at = -10 WHERE id = ?1",
                [current_id.to_string()],
            )
            .expect("current pre-epoch turn");
        drop(connection);
        let turns = ConversationReader::page_turns(
            &database,
            conversation_id,
            &PageRequest {
                cursor: None,
                limit: lettuce_types::PageLimit::new(1),
            },
        )
        .expect("first turn page");
        let cursor = turns.next_cursor.expect("turn cursor");
        let next = ConversationReader::page_turns(
            &database,
            conversation_id,
            &PageRequest {
                cursor: Some(cursor),
                limit: lettuce_types::PageLimit::new(1),
            },
        )
        .expect("second turn page");
        assert_eq!(next.items.len(), 1);
    }

    #[test]
    fn reader_rejects_corrupt_selected_speaker_ownership() {
        let database = Database::open_in_memory().expect("database");
        let (conversation_id, _revision_id, _candidate_id, _source_id, turn_id) =
            create_content_fixture(&database);
        let connection = database.connection().expect("connection");
        let user_participant: String = connection
            .query_row(
                "SELECT id FROM conversation_participants WHERE conversation_id = ?1 AND role = 'user'",
                [conversation_id.to_string()],
                |row| row.get(0),
            )
            .expect("user participant");
        connection
            .execute_batch(
                "DROP TRIGGER conversation_turn_final_update_contract;
                 DROP TRIGGER conversation_turn_selected_speaker_character_final_update;",
            )
            .expect("disable guards for corruption fixture");
        connection
            .execute(
                "UPDATE conversation_turns SET selected_speaker_participant_id = ?1, selected_speaker_details_json = '{\"format_version\":1,\"value\":{\"method\":\"explicit\",\"fallback\":\"none\",\"reference\":null,\"rationale_summary\":null,\"decision_model\":null,\"usage_event_id\":null}}' WHERE id = ?2",
                params![user_participant, turn_id.to_string()],
            )
            .expect("corrupt selected speaker");
        drop(connection);
        assert_eq!(
            ConversationReader::get_turn(&database, turn_id),
            Err(ConversationRepositoryError::Storage)
        );
    }
}
