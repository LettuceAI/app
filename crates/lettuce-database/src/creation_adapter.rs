use std::str::FromStr;

use lettuce_characters::{Persona, RepositoryError as PersonaRepositoryError};
use lettuce_creation::{
    AdmittedCreationToolCall, ConfirmedPersonaApply, CreationApplyReceipt, CreationApplyRepository,
    CreationAttemptFailureCode, CreationAttemptOwner, CreationAttemptRecovery,
    CreationAttemptRepository, CreationAttemptStatus, CreationAttemptSuccess,
    CreationAttemptSuccessSettlement, CreationInferenceAttempt, CreationInferenceRound,
    CreationOperationOutcome, CreationProposal, CreationRepositoryError, CreationRoundFinishReason,
    CreationStage, CreationTarget, CreationTargetKind, CreationToolCallEvidence, CreationTurn,
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

fn persona_error(error: PersonaRepositoryError) -> CreationRepositoryError {
    match error {
        PersonaRepositoryError::AlreadyExists => CreationRepositoryError::Conflict,
        PersonaRepositoryError::Invalid(_) => CreationRepositoryError::Invalid,
        _ => CreationRepositoryError::Storage,
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
                    output_tokens,finish_reason,provider_request_id,admitted_at \
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
        let persona =
            crate::persona_adapter::insert_persona(&transaction, persona).map_err(persona_error)?;
        let receipt = CreationApplyReceipt {
            workflow_id: request.workflow_id,
            workflow_revision: request.expected_workflow_revision,
            proposal_id: request.proposal_id,
            persona_id: persona.id,
            persona_revision: persona.revision,
            applied_at: request.now,
        };
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
                  finish_reason,provider_request_id,admitted_at) \
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
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
        AdmittedCreationToolCall, ConfirmedPersonaApply, CreationApplyRepository,
        CreationAttemptOwner, CreationAttemptRepository, CreationAttemptStatus, CreationDraft,
        CreationOperation, CreationOperationError, CreationRepositoryError,
        CreationRoundFinishReason, CreationStage, CreationTarget, CreationToolApply,
        CreationWorkflow, CreationWorkflowRepository, NewCreationAttempt,
        NewCreationInferenceRound, NewCreationToolCall, NewCreationTurn, NewCreationWorkflow,
        apply_creation_tool_calls,
    };
    use lettuce_types::{
        CreationProposalId, CreationTurnId, CreationWorkflowId, GenerationAttemptId, JobId,
        PersonaId, ReplayArtifactId, Revision, SceneId, TimestampMillis, ToolExecutionId,
    };

    use lettuce_characters::PersonaRepository;

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
