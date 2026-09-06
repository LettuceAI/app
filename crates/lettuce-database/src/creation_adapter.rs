use std::{collections::HashMap, str::FromStr};

use lettuce_characters::{
    Character, CharacterDefaults, CharacterMedia, CharacterPresentationV1, CharacterProfile,
    CharacterProvenance, CreateCharacterPlan, Persona, PersonaDraftUpdate,
    RepositoryError as AuthoredRepositoryError, Scene, SceneDocumentV1, SceneOwner, ScenePart,
};
use lettuce_context::{
    DetectionPolicy, KeywordMatchMode, LifecycleStatus as LorebookLifecycleStatus, Lorebook,
    LorebookBehaviorVersion, LorebookDetails, LorebookEntry, LorebookRepositoryError,
};
use lettuce_creation::{
    AdmittedCreationToolCall, ConfirmedCharacterApply, ConfirmedCharacterRevisionApply,
    ConfirmedLorebookApply, ConfirmedLorebookRevisionApply, ConfirmedPersonaApply,
    ConfirmedPersonaRevisionApply, CreationApplyReceipt, CreationApplyRepository,
    CreationAttemptFailureCode, CreationAttemptOwner, CreationAttemptRecovery,
    CreationAttemptRepository, CreationAttemptStatus, CreationAttemptSuccess,
    CreationAttemptSuccessSettlement, CreationCharacterApplyReceipt, CreationInferenceAttempt,
    CreationInferenceRound, CreationLorebookApplyReceipt, CreationOperationOutcome,
    CreationProposal, CreationRepositoryError, CreationRoundFinishReason, CreationStage,
    CreationTarget, CreationTargetKind, CreationToolCallEvidence, CreationTurn,
    CreationTurnAttemptAdmission, CreationWorkflow, CreationWorkflowRepository, NewCreationAttempt,
    NewCreationAttemptRecovery, NewCreationInferenceRound, NewCreationTurn, NewCreationTurnAttempt,
    NewCreationWorkflow, creation_tool_request, reduce_creation_tool_calls,
    validate_creation_tool_calls,
};
use lettuce_types::{
    CreationProposalId, CreationTurnId, CreationWorkflowId, GenerationAttemptId, JobId, Revision,
    TimestampMillis,
};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};

use crate::{Database, conversation_query, decode_versioned, encode_versioned};

const CREATION_JSON_VERSION: u32 = 1;

fn storage(_: impl std::fmt::Debug) -> CreationRepositoryError {
    CreationRepositoryError::Storage
}

fn encode<T: serde::Serialize>(value: &T) -> Result<String, CreationRepositoryError> {
    serde_json::to_string(value).map_err(|_| CreationRepositoryError::Invalid)
}

fn decode<T: serde::de::DeserializeOwned>(value: String) -> rusqlite::Result<T> {
    serde_json::from_str(&value).map_err(|_| rusqlite::Error::InvalidQuery)
}

fn stage_name(stage: CreationStage) -> &'static str {
    match stage {
        CreationStage::Drafting => "drafting",
        CreationStage::AwaitingReview => "awaiting_review",
        CreationStage::AwaitingConfirmation => "awaiting_confirmation",
    }
}

fn parse_stage(value: &str) -> rusqlite::Result<CreationStage> {
    match value {
        "drafting" => Ok(CreationStage::Drafting),
        "awaiting_review" => Ok(CreationStage::AwaitingReview),
        "awaiting_confirmation" => Ok(CreationStage::AwaitingConfirmation),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn target_name(target: CreationTargetKind) -> &'static str {
    match target {
        CreationTargetKind::Character => "character",
        CreationTargetKind::Persona => "persona",
        CreationTargetKind::Lorebook => "lorebook",
    }
}

fn parse_target(value: &str) -> rusqlite::Result<CreationTargetKind> {
    match value {
        "character" => Ok(CreationTargetKind::Character),
        "persona" => Ok(CreationTargetKind::Persona),
        "lorebook" => Ok(CreationTargetKind::Lorebook),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn attempt_status_name(status: CreationAttemptStatus) -> &'static str {
    match status {
        CreationAttemptStatus::Created => "created",
        CreationAttemptStatus::Running => "running",
        CreationAttemptStatus::Succeeded => "succeeded",
        CreationAttemptStatus::Failed => "failed",
        CreationAttemptStatus::Cancelled => "cancelled",
        CreationAttemptStatus::Interrupted => "interrupted",
    }
}

fn parse_attempt_status(value: &str) -> rusqlite::Result<CreationAttemptStatus> {
    match value {
        "created" => Ok(CreationAttemptStatus::Created),
        "running" => Ok(CreationAttemptStatus::Running),
        "succeeded" => Ok(CreationAttemptStatus::Succeeded),
        "failed" => Ok(CreationAttemptStatus::Failed),
        "cancelled" => Ok(CreationAttemptStatus::Cancelled),
        "interrupted" => Ok(CreationAttemptStatus::Interrupted),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn attempt_failure_name(failure: CreationAttemptFailureCode) -> &'static str {
    match failure {
        CreationAttemptFailureCode::ProviderUnavailable => "provider_unavailable",
        CreationAttemptFailureCode::ProviderRejected => "provider_rejected",
        CreationAttemptFailureCode::EmptyResponse => "empty_response",
        CreationAttemptFailureCode::TimedOut => "timed_out",
        CreationAttemptFailureCode::RoundLimit => "round_limit",
        CreationAttemptFailureCode::Internal => "internal",
    }
}

fn parse_attempt_failure(value: &str) -> rusqlite::Result<CreationAttemptFailureCode> {
    match value {
        "provider_unavailable" => Ok(CreationAttemptFailureCode::ProviderUnavailable),
        "provider_rejected" => Ok(CreationAttemptFailureCode::ProviderRejected),
        "empty_response" => Ok(CreationAttemptFailureCode::EmptyResponse),
        "timed_out" => Ok(CreationAttemptFailureCode::TimedOut),
        "round_limit" => Ok(CreationAttemptFailureCode::RoundLimit),
        "internal" => Ok(CreationAttemptFailureCode::Internal),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn parse_id<T: FromStr>(value: String) -> rusqlite::Result<T> {
    value.parse().map_err(|_| rusqlite::Error::InvalidQuery)
}

fn revision(value: i64) -> rusqlite::Result<Revision> {
    let value = u64::try_from(value).map_err(|_| rusqlite::Error::InvalidQuery)?;
    if value == 0 {
        return Err(rusqlite::Error::InvalidQuery);
    }
    Ok(Revision::new(value))
}

fn profile_fingerprint(value: Vec<u8>) -> rusqlite::Result<[u8; 32]> {
    value.try_into().map_err(|_| rusqlite::Error::InvalidQuery)
}

fn sql_u64(value: u64) -> Result<i64, CreationRepositoryError> {
    i64::try_from(value).map_err(|_| CreationRepositoryError::Storage)
}

fn authored_error(error: AuthoredRepositoryError) -> CreationRepositoryError {
    match error {
        AuthoredRepositoryError::NotFound => CreationRepositoryError::NotFound,
        AuthoredRepositoryError::AlreadyExists
        | AuthoredRepositoryError::StaleRevision { .. }
        | AuthoredRepositoryError::Archived
        | AuthoredRepositoryError::HasDependencies => CreationRepositoryError::Conflict,
        AuthoredRepositoryError::Invalid(_) => CreationRepositoryError::Invalid,
        _ => CreationRepositoryError::Storage,
    }
}

fn lorebook_error(error: LorebookRepositoryError) -> CreationRepositoryError {
    match error {
        LorebookRepositoryError::NotFound | LorebookRepositoryError::EntryNotFound => {
            CreationRepositoryError::NotFound
        }
        LorebookRepositoryError::Conflict => CreationRepositoryError::Conflict,
        LorebookRepositoryError::Invalid(_) => CreationRepositoryError::Invalid,
        LorebookRepositoryError::Failure(_) => CreationRepositoryError::Storage,
    }
}

fn load_apply_receipt(
    connection: &Connection,
    workflow_id: CreationWorkflowId,
) -> Result<Option<CreationApplyReceipt>, CreationRepositoryError> {
    connection
        .query_row(
            "SELECT workflow_revision,proposal_id,persona_id,persona_revision,applied_at \
             FROM creation_apply_receipts WHERE workflow_id=?1",
            [workflow_id.to_string()],
            |row| {
                Ok(CreationApplyReceipt {
                    workflow_id,
                    workflow_revision: revision(row.get(0)?)?,
                    proposal_id: parse_id(row.get(1)?)?,
                    persona_id: parse_id(row.get(2)?)?,
                    persona_revision: revision(row.get(3)?)?,
                    applied_at: TimestampMillis::new(row.get(4)?),
                })
            },
        )
        .optional()
        .map_err(storage)
}

fn insert_apply_receipt(
    transaction: &Transaction<'_>,
    receipt: &CreationApplyReceipt,
) -> Result<(), CreationRepositoryError> {
    transaction
        .execute(
            "INSERT INTO creation_apply_receipts \
             (workflow_id,workflow_revision,proposal_id,persona_id,persona_revision,applied_at) \
             VALUES (?1,?2,?3,?4,?5,?6)",
            params![
                receipt.workflow_id.to_string(),
                sql_u64(receipt.workflow_revision.get())?,
                receipt.proposal_id.to_string(),
                receipt.persona_id.to_string(),
                sql_u64(receipt.persona_revision.get())?,
                receipt.applied_at.get(),
            ],
        )
        .map_err(storage)?;
    Ok(())
}

fn load_character_apply_receipt(
    connection: &Connection,
    workflow_id: CreationWorkflowId,
) -> Result<Option<CreationCharacterApplyReceipt>, CreationRepositoryError> {
    connection
        .query_row(
            "SELECT workflow_revision,proposal_id,character_id,character_revision,applied_at \
             FROM creation_character_apply_receipts WHERE workflow_id=?1",
            [workflow_id.to_string()],
            |row| {
                Ok(CreationCharacterApplyReceipt {
                    workflow_id,
                    workflow_revision: revision(row.get(0)?)?,
                    proposal_id: parse_id(row.get(1)?)?,
                    character_id: parse_id(row.get(2)?)?,
                    character_revision: revision(row.get(3)?)?,
                    applied_at: TimestampMillis::new(row.get(4)?),
                })
            },
        )
        .optional()
        .map_err(storage)
}

fn insert_character_apply_receipt(
    transaction: &Transaction<'_>,
    receipt: &CreationCharacterApplyReceipt,
) -> Result<(), CreationRepositoryError> {
    transaction
        .execute(
            "INSERT INTO creation_character_apply_receipts \
             (workflow_id,workflow_revision,proposal_id,character_id,character_revision,applied_at) \
             VALUES (?1,?2,?3,?4,?5,?6)",
            params![
                receipt.workflow_id.to_string(),
                sql_u64(receipt.workflow_revision.get())?,
                receipt.proposal_id.to_string(),
                receipt.character_id.to_string(),
                sql_u64(receipt.character_revision.get())?,
                receipt.applied_at.get(),
            ],
        )
        .map_err(storage)?;
    Ok(())
}

fn load_lorebook_apply_receipt(
    connection: &Connection,
    workflow_id: CreationWorkflowId,
) -> Result<Option<CreationLorebookApplyReceipt>, CreationRepositoryError> {
    connection
        .query_row(
            "SELECT workflow_revision,proposal_id,lorebook_id,lorebook_revision,applied_at \
             FROM creation_lorebook_apply_receipts WHERE workflow_id=?1",
            [workflow_id.to_string()],
            |row| {
                Ok(CreationLorebookApplyReceipt {
                    workflow_id,
                    workflow_revision: revision(row.get(0)?)?,
                    proposal_id: parse_id(row.get(1)?)?,
                    lorebook_id: parse_id(row.get(2)?)?,
                    lorebook_revision: revision(row.get(3)?)?,
                    applied_at: TimestampMillis::new(row.get(4)?),
                })
            },
        )
        .optional()
        .map_err(storage)
}

fn insert_lorebook_apply_receipt(
    transaction: &Transaction<'_>,
    receipt: &CreationLorebookApplyReceipt,
) -> Result<(), CreationRepositoryError> {
    transaction
        .execute(
            "INSERT INTO creation_lorebook_apply_receipts \
             (workflow_id,workflow_revision,proposal_id,lorebook_id,lorebook_revision,applied_at) \
             VALUES (?1,?2,?3,?4,?5,?6)",
            params![
                receipt.workflow_id.to_string(),
                sql_u64(receipt.workflow_revision.get())?,
                receipt.proposal_id.to_string(),
                receipt.lorebook_id.to_string(),
                sql_u64(receipt.lorebook_revision.get())?,
                receipt.applied_at.get(),
            ],
        )
        .map_err(storage)?;
    Ok(())
}

fn load_workflow_conn(
    connection: &Connection,
    id: CreationWorkflowId,
) -> Result<CreationWorkflow, CreationRepositoryError> {
    let workflow = connection
        .query_row(
            "SELECT target_json,stage,current_proposal_id,revision,created_at,updated_at \
             FROM creation_workflows WHERE id=?1",
            [id.to_string()],
            |row| {
                Ok(CreationWorkflow {
                    id,
                    target: decode(row.get(0)?)?,
                    stage: parse_stage(&row.get::<_, String>(1)?)?,
                    current_proposal_id: parse_id(
                        row.get::<_, Option<String>>(2)?
                            .ok_or(rusqlite::Error::InvalidQuery)?,
                    )?,
                    revision: revision(row.get(3)?)?,
                    created_at: TimestampMillis::new(row.get(4)?),
                    updated_at: TimestampMillis::new(row.get(5)?),
                })
            },
        )
        .optional()
        .map_err(storage)?
        .ok_or(CreationRepositoryError::NotFound)?;
    workflow
        .target
        .validate()
        .map_err(|_| CreationRepositoryError::Invalid)?;
    let proposal = load_proposal_conn(connection, workflow.current_proposal_id)?;
    if proposal.stage != workflow.stage || proposal.draft.kind() != workflow.target.kind() {
        return Err(CreationRepositoryError::Invalid);
    }
    Ok(workflow)
}

fn load_proposal_conn(
    connection: &Connection,
    id: CreationProposalId,
) -> Result<CreationProposal, CreationRepositoryError> {
    let proposal = connection
        .query_row(
            "SELECT turn_id,parent_id,ordinal,stage,draft_json,outcomes_json,created_at \
             FROM creation_proposals WHERE id=?1",
            [id.to_string()],
            |row| {
                Ok(CreationProposal {
                    id,
                    turn_id: row.get::<_, Option<String>>(0)?.map(parse_id).transpose()?,
                    parent_id: row.get::<_, Option<String>>(1)?.map(parse_id).transpose()?,
                    ordinal: u32::try_from(row.get::<_, i64>(2)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    stage: parse_stage(&row.get::<_, String>(3)?)?,
                    draft: decode(row.get(4)?)?,
                    outcomes: decode::<Vec<CreationOperationOutcome>>(row.get(5)?)?,
                    created_at: TimestampMillis::new(row.get(6)?),
                })
            },
        )
        .optional()
        .map_err(storage)?
        .ok_or(CreationRepositoryError::NotFound)?;
    proposal
        .validate()
        .map_err(|_| CreationRepositoryError::Invalid)?;
    Ok(proposal)
}

fn load_turn_conn(
    connection: &Connection,
    id: CreationTurnId,
) -> Result<CreationTurn, CreationRepositoryError> {
    let turn = connection
        .query_row(
            "SELECT workflow_id,ordinal,base_proposal_id,user_message,created_at \
             FROM creation_turns WHERE id=?1",
            [id.to_string()],
            |row| {
                Ok(CreationTurn {
                    id,
                    workflow_id: parse_id(row.get(0)?)?,
                    ordinal: u32::try_from(row.get::<_, i64>(1)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    base_proposal_id: parse_id(row.get(2)?)?,
                    user_message: row.get(3)?,
                    created_at: TimestampMillis::new(row.get(4)?),
                })
            },
        )
        .optional()
        .map_err(storage)?
        .ok_or(CreationRepositoryError::NotFound)?;
    NewCreationTurn {
        id: turn.id,
        workflow_id: turn.workflow_id,
        base_proposal_id: turn.base_proposal_id,
        user_message: turn.user_message.clone(),
        now: turn.created_at,
    }
    .validate()
    .map_err(|_| CreationRepositoryError::Invalid)?;
    Ok(turn)
}

fn next_ordinal(
    transaction: &Transaction<'_>,
    table: &str,
    workflow_id: CreationWorkflowId,
) -> Result<u32, CreationRepositoryError> {
    let query = format!("SELECT COALESCE(MAX(ordinal), -1) + 1 FROM {table} WHERE workflow_id=?1");
    let ordinal: i64 = transaction
        .query_row(&query, [workflow_id.to_string()], |row| row.get(0))
        .map_err(storage)?;
    u32::try_from(ordinal).map_err(|_| CreationRepositoryError::Storage)
}

fn insert_proposal(
    transaction: &Transaction<'_>,
    workflow_id: CreationWorkflowId,
    proposal: &CreationProposal,
) -> Result<(), CreationRepositoryError> {
    transaction
        .execute(
            "INSERT INTO creation_proposals \
             (id,workflow_id,turn_id,parent_id,ordinal,stage,draft_json,outcomes_json,created_at) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                proposal.id.to_string(),
                workflow_id.to_string(),
                proposal.turn_id.map(|id| id.to_string()),
                proposal.parent_id.map(|id| id.to_string()),
                i64::from(proposal.ordinal),
                stage_name(proposal.stage),
                encode(&proposal.draft)?,
                encode(&proposal.outcomes)?,
                proposal.created_at.get(),
            ],
        )
        .map_err(storage)?;
    Ok(())
}

fn load_attempt_conn(
    connection: &Connection,
    id: GenerationAttemptId,
) -> Result<CreationInferenceAttempt, CreationRepositoryError> {
    let attempt = connection
        .query_row(
            "SELECT workflow_id,turn_id,ordinal,retry_parent_id,base_proposal_id,\
                    planned_proposal_id,target,stage,tool_request_json,job_id,profile_fingerprint,\
                    workflow_revision,status,failure,revision,created_at,started_at,finished_at,updated_at \
             FROM creation_inference_attempts WHERE id=?1",
            [id.to_string()],
            |row| {
                Ok(CreationInferenceAttempt {
                    id,
                    workflow_id: parse_id(row.get(0)?)?,
                    turn_id: parse_id(row.get(1)?)?,
                    ordinal: u16::try_from(row.get::<_, i64>(2)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    retry_parent_id: row.get::<_, Option<String>>(3)?.map(parse_id).transpose()?,
                    base_proposal_id: parse_id(row.get(4)?)?,
                    planned_proposal_id: parse_id(row.get(5)?)?,
                    target: parse_target(&row.get::<_, String>(6)?)?,
                    stage: parse_stage(&row.get::<_, String>(7)?)?,
                    tool_request: decode_versioned(
                        &row.get::<_, String>(8)?,
                        CREATION_JSON_VERSION,
                    )
                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    job_id: parse_id::<JobId>(row.get(9)?)?,
                    profile_fingerprint: profile_fingerprint(row.get(10)?)?,
                    workflow_revision: revision(row.get(11)?)?,
                    status: parse_attempt_status(&row.get::<_, String>(12)?)?,
                    failure: row
                        .get::<_, Option<String>>(13)?
                        .map(|value| parse_attempt_failure(&value))
                        .transpose()?,
                    revision: revision(row.get(14)?)?,
                    created_at: TimestampMillis::new(row.get(15)?),
                    started_at: row.get::<_, Option<i64>>(16)?.map(TimestampMillis::new),
                    finished_at: row.get::<_, Option<i64>>(17)?.map(TimestampMillis::new),
                    updated_at: TimestampMillis::new(row.get(18)?),
                })
            },
        )
        .optional()
        .map_err(storage)?
        .ok_or(CreationRepositoryError::NotFound)?;
    attempt
        .validate()
        .map_err(|_| CreationRepositoryError::Invalid)?;
    Ok(attempt)
}

fn list_calls_in(
    transaction: &Transaction<'_>,
    owner: CreationAttemptOwner,
    attempt_id: GenerationAttemptId,
) -> Result<Vec<CreationToolCallEvidence>, CreationRepositoryError> {
    let mut statement = transaction
        .prepare(
            "SELECT id,round_ordinal,ordinal,definition_name,definition_version,provider_call_id,\
                    arguments_json,raw_arguments,provider_replay_artifact_id,\
                    provider_replay_retention,admitted_at \
             FROM creation_admitted_tool_calls \
             WHERE workflow_id=?1 AND turn_id=?2 AND attempt_id=?3 ORDER BY ordinal",
        )
        .map_err(storage)?;
    let rows = statement
        .query_map(
            params![
                owner.workflow_id.to_string(),
                owner.turn_id.to_string(),
                attempt_id.to_string()
            ],
            |row| {
                let replay = conversation_query::replay_ref(transaction, row.get(8)?, row.get(9)?)
                    .map_err(|_| rusqlite::Error::InvalidQuery)?;
                Ok(CreationToolCallEvidence {
                    id: parse_id(row.get(0)?)?,
                    workflow_id: owner.workflow_id,
                    turn_id: owner.turn_id,
                    attempt_id,
                    round_ordinal: u8::try_from(row.get::<_, i64>(1)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    ordinal: u16::try_from(row.get::<_, i64>(2)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    definition_version: u32::try_from(row.get::<_, i64>(4)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    call: lettuce_conversations::ProposedToolCall {
                        provider_call_id: row.get(5)?,
                        name: row.get(3)?,
                        arguments: decode_versioned(
                            &row.get::<_, String>(6)?,
                            CREATION_JSON_VERSION,
                        )
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                        raw_arguments: row.get(7)?,
                        provider_replay: replay,
                    },
                    admitted_at: TimestampMillis::new(row.get(10)?),
                })
            },
        )
        .map_err(storage)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(storage)?;
    for call in &rows {
        call.validate()
            .map_err(|_| CreationRepositoryError::Invalid)?;
    }
    Ok(rows)
}

fn list_rounds_in(
    transaction: &Transaction<'_>,
    owner: CreationAttemptOwner,
    attempt_id: GenerationAttemptId,
) -> Result<Vec<CreationInferenceRound>, CreationRepositoryError> {
    let calls = list_calls_in(transaction, owner, attempt_id)?;
    let mut statement = transaction
        .prepare(
            "SELECT ordinal,first_call_ordinal,call_count,parts_json,\
                    provider_replay_artifact_id,provider_replay_retention,input_tokens,\
                    output_tokens,finish_reason,provider_request_id,admitted_at,cached_input_tokens,reasoning_tokens,cache_write_tokens,web_search_requests \
             FROM creation_inference_rounds \
             WHERE workflow_id=?1 AND turn_id=?2 AND attempt_id=?3 ORDER BY ordinal",
        )
        .map_err(storage)?;
    let rounds = statement
        .query_map(
            params![
                owner.workflow_id.to_string(),
                owner.turn_id.to_string(),
                attempt_id.to_string()
            ],
            |row| {
                let ordinal = u8::try_from(row.get::<_, i64>(0)?)
                    .map_err(|_| rusqlite::Error::InvalidQuery)?;
                let first_call_ordinal = u16::try_from(row.get::<_, i64>(1)?)
                    .map_err(|_| rusqlite::Error::InvalidQuery)?;
                let call_count = usize::try_from(row.get::<_, i64>(2)?)
                    .map_err(|_| rusqlite::Error::InvalidQuery)?;
                let start = usize::from(first_call_ordinal);
                let end = start
                    .checked_add(call_count)
                    .ok_or(rusqlite::Error::InvalidQuery)?;
                let round_calls = calls
                    .get(start..end)
                    .ok_or(rusqlite::Error::InvalidQuery)?
                    .to_vec();
                if round_calls.iter().any(|call| call.round_ordinal != ordinal) {
                    return Err(rusqlite::Error::InvalidQuery);
                }
                let provider_replay =
                    conversation_query::replay_ref(transaction, row.get(4)?, row.get(5)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?;
                Ok(CreationInferenceRound {
                    workflow_id: owner.workflow_id,
                    turn_id: owner.turn_id,
                    attempt_id,
                    ordinal,
                    first_call_ordinal,
                    parts: decode_versioned(&row.get::<_, String>(3)?, CREATION_JSON_VERSION)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    provider_replay,
                    usage: match (row.get::<_, Option<i64>>(6)?, row.get::<_, Option<i64>>(7)?) {
                        (Some(input_tokens), Some(output_tokens)) => {
                            Some(lettuce_conversations::InferenceUsage {
                                cache_write_tokens: row
                                    .get::<_, Option<i64>>(13)?
                                    .map(u64::try_from)
                                    .transpose()
                                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                                web_search_requests: row
                                    .get::<_, Option<i64>>(14)?
                                    .map(u64::try_from)
                                    .transpose()
                                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                                cached_input_tokens: row
                                    .get::<_, Option<i64>>(11)?
                                    .map(u64::try_from)
                                    .transpose()
                                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                                reasoning_tokens: row
                                    .get::<_, Option<i64>>(12)?
                                    .map(u64::try_from)
                                    .transpose()
                                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                                input_tokens: u64::try_from(input_tokens)
                                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                                output_tokens: u64::try_from(output_tokens)
                                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                            })
                        }
                        (None, None) => None,
                        _ => return Err(rusqlite::Error::InvalidQuery),
                    },
                    finish_reason: match row.get::<_, String>(8)?.as_str() {
                        "stop" => CreationRoundFinishReason::Stop,
                        "length" => CreationRoundFinishReason::Length,
                        _ => return Err(rusqlite::Error::InvalidQuery),
                    },
                    provider_request_id: row.get(9)?,
                    calls: round_calls,
                    admitted_at: TimestampMillis::new(row.get(10)?),
                })
            },
        )
        .map_err(storage)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(storage)?;
    for round in &rounds {
        round
            .validate()
            .map_err(|_| CreationRepositoryError::Invalid)?;
    }
    Ok(rounds)
}

impl CreationApplyRepository for Database {
    fn apply_new_persona(
        &self,
        request: ConfirmedPersonaApply,
    ) -> Result<CreationApplyReceipt, CreationRepositoryError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        if let Some(receipt) = load_apply_receipt(&transaction, request.workflow_id)? {
            if receipt.workflow_revision == request.expected_workflow_revision
                && receipt.proposal_id == request.proposal_id
                && receipt.persona_id == request.destination_persona_id
            {
                transaction.commit().map_err(storage)?;
                return Ok(receipt);
            }
            return Err(CreationRepositoryError::Conflict);
        }

        let workflow = load_workflow_conn(&transaction, request.workflow_id)?;
        if workflow.target != CreationTarget::NewPersona
            || workflow.stage != CreationStage::AwaitingConfirmation
            || workflow.revision != request.expected_workflow_revision
            || workflow.current_proposal_id != request.proposal_id
            || request.now.get() < workflow.updated_at.get()
        {
            return Err(CreationRepositoryError::Conflict);
        }
        let proposal = load_proposal_conn(&transaction, request.proposal_id)?;
        let (Some(title), Some(description)) = (match proposal.draft {
            lettuce_creation::CreationDraft::Persona { name, description } => (name, description),
            _ => return Err(CreationRepositoryError::Conflict),
        }) else {
            return Err(CreationRepositoryError::Invalid);
        };
        let persona = Persona::new(
            request.destination_persona_id,
            title,
            description,
            request.now,
        )
        .map_err(|_| CreationRepositoryError::Invalid)?;
        let persona = crate::persona_adapter::insert_persona(&transaction, persona)
            .map_err(authored_error)?;
        let receipt = CreationApplyReceipt {
            workflow_id: request.workflow_id,
            workflow_revision: request.expected_workflow_revision,
            proposal_id: request.proposal_id,
            persona_id: persona.id,
            persona_revision: persona.revision,
            applied_at: request.now,
        };
        insert_apply_receipt(&transaction, &receipt)?;
        transaction.commit().map_err(storage)?;
        Ok(receipt)
    }

    fn apply_existing_persona(
        &self,
        request: ConfirmedPersonaRevisionApply,
    ) -> Result<CreationApplyReceipt, CreationRepositoryError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let expected_target = CreationTarget::ExistingPersona {
            id: request.persona_id,
            revision: request.expected_persona_revision,
        };
        if let Some(receipt) = load_apply_receipt(&transaction, request.workflow_id)? {
            let workflow = load_workflow_conn(&transaction, request.workflow_id)?;
            let expected_result_revision = request
                .expected_persona_revision
                .next()
                .map_err(|_| CreationRepositoryError::Invalid)?;
            if workflow.target == expected_target
                && receipt.workflow_revision == request.expected_workflow_revision
                && receipt.proposal_id == request.proposal_id
                && receipt.persona_id == request.persona_id
                && receipt.persona_revision == expected_result_revision
            {
                transaction.commit().map_err(storage)?;
                return Ok(receipt);
            }
            return Err(CreationRepositoryError::Conflict);
        }

        let workflow = load_workflow_conn(&transaction, request.workflow_id)?;
        if workflow.target != expected_target
            || workflow.stage != CreationStage::AwaitingConfirmation
            || workflow.revision != request.expected_workflow_revision
            || workflow.current_proposal_id != request.proposal_id
            || request.now.get() < workflow.updated_at.get()
        {
            return Err(CreationRepositoryError::Conflict);
        }
        let proposal = load_proposal_conn(&transaction, request.proposal_id)?;
        let (Some(title), Some(description)) = (match proposal.draft {
            lettuce_creation::CreationDraft::Persona { name, description } => (name, description),
            _ => return Err(CreationRepositoryError::Conflict),
        }) else {
            return Err(CreationRepositoryError::Invalid);
        };
        let current = crate::persona_adapter::get_persona(&transaction, request.persona_id)
            .map_err(authored_error)?
            .ok_or(CreationRepositoryError::NotFound)?;
        let draft = PersonaDraftUpdate {
            title,
            description,
            nickname: current.nickname,
            design_description: current.design_description,
            avatar_crop: current.avatar_crop,
            image_recommendation: current.image_recommendation,
        };
        let persona = crate::persona_adapter::revise_persona(
            &transaction,
            request.persona_id,
            request.expected_persona_revision,
            draft,
            request.now,
        )
        .map_err(authored_error)?;
        let receipt = CreationApplyReceipt {
            workflow_id: request.workflow_id,
            workflow_revision: request.expected_workflow_revision,
            proposal_id: request.proposal_id,
            persona_id: persona.id,
            persona_revision: persona.revision,
            applied_at: request.now,
        };
        insert_apply_receipt(&transaction, &receipt)?;
        transaction.commit().map_err(storage)?;
        Ok(receipt)
    }

    fn apply_new_character(
        &self,
        request: ConfirmedCharacterApply,
    ) -> Result<CreationCharacterApplyReceipt, CreationRepositoryError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        if let Some(receipt) = load_character_apply_receipt(&transaction, request.workflow_id)? {
            let workflow = load_workflow_conn(&transaction, request.workflow_id)?;
            if workflow.target == CreationTarget::NewCharacter
                && receipt.workflow_revision == request.expected_workflow_revision
                && receipt.proposal_id == request.proposal_id
                && receipt.character_id == request.destination_character_id
            {
                transaction.commit().map_err(storage)?;
                return Ok(receipt);
            }
            return Err(CreationRepositoryError::Conflict);
        }

        let workflow = load_workflow_conn(&transaction, request.workflow_id)?;
        if workflow.target != CreationTarget::NewCharacter
            || workflow.stage != CreationStage::AwaitingConfirmation
            || workflow.revision != request.expected_workflow_revision
            || workflow.current_proposal_id != request.proposal_id
            || request.now.get() < workflow.updated_at.get()
        {
            return Err(CreationRepositoryError::Conflict);
        }
        let proposal = load_proposal_conn(&transaction, request.proposal_id)?;
        let (Some(name), Some(definition), draft_scenes) = (match proposal.draft {
            lettuce_creation::CreationDraft::Character {
                name,
                definition,
                scenes,
            } => (name, definition, scenes),
            _ => return Err(CreationRepositoryError::Conflict),
        }) else {
            return Err(CreationRepositoryError::Invalid);
        };
        let character = Character::new(
            request.destination_character_id,
            CharacterProfile {
                name,
                nickname: None,
                description: None,
                definition: Some(definition),
                design_description: None,
            },
            CharacterProvenance::default(),
            CharacterDefaults::default(),
            CharacterPresentationV1::default(),
            None,
            CharacterMedia::default(),
            request.now,
        )
        .map_err(|_| CreationRepositoryError::Invalid)?;
        let scenes = draft_scenes
            .into_iter()
            .enumerate()
            .map(|(ordinal, draft)| {
                let mut scene = Scene::new(
                    draft.id,
                    SceneOwner::Character(request.destination_character_id),
                    u32::try_from(ordinal).map_err(|_| CreationRepositoryError::Invalid)?,
                    SceneDocumentV1::new(vec![ScenePart::Text {
                        text: draft.content,
                    }])
                    .map_err(|_| CreationRepositoryError::Invalid)?,
                    request.now,
                )
                .map_err(|_| CreationRepositoryError::Invalid)?;
                scene.direction = draft.direction;
                scene
                    .validate()
                    .map_err(|_| CreationRepositoryError::Invalid)?;
                Ok(scene)
            })
            .collect::<Result<Vec<_>, CreationRepositoryError>>()?;
        let details = crate::character_adapter::insert_character_plan(
            &transaction,
            &CreateCharacterPlan {
                character,
                scenes,
                variants: Vec::new(),
                starters: Vec::new(),
            },
        )
        .map_err(authored_error)?;
        let receipt = CreationCharacterApplyReceipt {
            workflow_id: request.workflow_id,
            workflow_revision: request.expected_workflow_revision,
            proposal_id: request.proposal_id,
            character_id: details.character.id,
            character_revision: details.character.revision,
            applied_at: request.now,
        };
        insert_character_apply_receipt(&transaction, &receipt)?;
        transaction.commit().map_err(storage)?;
        Ok(receipt)
    }

    fn apply_existing_character(
        &self,
        request: ConfirmedCharacterRevisionApply,
    ) -> Result<CreationCharacterApplyReceipt, CreationRepositoryError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let expected_target = CreationTarget::ExistingCharacter {
            id: request.character_id,
            revision: request.expected_character_revision,
        };
        if let Some(receipt) = load_character_apply_receipt(&transaction, request.workflow_id)? {
            let workflow = load_workflow_conn(&transaction, request.workflow_id)?;
            let expected_result_revision = request
                .expected_character_revision
                .next()
                .map_err(|_| CreationRepositoryError::Invalid)?;
            if workflow.target == expected_target
                && receipt.workflow_revision == request.expected_workflow_revision
                && receipt.proposal_id == request.proposal_id
                && receipt.character_id == request.character_id
                && receipt.character_revision == expected_result_revision
            {
                transaction.commit().map_err(storage)?;
                return Ok(receipt);
            }
            return Err(CreationRepositoryError::Conflict);
        }
        let workflow = load_workflow_conn(&transaction, request.workflow_id)?;
        if workflow.target != expected_target
            || workflow.stage != CreationStage::AwaitingConfirmation
            || workflow.revision != request.expected_workflow_revision
            || workflow.current_proposal_id != request.proposal_id
            || request.now.get() < workflow.updated_at.get()
        {
            return Err(CreationRepositoryError::Conflict);
        }
        let proposal = load_proposal_conn(&transaction, request.proposal_id)?;
        let (Some(name), Some(definition), draft_scenes) = (match proposal.draft {
            lettuce_creation::CreationDraft::Character {
                name,
                definition,
                scenes,
            } => (name, definition, scenes),
            _ => return Err(CreationRepositoryError::Conflict),
        }) else {
            return Err(CreationRepositoryError::Invalid);
        };
        let current =
            crate::character_adapter::load_character_details(&transaction, request.character_id)
                .map_err(authored_error)?
                .ok_or(CreationRepositoryError::NotFound)?;
        if current.character.status == lettuce_characters::LifecycleStatus::Archived
            || current.character.revision != request.expected_character_revision
            || request.now.get() < current.character.updated_at.get()
        {
            return Err(CreationRepositoryError::Conflict);
        }
        let previous_by_id: HashMap<_, _> = current
            .scenes
            .iter()
            .cloned()
            .map(|scene| (scene.id, scene))
            .collect();
        let scenes = draft_scenes
            .into_iter()
            .enumerate()
            .map(|(ordinal, draft)| {
                let ordinal =
                    u32::try_from(ordinal).map_err(|_| CreationRepositoryError::Invalid)?;
                let content = SceneDocumentV1::new(vec![ScenePart::Text {
                    text: draft.content,
                }])
                .map_err(|_| CreationRepositoryError::Invalid)?;
                let Some(previous) = previous_by_id.get(&draft.id) else {
                    let mut scene = Scene::new(
                        draft.id,
                        SceneOwner::Character(request.character_id),
                        ordinal,
                        content,
                        request.now,
                    )
                    .map_err(|_| CreationRepositoryError::Invalid)?;
                    scene.direction = draft.direction;
                    scene
                        .validate()
                        .map_err(|_| CreationRepositoryError::Invalid)?;
                    return Ok(scene);
                };
                let changed = previous.content != content
                    || previous.direction != draft.direction
                    || previous.ordinal != ordinal;
                Ok(Scene {
                    ordinal,
                    content,
                    direction: draft.direction,
                    revision: if changed {
                        previous
                            .revision
                            .next()
                            .map_err(|_| CreationRepositoryError::Storage)?
                    } else {
                        previous.revision
                    },
                    updated_at: if changed {
                        request.now
                    } else {
                        previous.updated_at
                    },
                    ..previous.clone()
                })
            })
            .collect::<Result<Vec<_>, CreationRepositoryError>>()?;
        let retained_scene_ids: std::collections::HashSet<_> =
            scenes.iter().map(|scene| scene.id).collect();
        let mut profile = current.character.profile.clone();
        profile.name = name;
        profile.definition = Some(definition);
        let plan = CreateCharacterPlan {
            character: Character {
                profile,
                revision: request
                    .expected_character_revision
                    .next()
                    .map_err(|_| CreationRepositoryError::Storage)?,
                updated_at: request.now,
                ..current.character
            },
            scenes,
            variants: current
                .variants
                .into_iter()
                .filter(|variant| retained_scene_ids.contains(&variant.scene_id))
                .collect(),
            starters: current.starters,
        };
        let details = crate::character_adapter::replace_character_profile_scenes(
            &transaction,
            request.expected_character_revision,
            &plan,
        )
        .map_err(authored_error)?;
        let receipt = CreationCharacterApplyReceipt {
            workflow_id: request.workflow_id,
            workflow_revision: request.expected_workflow_revision,
            proposal_id: request.proposal_id,
            character_id: details.character.id,
            character_revision: details.character.revision,
            applied_at: request.now,
        };
        insert_character_apply_receipt(&transaction, &receipt)?;
        transaction.commit().map_err(storage)?;
        Ok(receipt)
    }

    fn apply_new_lorebook(
        &self,
        request: ConfirmedLorebookApply,
    ) -> Result<CreationLorebookApplyReceipt, CreationRepositoryError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        if let Some(receipt) = load_lorebook_apply_receipt(&transaction, request.workflow_id)? {
            let workflow = load_workflow_conn(&transaction, request.workflow_id)?;
            if workflow.target == CreationTarget::NewLorebook
                && receipt.workflow_revision == request.expected_workflow_revision
                && receipt.proposal_id == request.proposal_id
                && receipt.lorebook_id == request.destination_lorebook_id
            {
                transaction.commit().map_err(storage)?;
                return Ok(receipt);
            }
            return Err(CreationRepositoryError::Conflict);
        }

        let workflow = load_workflow_conn(&transaction, request.workflow_id)?;
        if workflow.target != CreationTarget::NewLorebook
            || workflow.stage != CreationStage::AwaitingConfirmation
            || workflow.revision != request.expected_workflow_revision
            || workflow.current_proposal_id != request.proposal_id
            || request.now.get() < workflow.updated_at.get()
        {
            return Err(CreationRepositoryError::Conflict);
        }
        let proposal = load_proposal_conn(&transaction, request.proposal_id)?;
        let (Some(name), draft_entries) = (match proposal.draft {
            lettuce_creation::CreationDraft::Lorebook {
                name,
                description: _,
                entries,
            } => (name, entries),
            _ => return Err(CreationRepositoryError::Conflict),
        }) else {
            return Err(CreationRepositoryError::Invalid);
        };
        let entries = draft_entries
            .into_iter()
            .enumerate()
            .map(|(ordinal, draft)| {
                Ok(LorebookEntry {
                    id: draft.id,
                    lorebook_id: request.destination_lorebook_id,
                    title: draft.title,
                    enabled: true,
                    always_active: true,
                    keywords: Vec::new(),
                    case_sensitive: false,
                    match_mode: KeywordMatchMode::Literal,
                    content: draft.content,
                    priority: 0,
                    ordinal: u32::try_from(ordinal)
                        .map_err(|_| CreationRepositoryError::Invalid)?,
                    revision: Revision::INITIAL,
                    created_at: request.now,
                    updated_at: request.now,
                })
            })
            .collect::<Result<Vec<_>, CreationRepositoryError>>()?;
        let details = LorebookDetails {
            book: Lorebook {
                id: request.destination_lorebook_id,
                status: LorebookLifecycleStatus::Active,
                name,
                detection_policy: DetectionPolicy::RecentMessageWindow,
                icon_asset_id: None,
                behavior_version: LorebookBehaviorVersion::LegacyV1,
                revision: Revision::INITIAL,
                created_at: request.now,
                updated_at: request.now,
            },
            entries,
        };
        let details = crate::lorebook_adapter::insert_lorebook_details(&transaction, &details)
            .map_err(lorebook_error)?;
        let receipt = CreationLorebookApplyReceipt {
            workflow_id: request.workflow_id,
            workflow_revision: request.expected_workflow_revision,
            proposal_id: request.proposal_id,
            lorebook_id: details.book.id,
            lorebook_revision: details.book.revision,
            applied_at: request.now,
        };
        insert_lorebook_apply_receipt(&transaction, &receipt)?;
        transaction.commit().map_err(storage)?;
        Ok(receipt)
    }

    fn apply_existing_lorebook(
        &self,
        request: ConfirmedLorebookRevisionApply,
    ) -> Result<CreationLorebookApplyReceipt, CreationRepositoryError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let expected_target = CreationTarget::ExistingLorebook {
            id: request.lorebook_id,
            revision: request.expected_lorebook_revision,
        };
        if let Some(receipt) = load_lorebook_apply_receipt(&transaction, request.workflow_id)? {
            let workflow = load_workflow_conn(&transaction, request.workflow_id)?;
            let expected_result_revision = request
                .expected_lorebook_revision
                .next()
                .map_err(|_| CreationRepositoryError::Invalid)?;
            if workflow.target == expected_target
                && receipt.workflow_revision == request.expected_workflow_revision
                && receipt.proposal_id == request.proposal_id
                && receipt.lorebook_id == request.lorebook_id
                && receipt.lorebook_revision == expected_result_revision
            {
                transaction.commit().map_err(storage)?;
                return Ok(receipt);
            }
            return Err(CreationRepositoryError::Conflict);
        }

        let workflow = load_workflow_conn(&transaction, request.workflow_id)?;
        if workflow.target != expected_target
            || workflow.stage != CreationStage::AwaitingConfirmation
            || workflow.revision != request.expected_workflow_revision
            || workflow.current_proposal_id != request.proposal_id
            || request.now.get() < workflow.updated_at.get()
        {
            return Err(CreationRepositoryError::Conflict);
        }
        let proposal = load_proposal_conn(&transaction, request.proposal_id)?;
        let (Some(name), draft_entries) = (match proposal.draft {
            lettuce_creation::CreationDraft::Lorebook {
                name,
                description: _,
                entries,
            } => (name, entries),
            _ => return Err(CreationRepositoryError::Conflict),
        }) else {
            return Err(CreationRepositoryError::Invalid);
        };
        let current = crate::lorebook_adapter::load_required(&transaction, request.lorebook_id)
            .map_err(lorebook_error)?;
        if current.book.status == LorebookLifecycleStatus::Archived
            || current.book.revision != request.expected_lorebook_revision
            || request.now.get() < current.book.updated_at.get()
        {
            return Err(CreationRepositoryError::Conflict);
        }
        let mut previous_by_id: HashMap<_, _> = current
            .entries
            .iter()
            .cloned()
            .map(|entry| (entry.id, entry))
            .collect();
        let entries = draft_entries
            .into_iter()
            .enumerate()
            .map(|(ordinal, draft)| {
                let ordinal =
                    u32::try_from(ordinal).map_err(|_| CreationRepositoryError::Invalid)?;
                let Some(previous) = previous_by_id.remove(&draft.id) else {
                    return Ok(LorebookEntry {
                        id: draft.id,
                        lorebook_id: request.lorebook_id,
                        title: draft.title,
                        enabled: true,
                        always_active: true,
                        keywords: Vec::new(),
                        case_sensitive: false,
                        match_mode: KeywordMatchMode::Literal,
                        content: draft.content,
                        priority: 0,
                        ordinal,
                        revision: Revision::INITIAL,
                        created_at: request.now,
                        updated_at: request.now,
                    });
                };
                let changed = previous.title != draft.title
                    || previous.content != draft.content
                    || previous.ordinal != ordinal;
                let revision = if changed {
                    previous
                        .revision
                        .next()
                        .map_err(|_| CreationRepositoryError::Storage)?
                } else {
                    previous.revision
                };
                Ok(LorebookEntry {
                    id: previous.id,
                    lorebook_id: previous.lorebook_id,
                    title: draft.title,
                    enabled: previous.enabled,
                    always_active: previous.always_active,
                    keywords: previous.keywords,
                    case_sensitive: previous.case_sensitive,
                    match_mode: previous.match_mode,
                    content: draft.content,
                    priority: previous.priority,
                    ordinal,
                    revision,
                    created_at: previous.created_at,
                    updated_at: if changed {
                        request.now
                    } else {
                        previous.updated_at
                    },
                })
            })
            .collect::<Result<Vec<_>, CreationRepositoryError>>()?;
        let details = LorebookDetails {
            book: Lorebook {
                name,
                revision: request
                    .expected_lorebook_revision
                    .next()
                    .map_err(|_| CreationRepositoryError::Storage)?,
                updated_at: request.now,
                ..current.book
            },
            entries,
        };
        let details = crate::lorebook_adapter::replace_lorebook_details(
            &transaction,
            request.expected_lorebook_revision,
            &details,
        )
        .map_err(lorebook_error)?;
        let receipt = CreationLorebookApplyReceipt {
            workflow_id: request.workflow_id,
            workflow_revision: request.expected_workflow_revision,
            proposal_id: request.proposal_id,
            lorebook_id: details.book.id,
            lorebook_revision: details.book.revision,
            applied_at: request.now,
        };
        insert_lorebook_apply_receipt(&transaction, &receipt)?;
        transaction.commit().map_err(storage)?;
        Ok(receipt)
    }
}

impl CreationWorkflowRepository for Database {
    fn create_workflow(
        &self,
        input: NewCreationWorkflow,
    ) -> Result<CreationWorkflow, CreationRepositoryError> {
        input
            .validate()
            .map_err(|_| CreationRepositoryError::Invalid)?;
        let initial = CreationProposal::initial(
            input.initial_proposal_id,
            input.initial_draft.clone(),
            input.now,
        )
        .map_err(|_| CreationRepositoryError::Invalid)?;
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        match load_workflow_conn(&transaction, input.id) {
            Ok(existing) => {
                let proposal = load_proposal_conn(&transaction, existing.current_proposal_id)?;
                if existing.target == input.target && proposal == initial {
                    transaction.commit().map_err(storage)?;
                    return Ok(existing);
                }
                return Err(CreationRepositoryError::Conflict);
            }
            Err(CreationRepositoryError::NotFound) => {}
            Err(error) => return Err(error),
        }
        transaction
            .execute(
                "INSERT INTO creation_workflows \
                 (id,target_json,stage,current_proposal_id,revision,created_at,updated_at) \
                 VALUES (?1,?2,'drafting',NULL,1,?3,?3)",
                params![
                    input.id.to_string(),
                    encode(&input.target)?,
                    input.now.get()
                ],
            )
            .map_err(storage)?;
        insert_proposal(&transaction, input.id, &initial)?;
        transaction
            .execute(
                "UPDATE creation_workflows SET current_proposal_id=?2 WHERE id=?1",
                params![input.id.to_string(), input.initial_proposal_id.to_string()],
            )
            .map_err(storage)?;
        let workflow = load_workflow_conn(&transaction, input.id)?;
        transaction.commit().map_err(storage)?;
        Ok(workflow)
    }

    fn load_workflow(
        &self,
        id: CreationWorkflowId,
    ) -> Result<CreationWorkflow, CreationRepositoryError> {
        load_workflow_conn(&*self.connection().map_err(storage)?, id)
    }

    fn load_proposal(
        &self,
        id: CreationProposalId,
    ) -> Result<CreationProposal, CreationRepositoryError> {
        load_proposal_conn(&*self.connection().map_err(storage)?, id)
    }

    fn load_turn(&self, id: CreationTurnId) -> Result<CreationTurn, CreationRepositoryError> {
        load_turn_conn(&*self.connection().map_err(storage)?, id)
    }

    fn record_user_turn(
        &self,
        input: NewCreationTurn,
    ) -> Result<CreationTurn, CreationRepositoryError> {
        input
            .validate()
            .map_err(|_| CreationRepositoryError::Invalid)?;
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        match load_turn_conn(&transaction, input.id) {
            Ok(existing) => {
                if existing.workflow_id == input.workflow_id
                    && existing.base_proposal_id == input.base_proposal_id
                    && existing.user_message == input.user_message
                    && existing.created_at == input.now
                {
                    transaction.commit().map_err(storage)?;
                    return Ok(existing);
                }
                return Err(CreationRepositoryError::Conflict);
            }
            Err(CreationRepositoryError::NotFound) => {}
            Err(error) => return Err(error),
        }
        let workflow = load_workflow_conn(&transaction, input.workflow_id)?;
        if workflow.current_proposal_id != input.base_proposal_id
            || input.now.get() < workflow.updated_at.get()
        {
            return Err(CreationRepositoryError::Conflict);
        }
        let ordinal = next_ordinal(&transaction, "creation_turns", input.workflow_id)?;
        transaction
            .execute(
                "INSERT INTO creation_turns \
                 (id,workflow_id,ordinal,base_proposal_id,user_message,created_at) \
                 VALUES (?1,?2,?3,?4,?5,?6)",
                params![
                    input.id.to_string(),
                    input.workflow_id.to_string(),
                    i64::from(ordinal),
                    input.base_proposal_id.to_string(),
                    input.user_message,
                    input.now.get(),
                ],
            )
            .map_err(storage)?;
        let turn = load_turn_conn(&transaction, input.id)?;
        transaction.commit().map_err(storage)?;
        Ok(turn)
    }

    fn append_proposal(
        &self,
        workflow_id: CreationWorkflowId,
        expected_workflow_revision: Revision,
        proposal: CreationProposal,
    ) -> Result<CreationWorkflow, CreationRepositoryError> {
        proposal
            .validate()
            .map_err(|_| CreationRepositoryError::Invalid)?;
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let workflow = load_workflow_conn(&transaction, workflow_id)?;
        if workflow.current_proposal_id == proposal.id {
            let existing = load_proposal_conn(&transaction, proposal.id)?;
            if existing == proposal {
                transaction.commit().map_err(storage)?;
                return Ok(workflow);
            }
            return Err(CreationRepositoryError::Conflict);
        }
        if workflow.revision != expected_workflow_revision
            || proposal.parent_id != Some(workflow.current_proposal_id)
            || proposal.draft.kind() != workflow.target.kind()
            || proposal.created_at.get() < workflow.updated_at.get()
        {
            return Err(CreationRepositoryError::Conflict);
        }
        let base = load_proposal_conn(&transaction, workflow.current_proposal_id)?;
        let expected = base
            .apply(
                proposal.id,
                proposal.turn_id.ok_or(CreationRepositoryError::Invalid)?,
                proposal
                    .outcomes
                    .iter()
                    .map(|outcome| outcome.operation.clone())
                    .collect(),
                proposal.created_at,
            )
            .map_err(|_| CreationRepositoryError::Invalid)?;
        if expected != proposal {
            return Err(CreationRepositoryError::Conflict);
        }
        let stage_allowed = matches!(
            (workflow.stage, proposal.stage),
            (
                CreationStage::Drafting,
                CreationStage::Drafting | CreationStage::AwaitingReview
            ) | (
                CreationStage::AwaitingReview,
                CreationStage::AwaitingReview | CreationStage::AwaitingConfirmation
            ) | (
                CreationStage::AwaitingConfirmation,
                CreationStage::AwaitingConfirmation
            )
        );
        if proposal.ordinal
            != base
                .ordinal
                .checked_add(1)
                .ok_or(CreationRepositoryError::Storage)?
            || !stage_allowed
        {
            return Err(CreationRepositoryError::Conflict);
        }
        let turn = load_turn_conn(
            &transaction,
            proposal.turn_id.ok_or(CreationRepositoryError::Invalid)?,
        )?;
        if turn.workflow_id != workflow_id
            || turn.base_proposal_id != workflow.current_proposal_id
            || turn.created_at.get() > proposal.created_at.get()
        {
            return Err(CreationRepositoryError::Conflict);
        }
        let expected_ordinal = next_ordinal(&transaction, "creation_proposals", workflow_id)?;
        if proposal.ordinal != expected_ordinal {
            return Err(CreationRepositoryError::Conflict);
        }
        insert_proposal(&transaction, workflow_id, &proposal)?;
        let next_revision = workflow
            .revision
            .next()
            .map_err(|_| CreationRepositoryError::Storage)?;
        let changed = transaction
            .execute(
                "UPDATE creation_workflows SET stage=?2,current_proposal_id=?3,revision=?4,updated_at=?5 \
                 WHERE id=?1 AND revision=?6 AND current_proposal_id=?7",
                params![
                    workflow_id.to_string(),
                    stage_name(proposal.stage),
                    proposal.id.to_string(),
                    sql_u64(next_revision.get())?,
                    proposal.created_at.get(),
                    sql_u64(expected_workflow_revision.get())?,
                    workflow.current_proposal_id.to_string(),
                ],
            )
            .map_err(storage)?;
        if changed != 1 {
            return Err(CreationRepositoryError::Conflict);
        }
        let updated = load_workflow_conn(&transaction, workflow_id)?;
        transaction.commit().map_err(storage)?;
        Ok(updated)
    }
}

impl CreationAttemptRepository for Database {
    fn admit_creation_turn_attempt(
        &self,
        input: NewCreationTurnAttempt,
    ) -> Result<CreationTurnAttemptAdmission, CreationRepositoryError> {
        NewCreationTurn {
            id: input.turn_id,
            workflow_id: input.workflow_id,
            base_proposal_id: input.base_proposal_id,
            user_message: input.user_message.clone(),
            now: input.now,
        }
        .validate()
        .map_err(|_| CreationRepositoryError::Invalid)?;
        if input.base_proposal_id == input.planned_proposal_id {
            return Err(CreationRepositoryError::Invalid);
        }
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        match load_attempt_conn(&transaction, input.attempt_id) {
            Ok(attempt) => {
                let turn = load_turn_conn(&transaction, input.turn_id)?;
                if turn.workflow_id == input.workflow_id
                    && turn.base_proposal_id == input.base_proposal_id
                    && turn.user_message == input.user_message
                    && turn.created_at == input.now
                    && attempt.workflow_id == input.workflow_id
                    && attempt.turn_id == input.turn_id
                    && attempt.ordinal == 0
                    && attempt.retry_parent_id.is_none()
                    && attempt.base_proposal_id == input.base_proposal_id
                    && attempt.planned_proposal_id == input.planned_proposal_id
                    && attempt.job_id == input.job_id
                    && attempt.profile_fingerprint == input.profile_fingerprint
                    && attempt.workflow_revision == input.expected_workflow_revision
                    && attempt.created_at == input.now
                {
                    transaction.commit().map_err(storage)?;
                    return Ok(CreationTurnAttemptAdmission { turn, attempt });
                }
                return Err(CreationRepositoryError::Conflict);
            }
            Err(CreationRepositoryError::NotFound) => {}
            Err(error) => return Err(error),
        }
        match load_turn_conn(&transaction, input.turn_id) {
            Ok(_) => return Err(CreationRepositoryError::Conflict),
            Err(CreationRepositoryError::NotFound) => {}
            Err(error) => return Err(error),
        }
        let workflow = load_workflow_conn(&transaction, input.workflow_id)?;
        let base = load_proposal_conn(&transaction, input.base_proposal_id)?;
        if workflow.revision != input.expected_workflow_revision
            || workflow.current_proposal_id != input.base_proposal_id
            || workflow.stage != base.stage
            || workflow.target.kind() != base.draft.kind()
            || input.now < workflow.updated_at
        {
            return Err(CreationRepositoryError::Conflict);
        }
        let tool_request = creation_tool_request(workflow.target.kind(), workflow.stage)
            .ok_or(CreationRepositoryError::Conflict)?;
        let turn_ordinal = next_ordinal(&transaction, "creation_turns", input.workflow_id)?;
        transaction
            .execute(
                "INSERT INTO creation_turns \
                 (id,workflow_id,ordinal,base_proposal_id,user_message,created_at) \
                 VALUES (?1,?2,?3,?4,?5,?6)",
                params![
                    input.turn_id.to_string(),
                    input.workflow_id.to_string(),
                    i64::from(turn_ordinal),
                    input.base_proposal_id.to_string(),
                    input.user_message,
                    input.now.get(),
                ],
            )
            .map_err(|error| match error.sqlite_error_code() {
                Some(rusqlite::ErrorCode::ConstraintViolation) => CreationRepositoryError::Conflict,
                _ => CreationRepositoryError::Storage,
            })?;
        transaction
            .execute(
                "INSERT INTO creation_inference_attempts \
                 (workflow_id,turn_id,id,ordinal,retry_parent_id,base_proposal_id,\
                  planned_proposal_id,target,stage,tool_request_json,job_id,profile_fingerprint,\
                  workflow_revision,status,failure,revision,created_at,started_at,finished_at,updated_at) \
                 VALUES (?1,?2,?3,0,NULL,?4,?5,?6,?7,?8,?9,?10,?11,'created',NULL,1,?12,NULL,NULL,?12)",
                params![
                    input.workflow_id.to_string(),
                    input.turn_id.to_string(),
                    input.attempt_id.to_string(),
                    input.base_proposal_id.to_string(),
                    input.planned_proposal_id.to_string(),
                    target_name(workflow.target.kind()),
                    stage_name(workflow.stage),
                    encode_versioned(&tool_request, CREATION_JSON_VERSION)
                        .map_err(|_| CreationRepositoryError::Invalid)?,
                    input.job_id.to_string(),
                    input.profile_fingerprint.as_slice(),
                    sql_u64(input.expected_workflow_revision.get())?,
                    input.now.get(),
                ],
            )
            .map_err(|error| match error.sqlite_error_code() {
                Some(rusqlite::ErrorCode::ConstraintViolation) => CreationRepositoryError::Conflict,
                _ => CreationRepositoryError::Storage,
            })?;
        let turn = load_turn_conn(&transaction, input.turn_id)?;
        let attempt = load_attempt_conn(&transaction, input.attempt_id)?;
        transaction.commit().map_err(storage)?;
        Ok(CreationTurnAttemptAdmission { turn, attempt })
    }

    fn create_creation_attempt(
        &self,
        input: NewCreationAttempt,
    ) -> Result<CreationInferenceAttempt, CreationRepositoryError> {
        if input.base_proposal_id == input.planned_proposal_id {
            return Err(CreationRepositoryError::Invalid);
        }
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        match load_attempt_conn(&transaction, input.id) {
            Ok(existing) => {
                if existing.workflow_id == input.owner.workflow_id
                    && existing.turn_id == input.owner.turn_id
                    && existing.base_proposal_id == input.base_proposal_id
                    && existing.planned_proposal_id == input.planned_proposal_id
                    && existing.retry_parent_id == input.retry_parent_id
                    && existing.job_id == input.job_id
                    && existing.profile_fingerprint == input.profile_fingerprint
                    && existing.created_at == input.now
                {
                    transaction.commit().map_err(storage)?;
                    return Ok(existing);
                }
                return Err(CreationRepositoryError::Conflict);
            }
            Err(CreationRepositoryError::NotFound) => {}
            Err(error) => return Err(error),
        }
        let workflow = load_workflow_conn(&transaction, input.owner.workflow_id)?;
        let turn = load_turn_conn(&transaction, input.owner.turn_id)?;
        let base = load_proposal_conn(&transaction, input.base_proposal_id)?;
        if workflow.current_proposal_id != input.base_proposal_id
            || turn.workflow_id != input.owner.workflow_id
            || turn.base_proposal_id != input.base_proposal_id
            || base.stage != workflow.stage
            || base.draft.kind() != workflow.target.kind()
            || input.now < turn.created_at
        {
            return Err(CreationRepositoryError::Conflict);
        }
        let tool_request = creation_tool_request(workflow.target.kind(), workflow.stage)
            .ok_or(CreationRepositoryError::Conflict)?;
        tool_request
            .validate()
            .map_err(|_| CreationRepositoryError::Invalid)?;
        let actual_next: i64 = transaction
            .query_row(
                "SELECT coalesce(max(ordinal) + 1, 0) FROM creation_inference_attempts \
                 WHERE workflow_id=?1 AND turn_id=?2",
                params![
                    input.owner.workflow_id.to_string(),
                    input.owner.turn_id.to_string()
                ],
                |row| row.get(0),
            )
            .map_err(storage)?;
        let ordinal = u16::try_from(actual_next).map_err(|_| CreationRepositoryError::Storage)?;
        match input.retry_parent_id {
            None if ordinal == 0 => {}
            Some(parent_id) if ordinal > 0 => {
                let parent = load_attempt_conn(&transaction, parent_id)?;
                if parent.workflow_id != input.owner.workflow_id
                    || parent.turn_id != input.owner.turn_id
                    || parent.ordinal.checked_add(1) != Some(ordinal)
                    || !matches!(
                        parent.status,
                        CreationAttemptStatus::Failed
                            | CreationAttemptStatus::Cancelled
                            | CreationAttemptStatus::Interrupted
                    )
                    || parent.base_proposal_id != input.base_proposal_id
                    || parent.target != workflow.target.kind()
                    || parent.stage != workflow.stage
                    || parent.tool_request != tool_request
                    || parent.profile_fingerprint != input.profile_fingerprint
                    || parent.job_id == input.job_id
                    || parent.workflow_revision != workflow.revision
                {
                    return Err(CreationRepositoryError::Conflict);
                }
            }
            _ => return Err(CreationRepositoryError::Conflict),
        }
        transaction
            .execute(
                "INSERT INTO creation_inference_attempts \
                 (workflow_id,turn_id,id,ordinal,retry_parent_id,base_proposal_id,\
                  planned_proposal_id,target,stage,tool_request_json,job_id,profile_fingerprint,\
                  workflow_revision,status,failure,revision,created_at,started_at,finished_at,updated_at) \
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,'created',NULL,1,?14,NULL,NULL,?14)",
                params![
                    input.owner.workflow_id.to_string(),
                    input.owner.turn_id.to_string(),
                    input.id.to_string(),
                    i64::from(ordinal),
                    input.retry_parent_id.map(|id| id.to_string()),
                    input.base_proposal_id.to_string(),
                    input.planned_proposal_id.to_string(),
                    target_name(workflow.target.kind()),
                    stage_name(workflow.stage),
                    encode_versioned(&tool_request, CREATION_JSON_VERSION)
                        .map_err(|_| CreationRepositoryError::Invalid)?,
                    input.job_id.to_string(),
                    input.profile_fingerprint.as_slice(),
                    sql_u64(workflow.revision.get())?,
                    input.now.get(),
                ],
            )
            .map_err(|error| match error.sqlite_error_code() {
                Some(rusqlite::ErrorCode::ConstraintViolation) => CreationRepositoryError::Conflict,
                _ => CreationRepositoryError::Storage,
            })?;
        let attempt = load_attempt_conn(&transaction, input.id)?;
        transaction.commit().map_err(storage)?;
        Ok(attempt)
    }

    fn recover_creation_attempt(
        &self,
        input: NewCreationAttemptRecovery,
    ) -> Result<CreationAttemptRecovery, CreationRepositoryError> {
        if input.parent_attempt_id == input.child_attempt_id {
            return Err(CreationRepositoryError::Invalid);
        }
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let parent = load_attempt_conn(&transaction, input.parent_attempt_id)?;
        if parent.workflow_id != input.owner.workflow_id
            || parent.turn_id != input.owner.turn_id
            || parent.profile_fingerprint != input.profile_fingerprint
            || parent.job_id == input.job_id
            || parent.base_proposal_id == input.planned_proposal_id
            || parent.planned_proposal_id == input.planned_proposal_id
        {
            return Err(CreationRepositoryError::Conflict);
        }
        match load_attempt_conn(&transaction, input.child_attempt_id) {
            Ok(child) => {
                if parent.status == CreationAttemptStatus::Interrupted
                    && parent.finished_at == Some(input.now)
                    && child.workflow_id == parent.workflow_id
                    && child.turn_id == parent.turn_id
                    && child.retry_parent_id == Some(parent.id)
                    && child.ordinal
                        == parent
                            .ordinal
                            .checked_add(1)
                            .ok_or(CreationRepositoryError::Storage)?
                    && child.base_proposal_id == parent.base_proposal_id
                    && child.planned_proposal_id == input.planned_proposal_id
                    && child.job_id == input.job_id
                    && child.profile_fingerprint == parent.profile_fingerprint
                    && child.workflow_revision == parent.workflow_revision
                    && child.status == CreationAttemptStatus::Created
                    && child.created_at == input.now
                    && list_rounds_in(&transaction, input.owner, child.id)?.is_empty()
                {
                    transaction.commit().map_err(storage)?;
                    return Ok(CreationAttemptRecovery { parent, child });
                }
                return Err(CreationRepositoryError::Conflict);
            }
            Err(CreationRepositoryError::NotFound) => {}
            Err(error) => return Err(error),
        }
        if parent.status != CreationAttemptStatus::Running || input.now < parent.updated_at {
            return Err(CreationRepositoryError::Conflict);
        }
        let workflow = load_workflow_conn(&transaction, parent.workflow_id)?;
        if workflow.current_proposal_id != parent.base_proposal_id
            || workflow.stage != parent.stage
            || workflow.revision != parent.workflow_revision
        {
            return Err(CreationRepositoryError::Conflict);
        }
        let actual_next: i64 = transaction
            .query_row(
                "SELECT coalesce(max(ordinal) + 1, 0) FROM creation_inference_attempts \
                 WHERE workflow_id=?1 AND turn_id=?2",
                params![parent.workflow_id.to_string(), parent.turn_id.to_string()],
                |row| row.get(0),
            )
            .map_err(storage)?;
        let child_ordinal = parent
            .ordinal
            .checked_add(1)
            .ok_or(CreationRepositoryError::Storage)?;
        if u16::try_from(actual_next).map_err(|_| CreationRepositoryError::Storage)?
            != child_ordinal
        {
            return Err(CreationRepositoryError::Conflict);
        }
        let interrupted = parent
            .transition(CreationAttemptStatus::Interrupted, None, input.now)
            .map_err(|_| CreationRepositoryError::Conflict)?;
        let changed = transaction
            .execute(
                "UPDATE creation_inference_attempts \
                 SET status='interrupted',failure=NULL,revision=?2,finished_at=?3,updated_at=?3 \
                 WHERE id=?1 AND revision=?4 AND status='running'",
                params![
                    parent.id.to_string(),
                    sql_u64(interrupted.revision.get())?,
                    input.now.get(),
                    sql_u64(parent.revision.get())?,
                ],
            )
            .map_err(storage)?;
        if changed != 1 {
            return Err(CreationRepositoryError::Conflict);
        }
        transaction
            .execute(
                "INSERT INTO creation_inference_attempts \
                 (workflow_id,turn_id,id,ordinal,retry_parent_id,base_proposal_id,\
                  planned_proposal_id,target,stage,tool_request_json,job_id,profile_fingerprint,\
                  workflow_revision,status,failure,revision,created_at,started_at,finished_at,updated_at) \
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,'created',NULL,1,?14,NULL,NULL,?14)",
                params![
                    parent.workflow_id.to_string(),
                    parent.turn_id.to_string(),
                    input.child_attempt_id.to_string(),
                    i64::from(child_ordinal),
                    parent.id.to_string(),
                    parent.base_proposal_id.to_string(),
                    input.planned_proposal_id.to_string(),
                    target_name(parent.target),
                    stage_name(parent.stage),
                    encode_versioned(&parent.tool_request, CREATION_JSON_VERSION)
                        .map_err(|_| CreationRepositoryError::Invalid)?,
                    input.job_id.to_string(),
                    input.profile_fingerprint.as_slice(),
                    sql_u64(parent.workflow_revision.get())?,
                    input.now.get(),
                ],
            )
            .map_err(|error| match error.sqlite_error_code() {
                Some(rusqlite::ErrorCode::ConstraintViolation) => CreationRepositoryError::Conflict,
                _ => CreationRepositoryError::Storage,
            })?;
        let parent = load_attempt_conn(&transaction, parent.id)?;
        let child = load_attempt_conn(&transaction, input.child_attempt_id)?;
        transaction.commit().map_err(storage)?;
        Ok(CreationAttemptRecovery { parent, child })
    }

    fn settle_creation_attempt_success(
        &self,
        input: CreationAttemptSuccessSettlement,
    ) -> Result<CreationAttemptSuccess, CreationRepositoryError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let attempt = load_attempt_conn(&transaction, input.attempt_id)?;
        if attempt.workflow_id != input.owner.workflow_id || attempt.turn_id != input.owner.turn_id
        {
            return Err(CreationRepositoryError::Conflict);
        }
        if attempt.status == CreationAttemptStatus::Succeeded {
            if attempt.revision
                != input
                    .expected_attempt_revision
                    .next()
                    .map_err(|_| CreationRepositoryError::Storage)?
                || attempt.finished_at != Some(input.now)
            {
                return Err(CreationRepositoryError::Conflict);
            }
            let proposal = match input.proposal {
                Some(expected) => {
                    let stored = load_proposal_conn(&transaction, attempt.planned_proposal_id)?;
                    if stored != expected {
                        return Err(CreationRepositoryError::Conflict);
                    }
                    Some(stored)
                }
                None => {
                    if !list_calls_in(&transaction, input.owner, attempt.id)?.is_empty() {
                        return Err(CreationRepositoryError::Conflict);
                    }
                    None
                }
            };
            let workflow = load_workflow_conn(&transaction, attempt.workflow_id)?;
            let expected_stored_revision = if proposal.is_some() {
                input
                    .expected_workflow_revision
                    .next()
                    .map_err(|_| CreationRepositoryError::Storage)?
            } else {
                input.expected_workflow_revision
            };
            if workflow.revision != expected_stored_revision
                || proposal.as_ref().is_some_and(|proposal| {
                    workflow.current_proposal_id != proposal.id || workflow.stage != proposal.stage
                })
                || proposal.is_none()
                    && (workflow.current_proposal_id != attempt.base_proposal_id
                        || workflow.stage != attempt.stage)
            {
                return Err(CreationRepositoryError::Conflict);
            }
            transaction.commit().map_err(storage)?;
            return Ok(CreationAttemptSuccess {
                attempt,
                workflow,
                proposal,
            });
        }
        if attempt.status != CreationAttemptStatus::Running
            || attempt.revision != input.expected_attempt_revision
        {
            return Err(CreationRepositoryError::Conflict);
        }
        let rounds = list_rounds_in(&transaction, input.owner, attempt.id)?;
        let last = rounds.last().ok_or(CreationRepositoryError::Conflict)?;
        if last.admitted_at != input.now {
            return Err(CreationRepositoryError::Conflict);
        }
        let calls = list_calls_in(&transaction, input.owner, attempt.id)?;
        let workflow = load_workflow_conn(&transaction, attempt.workflow_id)?;
        if workflow.revision != input.expected_workflow_revision
            || workflow.current_proposal_id != attempt.base_proposal_id
            || workflow.stage != attempt.stage
        {
            return Err(CreationRepositoryError::Conflict);
        }
        let proposal = match input.proposal {
            Some(proposal) => {
                let admitted = calls
                    .iter()
                    .map(|evidence| AdmittedCreationToolCall {
                        definition_version: evidence.definition_version,
                        call: evidence.call.clone(),
                    })
                    .collect::<Vec<_>>();
                let base = load_proposal_conn(&transaction, attempt.base_proposal_id)?;
                let derived = reduce_creation_tool_calls(
                    &base,
                    attempt.planned_proposal_id,
                    attempt.turn_id,
                    &admitted,
                    input.now,
                )
                .map_err(|_| CreationRepositoryError::Invalid)?
                .proposal;
                if proposal != derived
                    || (proposal.stage == attempt.stage && !last.calls.is_empty())
                {
                    return Err(CreationRepositoryError::Conflict);
                }
                insert_proposal(&transaction, attempt.workflow_id, &proposal)?;
                let next_revision = workflow
                    .revision
                    .next()
                    .map_err(|_| CreationRepositoryError::Storage)?;
                let changed = transaction
                    .execute(
                        "UPDATE creation_workflows SET stage=?2,current_proposal_id=?3,revision=?4,updated_at=?5 \
                         WHERE id=?1 AND revision=?6 AND current_proposal_id=?7",
                        params![
                            workflow.id.to_string(),
                            stage_name(proposal.stage),
                            proposal.id.to_string(),
                            sql_u64(next_revision.get())?,
                            input.now.get(),
                            sql_u64(workflow.revision.get())?,
                            attempt.base_proposal_id.to_string(),
                        ],
                    )
                    .map_err(storage)?;
                if changed != 1 {
                    return Err(CreationRepositoryError::Conflict);
                }
                Some(proposal)
            }
            None => {
                if !calls.is_empty() || !last.calls.is_empty() {
                    return Err(CreationRepositoryError::Conflict);
                }
                None
            }
        };
        let succeeded = attempt
            .transition(CreationAttemptStatus::Succeeded, None, input.now)
            .map_err(|_| CreationRepositoryError::Conflict)?;
        let changed = transaction
            .execute(
                "UPDATE creation_inference_attempts \
                 SET status='succeeded',failure=NULL,revision=?2,finished_at=?3,updated_at=?3 \
                 WHERE id=?1 AND revision=?4 AND status='running'",
                params![
                    attempt.id.to_string(),
                    sql_u64(succeeded.revision.get())?,
                    input.now.get(),
                    sql_u64(attempt.revision.get())?,
                ],
            )
            .map_err(storage)?;
        if changed != 1 {
            return Err(CreationRepositoryError::Conflict);
        }
        let attempt = load_attempt_conn(&transaction, attempt.id)?;
        let workflow = load_workflow_conn(&transaction, attempt.workflow_id)?;
        transaction.commit().map_err(storage)?;
        Ok(CreationAttemptSuccess {
            attempt,
            workflow,
            proposal,
        })
    }

    fn load_creation_attempt(
        &self,
        id: GenerationAttemptId,
    ) -> Result<CreationInferenceAttempt, CreationRepositoryError> {
        load_attempt_conn(&*self.connection().map_err(storage)?, id)
    }

    fn transition_creation_attempt(
        &self,
        id: GenerationAttemptId,
        expected_revision: Revision,
        next: CreationAttemptStatus,
        failure: Option<CreationAttemptFailureCode>,
        at: TimestampMillis,
    ) -> Result<CreationInferenceAttempt, CreationRepositoryError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let current = load_attempt_conn(&transaction, id)?;
        if current.revision != expected_revision {
            return Err(CreationRepositoryError::Conflict);
        }
        let updated = current
            .transition(next, failure, at)
            .map_err(|_| CreationRepositoryError::Invalid)?;
        let changed = transaction
            .execute(
                "UPDATE creation_inference_attempts \
                 SET status=?2,failure=?3,revision=?4,started_at=?5,finished_at=?6,updated_at=?7 \
                 WHERE id=?1 AND revision=?8",
                params![
                    id.to_string(),
                    attempt_status_name(updated.status),
                    updated.failure.map(attempt_failure_name),
                    sql_u64(updated.revision.get())?,
                    updated.started_at.map(TimestampMillis::get),
                    updated.finished_at.map(TimestampMillis::get),
                    updated.updated_at.get(),
                    sql_u64(expected_revision.get())?,
                ],
            )
            .map_err(storage)?;
        if changed != 1 {
            return Err(CreationRepositoryError::Conflict);
        }
        let stored = load_attempt_conn(&transaction, id)?;
        transaction.commit().map_err(storage)?;
        Ok(stored)
    }

    fn admit_creation_inference_round(
        &self,
        owner: CreationAttemptOwner,
        attempt_id: GenerationAttemptId,
        expected_round_ordinal: u8,
        expected_next_ordinal: u16,
        round: NewCreationInferenceRound,
    ) -> Result<CreationInferenceRound, CreationRepositoryError> {
        round
            .validate()
            .map_err(|_| CreationRepositoryError::Invalid)?;
        if round.ordinal != expected_round_ordinal {
            return Err(CreationRepositoryError::Invalid);
        }
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let attempt = load_attempt_conn(&transaction, attempt_id)?;
        if attempt.workflow_id != owner.workflow_id || attempt.turn_id != owner.turn_id {
            return Err(CreationRepositoryError::Conflict);
        }
        let requested_calls = round
            .calls
            .iter()
            .enumerate()
            .map(|(offset, call)| {
                let offset = u16::try_from(offset).map_err(|_| CreationRepositoryError::Invalid)?;
                let ordinal = expected_next_ordinal
                    .checked_add(offset)
                    .ok_or(CreationRepositoryError::Invalid)?;
                let evidence = CreationToolCallEvidence {
                    id: call.id,
                    workflow_id: owner.workflow_id,
                    turn_id: owner.turn_id,
                    attempt_id,
                    round_ordinal: round.ordinal,
                    ordinal,
                    definition_version: call.definition_version,
                    call: call.call.clone(),
                    admitted_at: round.admitted_at,
                };
                evidence
                    .validate()
                    .map_err(|_| CreationRepositoryError::Invalid)?;
                Ok(evidence)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let requested = CreationInferenceRound {
            workflow_id: owner.workflow_id,
            turn_id: owner.turn_id,
            attempt_id,
            ordinal: round.ordinal,
            first_call_ordinal: expected_next_ordinal,
            parts: round.parts,
            provider_replay: round.provider_replay,
            usage: round.usage,
            finish_reason: round.finish_reason,
            provider_request_id: round.provider_request_id,
            calls: requested_calls,
            admitted_at: round.admitted_at,
        };
        requested
            .validate()
            .map_err(|_| CreationRepositoryError::Invalid)?;
        let existing_rounds = list_rounds_in(&transaction, owner, attempt_id)?;
        if existing_rounds.len() != usize::from(expected_round_ordinal) {
            if existing_rounds.get(usize::from(expected_round_ordinal)) == Some(&requested) {
                transaction.commit().map_err(storage)?;
                return Ok(requested);
            }
            return Err(CreationRepositoryError::Conflict);
        }
        let existing_calls = list_calls_in(&transaction, owner, attempt_id)?;
        let actual_next =
            u16::try_from(existing_calls.len()).map_err(|_| CreationRepositoryError::Storage)?;
        if actual_next != expected_next_ordinal
            || attempt.status != CreationAttemptStatus::Running
            || requested.admitted_at < attempt.updated_at
        {
            return Err(CreationRepositoryError::Conflict);
        }
        let workflow = load_workflow_conn(&transaction, owner.workflow_id)?;
        let turn = load_turn_conn(&transaction, owner.turn_id)?;
        if workflow.current_proposal_id != attempt.base_proposal_id
            || workflow.stage != attempt.stage
            || turn.base_proposal_id != attempt.base_proposal_id
        {
            return Err(CreationRepositoryError::Conflict);
        }
        let base = load_proposal_conn(&transaction, attempt.base_proposal_id)?;
        let admitted = existing_calls
            .iter()
            .chain(&requested.calls)
            .map(|call| AdmittedCreationToolCall {
                definition_version: call.definition_version,
                call: call.call.clone(),
            })
            .collect::<Vec<_>>();
        if !admitted.is_empty() {
            validate_creation_tool_calls(&base, attempt.planned_proposal_id, &admitted)
                .map_err(|_| CreationRepositoryError::Invalid)?;
        }
        let (round_replay_id, round_replay_retention) = requested
            .provider_replay
            .as_ref()
            .map(|reference| {
                (
                    Some(reference.artifact_id.to_string()),
                    Some("conversation"),
                )
            })
            .unwrap_or((None, None));
        let (input_tokens, output_tokens) = requested
            .usage
            .as_ref()
            .map(|usage| {
                Ok((
                    Some(sql_u64(usage.input_tokens)?),
                    Some(sql_u64(usage.output_tokens)?),
                ))
            })
            .transpose()?
            .unwrap_or((None, None));
        transaction
            .execute(
                "INSERT INTO creation_inference_rounds \
                 (workflow_id,turn_id,attempt_id,ordinal,first_call_ordinal,call_count,parts_json,\
                  provider_replay_artifact_id,provider_replay_retention,input_tokens,output_tokens,\
                  finish_reason,provider_request_id,admitted_at,cached_input_tokens,reasoning_tokens,cache_write_tokens,web_search_requests) \
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18)",
                params![
                    requested.workflow_id.to_string(),
                    requested.turn_id.to_string(),
                    requested.attempt_id.to_string(),
                    i64::from(requested.ordinal),
                    i64::from(requested.first_call_ordinal),
                    i64::try_from(requested.calls.len())
                        .map_err(|_| CreationRepositoryError::Invalid)?,
                    encode_versioned(&requested.parts, CREATION_JSON_VERSION)
                        .map_err(|_| CreationRepositoryError::Invalid)?,
                    round_replay_id,
                    round_replay_retention,
                    input_tokens,
                    output_tokens,
                    match requested.finish_reason {
                        CreationRoundFinishReason::Stop => "stop",
                        CreationRoundFinishReason::Length => "length",
                    },
                    requested.provider_request_id.as_deref(),
                    requested.admitted_at.get(),
                    requested.usage.as_ref().and_then(|u| u.cached_input_tokens).map(sql_u64).transpose()?,
                    requested.usage.as_ref().and_then(|u| u.reasoning_tokens).map(sql_u64).transpose()?,
                    requested.usage.as_ref().and_then(|u| u.cache_write_tokens).map(sql_u64).transpose()?,
                    requested.usage.as_ref().and_then(|u| u.web_search_requests).map(sql_u64).transpose()?,
                ],
            )
            .map_err(|error| match error.sqlite_error_code() {
                Some(rusqlite::ErrorCode::ConstraintViolation) => CreationRepositoryError::Conflict,
                _ => CreationRepositoryError::Storage,
            })?;
        for evidence in &requested.calls {
            let (replay_id, replay_retention) = evidence
                .call
                .provider_replay
                .as_ref()
                .map(|reference| {
                    (
                        Some(reference.artifact_id.to_string()),
                        Some("conversation"),
                    )
                })
                .unwrap_or((None, None));
            transaction
                .execute(
                    "INSERT INTO creation_admitted_tool_calls \
                     (workflow_id,turn_id,attempt_id,round_ordinal,id,ordinal,definition_name,definition_version,\
                      provider_call_id,arguments_json,raw_arguments,provider_replay_artifact_id,\
                      provider_replay_retention,admitted_at) \
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
                    params![
                        evidence.workflow_id.to_string(),
                        evidence.turn_id.to_string(),
                        evidence.attempt_id.to_string(),
                        i64::from(evidence.round_ordinal),
                        evidence.id.to_string(),
                        i64::from(evidence.ordinal),
                        evidence.call.name,
                        i64::from(evidence.definition_version),
                        evidence.call.provider_call_id,
                        encode_versioned(&evidence.call.arguments, CREATION_JSON_VERSION)
                            .map_err(|_| CreationRepositoryError::Invalid)?,
                        evidence.call.raw_arguments,
                        replay_id,
                        replay_retention,
                        evidence.admitted_at.get(),
                    ],
                )
                .map_err(|error| match error.sqlite_error_code() {
                    Some(rusqlite::ErrorCode::ConstraintViolation) => {
                        CreationRepositoryError::Conflict
                    }
                    _ => CreationRepositoryError::Storage,
                })?;
        }
        let stored = list_rounds_in(&transaction, owner, attempt_id)?;
        let admitted = stored
            .last()
            .cloned()
            .ok_or(CreationRepositoryError::Storage)?;
        if admitted != requested {
            return Err(CreationRepositoryError::Conflict);
        }
        transaction.commit().map_err(storage)?;
        Ok(admitted)
    }

    fn list_creation_inference_rounds(
        &self,
        owner: CreationAttemptOwner,
        attempt_id: GenerationAttemptId,
    ) -> Result<Vec<CreationInferenceRound>, CreationRepositoryError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(storage)?;
        let attempt = load_attempt_conn(&transaction, attempt_id)?;
        if attempt.workflow_id != owner.workflow_id || attempt.turn_id != owner.turn_id {
            return Err(CreationRepositoryError::Conflict);
        }
        let rounds = list_rounds_in(&transaction, owner, attempt_id)?;
        transaction.commit().map_err(storage)?;
        Ok(rounds)
    }

    fn list_creation_tool_calls(
        &self,
        owner: CreationAttemptOwner,
        attempt_id: GenerationAttemptId,
    ) -> Result<Vec<CreationToolCallEvidence>, CreationRepositoryError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(storage)?;
        let attempt = load_attempt_conn(&transaction, attempt_id)?;
        if attempt.workflow_id != owner.workflow_id || attempt.turn_id != owner.turn_id {
            return Err(CreationRepositoryError::Conflict);
        }
        let calls = list_calls_in(&transaction, owner, attempt_id)?;
        transaction.commit().map_err(storage)?;
        Ok(calls)
    }
}

#[cfg(test)]
mod tests {
    use lettuce_conversations::{
        ArtifactCodec, ArtifactRetention, ConversationArtifactStore, MessagePart, ProposedToolCall,
        ProtectedArtifactBytes, ReplayArtifactDraft, ReplayArtifactRef,
    };
    use lettuce_creation::{
        AdmittedCreationToolCall, ConfirmedCharacterApply, ConfirmedCharacterRevisionApply,
        ConfirmedLorebookApply, ConfirmedLorebookRevisionApply, ConfirmedPersonaApply,
        ConfirmedPersonaRevisionApply, CreationApplyRepository, CreationAttemptOwner,
        CreationAttemptRepository, CreationAttemptStatus, CreationDraft, CreationLorebookEntry,
        CreationOperation, CreationOperationError, CreationRepositoryError,
        CreationRoundFinishReason, CreationScene, CreationStage, CreationTarget, CreationToolApply,
        CreationWorkflow, CreationWorkflowRepository, NewCreationAttempt,
        NewCreationInferenceRound, NewCreationToolCall, NewCreationTurn, NewCreationWorkflow,
        apply_creation_tool_calls,
    };
    use lettuce_types::{
        AssetId, CharacterId, CreationProposalId, CreationTurnId, CreationWorkflowId,
        GenerationAttemptId, JobId, LorebookEntryId, LorebookId, MediaBlobId, ModelArtifactId,
        PersonaId, ReplayArtifactId, Revision, SceneId, TimestampMillis, ToolExecutionId,
    };

    use lettuce_characters::{
        Character, CharacterDefaults, CharacterMedia, CharacterPresentationV1, CharacterProfile,
        CharacterProvenance, CharacterRepository, CreateCharacterPlan, Crop, ImageRecommendation,
        LifecycleStatus, Persona, PersonaArchiveRequest, PersonaDraftUpdate, PersonaMedia,
        PersonaMediaLink, PersonaMediaSlot, PersonaRepository, Scene, SceneDocumentV1, SceneOwner,
        ScenePart,
    };
    use lettuce_context::{
        DetectionPolicy, KeywordMatchMode, LifecycleStatus as LorebookLifecycleStatus,
        LorebookBehaviorVersion, LorebookEntryDraft, LorebookMetadataDraft, LorebookRepository,
    };

    use crate::Database;

    fn admitted(name: &str, arguments: serde_json::Value) -> AdmittedCreationToolCall {
        AdmittedCreationToolCall {
            definition_version: 1,
            call: ProposedToolCall {
                provider_call_id: Some(format!("call-{name}")),
                name: name.to_owned(),
                arguments,
                raw_arguments: None,
                provider_replay: None,
            },
        }
    }

    fn new_call(name: &str, arguments: serde_json::Value) -> NewCreationToolCall {
        NewCreationToolCall {
            id: ToolExecutionId::new(),
            definition_version: 1,
            call: ProposedToolCall {
                provider_call_id: Some(format!("provider-{}", ToolExecutionId::new())),
                name: name.to_owned(),
                arguments,
                raw_arguments: None,
                provider_replay: None,
            },
        }
    }

    fn new_round(
        ordinal: u8,
        mut calls: Vec<NewCreationToolCall>,
        text: Option<&str>,
        replay: Option<ReplayArtifactRef>,
        admitted_at: TimestampMillis,
    ) -> NewCreationInferenceRound {
        for call in &mut calls {
            call.call.provider_replay = replay.clone();
        }
        NewCreationInferenceRound {
            ordinal,
            parts: text
                .map(|text| vec![MessagePart::Text { text: text.into() }])
                .unwrap_or_default(),
            provider_replay: replay,
            usage: Some(lettuce_conversations::InferenceUsage {
                cache_write_tokens: Some(3),
                web_search_requests: Some(0),
                cached_input_tokens: Some(0),
                reasoning_tokens: Some(1),
                input_tokens: 10 + u64::from(ordinal),
                output_tokens: 2 + u64::from(ordinal),
            }),
            finish_reason: CreationRoundFinishReason::Stop,
            provider_request_id: Some(format!("request-{ordinal}")),
            calls,
            admitted_at,
        }
    }

    fn confirmed_workflow(
        database: &Database,
        target: CreationTarget,
        draft: CreationDraft,
        timestamp: i64,
    ) -> (CreationWorkflow, CreationProposalId) {
        let workflow_id = CreationWorkflowId::new();
        let initial_id = CreationProposalId::new();
        let initial = database
            .create_workflow(NewCreationWorkflow {
                id: workflow_id,
                initial_proposal_id: initial_id,
                target,
                initial_draft: draft,
                now: TimestampMillis::new(timestamp),
            })
            .expect("create workflow");
        let review_turn = database
            .record_user_turn(NewCreationTurn {
                id: CreationTurnId::new(),
                workflow_id,
                base_proposal_id: initial_id,
                user_message: "Show the preview".into(),
                now: TimestampMillis::new(timestamp + 1),
            })
            .expect("record review turn");
        let reviewed_proposal = database
            .load_proposal(initial_id)
            .expect("load initial proposal")
            .apply(
                CreationProposalId::new(),
                review_turn.id,
                vec![CreationOperation::ShowPreview],
                TimestampMillis::new(timestamp + 2),
            )
            .expect("build review proposal");
        let reviewed = database
            .append_proposal(workflow_id, initial.revision, reviewed_proposal.clone())
            .expect("append review proposal");
        let confirmation_turn = database
            .record_user_turn(NewCreationTurn {
                id: CreationTurnId::new(),
                workflow_id,
                base_proposal_id: reviewed_proposal.id,
                user_message: "Confirm".into(),
                now: TimestampMillis::new(timestamp + 3),
            })
            .expect("record confirmation turn");
        let confirmation = reviewed_proposal
            .apply(
                CreationProposalId::new(),
                confirmation_turn.id,
                vec![CreationOperation::RequestConfirmation],
                TimestampMillis::new(timestamp + 4),
            )
            .expect("build confirmation proposal");
        let workflow = database
            .append_proposal(workflow_id, reviewed.revision, confirmation.clone())
            .expect("append confirmation proposal");
        (workflow, confirmation.id)
    }

    fn lorebook_entry_draft(
        title: &str,
        content: &str,
        enabled: bool,
        always_active: bool,
        keywords: Vec<&str>,
        case_sensitive: bool,
        priority: i32,
    ) -> LorebookEntryDraft {
        LorebookEntryDraft {
            title: title.into(),
            enabled,
            always_active,
            keywords: keywords.into_iter().map(str::to_owned).collect(),
            case_sensitive,
            match_mode: KeywordMatchMode::Literal,
            content: content.into(),
            priority,
        }
    }

    fn image_asset(database: &Database, marker: u8) -> AssetId {
        let blob_id = MediaBlobId::new();
        let asset_id = AssetId::new();
        let connection = database.connection().expect("database lock");
        connection
            .execute(
                "INSERT INTO media_blobs \
                 (id,content_hash,kind,mime_type,byte_size,width,height,validation_version,state,created_at,updated_at) \
                 VALUES (?1,?2,'image','image/png',1,1,1,1,'ready',1,1)",
                rusqlite::params![blob_id.to_string(), format!("{marker:02x}").repeat(32)],
            )
            .expect("insert image blob");
        connection
            .execute(
                "INSERT INTO media_assets \
                 (id,blob_id,blob_kind,kind,origin,retention,provenance_json,revision,created_at,updated_at) \
                 VALUES (?1,?2,'image','illustration','upload','library','{}',1,1,1)",
                rusqlite::params![asset_id.to_string(), blob_id.to_string()],
            )
            .expect("insert image asset");
        asset_id
    }

    fn rich_persona(database: &Database, title: &str) -> Persona {
        let mut persona = Persona::new(
            PersonaId::new(),
            title.into(),
            "Original description".into(),
            TimestampMillis::new(1),
        )
        .expect("persona");
        persona.nickname = Some("Nick".into());
        persona.design_description = Some("Original design".into());
        persona.avatar_crop = Some(Crop::new(0.2, 0.3, 1.2).expect("crop"));
        persona.image_recommendation = Some(ImageRecommendation {
            artifact_id: Some(ModelArtifactId::new()),
            unresolved_legacy_name: None,
            strength: 0.8,
        });
        persona.media = PersonaMedia {
            links: vec![PersonaMediaLink {
                asset_id: image_asset(database, title.as_bytes()[0]),
                slot: PersonaMediaSlot::Avatar,
                ordinal: 0,
            }],
        };
        PersonaRepository::create(database, persona).expect("create persona")
    }

    #[test]
    fn confirmed_new_persona_apply_is_atomic_idempotent_and_domain_validated() {
        let database = Database::open_in_memory().expect("database");
        let (workflow, proposal_id) = confirmed_workflow(
            &database,
            CreationTarget::NewPersona,
            CreationDraft::Persona {
                name: Some("Navigator".into()),
                description: Some("Keeps the journey on course.".into()),
            },
            10,
        );
        let persona_id = PersonaId::new();
        let request = ConfirmedPersonaApply {
            workflow_id: workflow.id,
            expected_workflow_revision: workflow.revision,
            proposal_id,
            destination_persona_id: persona_id,
            now: TimestampMillis::new(15),
        };
        let receipt = database
            .apply_new_persona(request.clone())
            .expect("apply persona");
        assert_eq!(receipt.persona_id, persona_id);
        assert_eq!(receipt.persona_revision, Revision::INITIAL);
        let persona = PersonaRepository::get(&database, persona_id)
            .expect("load persona")
            .expect("persona exists");
        assert_eq!(persona.title, "Navigator");
        assert_eq!(persona.description, "Keeps the journey on course.");
        assert!(persona.media.links.is_empty());
        assert_eq!(
            PersonaRepository::get_default_snapshot(&database)
                .expect("default snapshot")
                .state
                .persona_id,
            None
        );

        let mut retry = request.clone();
        retry.now = TimestampMillis::new(99);
        assert_eq!(
            database.apply_new_persona(retry).expect("exact retry"),
            receipt
        );
        let mut changed_destination = request.clone();
        changed_destination.destination_persona_id = PersonaId::new();
        assert_eq!(
            database.apply_new_persona(changed_destination),
            Err(CreationRepositoryError::Conflict)
        );
        let mut changed_revision = request.clone();
        changed_revision.expected_workflow_revision =
            Revision::new(request.expected_workflow_revision.get() + 1);
        assert_eq!(
            database.apply_new_persona(changed_revision),
            Err(CreationRepositoryError::Conflict)
        );

        let connection = database.connection().expect("database lock");
        assert!(
            connection
                .execute(
                    "UPDATE creation_apply_receipts SET applied_at=100 WHERE workflow_id=?1",
                    [workflow.id.to_string()],
                )
                .is_err()
        );
        assert!(
            connection
                .execute(
                    "DELETE FROM creation_apply_receipts WHERE workflow_id=?1",
                    [workflow.id.to_string()],
                )
                .is_err()
        );
        assert_eq!(
            connection
                .query_row("SELECT count(*) FROM personas", [], |row| row
                    .get::<_, i64>(0))
                .expect("persona count"),
            1
        );
    }

    #[test]
    fn new_persona_apply_rejects_unconfirmed_stale_incomplete_and_duplicate_inputs() {
        let database = Database::open_in_memory().expect("database");
        assert_eq!(
            database.create_workflow(NewCreationWorkflow {
                id: CreationWorkflowId::new(),
                initial_proposal_id: CreationProposalId::new(),
                target: CreationTarget::NewPersona,
                initial_draft: CreationDraft::Persona {
                    name: Some(" ".into()),
                    description: Some("Description".into()),
                },
                now: TimestampMillis::new(1),
            }),
            Err(CreationRepositoryError::Invalid)
        );
        let draft = CreationDraft::Persona {
            name: Some("Complete".into()),
            description: Some("Complete description".into()),
        };
        let drafting_id = CreationWorkflowId::new();
        let drafting_proposal_id = CreationProposalId::new();
        let drafting = database
            .create_workflow(NewCreationWorkflow {
                id: drafting_id,
                initial_proposal_id: drafting_proposal_id,
                target: CreationTarget::NewPersona,
                initial_draft: draft.clone(),
                now: TimestampMillis::new(1),
            })
            .expect("drafting workflow");
        let destination = PersonaId::new();
        assert_eq!(
            database.apply_new_persona(ConfirmedPersonaApply {
                workflow_id: drafting.id,
                expected_workflow_revision: drafting.revision,
                proposal_id: drafting_proposal_id,
                destination_persona_id: destination,
                now: TimestampMillis::new(2),
            }),
            Err(CreationRepositoryError::Conflict)
        );

        let (confirmed, confirmed_proposal_id) =
            confirmed_workflow(&database, CreationTarget::NewPersona, draft.clone(), 10);
        let base_request = ConfirmedPersonaApply {
            workflow_id: confirmed.id,
            expected_workflow_revision: confirmed.revision,
            proposal_id: confirmed_proposal_id,
            destination_persona_id: destination,
            now: TimestampMillis::new(15),
        };
        let mut stale_proposal = base_request.clone();
        stale_proposal.proposal_id = CreationProposalId::new();
        assert_eq!(
            database.apply_new_persona(stale_proposal),
            Err(CreationRepositoryError::Conflict)
        );
        let mut stale_workflow = base_request.clone();
        stale_workflow.expected_workflow_revision = Revision::new(2);
        assert_eq!(
            database.apply_new_persona(stale_workflow),
            Err(CreationRepositoryError::Conflict)
        );

        let (incomplete, incomplete_proposal_id) = confirmed_workflow(
            &database,
            CreationTarget::NewPersona,
            CreationDraft::Persona {
                name: Some("Incomplete".into()),
                description: None,
            },
            20,
        );
        assert_eq!(
            database.apply_new_persona(ConfirmedPersonaApply {
                workflow_id: incomplete.id,
                expected_workflow_revision: incomplete.revision,
                proposal_id: incomplete_proposal_id,
                destination_persona_id: PersonaId::new(),
                now: TimestampMillis::new(25),
            }),
            Err(CreationRepositoryError::Invalid)
        );

        let (character, character_proposal_id) = confirmed_workflow(
            &database,
            CreationTarget::NewCharacter,
            CreationDraft::Character {
                name: Some("Wrong target".into()),
                definition: Some("Not a persona".into()),
                scenes: Vec::new(),
            },
            30,
        );
        assert_eq!(
            database.apply_new_persona(ConfirmedPersonaApply {
                workflow_id: character.id,
                expected_workflow_revision: character.revision,
                proposal_id: character_proposal_id,
                destination_persona_id: PersonaId::new(),
                now: TimestampMillis::new(35),
            }),
            Err(CreationRepositoryError::Conflict)
        );

        database
            .apply_new_persona(base_request)
            .expect("first destination apply");
        let (duplicate, duplicate_proposal_id) =
            confirmed_workflow(&database, CreationTarget::NewPersona, draft, 40);
        assert_eq!(
            database.apply_new_persona(ConfirmedPersonaApply {
                workflow_id: duplicate.id,
                expected_workflow_revision: duplicate.revision,
                proposal_id: duplicate_proposal_id,
                destination_persona_id: destination,
                now: TimestampMillis::new(45),
            }),
            Err(CreationRepositoryError::Conflict)
        );
    }

    #[test]
    fn confirmed_existing_persona_apply_preserves_owned_fields_and_retries_exactly() {
        let database = Database::open_in_memory().expect("database");
        let original = rich_persona(&database, "Original");
        let default = PersonaRepository::set_default(
            &database,
            original.id,
            Revision::INITIAL,
            TimestampMillis::new(2),
        )
        .expect("set default");
        let (workflow, proposal_id) = confirmed_workflow(
            &database,
            CreationTarget::ExistingPersona {
                id: original.id,
                revision: original.revision,
            },
            CreationDraft::Persona {
                name: Some("Revised".into()),
                description: Some("Revised description".into()),
            },
            10,
        );
        let request = ConfirmedPersonaRevisionApply {
            workflow_id: workflow.id,
            expected_workflow_revision: workflow.revision,
            proposal_id,
            persona_id: original.id,
            expected_persona_revision: original.revision,
            now: TimestampMillis::new(15),
        };
        let receipt = database
            .apply_existing_persona(request.clone())
            .expect("apply existing persona");
        assert_eq!(receipt.persona_revision, Revision::new(2));
        let revised = PersonaRepository::get(&database, original.id)
            .expect("load persona")
            .expect("persona exists");
        assert_eq!(revised.title, "Revised");
        assert_eq!(revised.description, "Revised description");
        assert_eq!(revised.nickname, original.nickname);
        assert_eq!(revised.design_description, original.design_description);
        assert_eq!(revised.avatar_crop, original.avatar_crop);
        assert_eq!(revised.image_recommendation, original.image_recommendation);
        assert_eq!(revised.media, original.media);
        assert_eq!(revised.status, LifecycleStatus::Active);
        assert_eq!(revised.created_at, original.created_at);
        assert_eq!(
            PersonaRepository::get_default_snapshot(&database)
                .expect("default snapshot")
                .state,
            default
        );

        let mut retry = request.clone();
        retry.now = TimestampMillis::new(99);
        assert_eq!(
            database
                .apply_existing_persona(retry)
                .expect("exact retry after revision advancement"),
            receipt
        );
        assert_eq!(
            PersonaRepository::get(&database, original.id)
                .expect("load after retry")
                .expect("persona exists")
                .revision,
            Revision::new(2)
        );
        let mut changed_target = request.clone();
        changed_target.persona_id = PersonaId::new();
        assert_eq!(
            database.apply_existing_persona(changed_target),
            Err(CreationRepositoryError::Conflict)
        );
        let mut stale_proposal = request.clone();
        stale_proposal.proposal_id = CreationProposalId::new();
        assert_eq!(
            database.apply_existing_persona(stale_proposal),
            Err(CreationRepositoryError::Conflict)
        );
        let mut stale_workflow = request.clone();
        stale_workflow.expected_workflow_revision =
            Revision::new(request.expected_workflow_revision.get() + 1);
        assert_eq!(
            database.apply_existing_persona(stale_workflow),
            Err(CreationRepositoryError::Conflict)
        );
        let mut stale_target_revision = request.clone();
        stale_target_revision.expected_persona_revision = Revision::new(2);
        assert_eq!(
            database.apply_existing_persona(stale_target_revision),
            Err(CreationRepositoryError::Conflict)
        );

        let (second_workflow, second_proposal_id) = confirmed_workflow(
            &database,
            CreationTarget::ExistingPersona {
                id: revised.id,
                revision: revised.revision,
            },
            CreationDraft::Persona {
                name: Some("Revised again".into()),
                description: Some("Second revision".into()),
            },
            100,
        );
        let second_receipt = database
            .apply_existing_persona(ConfirmedPersonaRevisionApply {
                workflow_id: second_workflow.id,
                expected_workflow_revision: second_workflow.revision,
                proposal_id: second_proposal_id,
                persona_id: revised.id,
                expected_persona_revision: revised.revision,
                now: TimestampMillis::new(105),
            })
            .expect("apply second workflow");
        assert_eq!(second_receipt.persona_revision, Revision::new(3));
        let connection = database.connection().expect("database lock");
        assert_eq!(
            connection
                .query_row(
                    "SELECT count(*) FROM creation_apply_receipts WHERE persona_id=?1",
                    [original.id.to_string()],
                    |row| row.get::<_, i64>(0),
                )
                .expect("receipt count"),
            2
        );
    }

    #[test]
    fn existing_persona_apply_rejects_stale_archived_missing_and_incomplete_targets() {
        let database = Database::open_in_memory().expect("database");
        let stale = rich_persona(&database, "Stale");
        let drafting_proposal_id = CreationProposalId::new();
        let drafting = database
            .create_workflow(NewCreationWorkflow {
                id: CreationWorkflowId::new(),
                initial_proposal_id: drafting_proposal_id,
                target: CreationTarget::ExistingPersona {
                    id: stale.id,
                    revision: stale.revision,
                },
                initial_draft: CreationDraft::Persona {
                    name: Some("Drafting".into()),
                    description: Some("Not confirmed".into()),
                },
                now: TimestampMillis::new(2),
            })
            .expect("drafting workflow");
        assert_eq!(
            database.apply_existing_persona(ConfirmedPersonaRevisionApply {
                workflow_id: drafting.id,
                expected_workflow_revision: drafting.revision,
                proposal_id: drafting_proposal_id,
                persona_id: stale.id,
                expected_persona_revision: stale.revision,
                now: TimestampMillis::new(3),
            }),
            Err(CreationRepositoryError::Conflict)
        );
        let (wrong_target, wrong_target_proposal_id) = confirmed_workflow(
            &database,
            CreationTarget::NewPersona,
            CreationDraft::Persona {
                name: Some("Wrong target".into()),
                description: Some("Not an existing target".into()),
            },
            4,
        );
        assert_eq!(
            database.apply_existing_persona(ConfirmedPersonaRevisionApply {
                workflow_id: wrong_target.id,
                expected_workflow_revision: wrong_target.revision,
                proposal_id: wrong_target_proposal_id,
                persona_id: stale.id,
                expected_persona_revision: stale.revision,
                now: TimestampMillis::new(9),
            }),
            Err(CreationRepositoryError::Conflict)
        );
        let (stale_workflow, stale_proposal_id) = confirmed_workflow(
            &database,
            CreationTarget::ExistingPersona {
                id: stale.id,
                revision: stale.revision,
            },
            CreationDraft::Persona {
                name: Some("Stale edit".into()),
                description: Some("Should conflict".into()),
            },
            10,
        );
        PersonaRepository::revise(
            &database,
            stale.id,
            stale.revision,
            PersonaDraftUpdate {
                title: stale.title.clone(),
                description: stale.description.clone(),
                nickname: stale.nickname.clone(),
                design_description: stale.design_description.clone(),
                avatar_crop: stale.avatar_crop,
                image_recommendation: stale.image_recommendation.clone(),
            },
            TimestampMillis::new(15),
        )
        .expect("advance persona");
        assert_eq!(
            database.apply_existing_persona(ConfirmedPersonaRevisionApply {
                workflow_id: stale_workflow.id,
                expected_workflow_revision: stale_workflow.revision,
                proposal_id: stale_proposal_id,
                persona_id: stale.id,
                expected_persona_revision: stale.revision,
                now: TimestampMillis::new(16),
            }),
            Err(CreationRepositoryError::Conflict)
        );

        let archived = rich_persona(&database, "Archived");
        let archived = PersonaRepository::archive(
            &database,
            PersonaArchiveRequest {
                persona_id: archived.id,
                expected_persona_revision: archived.revision,
                expected_default_revision: None,
                now: TimestampMillis::new(20),
            },
        )
        .expect("archive persona")
        .persona;
        let (archived_workflow, archived_proposal_id) = confirmed_workflow(
            &database,
            CreationTarget::ExistingPersona {
                id: archived.id,
                revision: archived.revision,
            },
            CreationDraft::Persona {
                name: Some("Archived edit".into()),
                description: Some("Should conflict".into()),
            },
            30,
        );
        assert_eq!(
            database.apply_existing_persona(ConfirmedPersonaRevisionApply {
                workflow_id: archived_workflow.id,
                expected_workflow_revision: archived_workflow.revision,
                proposal_id: archived_proposal_id,
                persona_id: archived.id,
                expected_persona_revision: archived.revision,
                now: TimestampMillis::new(35),
            }),
            Err(CreationRepositoryError::Conflict)
        );

        let missing_id = PersonaId::new();
        let (missing_workflow, missing_proposal_id) = confirmed_workflow(
            &database,
            CreationTarget::ExistingPersona {
                id: missing_id,
                revision: Revision::INITIAL,
            },
            CreationDraft::Persona {
                name: Some("Missing".into()),
                description: Some("Missing persona".into()),
            },
            40,
        );
        assert_eq!(
            database.apply_existing_persona(ConfirmedPersonaRevisionApply {
                workflow_id: missing_workflow.id,
                expected_workflow_revision: missing_workflow.revision,
                proposal_id: missing_proposal_id,
                persona_id: missing_id,
                expected_persona_revision: Revision::INITIAL,
                now: TimestampMillis::new(45),
            }),
            Err(CreationRepositoryError::NotFound)
        );

        let incomplete = rich_persona(&database, "Incomplete");
        let (incomplete_workflow, incomplete_proposal_id) = confirmed_workflow(
            &database,
            CreationTarget::ExistingPersona {
                id: incomplete.id,
                revision: incomplete.revision,
            },
            CreationDraft::Persona {
                name: Some("Incomplete edit".into()),
                description: None,
            },
            50,
        );
        assert_eq!(
            database.apply_existing_persona(ConfirmedPersonaRevisionApply {
                workflow_id: incomplete_workflow.id,
                expected_workflow_revision: incomplete_workflow.revision,
                proposal_id: incomplete_proposal_id,
                persona_id: incomplete.id,
                expected_persona_revision: incomplete.revision,
                now: TimestampMillis::new(55),
            }),
            Err(CreationRepositoryError::Invalid)
        );
    }

    #[test]
    fn confirmed_new_character_apply_commits_the_exact_minimal_graph_and_retries() {
        let database = Database::open_in_memory().expect("database");
        let first_scene_id = SceneId::new();
        let second_scene_id = SceneId::new();
        let (workflow, proposal_id) = confirmed_workflow(
            &database,
            CreationTarget::NewCharacter,
            CreationDraft::Character {
                name: Some("Aster".into()),
                definition: Some("A quiet navigator with a precise voice.".into()),
                scenes: vec![
                    CreationScene {
                        id: first_scene_id,
                        content: "The observatory is silent.".into(),
                        direction: Some("Speak softly".into()),
                    },
                    CreationScene {
                        id: second_scene_id,
                        content: "Dawn reaches the harbor.".into(),
                        direction: None,
                    },
                ],
            },
            10,
        );
        let character_id = CharacterId::new();
        let request = ConfirmedCharacterApply {
            workflow_id: workflow.id,
            expected_workflow_revision: workflow.revision,
            proposal_id,
            destination_character_id: character_id,
            now: TimestampMillis::new(15),
        };
        let receipt = database
            .apply_new_character(request.clone())
            .expect("apply character");
        assert_eq!(receipt.character_id, character_id);
        assert_eq!(receipt.character_revision, Revision::INITIAL);
        let details = CharacterRepository::get(&database, character_id)
            .expect("load character")
            .expect("character exists");
        assert_eq!(details.character.profile.name, "Aster");
        assert_eq!(
            details.character.profile.definition.as_deref(),
            Some("A quiet navigator with a precise voice.")
        );
        assert_eq!(details.character.profile.description, None);
        assert_eq!(details.character.provenance, CharacterProvenance::default());
        assert_eq!(details.character.defaults, CharacterDefaults::default());
        assert_eq!(
            details.character.presentation,
            CharacterPresentationV1::default()
        );
        assert_eq!(details.character.media, CharacterMedia::default());
        assert_eq!(details.character.image_recommendation, None);
        assert!(details.variants.is_empty());
        assert!(details.starters.is_empty());
        assert_eq!(
            details
                .scenes
                .iter()
                .map(|scene| (scene.id, scene.ordinal, scene.direction.as_deref()))
                .collect::<Vec<_>>(),
            vec![
                (first_scene_id, 0, Some("Speak softly")),
                (second_scene_id, 1, None),
            ]
        );
        assert_eq!(
            details.scenes[0].content.parts,
            vec![ScenePart::Text {
                text: "The observatory is silent.".into()
            }]
        );
        assert_eq!(
            details.scenes[1].content.parts,
            vec![ScenePart::Text {
                text: "Dawn reaches the harbor.".into()
            }]
        );

        let mut retry = request.clone();
        retry.now = TimestampMillis::new(99);
        assert_eq!(
            database.apply_new_character(retry).expect("exact retry"),
            receipt
        );
        let mut changed_destination = request.clone();
        changed_destination.destination_character_id = CharacterId::new();
        assert_eq!(
            database.apply_new_character(changed_destination),
            Err(CreationRepositoryError::Conflict)
        );
        let mut changed_proposal = request.clone();
        changed_proposal.proposal_id = CreationProposalId::new();
        assert_eq!(
            database.apply_new_character(changed_proposal),
            Err(CreationRepositoryError::Conflict)
        );
        let mut changed_revision = request.clone();
        changed_revision.expected_workflow_revision =
            Revision::new(request.expected_workflow_revision.get() + 1);
        assert_eq!(
            database.apply_new_character(changed_revision),
            Err(CreationRepositoryError::Conflict)
        );

        let connection = database.connection().expect("database lock");
        assert!(
            connection
                .execute(
                    "UPDATE creation_character_apply_receipts SET applied_at=100 WHERE workflow_id=?1",
                    [workflow.id.to_string()],
                )
                .is_err()
        );
        assert!(
            connection
                .execute(
                    "DELETE FROM creation_character_apply_receipts WHERE workflow_id=?1",
                    [workflow.id.to_string()],
                )
                .is_err()
        );
    }

    #[test]
    fn new_character_apply_rejects_invalid_state_and_accepts_an_empty_optional_graph() {
        let database = Database::open_in_memory().expect("database");
        let duplicate_scene_id = SceneId::new();
        assert_eq!(
            database.create_workflow(NewCreationWorkflow {
                id: CreationWorkflowId::new(),
                initial_proposal_id: CreationProposalId::new(),
                target: CreationTarget::NewCharacter,
                initial_draft: CreationDraft::Character {
                    name: Some("Duplicate scenes".into()),
                    definition: Some("Invalid graph".into()),
                    scenes: vec![
                        CreationScene {
                            id: duplicate_scene_id,
                            content: "First".into(),
                            direction: None,
                        },
                        CreationScene {
                            id: duplicate_scene_id,
                            content: "Second".into(),
                            direction: None,
                        },
                    ],
                },
                now: TimestampMillis::new(1),
            }),
            Err(CreationRepositoryError::Invalid)
        );

        let drafting_proposal_id = CreationProposalId::new();
        let drafting = database
            .create_workflow(NewCreationWorkflow {
                id: CreationWorkflowId::new(),
                initial_proposal_id: drafting_proposal_id,
                target: CreationTarget::NewCharacter,
                initial_draft: CreationDraft::Character {
                    name: Some("Drafting".into()),
                    definition: Some("Not confirmed".into()),
                    scenes: Vec::new(),
                },
                now: TimestampMillis::new(2),
            })
            .expect("drafting workflow");
        assert_eq!(
            database.apply_new_character(ConfirmedCharacterApply {
                workflow_id: drafting.id,
                expected_workflow_revision: drafting.revision,
                proposal_id: drafting_proposal_id,
                destination_character_id: CharacterId::new(),
                now: TimestampMillis::new(3),
            }),
            Err(CreationRepositoryError::Conflict)
        );

        let (wrong_target, wrong_target_proposal_id) = confirmed_workflow(
            &database,
            CreationTarget::NewPersona,
            CreationDraft::Persona {
                name: Some("Wrong target".into()),
                description: Some("Not a character".into()),
            },
            10,
        );
        assert_eq!(
            database.apply_new_character(ConfirmedCharacterApply {
                workflow_id: wrong_target.id,
                expected_workflow_revision: wrong_target.revision,
                proposal_id: wrong_target_proposal_id,
                destination_character_id: CharacterId::new(),
                now: TimestampMillis::new(15),
            }),
            Err(CreationRepositoryError::Conflict)
        );

        for (name, definition) in [(None, Some("Definition")), (Some("Name"), None)] {
            let (incomplete, incomplete_proposal_id) = confirmed_workflow(
                &database,
                CreationTarget::NewCharacter,
                CreationDraft::Character {
                    name: name.map(str::to_owned),
                    definition: definition.map(str::to_owned),
                    scenes: Vec::new(),
                },
                20,
            );
            assert_eq!(
                database.apply_new_character(ConfirmedCharacterApply {
                    workflow_id: incomplete.id,
                    expected_workflow_revision: incomplete.revision,
                    proposal_id: incomplete_proposal_id,
                    destination_character_id: CharacterId::new(),
                    now: TimestampMillis::new(25),
                }),
                Err(CreationRepositoryError::Invalid)
            );
        }

        let (empty, empty_proposal_id) = confirmed_workflow(
            &database,
            CreationTarget::NewCharacter,
            CreationDraft::Character {
                name: Some("Minimal".into()),
                definition: Some("A complete minimal character.".into()),
                scenes: Vec::new(),
            },
            30,
        );
        let minimal_id = CharacterId::new();
        database
            .apply_new_character(ConfirmedCharacterApply {
                workflow_id: empty.id,
                expected_workflow_revision: empty.revision,
                proposal_id: empty_proposal_id,
                destination_character_id: minimal_id,
                now: TimestampMillis::new(35),
            })
            .expect("apply minimal character");
        let minimal = CharacterRepository::get(&database, minimal_id)
            .expect("load minimal")
            .expect("minimal character exists");
        assert!(minimal.scenes.is_empty());
        assert!(minimal.variants.is_empty());
        assert!(minimal.starters.is_empty());

        let (duplicate, duplicate_proposal_id) = confirmed_workflow(
            &database,
            CreationTarget::NewCharacter,
            CreationDraft::Character {
                name: Some("Duplicate destination".into()),
                definition: Some("Must roll back".into()),
                scenes: Vec::new(),
            },
            40,
        );
        assert_eq!(
            database.apply_new_character(ConfirmedCharacterApply {
                workflow_id: duplicate.id,
                expected_workflow_revision: duplicate.revision,
                proposal_id: duplicate_proposal_id,
                destination_character_id: minimal_id,
                now: TimestampMillis::new(45),
            }),
            Err(CreationRepositoryError::Conflict)
        );
        let connection = database.connection().expect("database lock");
        assert_eq!(
            connection
                .query_row(
                    "SELECT count(*) FROM creation_character_apply_receipts WHERE workflow_id=?1",
                    [duplicate.id.to_string()],
                    |row| row.get::<_, i64>(0),
                )
                .expect("receipt count"),
            0
        );
    }

    #[test]
    fn confirmed_existing_character_apply_reconciles_scenes_and_preserves_profile_graph() {
        let database = Database::open_in_memory().expect("database");
        let character_id = CharacterId::new();
        let removed_id = SceneId::new();
        let retained_id = SceneId::new();
        let mut character = Character::new(
            character_id,
            CharacterProfile {
                name: "Old name".into(),
                nickname: Some("Navigator".into()),
                description: Some("Preserved description".into()),
                definition: Some("Old definition".into()),
                design_description: Some("Preserved design".into()),
            },
            CharacterProvenance::default(),
            CharacterDefaults::default(),
            CharacterPresentationV1::default(),
            None,
            CharacterMedia::default(),
            TimestampMillis::new(1),
        )
        .expect("character");
        character.image_recommendation = Some(ImageRecommendation {
            artifact_id: None,
            unresolved_legacy_name: Some("Preserved recommendation".into()),
            strength: 0.75,
        });
        character.validate().expect("valid character");
        let removed = Scene::new(
            removed_id,
            SceneOwner::Character(character_id),
            0,
            SceneDocumentV1::new(vec![ScenePart::Text {
                text: "Remove me".into(),
            }])
            .expect("document"),
            TimestampMillis::new(1),
        )
        .expect("removed scene");
        let mut retained = Scene::new(
            retained_id,
            SceneOwner::Character(character_id),
            1,
            SceneDocumentV1::new(vec![ScenePart::Text {
                text: "Old retained text".into(),
            }])
            .expect("document"),
            TimestampMillis::new(1),
        )
        .expect("retained scene");
        retained.direction = Some("Old direction".into());
        let original = CharacterRepository::create(
            &database,
            CreateCharacterPlan {
                character,
                scenes: vec![removed, retained.clone()],
                variants: Vec::new(),
                starters: Vec::new(),
            },
        )
        .expect("create character");
        let added_id = SceneId::new();
        let (workflow, proposal_id) = confirmed_workflow(
            &database,
            CreationTarget::ExistingCharacter {
                id: character_id,
                revision: original.character.revision,
            },
            CreationDraft::Character {
                name: Some("New name".into()),
                definition: Some("New definition".into()),
                scenes: vec![
                    CreationScene {
                        id: retained_id,
                        content: "Updated retained text".into(),
                        direction: Some("New direction".into()),
                    },
                    CreationScene {
                        id: added_id,
                        content: "Brand new scene".into(),
                        direction: None,
                    },
                ],
            },
            10,
        );
        let request = ConfirmedCharacterRevisionApply {
            workflow_id: workflow.id,
            expected_workflow_revision: workflow.revision,
            proposal_id,
            character_id,
            expected_character_revision: original.character.revision,
            now: TimestampMillis::new(15),
        };
        let receipt = database
            .apply_existing_character(request.clone())
            .expect("apply character edit");
        assert_eq!(receipt.character_revision, Revision::new(2));
        let revised = CharacterRepository::get(&database, character_id)
            .expect("load revised")
            .expect("revised exists");
        assert_eq!(revised.character.profile.name, "New name");
        assert_eq!(
            revised.character.profile.definition.as_deref(),
            Some("New definition")
        );
        assert_eq!(
            revised.character.profile.nickname,
            original.character.profile.nickname
        );
        assert_eq!(
            revised.character.profile.description,
            original.character.profile.description
        );
        assert_eq!(
            revised.character.profile.design_description,
            original.character.profile.design_description
        );
        assert_eq!(revised.character.provenance, original.character.provenance);
        assert_eq!(revised.character.defaults, original.character.defaults);
        assert_eq!(
            revised.character.presentation,
            original.character.presentation
        );
        assert_eq!(revised.character.media, original.character.media);
        assert_eq!(
            revised.character.image_recommendation,
            original.character.image_recommendation
        );
        assert_eq!(
            revised
                .scenes
                .iter()
                .map(|scene| (scene.id, scene.ordinal, scene.direction.as_deref()))
                .collect::<Vec<_>>(),
            vec![(retained_id, 0, Some("New direction")), (added_id, 1, None)]
        );
        assert_eq!(revised.scenes[0].revision, Revision::new(2));
        assert_eq!(revised.scenes[0].created_at, retained.created_at);
        assert_eq!(revised.scenes[1].revision, Revision::INITIAL);
        assert_eq!(
            revised.scenes[0].content.parts,
            vec![ScenePart::Text {
                text: "Updated retained text".into()
            }]
        );
        let mut retry = request.clone();
        retry.now = TimestampMillis::new(99);
        assert_eq!(
            database
                .apply_existing_character(retry)
                .expect("exact retry"),
            receipt
        );
        let mut changed_target = request.clone();
        changed_target.character_id = CharacterId::new();
        assert_eq!(
            database.apply_existing_character(changed_target),
            Err(CreationRepositoryError::Conflict)
        );
        let mut changed_proposal = request.clone();
        changed_proposal.proposal_id = CreationProposalId::new();
        assert_eq!(
            database.apply_existing_character(changed_proposal),
            Err(CreationRepositoryError::Conflict)
        );
        let unchanged_scenes = revised
            .scenes
            .iter()
            .map(|scene| CreationScene {
                id: scene.id,
                content: match &scene.content.parts[0] {
                    ScenePart::Text { text } => text.clone(),
                    _ => panic!("expected text scene"),
                },
                direction: scene.direction.clone(),
            })
            .collect();
        let (second_workflow, second_proposal_id) = confirmed_workflow(
            &database,
            CreationTarget::ExistingCharacter {
                id: character_id,
                revision: revised.character.revision,
            },
            CreationDraft::Character {
                name: Some("Final name".into()),
                definition: Some("Final definition".into()),
                scenes: unchanged_scenes,
            },
            20,
        );
        database
            .apply_existing_character(ConfirmedCharacterRevisionApply {
                workflow_id: second_workflow.id,
                expected_workflow_revision: second_workflow.revision,
                proposal_id: second_proposal_id,
                character_id,
                expected_character_revision: revised.character.revision,
                now: TimestampMillis::new(25),
            })
            .expect("second workflow");
        let final_details = CharacterRepository::get(&database, character_id)
            .expect("load final")
            .expect("final exists");
        assert_eq!(final_details.character.revision, Revision::new(3));
        assert_eq!(final_details.character.profile.name, "Final name");
        assert_eq!(final_details.scenes, revised.scenes);
        let connection = database.connection().expect("database lock");
        assert_eq!(
            connection
                .query_row(
                    "SELECT count(*) FROM creation_character_apply_receipts WHERE character_id=?1",
                    [character_id.to_string()],
                    |row| row.get::<_, i64>(0),
                )
                .expect("receipt count"),
            2
        );
    }

    #[test]
    fn existing_character_apply_rejects_dependent_scene_removal_and_incomplete_drafts() {
        let database = Database::open_in_memory().expect("database");
        let character_id = CharacterId::new();
        let scene_id = SceneId::new();
        let defaults = CharacterDefaults {
            default_scene_id: Some(scene_id),
            ..CharacterDefaults::default()
        };
        let character = Character::new(
            character_id,
            CharacterProfile {
                name: "Dependent".into(),
                nickname: None,
                description: None,
                definition: Some("Definition".into()),
                design_description: None,
            },
            CharacterProvenance::default(),
            defaults,
            CharacterPresentationV1::default(),
            None,
            CharacterMedia::default(),
            TimestampMillis::new(1),
        )
        .expect("character");
        let scene = Scene::new(
            scene_id,
            SceneOwner::Character(character_id),
            0,
            SceneDocumentV1::new(vec![ScenePart::Text {
                text: "Default scene".into(),
            }])
            .expect("document"),
            TimestampMillis::new(1),
        )
        .expect("scene");
        let original = CharacterRepository::create(
            &database,
            CreateCharacterPlan {
                character,
                scenes: vec![scene],
                variants: Vec::new(),
                starters: Vec::new(),
            },
        )
        .expect("create character");
        let (dependent, dependent_proposal_id) = confirmed_workflow(
            &database,
            CreationTarget::ExistingCharacter {
                id: character_id,
                revision: original.character.revision,
            },
            CreationDraft::Character {
                name: Some("Dependent edit".into()),
                definition: Some("Updated".into()),
                scenes: Vec::new(),
            },
            10,
        );
        assert_eq!(
            database.apply_existing_character(ConfirmedCharacterRevisionApply {
                workflow_id: dependent.id,
                expected_workflow_revision: dependent.revision,
                proposal_id: dependent_proposal_id,
                character_id,
                expected_character_revision: original.character.revision,
                now: TimestampMillis::new(15),
            }),
            Err(CreationRepositoryError::Conflict)
        );
        assert_eq!(
            CharacterRepository::get(&database, character_id)
                .expect("load after rejection")
                .expect("character exists"),
            original
        );

        let (incomplete, incomplete_proposal_id) = confirmed_workflow(
            &database,
            CreationTarget::ExistingCharacter {
                id: character_id,
                revision: original.character.revision,
            },
            CreationDraft::Character {
                name: Some("Incomplete".into()),
                definition: None,
                scenes: vec![CreationScene {
                    id: scene_id,
                    content: "Default scene".into(),
                    direction: None,
                }],
            },
            20,
        );
        assert_eq!(
            database.apply_existing_character(ConfirmedCharacterRevisionApply {
                workflow_id: incomplete.id,
                expected_workflow_revision: incomplete.revision,
                proposal_id: incomplete_proposal_id,
                character_id,
                expected_character_revision: original.character.revision,
                now: TimestampMillis::new(25),
            }),
            Err(CreationRepositoryError::Invalid)
        );
        let missing_id = CharacterId::new();
        let (missing, missing_proposal_id) = confirmed_workflow(
            &database,
            CreationTarget::ExistingCharacter {
                id: missing_id,
                revision: Revision::INITIAL,
            },
            CreationDraft::Character {
                name: Some("Missing".into()),
                definition: Some("Missing".into()),
                scenes: Vec::new(),
            },
            30,
        );
        assert_eq!(
            database.apply_existing_character(ConfirmedCharacterRevisionApply {
                workflow_id: missing.id,
                expected_workflow_revision: missing.revision,
                proposal_id: missing_proposal_id,
                character_id: missing_id,
                expected_character_revision: Revision::INITIAL,
                now: TimestampMillis::new(35),
            }),
            Err(CreationRepositoryError::NotFound)
        );
        let archived = CharacterRepository::archive(
            &database,
            character_id,
            original.character.revision,
            TimestampMillis::new(40),
        )
        .expect("archive character");
        let (archived_workflow, archived_proposal_id) = confirmed_workflow(
            &database,
            CreationTarget::ExistingCharacter {
                id: character_id,
                revision: archived.revision,
            },
            CreationDraft::Character {
                name: Some("Archived edit".into()),
                definition: Some("Archived".into()),
                scenes: vec![CreationScene {
                    id: scene_id,
                    content: "Default scene".into(),
                    direction: None,
                }],
            },
            41,
        );
        assert_eq!(
            database.apply_existing_character(ConfirmedCharacterRevisionApply {
                workflow_id: archived_workflow.id,
                expected_workflow_revision: archived_workflow.revision,
                proposal_id: archived_proposal_id,
                character_id,
                expected_character_revision: archived.revision,
                now: TimestampMillis::new(46),
            }),
            Err(CreationRepositoryError::Conflict)
        );
    }

    #[test]
    fn confirmed_new_lorebook_apply_commits_ordered_always_active_entries_and_retries() {
        let database = Database::open_in_memory().expect("database");
        let first_entry_id = LorebookEntryId::new();
        let second_entry_id = LorebookEntryId::new();
        let (workflow, proposal_id) = confirmed_workflow(
            &database,
            CreationTarget::NewLorebook,
            CreationDraft::Lorebook {
                name: Some("Harbor lore".into()),
                description: Some("Proposal-only summary".into()),
                entries: vec![
                    CreationLorebookEntry {
                        id: first_entry_id,
                        title: "Old lighthouse".into(),
                        content: "The lighthouse has been dark for twenty years.".into(),
                    },
                    CreationLorebookEntry {
                        id: second_entry_id,
                        title: "Harbor master".into(),
                        content: "Mara records every ship that enters.".into(),
                    },
                ],
            },
            10,
        );
        let lorebook_id = LorebookId::new();
        let request = ConfirmedLorebookApply {
            workflow_id: workflow.id,
            expected_workflow_revision: workflow.revision,
            proposal_id,
            destination_lorebook_id: lorebook_id,
            now: TimestampMillis::new(15),
        };
        let receipt = database
            .apply_new_lorebook(request.clone())
            .expect("apply lorebook");
        assert_eq!(receipt.lorebook_id, lorebook_id);
        assert_eq!(receipt.lorebook_revision, Revision::INITIAL);
        let details = LorebookRepository::get(&database, lorebook_id)
            .expect("load lorebook")
            .expect("lorebook exists");
        assert_eq!(details.book.name, "Harbor lore");
        assert_eq!(
            details.book.detection_policy,
            DetectionPolicy::RecentMessageWindow
        );
        assert_eq!(
            details.book.behavior_version,
            LorebookBehaviorVersion::LegacyV1
        );
        assert_eq!(details.book.icon_asset_id, None);
        assert_eq!(
            details
                .entries
                .iter()
                .map(|entry| (
                    entry.id,
                    entry.ordinal,
                    entry.title.as_str(),
                    entry.content.as_str()
                ))
                .collect::<Vec<_>>(),
            vec![
                (
                    first_entry_id,
                    0,
                    "Old lighthouse",
                    "The lighthouse has been dark for twenty years."
                ),
                (
                    second_entry_id,
                    1,
                    "Harbor master",
                    "Mara records every ship that enters."
                ),
            ]
        );
        assert!(details.entries.iter().all(|entry| {
            entry.enabled
                && entry.always_active
                && entry.keywords.is_empty()
                && !entry.case_sensitive
                && entry.match_mode == KeywordMatchMode::Literal
                && entry.priority == 0
                && entry.revision == Revision::INITIAL
        }));

        let mut retry = request.clone();
        retry.now = TimestampMillis::new(99);
        assert_eq!(
            database.apply_new_lorebook(retry).expect("exact retry"),
            receipt
        );
        let mut changed_destination = request.clone();
        changed_destination.destination_lorebook_id = LorebookId::new();
        assert_eq!(
            database.apply_new_lorebook(changed_destination),
            Err(CreationRepositoryError::Conflict)
        );
        let mut changed_proposal = request.clone();
        changed_proposal.proposal_id = CreationProposalId::new();
        assert_eq!(
            database.apply_new_lorebook(changed_proposal),
            Err(CreationRepositoryError::Conflict)
        );
        let mut changed_revision = request.clone();
        changed_revision.expected_workflow_revision =
            Revision::new(request.expected_workflow_revision.get() + 1);
        assert_eq!(
            database.apply_new_lorebook(changed_revision),
            Err(CreationRepositoryError::Conflict)
        );
        let connection = database.connection().expect("database lock");
        assert!(
            connection
                .execute(
                    "UPDATE creation_lorebook_apply_receipts SET applied_at=100 WHERE workflow_id=?1",
                    [workflow.id.to_string()],
                )
                .is_err()
        );
        assert!(
            connection
                .execute(
                    "DELETE FROM creation_lorebook_apply_receipts WHERE workflow_id=?1",
                    [workflow.id.to_string()],
                )
                .is_err()
        );
    }

    #[test]
    fn new_lorebook_apply_rejects_invalid_state_and_accepts_empty_entries() {
        let database = Database::open_in_memory().expect("database");
        let duplicate_entry_id = LorebookEntryId::new();
        assert_eq!(
            database.create_workflow(NewCreationWorkflow {
                id: CreationWorkflowId::new(),
                initial_proposal_id: CreationProposalId::new(),
                target: CreationTarget::NewLorebook,
                initial_draft: CreationDraft::Lorebook {
                    name: Some("Duplicate entries".into()),
                    description: None,
                    entries: vec![
                        CreationLorebookEntry {
                            id: duplicate_entry_id,
                            title: "First".into(),
                            content: "First content".into(),
                        },
                        CreationLorebookEntry {
                            id: duplicate_entry_id,
                            title: "Second".into(),
                            content: "Second content".into(),
                        },
                    ],
                },
                now: TimestampMillis::new(1),
            }),
            Err(CreationRepositoryError::Invalid)
        );

        let drafting_proposal_id = CreationProposalId::new();
        let drafting = database
            .create_workflow(NewCreationWorkflow {
                id: CreationWorkflowId::new(),
                initial_proposal_id: drafting_proposal_id,
                target: CreationTarget::NewLorebook,
                initial_draft: CreationDraft::Lorebook {
                    name: Some("Drafting".into()),
                    description: None,
                    entries: Vec::new(),
                },
                now: TimestampMillis::new(2),
            })
            .expect("drafting workflow");
        assert_eq!(
            database.apply_new_lorebook(ConfirmedLorebookApply {
                workflow_id: drafting.id,
                expected_workflow_revision: drafting.revision,
                proposal_id: drafting_proposal_id,
                destination_lorebook_id: LorebookId::new(),
                now: TimestampMillis::new(3),
            }),
            Err(CreationRepositoryError::Conflict)
        );

        let (wrong_target, wrong_target_proposal_id) = confirmed_workflow(
            &database,
            CreationTarget::NewPersona,
            CreationDraft::Persona {
                name: Some("Wrong target".into()),
                description: Some("Not a lorebook".into()),
            },
            10,
        );
        assert_eq!(
            database.apply_new_lorebook(ConfirmedLorebookApply {
                workflow_id: wrong_target.id,
                expected_workflow_revision: wrong_target.revision,
                proposal_id: wrong_target_proposal_id,
                destination_lorebook_id: LorebookId::new(),
                now: TimestampMillis::new(15),
            }),
            Err(CreationRepositoryError::Conflict)
        );

        let (incomplete, incomplete_proposal_id) = confirmed_workflow(
            &database,
            CreationTarget::NewLorebook,
            CreationDraft::Lorebook {
                name: None,
                description: Some("Summary".into()),
                entries: Vec::new(),
            },
            20,
        );
        assert_eq!(
            database.apply_new_lorebook(ConfirmedLorebookApply {
                workflow_id: incomplete.id,
                expected_workflow_revision: incomplete.revision,
                proposal_id: incomplete_proposal_id,
                destination_lorebook_id: LorebookId::new(),
                now: TimestampMillis::new(25),
            }),
            Err(CreationRepositoryError::Invalid)
        );

        let (empty, empty_proposal_id) = confirmed_workflow(
            &database,
            CreationTarget::NewLorebook,
            CreationDraft::Lorebook {
                name: Some("Empty lorebook".into()),
                description: None,
                entries: Vec::new(),
            },
            30,
        );
        let empty_id = LorebookId::new();
        database
            .apply_new_lorebook(ConfirmedLorebookApply {
                workflow_id: empty.id,
                expected_workflow_revision: empty.revision,
                proposal_id: empty_proposal_id,
                destination_lorebook_id: empty_id,
                now: TimestampMillis::new(35),
            })
            .expect("apply empty lorebook");
        assert!(
            LorebookRepository::get(&database, empty_id)
                .expect("load empty")
                .expect("empty lorebook exists")
                .entries
                .is_empty()
        );

        let (duplicate, duplicate_proposal_id) = confirmed_workflow(
            &database,
            CreationTarget::NewLorebook,
            CreationDraft::Lorebook {
                name: Some("Duplicate destination".into()),
                description: None,
                entries: Vec::new(),
            },
            40,
        );
        assert_eq!(
            database.apply_new_lorebook(ConfirmedLorebookApply {
                workflow_id: duplicate.id,
                expected_workflow_revision: duplicate.revision,
                proposal_id: duplicate_proposal_id,
                destination_lorebook_id: empty_id,
                now: TimestampMillis::new(45),
            }),
            Err(CreationRepositoryError::Conflict)
        );
        let connection = database.connection().expect("database lock");
        assert_eq!(
            connection
                .query_row(
                    "SELECT count(*) FROM creation_lorebook_apply_receipts WHERE workflow_id=?1",
                    [duplicate.id.to_string()],
                    |row| row.get::<_, i64>(0),
                )
                .expect("receipt count"),
            0
        );
    }

    #[test]
    fn confirmed_existing_lorebook_apply_reconciles_ids_and_preserves_entry_policies() {
        let database = Database::open_in_memory().expect("database");
        let icon = image_asset(&database, 41);
        let original = LorebookRepository::create(
            &database,
            LorebookMetadataDraft {
                name: "Old atlas".into(),
                detection_policy: DetectionPolicy::LatestUserMessage,
                icon_asset_id: Some(icon),
                behavior_version: LorebookBehaviorVersion::DeterministicV2,
            },
            vec![
                lorebook_entry_draft("Remove", "obsolete", true, false, vec!["old"], false, 2),
                lorebook_entry_draft(
                    "Keep",
                    "old content",
                    false,
                    false,
                    vec!["Harbor", "Mara"],
                    true,
                    9,
                ),
            ],
            TimestampMillis::new(1),
        )
        .expect("create lorebook");
        let removed_id = original.entries[0].id;
        let retained = original.entries[1].clone();
        let added_id = LorebookEntryId::new();
        let (workflow, proposal_id) = confirmed_workflow(
            &database,
            CreationTarget::ExistingLorebook {
                id: original.book.id,
                revision: original.book.revision,
            },
            CreationDraft::Lorebook {
                name: Some("Revised atlas".into()),
                description: Some("proposal-only".into()),
                entries: vec![
                    CreationLorebookEntry {
                        id: retained.id,
                        title: "Harbor keeper".into(),
                        content: "Mara keeps the harbor ledger.".into(),
                    },
                    CreationLorebookEntry {
                        id: added_id,
                        title: "Lighthouse".into(),
                        content: "The lighthouse is dark.".into(),
                    },
                ],
            },
            10,
        );
        let request = ConfirmedLorebookRevisionApply {
            workflow_id: workflow.id,
            expected_workflow_revision: workflow.revision,
            proposal_id,
            lorebook_id: original.book.id,
            expected_lorebook_revision: original.book.revision,
            now: TimestampMillis::new(15),
        };
        let receipt = database
            .apply_existing_lorebook(request.clone())
            .expect("apply existing lorebook");
        assert_eq!(receipt.lorebook_revision, Revision::new(2));
        let revised = LorebookRepository::get(&database, original.book.id)
            .expect("load revised")
            .expect("revised exists");
        assert_eq!(revised.book.name, "Revised atlas");
        assert_eq!(revised.book.status, LorebookLifecycleStatus::Active);
        assert_eq!(
            revised.book.detection_policy,
            DetectionPolicy::LatestUserMessage
        );
        assert_eq!(revised.book.icon_asset_id, Some(icon));
        assert_eq!(
            revised.book.behavior_version,
            LorebookBehaviorVersion::DeterministicV2
        );
        assert_eq!(revised.book.created_at, original.book.created_at);
        assert!(revised.entries.iter().all(|entry| entry.id != removed_id));
        assert_eq!(revised.entries.len(), 2);
        let kept = &revised.entries[0];
        assert_eq!(kept.id, retained.id);
        assert_eq!(kept.title, "Harbor keeper");
        assert_eq!(kept.content, "Mara keeps the harbor ledger.");
        assert!(!kept.enabled);
        assert!(!kept.always_active);
        assert_eq!(kept.keywords, vec!["Harbor", "Mara"]);
        assert!(kept.case_sensitive);
        assert_eq!(kept.priority, 9);
        assert_eq!(kept.ordinal, 0);
        assert_eq!(kept.revision, Revision::new(2));
        assert_eq!(kept.created_at, retained.created_at);
        assert_eq!(kept.updated_at, TimestampMillis::new(15));
        let added = &revised.entries[1];
        assert_eq!(added.id, added_id);
        assert!(added.enabled && added.always_active);
        assert!(added.keywords.is_empty());
        assert!(!added.case_sensitive);
        assert_eq!(added.match_mode, KeywordMatchMode::Literal);
        assert_eq!(added.priority, 0);
        assert_eq!(added.revision, Revision::INITIAL);
        assert_eq!(added.created_at, TimestampMillis::new(15));

        let mut retry = request.clone();
        retry.now = TimestampMillis::new(99);
        assert_eq!(
            database
                .apply_existing_lorebook(retry)
                .expect("exact retry"),
            receipt
        );
        let mut changed_revision = request.clone();
        changed_revision.expected_lorebook_revision = Revision::new(2);
        assert_eq!(
            database.apply_existing_lorebook(changed_revision),
            Err(CreationRepositoryError::Conflict)
        );
        let mut changed_target = request.clone();
        changed_target.lorebook_id = LorebookId::new();
        assert_eq!(
            database.apply_existing_lorebook(changed_target),
            Err(CreationRepositoryError::Conflict)
        );
        let mut changed_proposal = request.clone();
        changed_proposal.proposal_id = CreationProposalId::new();
        assert_eq!(
            database.apply_existing_lorebook(changed_proposal),
            Err(CreationRepositoryError::Conflict)
        );

        let unchanged_entries = revised
            .entries
            .iter()
            .map(|entry| CreationLorebookEntry {
                id: entry.id,
                title: entry.title.clone(),
                content: entry.content.clone(),
            })
            .collect();
        let (second_workflow, second_proposal_id) = confirmed_workflow(
            &database,
            CreationTarget::ExistingLorebook {
                id: revised.book.id,
                revision: revised.book.revision,
            },
            CreationDraft::Lorebook {
                name: Some("Final atlas".into()),
                description: None,
                entries: unchanged_entries,
            },
            20,
        );
        database
            .apply_existing_lorebook(ConfirmedLorebookRevisionApply {
                workflow_id: second_workflow.id,
                expected_workflow_revision: second_workflow.revision,
                proposal_id: second_proposal_id,
                lorebook_id: revised.book.id,
                expected_lorebook_revision: revised.book.revision,
                now: TimestampMillis::new(25),
            })
            .expect("second workflow");
        let final_details = LorebookRepository::get(&database, revised.book.id)
            .expect("load final")
            .expect("final exists");
        assert_eq!(final_details.book.revision, Revision::new(3));
        assert_eq!(final_details.book.name, "Final atlas");
        assert_eq!(final_details.entries, revised.entries);
        let connection = database.connection().expect("database lock");
        assert_eq!(
            connection
                .query_row(
                    "SELECT count(*) FROM creation_lorebook_apply_receipts WHERE lorebook_id=?1",
                    [revised.book.id.to_string()],
                    |row| row.get::<_, i64>(0),
                )
                .expect("receipt count"),
            2
        );
    }

    #[test]
    fn existing_lorebook_apply_rejects_invalid_targets_and_rolls_back_collisions() {
        let database = Database::open_in_memory().expect("database");
        let target = LorebookRepository::create(
            &database,
            LorebookMetadataDraft {
                name: "Target".into(),
                detection_policy: DetectionPolicy::RecentMessageWindow,
                icon_asset_id: None,
                behavior_version: LorebookBehaviorVersion::LegacyV1,
            },
            vec![lorebook_entry_draft(
                "Existing",
                "content",
                true,
                false,
                vec!["key"],
                false,
                0,
            )],
            TimestampMillis::new(1),
        )
        .expect("target");

        let duplicate_id = target.entries[0].id;
        assert_eq!(
            database.create_workflow(NewCreationWorkflow {
                id: CreationWorkflowId::new(),
                initial_proposal_id: CreationProposalId::new(),
                target: CreationTarget::ExistingLorebook {
                    id: target.book.id,
                    revision: target.book.revision,
                },
                initial_draft: CreationDraft::Lorebook {
                    name: Some("Duplicate".into()),
                    description: None,
                    entries: vec![
                        CreationLorebookEntry {
                            id: duplicate_id,
                            title: "One".into(),
                            content: "one".into(),
                        },
                        CreationLorebookEntry {
                            id: duplicate_id,
                            title: "Two".into(),
                            content: "two".into(),
                        },
                    ],
                },
                now: TimestampMillis::new(2),
            }),
            Err(CreationRepositoryError::Invalid)
        );

        let drafting_proposal_id = CreationProposalId::new();
        let drafting = database
            .create_workflow(NewCreationWorkflow {
                id: CreationWorkflowId::new(),
                initial_proposal_id: drafting_proposal_id,
                target: CreationTarget::ExistingLorebook {
                    id: target.book.id,
                    revision: target.book.revision,
                },
                initial_draft: CreationDraft::Lorebook {
                    name: Some("Drafting".into()),
                    description: None,
                    entries: Vec::new(),
                },
                now: TimestampMillis::new(3),
            })
            .expect("drafting workflow");
        assert_eq!(
            database.apply_existing_lorebook(ConfirmedLorebookRevisionApply {
                workflow_id: drafting.id,
                expected_workflow_revision: drafting.revision,
                proposal_id: drafting_proposal_id,
                lorebook_id: target.book.id,
                expected_lorebook_revision: target.book.revision,
                now: TimestampMillis::new(4),
            }),
            Err(CreationRepositoryError::Conflict)
        );

        let (wrong_target, wrong_target_proposal_id) = confirmed_workflow(
            &database,
            CreationTarget::NewLorebook,
            CreationDraft::Lorebook {
                name: Some("Wrong target".into()),
                description: None,
                entries: Vec::new(),
            },
            5,
        );
        assert_eq!(
            database.apply_existing_lorebook(ConfirmedLorebookRevisionApply {
                workflow_id: wrong_target.id,
                expected_workflow_revision: wrong_target.revision,
                proposal_id: wrong_target_proposal_id,
                lorebook_id: target.book.id,
                expected_lorebook_revision: target.book.revision,
                now: TimestampMillis::new(10),
            }),
            Err(CreationRepositoryError::Conflict)
        );

        let (incomplete, incomplete_proposal_id) = confirmed_workflow(
            &database,
            CreationTarget::ExistingLorebook {
                id: target.book.id,
                revision: target.book.revision,
            },
            CreationDraft::Lorebook {
                name: None,
                description: None,
                entries: Vec::new(),
            },
            10,
        );
        assert_eq!(
            database.apply_existing_lorebook(ConfirmedLorebookRevisionApply {
                workflow_id: incomplete.id,
                expected_workflow_revision: incomplete.revision,
                proposal_id: incomplete_proposal_id,
                lorebook_id: target.book.id,
                expected_lorebook_revision: target.book.revision,
                now: TimestampMillis::new(15),
            }),
            Err(CreationRepositoryError::Invalid)
        );

        let missing_id = LorebookId::new();
        let (missing, missing_proposal_id) = confirmed_workflow(
            &database,
            CreationTarget::ExistingLorebook {
                id: missing_id,
                revision: Revision::INITIAL,
            },
            CreationDraft::Lorebook {
                name: Some("Missing".into()),
                description: None,
                entries: Vec::new(),
            },
            20,
        );
        assert_eq!(
            database.apply_existing_lorebook(ConfirmedLorebookRevisionApply {
                workflow_id: missing.id,
                expected_workflow_revision: missing.revision,
                proposal_id: missing_proposal_id,
                lorebook_id: missing_id,
                expected_lorebook_revision: Revision::INITIAL,
                now: TimestampMillis::new(25),
            }),
            Err(CreationRepositoryError::NotFound)
        );

        let archived = LorebookRepository::create(
            &database,
            LorebookMetadataDraft {
                name: "Archive".into(),
                detection_policy: DetectionPolicy::RecentMessageWindow,
                icon_asset_id: None,
                behavior_version: LorebookBehaviorVersion::LegacyV1,
            },
            Vec::new(),
            TimestampMillis::new(30),
        )
        .expect("archived target");
        let archived = LorebookRepository::archive(
            &database,
            archived.book.id,
            archived.book.revision,
            TimestampMillis::new(31),
        )
        .expect("archive");
        let (archived_workflow, archived_proposal_id) = confirmed_workflow(
            &database,
            CreationTarget::ExistingLorebook {
                id: archived.details.book.id,
                revision: archived.book_revision,
            },
            CreationDraft::Lorebook {
                name: Some("Still archived".into()),
                description: None,
                entries: Vec::new(),
            },
            32,
        );
        assert_eq!(
            database.apply_existing_lorebook(ConfirmedLorebookRevisionApply {
                workflow_id: archived_workflow.id,
                expected_workflow_revision: archived_workflow.revision,
                proposal_id: archived_proposal_id,
                lorebook_id: archived.details.book.id,
                expected_lorebook_revision: archived.book_revision,
                now: TimestampMillis::new(37),
            }),
            Err(CreationRepositoryError::Conflict)
        );

        let stale_target = LorebookRepository::create(
            &database,
            LorebookMetadataDraft {
                name: "Stale".into(),
                detection_policy: DetectionPolicy::RecentMessageWindow,
                icon_asset_id: None,
                behavior_version: LorebookBehaviorVersion::LegacyV1,
            },
            Vec::new(),
            TimestampMillis::new(38),
        )
        .expect("stale target");
        let (stale_workflow, stale_proposal_id) = confirmed_workflow(
            &database,
            CreationTarget::ExistingLorebook {
                id: stale_target.book.id,
                revision: stale_target.book.revision,
            },
            CreationDraft::Lorebook {
                name: Some("Reviewed stale".into()),
                description: None,
                entries: Vec::new(),
            },
            39,
        );
        LorebookRepository::revise_metadata(
            &database,
            stale_target.book.id,
            stale_target.book.revision,
            LorebookMetadataDraft {
                name: "Concurrent change".into(),
                detection_policy: stale_target.book.detection_policy,
                icon_asset_id: stale_target.book.icon_asset_id,
                behavior_version: stale_target.book.behavior_version,
            },
            TimestampMillis::new(44),
        )
        .expect("concurrent revision");
        assert_eq!(
            database.apply_existing_lorebook(ConfirmedLorebookRevisionApply {
                workflow_id: stale_workflow.id,
                expected_workflow_revision: stale_workflow.revision,
                proposal_id: stale_proposal_id,
                lorebook_id: stale_target.book.id,
                expected_lorebook_revision: stale_target.book.revision,
                now: TimestampMillis::new(45),
            }),
            Err(CreationRepositoryError::Conflict)
        );

        let foreign = LorebookRepository::create(
            &database,
            LorebookMetadataDraft {
                name: "Foreign".into(),
                detection_policy: DetectionPolicy::RecentMessageWindow,
                icon_asset_id: None,
                behavior_version: LorebookBehaviorVersion::LegacyV1,
            },
            vec![lorebook_entry_draft(
                "Foreign entry",
                "foreign",
                true,
                false,
                vec!["foreign"],
                false,
                0,
            )],
            TimestampMillis::new(40),
        )
        .expect("foreign");
        let before_collision = LorebookRepository::get(&database, target.book.id)
            .expect("load before")
            .expect("target exists");
        let (collision, collision_proposal_id) = confirmed_workflow(
            &database,
            CreationTarget::ExistingLorebook {
                id: target.book.id,
                revision: target.book.revision,
            },
            CreationDraft::Lorebook {
                name: Some("Must roll back".into()),
                description: None,
                entries: vec![CreationLorebookEntry {
                    id: foreign.entries[0].id,
                    title: "Collision".into(),
                    content: "collision".into(),
                }],
            },
            41,
        );
        assert_eq!(
            database.apply_existing_lorebook(ConfirmedLorebookRevisionApply {
                workflow_id: collision.id,
                expected_workflow_revision: collision.revision,
                proposal_id: collision_proposal_id,
                lorebook_id: target.book.id,
                expected_lorebook_revision: target.book.revision,
                now: TimestampMillis::new(46),
            }),
            Err(CreationRepositoryError::Conflict)
        );
        assert_eq!(
            LorebookRepository::get(&database, target.book.id)
                .expect("load after")
                .expect("target exists"),
            before_collision
        );
        let connection = database.connection().expect("database lock");
        assert_eq!(
            connection
                .query_row(
                    "SELECT count(*) FROM creation_lorebook_apply_receipts WHERE workflow_id=?1",
                    [collision.id.to_string()],
                    |row| row.get::<_, i64>(0),
                )
                .expect("receipt count"),
            0
        );
        drop(connection);

        let (empty, empty_proposal_id) = confirmed_workflow(
            &database,
            CreationTarget::ExistingLorebook {
                id: target.book.id,
                revision: target.book.revision,
            },
            CreationDraft::Lorebook {
                name: Some("Empty target".into()),
                description: None,
                entries: Vec::new(),
            },
            50,
        );
        database
            .apply_existing_lorebook(ConfirmedLorebookRevisionApply {
                workflow_id: empty.id,
                expected_workflow_revision: empty.revision,
                proposal_id: empty_proposal_id,
                lorebook_id: target.book.id,
                expected_lorebook_revision: target.book.revision,
                now: TimestampMillis::new(55),
            })
            .expect("empty graph");
        assert!(
            LorebookRepository::get(&database, target.book.id)
                .expect("load empty")
                .expect("target exists")
                .entries
                .is_empty()
        );
    }

    #[test]
    fn durable_turns_proposals_and_review_transitions_are_atomic_and_retryable() {
        let database = Database::open_in_memory().expect("database");
        let workflow_id = CreationWorkflowId::new();
        let initial_id = CreationProposalId::new();
        let input = NewCreationWorkflow {
            id: workflow_id,
            initial_proposal_id: initial_id,
            target: CreationTarget::NewCharacter,
            initial_draft: CreationDraft::Character {
                name: None,
                definition: None,
                scenes: Vec::new(),
            },
            now: TimestampMillis::new(10),
        };
        let workflow = database
            .create_workflow(input.clone())
            .expect("create workflow");
        assert_eq!(
            database.create_workflow(input).expect("exact retry"),
            workflow
        );

        let turn_id = CreationTurnId::new();
        let turn_input = NewCreationTurn {
            id: turn_id,
            workflow_id,
            base_proposal_id: initial_id,
            user_message: "Build a quiet guide".to_owned(),
            now: TimestampMillis::new(11),
        };
        let turn = database
            .record_user_turn(turn_input.clone())
            .expect("durable turn");
        assert_eq!(
            database
                .record_user_turn(turn_input)
                .expect("turn exact retry"),
            turn
        );
        assert_eq!(
            database
                .load_workflow(workflow_id)
                .expect("workflow before inference")
                .current_proposal_id,
            initial_id
        );

        let base = database.load_proposal(initial_id).expect("base proposal");
        let competing_turn = database
            .record_user_turn(NewCreationTurn {
                id: CreationTurnId::new(),
                workflow_id,
                base_proposal_id: initial_id,
                user_message: "Use another name".to_owned(),
                now: TimestampMillis::new(11),
            })
            .expect("competing durable turn");
        let competing = base
            .apply(
                CreationProposalId::new(),
                competing_turn.id,
                vec![CreationOperation::SetName {
                    value: "Bramble".to_owned(),
                }],
                TimestampMillis::new(12),
            )
            .expect("competing proposal");
        let scene_id = SceneId::new();
        let proposal = base
            .apply(
                CreationProposalId::new(),
                turn_id,
                vec![
                    CreationOperation::UpdateScene {
                        id: SceneId::new(),
                        content: "missing".to_owned(),
                        direction: None,
                    },
                    CreationOperation::SetName {
                        value: "Aster".to_owned(),
                    },
                    CreationOperation::AddScene {
                        id: scene_id,
                        content: "Welcome.".to_owned(),
                        direction: Some("calmly".to_owned()),
                    },
                    CreationOperation::ShowPreview,
                ],
                TimestampMillis::new(12),
            )
            .expect("reduce proposal");
        assert_eq!(
            proposal.outcomes[0].error,
            Some(CreationOperationError::NotFound)
        );
        assert!(
            proposal.outcomes[1..]
                .iter()
                .all(|outcome| outcome.succeeded())
        );
        let mut forged = proposal.clone();
        forged.stage = CreationStage::AwaitingConfirmation;
        assert_eq!(
            database.append_proposal(workflow_id, Revision::INITIAL, forged),
            Err(lettuce_creation::CreationRepositoryError::Conflict)
        );
        let reviewed = database
            .append_proposal(workflow_id, Revision::INITIAL, proposal.clone())
            .expect("append reviewed proposal");
        assert_eq!(reviewed.stage, CreationStage::AwaitingReview);
        assert_eq!(reviewed.revision, Revision::new(2));
        assert_eq!(
            database.append_proposal(workflow_id, Revision::INITIAL, competing),
            Err(lettuce_creation::CreationRepositoryError::Conflict)
        );
        assert_eq!(
            database
                .append_proposal(workflow_id, Revision::INITIAL, proposal.clone())
                .expect("proposal exact retry"),
            reviewed
        );

        let stale_turn = database.record_user_turn(NewCreationTurn {
            id: CreationTurnId::new(),
            workflow_id,
            base_proposal_id: initial_id,
            user_message: "stale".to_owned(),
            now: TimestampMillis::new(13),
        });
        assert_eq!(
            stale_turn,
            Err(lettuce_creation::CreationRepositoryError::Conflict)
        );

        let confirm_turn = database
            .record_user_turn(NewCreationTurn {
                id: CreationTurnId::new(),
                workflow_id,
                base_proposal_id: proposal.id,
                user_message: "Looks good".to_owned(),
                now: TimestampMillis::new(14),
            })
            .expect("confirmation turn");
        let confirmation = proposal
            .apply(
                CreationProposalId::new(),
                confirm_turn.id,
                vec![CreationOperation::RequestConfirmation],
                TimestampMillis::new(15),
            )
            .expect("confirmation proposal");
        let confirmed = database
            .append_proposal(workflow_id, reviewed.revision, confirmation.clone())
            .expect("append confirmation");
        assert_eq!(confirmed.stage, CreationStage::AwaitingConfirmation);
        assert_eq!(
            database
                .load_proposal(confirmation.id)
                .expect("stored confirmation"),
            confirmation
        );

        let connection = database.connection().expect("database lock");
        let authored_rows: i64 = connection
            .query_row(
                "SELECT (SELECT count(*) FROM characters) + \
                        (SELECT count(*) FROM personas) + \
                        (SELECT count(*) FROM lorebooks)",
                [],
                |row| row.get(0),
            )
            .expect("authored rows");
        assert_eq!(authored_rows, 0);
        assert!(
            connection
                .execute(
                    "UPDATE creation_proposals SET stage='drafting' WHERE id=?1",
                    [confirmation.id.to_string()],
                )
                .is_err()
        );
        assert!(
            connection
                .execute(
                    "UPDATE creation_turns SET user_message='changed' WHERE id=?1",
                    [confirm_turn.id.to_string()],
                )
                .is_err()
        );
    }

    #[test]
    fn native_tool_bridge_persists_one_retry_stable_proposal_and_outputs() {
        let database = Database::open_in_memory().expect("database");
        let workflow_id = CreationWorkflowId::new();
        let initial_id = CreationProposalId::new();
        let workflow = database
            .create_workflow(NewCreationWorkflow {
                id: workflow_id,
                initial_proposal_id: initial_id,
                target: CreationTarget::NewPersona,
                initial_draft: CreationDraft::Persona {
                    name: None,
                    description: None,
                },
                now: TimestampMillis::new(1),
            })
            .expect("workflow");
        let turn = database
            .record_user_turn(NewCreationTurn {
                id: CreationTurnId::new(),
                workflow_id,
                base_proposal_id: initial_id,
                user_message: "Create a navigator persona".to_owned(),
                now: TimestampMillis::new(2),
            })
            .expect("turn");
        let proposal_id = CreationProposalId::new();
        let calls = vec![
            admitted("set_persona_name", serde_json::json!({"name": "Navigator"})),
            admitted(
                "set_persona_description",
                serde_json::json!({"description": "Charts careful routes."}),
            ),
            admitted("show_preview", serde_json::json!({})),
        ];
        let committed = apply_creation_tool_calls(
            &database,
            CreationToolApply {
                workflow_id,
                expected_workflow_revision: workflow.revision,
                base_proposal_id: initial_id,
                proposal_id,
                turn_id: turn.id,
                calls: calls.clone(),
                now: TimestampMillis::new(3),
            },
        )
        .expect("tool commit");
        assert_eq!(committed.workflow.stage, CreationStage::AwaitingReview);
        assert_eq!(committed.outputs.len(), calls.len());
        assert!(committed.outputs.iter().all(|output| !output.is_error));
        assert_eq!(committed.outputs[0].value["tool"], "set_persona_name");
        let retry = apply_creation_tool_calls(
            &database,
            CreationToolApply {
                workflow_id,
                expected_workflow_revision: workflow.revision,
                base_proposal_id: initial_id,
                proposal_id,
                turn_id: turn.id,
                calls,
                now: TimestampMillis::new(3),
            },
        )
        .expect("exact tool retry");
        assert_eq!(retry, committed);
    }

    #[test]
    fn creation_attempts_admit_exact_calls_before_reduction_and_retry_safely() {
        let database = Database::open_in_memory().expect("database");
        let workflow_id = CreationWorkflowId::new();
        let initial_id = CreationProposalId::new();
        let workflow = database
            .create_workflow(NewCreationWorkflow {
                id: workflow_id,
                initial_proposal_id: initial_id,
                target: CreationTarget::NewPersona,
                initial_draft: CreationDraft::Persona {
                    name: None,
                    description: None,
                },
                now: TimestampMillis::new(1),
            })
            .expect("workflow");
        let turn = database
            .record_user_turn(NewCreationTurn {
                id: CreationTurnId::new(),
                workflow_id,
                base_proposal_id: initial_id,
                user_message: "Create a cartographer".to_owned(),
                now: TimestampMillis::new(2),
            })
            .expect("turn");
        let owner = CreationAttemptOwner {
            workflow_id,
            turn_id: turn.id,
        };
        let parent_id = GenerationAttemptId::new();
        let parent_input = NewCreationAttempt {
            id: parent_id,
            owner,
            base_proposal_id: initial_id,
            planned_proposal_id: CreationProposalId::new(),
            retry_parent_id: None,
            job_id: JobId::new(),
            profile_fingerprint: [7; 32],
            now: TimestampMillis::new(3),
        };
        let parent = database
            .create_creation_attempt(parent_input.clone())
            .expect("attempt");
        assert_eq!(
            database
                .create_creation_attempt(parent_input.clone())
                .expect("exact attempt retry"),
            parent
        );
        let mut changed_attempt = parent_input;
        changed_attempt.planned_proposal_id = CreationProposalId::new();
        assert_eq!(
            database.create_creation_attempt(changed_attempt),
            Err(CreationRepositoryError::Conflict)
        );
        let parent = database
            .transition_creation_attempt(
                parent_id,
                parent.revision,
                CreationAttemptStatus::Running,
                None,
                TimestampMillis::new(4),
            )
            .expect("run parent");

        let replay_id = ReplayArtifactId::new();
        let replay_bytes = ProtectedArtifactBytes::new(b"{\"thought\":\"signed\"}".to_vec())
            .expect("replay bytes");
        let replay = database
            .put_replay(ReplayArtifactDraft {
                artifact_id: replay_id,
                digest: replay_bytes.digest(),
                schema_version: 1,
                byte_size: u64::try_from(replay_bytes.len()).expect("size"),
                codec: ArtifactCodec::Json,
                retention: ArtifactRetention::Conversation,
                bytes: replay_bytes,
            })
            .expect("replay");
        let mut parent_calls = vec![
            new_call(
                "set_persona_name",
                serde_json::json!({"name": "Cartographer"}),
            ),
            new_call(
                "set_persona_description",
                serde_json::json!({"description": "Maps difficult paths."}),
            ),
        ];
        parent_calls[0].call.raw_arguments = Some("{\"name\":\"Cartographer\"}".to_owned());
        let parent_round = new_round(
            0,
            parent_calls.clone(),
            Some("I will draft that."),
            Some(replay.clone()),
            TimestampMillis::new(5),
        );
        let admitted = database
            .admit_creation_inference_round(owner, parent_id, 0, 0, parent_round.clone())
            .expect("admit round");
        assert_eq!(admitted.calls.len(), 2);
        assert_eq!(admitted.parts.len(), 1);
        assert_eq!(admitted.calls[0].call.provider_replay, Some(replay.clone()));
        assert_eq!(
            database
                .admit_creation_inference_round(owner, parent_id, 0, 0, parent_round.clone())
                .expect("exact round retry"),
            admitted
        );
        let mut changed_calls = parent_calls.clone();
        changed_calls[1].call.arguments = serde_json::json!({"description": "Changed retry."});
        assert_eq!(
            database.admit_creation_inference_round(
                owner,
                parent_id,
                0,
                0,
                new_round(
                    0,
                    changed_calls,
                    Some("I will draft that."),
                    Some(replay.clone()),
                    TimestampMillis::new(5),
                ),
            ),
            Err(CreationRepositoryError::Conflict)
        );
        let mut duplicate_provider = parent_calls.clone();
        duplicate_provider[1].call.provider_call_id =
            duplicate_provider[0].call.provider_call_id.clone();
        assert_eq!(
            database.admit_creation_inference_round(
                owner,
                parent_id,
                1,
                2,
                new_round(1, duplicate_provider, None, None, TimestampMillis::new(6)),
            ),
            Err(CreationRepositoryError::Invalid)
        );
        let mut wrong_version = vec![new_call(
            "set_persona_name",
            serde_json::json!({"name": "Other"}),
        )];
        wrong_version[0].definition_version = 2;
        assert_eq!(
            database.admit_creation_inference_round(
                owner,
                parent_id,
                1,
                2,
                new_round(1, wrong_version, None, None, TimestampMillis::new(6)),
            ),
            Err(CreationRepositoryError::Invalid)
        );
        assert_eq!(
            database.admit_creation_inference_round(
                CreationAttemptOwner {
                    workflow_id,
                    turn_id: CreationTurnId::new(),
                },
                parent_id,
                1,
                2,
                new_round(
                    1,
                    vec![new_call("show_preview", serde_json::json!({}))],
                    None,
                    None,
                    TimestampMillis::new(6),
                ),
            ),
            Err(CreationRepositoryError::Conflict)
        );
        database
            .cleanup_orphan_replay(replay_id)
            .expect("referenced replay is retained");
        database.verify_replay(&replay).expect("replay remains");

        let cancelled = database
            .transition_creation_attempt(
                parent_id,
                parent.revision,
                CreationAttemptStatus::Cancelled,
                None,
                TimestampMillis::new(6),
            )
            .expect("cancel parent");
        assert_eq!(cancelled.status, CreationAttemptStatus::Cancelled);
        let child_id = GenerationAttemptId::new();
        let child_input = NewCreationAttempt {
            id: child_id,
            owner,
            base_proposal_id: initial_id,
            planned_proposal_id: CreationProposalId::new(),
            retry_parent_id: Some(parent_id),
            job_id: JobId::new(),
            profile_fingerprint: parent.profile_fingerprint,
            now: TimestampMillis::new(7),
        };
        let mut reused_job = child_input.clone();
        reused_job.job_id = parent.job_id;
        assert_eq!(
            database.create_creation_attempt(reused_job),
            Err(CreationRepositoryError::Conflict)
        );
        let mut changed_profile = child_input.clone();
        changed_profile.profile_fingerprint = [8; 32];
        assert_eq!(
            database.create_creation_attempt(changed_profile),
            Err(CreationRepositoryError::Conflict)
        );
        let child = database
            .create_creation_attempt(child_input)
            .expect("retry child");
        assert_eq!(child.ordinal, 1);
        let child = database
            .transition_creation_attempt(
                child_id,
                child.revision,
                CreationAttemptStatus::Running,
                None,
                TimestampMillis::new(8),
            )
            .expect("run child");
        let child_calls = vec![
            new_call(
                "set_persona_name",
                serde_json::json!({"name": "Cartographer"}),
            ),
            new_call("show_preview", serde_json::json!({})),
        ];
        let child_round = new_round(
            0,
            child_calls.clone(),
            Some("Here is the draft."),
            None,
            TimestampMillis::new(9),
        );
        let child_round = database
            .admit_creation_inference_round(owner, child_id, 0, 0, child_round.clone())
            .expect("admit child round");
        let child_evidence = child_round.calls.clone();
        let terminal_round = database
            .admit_creation_inference_round(
                owner,
                child_id,
                1,
                2,
                new_round(
                    1,
                    Vec::new(),
                    Some("Review it when ready."),
                    None,
                    TimestampMillis::new(10),
                ),
            )
            .expect("admit text-only terminal round");
        assert!(terminal_round.calls.is_empty());
        assert_eq!(
            database
                .list_creation_inference_rounds(owner, child_id)
                .expect("round history"),
            vec![child_round.clone(), terminal_round]
        );
        let committed = apply_creation_tool_calls(
            &database,
            CreationToolApply {
                workflow_id,
                expected_workflow_revision: workflow.revision,
                base_proposal_id: initial_id,
                proposal_id: child.planned_proposal_id,
                turn_id: turn.id,
                calls: child_evidence
                    .iter()
                    .map(|evidence| AdmittedCreationToolCall {
                        definition_version: evidence.definition_version,
                        call: evidence.call.clone(),
                    })
                    .collect(),
                now: TimestampMillis::new(11),
            },
        )
        .expect("reduce admitted child calls");
        assert_eq!(committed.workflow.stage, CreationStage::AwaitingReview);
        assert_eq!(
            database
                .admit_creation_inference_round(
                    owner,
                    child_id,
                    0,
                    0,
                    new_round(
                        0,
                        child_calls,
                        Some("Here is the draft."),
                        None,
                        TimestampMillis::new(9),
                    ),
                )
                .expect("exact admission retry survives base advancement"),
            child_round
        );
        assert_eq!(
            database.admit_creation_inference_round(
                owner,
                child_id,
                2,
                2,
                new_round(
                    2,
                    vec![new_call("show_preview", serde_json::json!({}))],
                    None,
                    None,
                    TimestampMillis::new(12),
                ),
            ),
            Err(CreationRepositoryError::Conflict),
            "workflow advancement makes the attempt base stale"
        );
        let succeeded = database
            .transition_creation_attempt(
                child_id,
                child.revision,
                CreationAttemptStatus::Succeeded,
                None,
                TimestampMillis::new(12),
            )
            .expect("finish child");
        assert_eq!(succeeded.status, CreationAttemptStatus::Succeeded);

        let connection = database.connection().expect("database lock");
        assert!(
            connection
                .execute(
                    "UPDATE creation_admitted_tool_calls SET definition_version=2 WHERE id=?1",
                    [child_evidence[0].id.to_string()],
                )
                .is_err()
        );
        assert!(
            connection
                .execute(
                    "UPDATE creation_inference_rounds SET call_count=0 \
                     WHERE attempt_id=?1 AND ordinal=0",
                    [child_id.to_string()],
                )
                .is_err()
        );
        assert!(
            connection
                .execute(
                    "UPDATE creation_inference_attempts SET base_proposal_id=?2 WHERE id=?1",
                    [child_id.to_string(), CreationProposalId::new().to_string()],
                )
                .is_err()
        );
    }
}
