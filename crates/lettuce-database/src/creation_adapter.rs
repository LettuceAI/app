use std::str::FromStr;

use lettuce_creation::{
    AdmittedCreationToolCall, CreationAttemptFailureCode, CreationAttemptOwner,
    CreationAttemptRepository, CreationAttemptStatus, CreationInferenceAttempt,
    CreationInferenceRound, CreationOperationOutcome, CreationProposal, CreationRepositoryError,
    CreationStage, CreationTargetKind, CreationToolCallEvidence, CreationTurn, CreationWorkflow,
    CreationWorkflowRepository, NewCreationAttempt, NewCreationInferenceRound, NewCreationTurn,
    NewCreationWorkflow, creation_tool_request, validate_creation_tool_calls,
};
use lettuce_types::{
    CreationProposalId, CreationTurnId, CreationWorkflowId, GenerationAttemptId, Revision,
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
        CreationAttemptFailureCode::Internal => "internal",
    }
}

fn parse_attempt_failure(value: &str) -> rusqlite::Result<CreationAttemptFailureCode> {
    match value {
        "provider_unavailable" => Ok(CreationAttemptFailureCode::ProviderUnavailable),
        "provider_rejected" => Ok(CreationAttemptFailureCode::ProviderRejected),
        "empty_response" => Ok(CreationAttemptFailureCode::EmptyResponse),
        "timed_out" => Ok(CreationAttemptFailureCode::TimedOut),
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

fn sql_u64(value: u64) -> Result<i64, CreationRepositoryError> {
    i64::try_from(value).map_err(|_| CreationRepositoryError::Storage)
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
                    planned_proposal_id,target,stage,tool_request_json,status,failure,revision,\
                    created_at,started_at,finished_at,updated_at \
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
                    status: parse_attempt_status(&row.get::<_, String>(9)?)?,
                    failure: row
                        .get::<_, Option<String>>(10)?
                        .map(|value| parse_attempt_failure(&value))
                        .transpose()?,
                    revision: revision(row.get(11)?)?,
                    created_at: TimestampMillis::new(row.get(12)?),
                    started_at: row.get::<_, Option<i64>>(13)?.map(TimestampMillis::new),
                    finished_at: row.get::<_, Option<i64>>(14)?.map(TimestampMillis::new),
                    updated_at: TimestampMillis::new(row.get(15)?),
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
                    provider_replay_artifact_id,provider_replay_retention,admitted_at \
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
                    calls: round_calls,
                    admitted_at: TimestampMillis::new(row.get(6)?),
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
                  planned_proposal_id,target,stage,tool_request_json,status,failure,revision,\
                  created_at,started_at,finished_at,updated_at) \
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,'created',NULL,1,?11,NULL,NULL,?11)",
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
        transaction
            .execute(
                "INSERT INTO creation_inference_rounds \
                 (workflow_id,turn_id,attempt_id,ordinal,first_call_ordinal,call_count,parts_json,\
                  provider_replay_artifact_id,provider_replay_retention,admitted_at) \
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
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
        AdmittedCreationToolCall, CreationAttemptOwner, CreationAttemptRepository,
        CreationAttemptStatus, CreationDraft, CreationOperation, CreationOperationError,
        CreationRepositoryError, CreationStage, CreationTarget, CreationToolApply,
        CreationWorkflowRepository, NewCreationAttempt, NewCreationInferenceRound,
        NewCreationToolCall, NewCreationTurn, NewCreationWorkflow, apply_creation_tool_calls,
    };
    use lettuce_types::{
        CreationProposalId, CreationTurnId, CreationWorkflowId, GenerationAttemptId,
        ReplayArtifactId, Revision, SceneId, TimestampMillis, ToolExecutionId,
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
            calls,
            admitted_at,
        }
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
        let child = database
            .create_creation_attempt(NewCreationAttempt {
                id: child_id,
                owner,
                base_proposal_id: initial_id,
                planned_proposal_id: CreationProposalId::new(),
                retry_parent_id: Some(parent_id),
                now: TimestampMillis::new(7),
            })
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
