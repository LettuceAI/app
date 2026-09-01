use std::str::FromStr;

use lettuce_conversations::{InferenceUsage, MessageRenderSource, ProposedToolCall};
use lettuce_memory::{
    DynamicMemoryAttempt, DynamicMemoryAttemptFailureCode, DynamicMemoryAttemptRecovery,
    DynamicMemoryAttemptStatus, DynamicMemoryInferenceRound, DynamicMemoryRoundFinishReason,
    DynamicMemoryRun, DynamicMemoryRunAttemptAdmission, DynamicMemoryRunRepository,
    DynamicMemoryRunRepositoryError, DynamicMemorySourceMessage, DynamicMemoryToolCallEvidence,
    NewDynamicMemoryAttemptRecovery, NewDynamicMemoryInferenceRound, NewDynamicMemoryRunAttempt,
    dynamic_memory_tool_request,
};
use lettuce_types::{DynamicMemoryAttemptId, DynamicMemoryRunId, JobId, Revision, TimestampMillis};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};

use crate::{Database, conversation_query, decode_versioned, encode_versioned};

const JSON_VERSION: u32 = 1;

fn storage(_: impl std::fmt::Debug) -> DynamicMemoryRunRepositoryError {
    DynamicMemoryRunRepositoryError::Storage
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

fn sql_u64(value: u64) -> Result<i64, DynamicMemoryRunRepositoryError> {
    i64::try_from(value).map_err(|_| DynamicMemoryRunRepositoryError::Storage)
}

fn status_name(status: DynamicMemoryAttemptStatus) -> &'static str {
    match status {
        DynamicMemoryAttemptStatus::Created => "created",
        DynamicMemoryAttemptStatus::Processing => "processing",
        DynamicMemoryAttemptStatus::Succeeded => "succeeded",
        DynamicMemoryAttemptStatus::Failed => "failed",
        DynamicMemoryAttemptStatus::Cancelled => "cancelled",
        DynamicMemoryAttemptStatus::Interrupted => "interrupted",
    }
}

fn parse_status(value: &str) -> rusqlite::Result<DynamicMemoryAttemptStatus> {
    match value {
        "created" => Ok(DynamicMemoryAttemptStatus::Created),
        "processing" => Ok(DynamicMemoryAttemptStatus::Processing),
        "succeeded" => Ok(DynamicMemoryAttemptStatus::Succeeded),
        "failed" => Ok(DynamicMemoryAttemptStatus::Failed),
        "cancelled" => Ok(DynamicMemoryAttemptStatus::Cancelled),
        "interrupted" => Ok(DynamicMemoryAttemptStatus::Interrupted),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn failure_name(failure: DynamicMemoryAttemptFailureCode) -> &'static str {
    match failure {
        DynamicMemoryAttemptFailureCode::ProviderUnavailable => "provider_unavailable",
        DynamicMemoryAttemptFailureCode::ProviderRejected => "provider_rejected",
        DynamicMemoryAttemptFailureCode::EmptyResponse => "empty_response",
        DynamicMemoryAttemptFailureCode::TimedOut => "timed_out",
        DynamicMemoryAttemptFailureCode::RoundLimit => "round_limit",
        DynamicMemoryAttemptFailureCode::Internal => "internal",
    }
}

fn parse_failure(value: &str) -> rusqlite::Result<DynamicMemoryAttemptFailureCode> {
    match value {
        "provider_unavailable" => Ok(DynamicMemoryAttemptFailureCode::ProviderUnavailable),
        "provider_rejected" => Ok(DynamicMemoryAttemptFailureCode::ProviderRejected),
        "empty_response" => Ok(DynamicMemoryAttemptFailureCode::EmptyResponse),
        "timed_out" => Ok(DynamicMemoryAttemptFailureCode::TimedOut),
        "round_limit" => Ok(DynamicMemoryAttemptFailureCode::RoundLimit),
        "internal" => Ok(DynamicMemoryAttemptFailureCode::Internal),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn load_run_in(
    connection: &Connection,
    id: DynamicMemoryRunId,
) -> Result<DynamicMemoryRun, DynamicMemoryRunRepositoryError> {
    let mut run = connection
        .query_row(
            "SELECT conversation_id,space_id,profile_json,tool_request_json,created_at \
             FROM dynamic_memory_runs WHERE id=?1",
            [id.to_string()],
            |row| {
                Ok(DynamicMemoryRun {
                    id,
                    conversation_id: parse_id(row.get(0)?)?,
                    space_id: parse_id(row.get(1)?)?,
                    source_messages: Vec::new(),
                    profile: decode_versioned(&row.get::<_, String>(2)?, JSON_VERSION)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    tool_request: decode_versioned(&row.get::<_, String>(3)?, JSON_VERSION)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    created_at: TimestampMillis::new(row.get(4)?),
                })
            },
        )
        .optional()
        .map_err(storage)?
        .ok_or(DynamicMemoryRunRepositoryError::NotFound)?;
    let mut statement = connection
        .prepare(
            "SELECT message_id,revision_id,candidate_id FROM dynamic_memory_run_source_messages \
             WHERE run_id=?1 ORDER BY ordinal",
        )
        .map_err(storage)?;
    run.source_messages = statement
        .query_map([id.to_string()], |row| {
            Ok(DynamicMemorySourceMessage {
                message_id: parse_id(row.get(0)?)?,
                render_source: match (
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ) {
                    (Some(id), None) => MessageRenderSource::Revision(parse_id(id)?),
                    (None, Some(id)) => MessageRenderSource::Candidate(parse_id(id)?),
                    _ => return Err(rusqlite::Error::InvalidQuery),
                },
            })
        })
        .map_err(storage)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(storage)?;
    run.validate()
        .map_err(|_| DynamicMemoryRunRepositoryError::Invalid)?;
    Ok(run)
}

fn load_attempt_in(
    connection: &Connection,
    id: DynamicMemoryAttemptId,
) -> Result<DynamicMemoryAttempt, DynamicMemoryRunRepositoryError> {
    let attempt = connection
        .query_row(
            "SELECT run_id,ordinal,retry_parent_id,job_id,status,failure,revision,created_at,\
                    started_at,finished_at,updated_at \
             FROM dynamic_memory_run_attempts WHERE id=?1",
            [id.to_string()],
            |row| {
                Ok(DynamicMemoryAttempt {
                    id,
                    run_id: parse_id(row.get(0)?)?,
                    ordinal: u16::try_from(row.get::<_, i64>(1)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    retry_parent_id: row.get::<_, Option<String>>(2)?.map(parse_id).transpose()?,
                    job_id: parse_id::<JobId>(row.get(3)?)?,
                    status: parse_status(&row.get::<_, String>(4)?)?,
                    failure: row
                        .get::<_, Option<String>>(5)?
                        .map(|value| parse_failure(&value))
                        .transpose()?,
                    revision: revision(row.get(6)?)?,
                    created_at: TimestampMillis::new(row.get(7)?),
                    started_at: row.get::<_, Option<i64>>(8)?.map(TimestampMillis::new),
                    finished_at: row.get::<_, Option<i64>>(9)?.map(TimestampMillis::new),
                    updated_at: TimestampMillis::new(row.get(10)?),
                })
            },
        )
        .optional()
        .map_err(storage)?
        .ok_or(DynamicMemoryRunRepositoryError::NotFound)?;
    attempt
        .validate()
        .map_err(|_| DynamicMemoryRunRepositoryError::Invalid)?;
    Ok(attempt)
}

fn list_calls_in(
    transaction: &Transaction<'_>,
    run_id: DynamicMemoryRunId,
    attempt_id: DynamicMemoryAttemptId,
) -> Result<Vec<DynamicMemoryToolCallEvidence>, DynamicMemoryRunRepositoryError> {
    let mut statement = transaction
        .prepare(
            "SELECT id,round_ordinal,ordinal,definition_name,definition_version,provider_call_id,\
                    arguments_json,raw_arguments,provider_replay_artifact_id,\
                    provider_replay_retention,admitted_at \
             FROM dynamic_memory_admitted_tool_calls \
             WHERE run_id=?1 AND attempt_id=?2 ORDER BY ordinal",
        )
        .map_err(storage)?;
    let calls = statement
        .query_map(params![run_id.to_string(), attempt_id.to_string()], |row| {
            Ok(DynamicMemoryToolCallEvidence {
                id: parse_id(row.get(0)?)?,
                run_id,
                attempt_id,
                round_ordinal: u8::try_from(row.get::<_, i64>(1)?)
                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                ordinal: u16::try_from(row.get::<_, i64>(2)?)
                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                definition_version: u32::try_from(row.get::<_, i64>(4)?)
                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                call: ProposedToolCall {
                    provider_call_id: row.get(5)?,
                    name: row.get(3)?,
                    arguments: decode_versioned(&row.get::<_, String>(6)?, JSON_VERSION)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    raw_arguments: row.get(7)?,
                    provider_replay: conversation_query::replay_ref(
                        transaction,
                        row.get(8)?,
                        row.get(9)?,
                    )
                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                },
                admitted_at: TimestampMillis::new(row.get(10)?),
            })
        })
        .map_err(storage)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(storage)?;
    for call in &calls {
        call.validate()
            .map_err(|_| DynamicMemoryRunRepositoryError::Invalid)?;
    }
    Ok(calls)
}

fn list_rounds_in(
    transaction: &Transaction<'_>,
    run_id: DynamicMemoryRunId,
    attempt_id: DynamicMemoryAttemptId,
) -> Result<Vec<DynamicMemoryInferenceRound>, DynamicMemoryRunRepositoryError> {
    let calls = list_calls_in(transaction, run_id, attempt_id)?;
    let mut statement = transaction
        .prepare(
            "SELECT ordinal,first_call_ordinal,call_count,parts_json,provider_replay_artifact_id,\
                    provider_replay_retention,input_tokens,output_tokens,finish_reason,\
                    provider_request_id,admitted_at \
             FROM dynamic_memory_inference_rounds \
             WHERE run_id=?1 AND attempt_id=?2 ORDER BY ordinal",
        )
        .map_err(storage)?;
    let rounds = statement
        .query_map(params![run_id.to_string(), attempt_id.to_string()], |row| {
            let ordinal =
                u8::try_from(row.get::<_, i64>(0)?).map_err(|_| rusqlite::Error::InvalidQuery)?;
            let first_call_ordinal =
                u16::try_from(row.get::<_, i64>(1)?).map_err(|_| rusqlite::Error::InvalidQuery)?;
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
            Ok(DynamicMemoryInferenceRound {
                run_id,
                attempt_id,
                ordinal,
                first_call_ordinal,
                parts: decode_versioned(&row.get::<_, String>(3)?, JSON_VERSION)
                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                provider_replay: conversation_query::replay_ref(
                    transaction,
                    row.get(4)?,
                    row.get(5)?,
                )
                .map_err(|_| rusqlite::Error::InvalidQuery)?,
                usage: match (row.get::<_, Option<i64>>(6)?, row.get::<_, Option<i64>>(7)?) {
                    (Some(input), Some(output)) => Some(InferenceUsage {
                        input_tokens: u64::try_from(input)
                            .map_err(|_| rusqlite::Error::InvalidQuery)?,
                        output_tokens: u64::try_from(output)
                            .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    }),
                    (None, None) => None,
                    _ => return Err(rusqlite::Error::InvalidQuery),
                },
                finish_reason: match row.get::<_, String>(8)?.as_str() {
                    "stop" => DynamicMemoryRoundFinishReason::Stop,
                    "length" => DynamicMemoryRoundFinishReason::Length,
                    _ => return Err(rusqlite::Error::InvalidQuery),
                },
                provider_request_id: row.get(9)?,
                calls: round_calls,
                admitted_at: TimestampMillis::new(row.get(10)?),
            })
        })
        .map_err(storage)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(storage)?;
    for round in &rounds {
        round
            .validate()
            .map_err(|_| DynamicMemoryRunRepositoryError::Invalid)?;
    }
    Ok(rounds)
}

fn insert_round_in(
    transaction: &Transaction<'_>,
    round: &DynamicMemoryInferenceRound,
) -> Result<(), DynamicMemoryRunRepositoryError> {
    let (replay_id, replay_retention) = round
        .provider_replay
        .as_ref()
        .map(|reference| {
            (
                Some(reference.artifact_id.to_string()),
                Some("conversation"),
            )
        })
        .unwrap_or((None, None));
    let (input_tokens, output_tokens) = round
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
            "INSERT INTO dynamic_memory_inference_rounds \
             (run_id,attempt_id,ordinal,first_call_ordinal,call_count,parts_json,\
              provider_replay_artifact_id,provider_replay_retention,input_tokens,output_tokens,\
              finish_reason,provider_request_id,admitted_at) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
            params![
                round.run_id.to_string(),
                round.attempt_id.to_string(),
                i64::from(round.ordinal),
                i64::from(round.first_call_ordinal),
                i64::try_from(round.calls.len()).map_err(storage)?,
                encode_versioned(&round.parts, JSON_VERSION).map_err(storage)?,
                replay_id,
                replay_retention,
                input_tokens,
                output_tokens,
                match round.finish_reason {
                    DynamicMemoryRoundFinishReason::Stop => "stop",
                    DynamicMemoryRoundFinishReason::Length => "length",
                },
                round.provider_request_id.as_deref(),
                round.admitted_at.get(),
            ],
        )
        .map_err(storage)?;
    for evidence in &round.calls {
        let (call_replay_id, call_replay_retention) = evidence
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
                "INSERT INTO dynamic_memory_admitted_tool_calls \
                 (run_id,attempt_id,round_ordinal,id,ordinal,definition_name,definition_version,\
                  provider_call_id,arguments_json,raw_arguments,provider_replay_artifact_id,\
                  provider_replay_retention,admitted_at) \
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
                params![
                    evidence.run_id.to_string(),
                    evidence.attempt_id.to_string(),
                    i64::from(evidence.round_ordinal),
                    evidence.id.to_string(),
                    i64::from(evidence.ordinal),
                    evidence.call.name,
                    i64::from(evidence.definition_version),
                    evidence.call.provider_call_id,
                    encode_versioned(&evidence.call.arguments, JSON_VERSION).map_err(storage)?,
                    evidence.call.raw_arguments,
                    call_replay_id,
                    call_replay_retention,
                    evidence.admitted_at.get(),
                ],
            )
            .map_err(storage)?;
    }
    Ok(())
}

impl DynamicMemoryRunRepository for Database {
    fn admit_dynamic_memory_run_attempt(
        &self,
        input: NewDynamicMemoryRunAttempt,
    ) -> Result<DynamicMemoryRunAttemptAdmission, DynamicMemoryRunRepositoryError> {
        let requested_run = DynamicMemoryRun {
            id: input.run_id,
            conversation_id: input.conversation_id,
            space_id: input.space_id,
            source_messages: input.source_messages,
            profile: input.profile,
            tool_request: dynamic_memory_tool_request(),
            created_at: input.now,
        };
        requested_run
            .validate()
            .map_err(|_| DynamicMemoryRunRepositoryError::Invalid)?;
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        match load_run_in(&transaction, input.run_id) {
            Ok(run) => {
                let attempt = load_attempt_in(&transaction, input.attempt_id)?;
                if run == requested_run
                    && attempt.run_id == run.id
                    && attempt.ordinal == 0
                    && attempt.retry_parent_id.is_none()
                    && attempt.job_id == input.job_id
                    && attempt.created_at == input.now
                {
                    transaction.commit().map_err(storage)?;
                    return Ok(DynamicMemoryRunAttemptAdmission { run, attempt });
                }
                return Err(DynamicMemoryRunRepositoryError::Conflict);
            }
            Err(DynamicMemoryRunRepositoryError::NotFound) => {}
            Err(error) => return Err(error),
        }
        match load_attempt_in(&transaction, input.attempt_id) {
            Ok(_) => return Err(DynamicMemoryRunRepositoryError::Conflict),
            Err(DynamicMemoryRunRepositoryError::NotFound) => {}
            Err(error) => return Err(error),
        }
        transaction
            .execute(
                "INSERT INTO dynamic_memory_runs \
                 (id,conversation_id,space_id,profile_json,tool_request_json,created_at) \
                 VALUES (?1,?2,?3,?4,?5,?6)",
                params![
                    requested_run.id.to_string(),
                    requested_run.conversation_id.to_string(),
                    requested_run.space_id.to_string(),
                    encode_versioned(&requested_run.profile, JSON_VERSION).map_err(storage)?,
                    encode_versioned(&requested_run.tool_request, JSON_VERSION).map_err(storage)?,
                    requested_run.created_at.get(),
                ],
            )
            .map_err(|error| match error.sqlite_error_code() {
                Some(rusqlite::ErrorCode::ConstraintViolation) => {
                    DynamicMemoryRunRepositoryError::Conflict
                }
                _ => DynamicMemoryRunRepositoryError::Storage,
            })?;
        for (ordinal, source) in requested_run.source_messages.iter().enumerate() {
            let (revision_id, candidate_id) = match source.render_source {
                MessageRenderSource::Revision(id) => (Some(id.to_string()), None),
                MessageRenderSource::Candidate(id) => (None, Some(id.to_string())),
            };
            transaction
                .execute(
                    "INSERT INTO dynamic_memory_run_source_messages \
                     (run_id,conversation_id,message_id,revision_id,candidate_id,ordinal) \
                     VALUES (?1,?2,?3,?4,?5,?6)",
                    params![
                        requested_run.id.to_string(),
                        requested_run.conversation_id.to_string(),
                        source.message_id.to_string(),
                        revision_id,
                        candidate_id,
                        i64::try_from(ordinal).map_err(storage)?,
                    ],
                )
                .map_err(|error| match error.sqlite_error_code() {
                    Some(rusqlite::ErrorCode::ConstraintViolation) => {
                        DynamicMemoryRunRepositoryError::Conflict
                    }
                    _ => DynamicMemoryRunRepositoryError::Storage,
                })?;
        }
        transaction
            .execute(
                "INSERT INTO dynamic_memory_run_attempts \
                 (run_id,id,ordinal,retry_parent_id,job_id,status,failure,revision,created_at,\
                  started_at,finished_at,updated_at) \
                 VALUES (?1,?2,0,NULL,?3,'created',NULL,1,?4,NULL,NULL,?4)",
                params![
                    requested_run.id.to_string(),
                    input.attempt_id.to_string(),
                    input.job_id.to_string(),
                    input.now.get(),
                ],
            )
            .map_err(storage)?;
        let run = load_run_in(&transaction, input.run_id)?;
        let attempt = load_attempt_in(&transaction, input.attempt_id)?;
        transaction.commit().map_err(storage)?;
        Ok(DynamicMemoryRunAttemptAdmission { run, attempt })
    }

    fn load_dynamic_memory_run(
        &self,
        id: DynamicMemoryRunId,
    ) -> Result<DynamicMemoryRun, DynamicMemoryRunRepositoryError> {
        let connection = self.connection().map_err(storage)?;
        load_run_in(&connection, id)
    }

    fn load_dynamic_memory_attempt(
        &self,
        id: DynamicMemoryAttemptId,
    ) -> Result<DynamicMemoryAttempt, DynamicMemoryRunRepositoryError> {
        let connection = self.connection().map_err(storage)?;
        load_attempt_in(&connection, id)
    }

    fn load_latest_dynamic_memory_attempt(
        &self,
        run_id: DynamicMemoryRunId,
    ) -> Result<DynamicMemoryAttempt, DynamicMemoryRunRepositoryError> {
        let connection = self.connection().map_err(storage)?;
        let id = connection
            .query_row(
                "SELECT id FROM dynamic_memory_run_attempts \
                 WHERE run_id=?1 ORDER BY ordinal DESC LIMIT 1",
                [run_id.to_string()],
                |row| parse_id(row.get(0)?),
            )
            .optional()
            .map_err(storage)?
            .ok_or(DynamicMemoryRunRepositoryError::NotFound)?;
        load_attempt_in(&connection, id)
    }

    fn transition_dynamic_memory_attempt(
        &self,
        id: DynamicMemoryAttemptId,
        expected_revision: Revision,
        next: DynamicMemoryAttemptStatus,
        failure: Option<DynamicMemoryAttemptFailureCode>,
        at: TimestampMillis,
    ) -> Result<DynamicMemoryAttempt, DynamicMemoryRunRepositoryError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let current = load_attempt_in(&transaction, id)?;
        if current.revision != expected_revision {
            return Err(DynamicMemoryRunRepositoryError::Conflict);
        }
        let updated = current
            .transition(next, failure, at)
            .map_err(|_| DynamicMemoryRunRepositoryError::Conflict)?;
        let changed = transaction
            .execute(
                "UPDATE dynamic_memory_run_attempts \
                 SET status=?2,failure=?3,revision=?4,started_at=?5,finished_at=?6,updated_at=?7 \
                 WHERE id=?1 AND revision=?8",
                params![
                    id.to_string(),
                    status_name(updated.status),
                    updated.failure.map(failure_name),
                    sql_u64(updated.revision.get())?,
                    updated.started_at.map(TimestampMillis::get),
                    updated.finished_at.map(TimestampMillis::get),
                    updated.updated_at.get(),
                    sql_u64(expected_revision.get())?,
                ],
            )
            .map_err(storage)?;
        if changed != 1 {
            return Err(DynamicMemoryRunRepositoryError::Conflict);
        }
        let stored = load_attempt_in(&transaction, id)?;
        transaction.commit().map_err(storage)?;
        Ok(stored)
    }

    fn recover_dynamic_memory_attempt(
        &self,
        input: NewDynamicMemoryAttemptRecovery,
    ) -> Result<DynamicMemoryAttemptRecovery, DynamicMemoryRunRepositoryError> {
        if input.parent_attempt_id == input.child_attempt_id {
            return Err(DynamicMemoryRunRepositoryError::Invalid);
        }
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let run = load_run_in(&transaction, input.run_id)?;
        let parent = load_attempt_in(&transaction, input.parent_attempt_id)?;
        if parent.run_id != input.run_id || parent.job_id == input.job_id {
            return Err(DynamicMemoryRunRepositoryError::Conflict);
        }
        let parent_rounds = list_rounds_in(&transaction, input.run_id, parent.id)?;
        match load_attempt_in(&transaction, input.child_attempt_id) {
            Ok(child) => {
                let expected_ordinal = parent
                    .ordinal
                    .checked_add(1)
                    .ok_or(DynamicMemoryRunRepositoryError::Storage)?;
                let mut expected_rounds = parent_rounds.clone();
                for round in &mut expected_rounds {
                    round.attempt_id = child.id;
                    for call in &mut round.calls {
                        call.attempt_id = child.id;
                    }
                }
                if parent.status != DynamicMemoryAttemptStatus::Interrupted
                    || parent.finished_at != Some(input.now)
                    || child.run_id != input.run_id
                    || child.ordinal != expected_ordinal
                    || child.retry_parent_id != Some(parent.id)
                    || child.job_id != input.job_id
                    || child.status != DynamicMemoryAttemptStatus::Processing
                    || child.created_at != input.now
                    || list_rounds_in(&transaction, input.run_id, child.id)? != expected_rounds
                {
                    return Err(DynamicMemoryRunRepositoryError::Conflict);
                }
                transaction.commit().map_err(storage)?;
                return Ok(DynamicMemoryAttemptRecovery { run, parent, child });
            }
            Err(DynamicMemoryRunRepositoryError::NotFound) => {}
            Err(error) => return Err(error),
        }
        if parent.status != DynamicMemoryAttemptStatus::Processing || input.now < parent.updated_at
        {
            return Err(DynamicMemoryRunRepositoryError::Conflict);
        }
        let child_ordinal = parent
            .ordinal
            .checked_add(1)
            .ok_or(DynamicMemoryRunRepositoryError::Storage)?;
        let actual_next: i64 = transaction
            .query_row(
                "SELECT coalesce(max(ordinal) + 1, 0) FROM dynamic_memory_run_attempts \
                 WHERE run_id=?1",
                [input.run_id.to_string()],
                |row| row.get(0),
            )
            .map_err(storage)?;
        if u16::try_from(actual_next).map_err(storage)? != child_ordinal {
            return Err(DynamicMemoryRunRepositoryError::Conflict);
        }
        let interrupted = parent
            .transition(DynamicMemoryAttemptStatus::Interrupted, None, input.now)
            .map_err(|_| DynamicMemoryRunRepositoryError::Conflict)?;
        let changed = transaction
            .execute(
                "UPDATE dynamic_memory_run_attempts \
                 SET status='interrupted',revision=?2,finished_at=?3,updated_at=?3 \
                 WHERE id=?1 AND revision=?4 AND status='processing'",
                params![
                    parent.id.to_string(),
                    sql_u64(interrupted.revision.get())?,
                    input.now.get(),
                    sql_u64(parent.revision.get())?,
                ],
            )
            .map_err(storage)?;
        if changed != 1 {
            return Err(DynamicMemoryRunRepositoryError::Conflict);
        }
        transaction
            .execute(
                "INSERT INTO dynamic_memory_run_attempts \
                 (run_id,id,ordinal,retry_parent_id,job_id,status,failure,revision,created_at,\
                  started_at,finished_at,updated_at) \
                 VALUES (?1,?2,?3,?4,?5,'processing',NULL,1,?6,?6,NULL,?6)",
                params![
                    input.run_id.to_string(),
                    input.child_attempt_id.to_string(),
                    i64::from(child_ordinal),
                    parent.id.to_string(),
                    input.job_id.to_string(),
                    input.now.get(),
                ],
            )
            .map_err(|error| match error.sqlite_error_code() {
                Some(rusqlite::ErrorCode::ConstraintViolation) => {
                    DynamicMemoryRunRepositoryError::Conflict
                }
                _ => DynamicMemoryRunRepositoryError::Storage,
            })?;
        for mut round in parent_rounds {
            round.attempt_id = input.child_attempt_id;
            for call in &mut round.calls {
                call.attempt_id = input.child_attempt_id;
            }
            insert_round_in(&transaction, &round)?;
        }
        let parent = load_attempt_in(&transaction, parent.id)?;
        let child = load_attempt_in(&transaction, input.child_attempt_id)?;
        transaction.commit().map_err(storage)?;
        Ok(DynamicMemoryAttemptRecovery { run, parent, child })
    }

    fn admit_dynamic_memory_inference_round(
        &self,
        run_id: DynamicMemoryRunId,
        attempt_id: DynamicMemoryAttemptId,
        expected_round_ordinal: u8,
        expected_next_call_ordinal: u16,
        round: NewDynamicMemoryInferenceRound,
    ) -> Result<DynamicMemoryInferenceRound, DynamicMemoryRunRepositoryError> {
        round
            .validate()
            .map_err(|_| DynamicMemoryRunRepositoryError::Invalid)?;
        if round.ordinal != expected_round_ordinal {
            return Err(DynamicMemoryRunRepositoryError::Invalid);
        }
        let requested = DynamicMemoryInferenceRound {
            run_id,
            attempt_id,
            ordinal: round.ordinal,
            first_call_ordinal: expected_next_call_ordinal,
            parts: round.parts,
            provider_replay: round.provider_replay,
            usage: round.usage,
            finish_reason: round.finish_reason,
            provider_request_id: round.provider_request_id,
            calls: round
                .calls
                .into_iter()
                .enumerate()
                .map(|(offset, call)| {
                    Ok(DynamicMemoryToolCallEvidence {
                        id: call.id,
                        run_id,
                        attempt_id,
                        round_ordinal: expected_round_ordinal,
                        ordinal: expected_next_call_ordinal
                            .checked_add(u16::try_from(offset).map_err(storage)?)
                            .ok_or(DynamicMemoryRunRepositoryError::Invalid)?,
                        definition_version: call.definition_version,
                        call: call.call,
                        admitted_at: round.admitted_at,
                    })
                })
                .collect::<Result<Vec<_>, DynamicMemoryRunRepositoryError>>()?,
            admitted_at: round.admitted_at,
        };
        requested
            .validate()
            .map_err(|_| DynamicMemoryRunRepositoryError::Invalid)?;
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let attempt = load_attempt_in(&transaction, attempt_id)?;
        if attempt.run_id != run_id {
            return Err(DynamicMemoryRunRepositoryError::Conflict);
        }
        let existing = list_rounds_in(&transaction, run_id, attempt_id)?;
        if existing.len() != usize::from(expected_round_ordinal) {
            if existing.get(usize::from(expected_round_ordinal)) == Some(&requested) {
                transaction.commit().map_err(storage)?;
                return Ok(requested);
            }
            return Err(DynamicMemoryRunRepositoryError::Conflict);
        }
        let actual_next = list_calls_in(&transaction, run_id, attempt_id)?.len();
        if u16::try_from(actual_next).map_err(storage)? != expected_next_call_ordinal
            || attempt.status != DynamicMemoryAttemptStatus::Processing
            || requested.admitted_at < attempt.updated_at
        {
            return Err(DynamicMemoryRunRepositoryError::Conflict);
        }
        insert_round_in(&transaction, &requested)?;
        let stored = list_rounds_in(&transaction, run_id, attempt_id)?
            .last()
            .cloned()
            .ok_or(DynamicMemoryRunRepositoryError::Storage)?;
        if stored != requested {
            return Err(DynamicMemoryRunRepositoryError::Conflict);
        }
        transaction.commit().map_err(storage)?;
        Ok(stored)
    }

    fn list_dynamic_memory_inference_rounds(
        &self,
        run_id: DynamicMemoryRunId,
        attempt_id: DynamicMemoryAttemptId,
    ) -> Result<Vec<DynamicMemoryInferenceRound>, DynamicMemoryRunRepositoryError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(storage)?;
        let attempt = load_attempt_in(&transaction, attempt_id)?;
        if attempt.run_id != run_id {
            return Err(DynamicMemoryRunRepositoryError::Conflict);
        }
        let rounds = list_rounds_in(&transaction, run_id, attempt_id)?;
        transaction.commit().map_err(storage)?;
        Ok(rounds)
    }

    fn list_dynamic_memory_tool_calls(
        &self,
        run_id: DynamicMemoryRunId,
        attempt_id: DynamicMemoryAttemptId,
    ) -> Result<Vec<DynamicMemoryToolCallEvidence>, DynamicMemoryRunRepositoryError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(storage)?;
        let attempt = load_attempt_in(&transaction, attempt_id)?;
        if attempt.run_id != run_id {
            return Err(DynamicMemoryRunRepositoryError::Conflict);
        }
        let calls = list_calls_in(&transaction, run_id, attempt_id)?;
        transaction.commit().map_err(storage)?;
        Ok(calls)
    }
}

#[cfg(test)]
mod tests {
    use lettuce_conversations::{
        MessagePart, MessageRenderSource, OutputPolicy, ResolvedInferenceProfile, SafetyContext,
        ToolPolicy,
    };
    use lettuce_memory::{
        DynamicMemoryAttemptFailureCode, DynamicMemoryAttemptStatus,
        DynamicMemoryRoundFinishReason, DynamicMemoryRunRepository, DynamicMemorySourceMessage,
        NewDynamicMemoryAttemptRecovery, NewDynamicMemoryInferenceRound,
        NewDynamicMemoryRunAttempt, NewDynamicMemoryToolCall,
    };
    use lettuce_models::{
        CapabilityStatus, ChatParameterResolutionInput, ChatRequirements, ExpectedModelIdentity,
        ModelCapabilities, ModelKind, ModelProfile, ModelProfileConfig, ProviderAccount,
        ProviderConfig, ProviderProtocol,
    };
    use lettuce_settings::SecretOwnerId;
    use lettuce_types::{
        CharacterId, ConversationBranchId, ConversationId, ConversationParticipantId,
        DynamicMemoryAttemptId, DynamicMemoryRunId, JobId, MemorySpaceId, MessageId,
        MessageRevisionId, ModelProfileId, ProviderAccountId, Revision, TimestampMillis,
        ToolExecutionId,
    };
    use rusqlite::{TransactionBehavior, params};
    use serde_json::json;

    use crate::Database;

    fn profile() -> ResolvedInferenceProfile {
        let account_id = ProviderAccountId::new();
        let profile_id = ModelProfileId::new();
        let account = ProviderAccount {
            id: account_id,
            secret_owner_id: SecretOwnerId::new(),
            provider_kind: "ollama".into(),
            protocol: ProviderProtocol::Ollama,
            label: "Ollama".into(),
            endpoint: Some("http://127.0.0.1:11434".into()),
            enabled: true,
            streaming_enabled: false,
            allow_invalid_tls: false,
            api_key_ref: None,
            secret_headers: Vec::new(),
            config: ProviderConfig::Standard,
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(1),
            updated_at: TimestampMillis::new(1),
        };
        let model = ModelProfile {
            id: profile_id,
            provider_account_id: account_id,
            external_model_id: "memory-model".into(),
            display_name: "Memory model".into(),
            kind: ModelKind::Chat,
            config: ModelProfileConfig {
                chat_parameters: Default::default(),
                capabilities: ModelCapabilities {
                    input_modalities: lettuce_models::ModalityCapabilities {
                        text: CapabilityStatus::Supported,
                        ..Default::default()
                    },
                    output_modalities: lettuce_models::ModalityCapabilities {
                        text: CapabilityStatus::Supported,
                        ..Default::default()
                    },
                    tools: CapabilityStatus::Supported,
                    ..Default::default()
                },
            },
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(1),
            updated_at: TimestampMillis::new(1),
        };
        let expected = ExpectedModelIdentity {
            model_profile_id: profile_id,
            model_revision: model.revision,
            provider_account_id: account_id,
            provider_account_revision: account.revision,
            external_model_id: model.external_model_id.clone(),
            display_name: model.display_name.clone(),
            provider_protocol: account.protocol,
            model_kind: ModelKind::Chat,
        };
        ResolvedInferenceProfile {
            chat_profile: lettuce_models::resolve_chat_profile(
                &expected,
                &model,
                &account,
                &ChatParameterResolutionInput::default(),
                &ChatRequirements {
                    require_tools: true,
                    ..Default::default()
                },
            )
            .expect("profile"),
            tool_policy: ToolPolicy::Required,
            output_policy: OutputPolicy::Plain,
            safety_policy: SafetyContext::Standard,
            correlation_id: None,
        }
    }

    fn conversation_fixture(
        database: &Database,
    ) -> (
        ConversationId,
        MemorySpaceId,
        Vec<DynamicMemorySourceMessage>,
    ) {
        let conversation_id = ConversationId::new();
        let branch_id = ConversationBranchId::new();
        let space_id = MemorySpaceId::new();
        let messages = vec![MessageId::new(), MessageId::new()];
        let revisions = vec![MessageRevisionId::new(), MessageRevisionId::new()];
        let mut connection = database.connection().expect("connection");
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("transaction");
        transaction
            .execute(
                "INSERT INTO conversations \
                 (id,kind,lifecycle,title,active_branch_id,kind_json,revision,next_timeline_ordinal,created_at,updated_at) \
                 VALUES (?1,'direct','active','Memory test',?2,?3,1,3,1,1)",
                params![
                    conversation_id.to_string(),
                    branch_id.to_string(),
                    json!({"format_version":1,"value":{"kind":"direct"}}).to_string(),
                ],
            )
            .expect("conversation");
        transaction
            .execute(
                "INSERT INTO conversation_branches \
                 (conversation_id,id,parent_branch_id,fork_message_id,head_message_id,status,revision,created_at,updated_at) \
                 VALUES (?1,?2,NULL,NULL,NULL,'active',1,1,1)",
                params![conversation_id.to_string(), branch_id.to_string()],
            )
            .expect("branch");
        let participants = [
            ConversationParticipantId::new(),
            ConversationParticipantId::new(),
        ];
        for (ordinal, participant_id) in participants.iter().enumerate() {
            transaction
                .execute(
                    "INSERT INTO conversation_participants \
                     (conversation_id,id,role,ordinal,source_kind,source_id,enabled,muted,\
                      display_name,authored_description,model_selection_json,revision,created_at,updated_at) \
                     VALUES (?1,?2,?3,?4,?5,?6,1,0,?7,NULL,?8,1,1,1)",
                    params![
                        conversation_id.to_string(),
                        participant_id.to_string(),
                        if ordinal == 0 { "user" } else { "character" },
                        i64::try_from(ordinal).expect("ordinal"),
                        if ordinal == 0 { "user" } else { "character" },
                        if ordinal == 0 {
                            None
                        } else {
                            Some(CharacterId::new().to_string())
                        },
                        if ordinal == 0 { "User" } else { "Character" },
                        json!({"format_version":1,"value":{"kind":"inherit"}}).to_string(),
                    ],
                )
                .expect("participant");
        }
        for (offset, message_id) in messages.iter().enumerate() {
            let revision_id = revisions[offset];
            transaction
                .execute(
                    "INSERT INTO conversation_messages \
                     (conversation_id,id,branch_id,parent_message_id,author_participant_id,role,\
                      timeline_ordinal,logical_time,effective_time,visibility,pinned,scene_edited,\
                      active_revision_id,active_candidate_id,revision,created_at,updated_at) \
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?7,?7,'visible',0,0,?8,NULL,1,?7,?7)",
                    params![
                        conversation_id.to_string(),
                        message_id.to_string(),
                        branch_id.to_string(),
                        offset
                            .checked_sub(1)
                            .map(|index| messages[index].to_string()),
                        participants[offset].to_string(),
                        if offset == 0 { "user" } else { "assistant" },
                        i64::try_from(offset + 1).expect("ordinal"),
                        revision_id.to_string(),
                    ],
                )
                .expect("message");
            transaction
                .execute(
                    "INSERT INTO conversation_message_revisions \
                     (conversation_id,id,message_id,branch_id,sequence,parts_json,authored_at,\
                      source_turn_id,provider_replay_artifact_id,provider_replay_retention) \
                     VALUES (?1,?2,?3,?4,1,?5,?6,NULL,NULL,NULL)",
                    params![
                        conversation_id.to_string(),
                        revision_id.to_string(),
                        message_id.to_string(),
                        branch_id.to_string(),
                        json!({"format_version":1,"value":[{"kind":"text","details":{"text":"message"}}]}).to_string(),
                        i64::try_from(offset + 1).expect("time"),
                    ],
                )
                .expect("revision");
        }
        transaction
            .execute(
                "INSERT INTO memory_spaces (id,revision) VALUES (?1,1)",
                [space_id.to_string()],
            )
            .expect("space");
        transaction
            .execute(
                "INSERT INTO conversation_memory_spaces (conversation_id,space_id) VALUES (?1,?2)",
                params![conversation_id.to_string(), space_id.to_string()],
            )
            .expect("binding");
        transaction.commit().expect("commit");
        (
            conversation_id,
            space_id,
            messages
                .into_iter()
                .zip(revisions)
                .map(|(message_id, revision_id)| DynamicMemorySourceMessage {
                    message_id,
                    render_source: MessageRenderSource::Revision(revision_id),
                })
                .collect(),
        )
    }

    fn visible_counts(database: &Database, conversation_id: ConversationId) -> (i64, i64) {
        let connection = database.connection().expect("connection");
        let messages = connection
            .query_row(
                "SELECT count(*) FROM conversation_messages WHERE conversation_id=?1",
                [conversation_id.to_string()],
                |row| row.get(0),
            )
            .expect("message count");
        let turns = connection
            .query_row(
                "SELECT count(*) FROM conversation_turns WHERE conversation_id=?1",
                [conversation_id.to_string()],
                |row| row.get(0),
            )
            .expect("turn count");
        (messages, turns)
    }

    #[test]
    fn background_run_replays_and_recovers_without_visible_conversation_mutation() {
        let database = Database::open_in_memory().expect("database");
        let (conversation_id, space_id, messages) = conversation_fixture(&database);
        let before = visible_counts(&database, conversation_id);
        let run_id = DynamicMemoryRunId::new();
        let parent_id = DynamicMemoryAttemptId::new();
        let admission = NewDynamicMemoryRunAttempt {
            run_id,
            attempt_id: parent_id,
            conversation_id,
            space_id,
            source_messages: messages.clone(),
            profile: profile(),
            job_id: JobId::new(),
            now: TimestampMillis::new(10),
        };
        let admitted = database
            .admit_dynamic_memory_run_attempt(admission.clone())
            .expect("admit");
        assert_eq!(
            database
                .admit_dynamic_memory_run_attempt(admission)
                .expect("exact replay"),
            admitted
        );
        let parent = database
            .transition_dynamic_memory_attempt(
                parent_id,
                admitted.attempt.revision,
                DynamicMemoryAttemptStatus::Processing,
                None,
                TimestampMillis::new(11),
            )
            .expect("processing");
        let call_id = ToolExecutionId::new();
        let round = NewDynamicMemoryInferenceRound {
            ordinal: 0,
            parts: vec![MessagePart::ReasoningSummary {
                text: "Found a durable preference".into(),
            }],
            provider_replay: None,
            usage: None,
            finish_reason: DynamicMemoryRoundFinishReason::Stop,
            provider_request_id: Some("memory-request-1".into()),
            calls: vec![NewDynamicMemoryToolCall {
                id: call_id,
                definition_version: 1,
                call: lettuce_conversations::ProposedToolCall {
                    provider_call_id: Some("call-1".into()),
                    name: "create_memory".into(),
                    arguments: json!({
                        "text":"The user prefers tea",
                        "category":"preference",
                        "source_message_id":messages[0].message_id.to_string()
                    }),
                    raw_arguments: None,
                    provider_replay: None,
                },
            }],
            admitted_at: TimestampMillis::new(12),
        };
        let admitted_round = database
            .admit_dynamic_memory_inference_round(run_id, parent_id, 0, 0, round.clone())
            .expect("round");
        assert_eq!(
            database
                .admit_dynamic_memory_inference_round(run_id, parent_id, 0, 0, round)
                .expect("round replay"),
            admitted_round
        );
        let child_id = DynamicMemoryAttemptId::new();
        let recovery = NewDynamicMemoryAttemptRecovery {
            run_id,
            parent_attempt_id: parent_id,
            child_attempt_id: child_id,
            job_id: JobId::new(),
            now: TimestampMillis::new(13),
        };
        let recovered = database
            .recover_dynamic_memory_attempt(recovery.clone())
            .expect("recovery");
        assert_eq!(
            recovered.parent.status,
            DynamicMemoryAttemptStatus::Interrupted
        );
        assert_eq!(
            recovered.child.status,
            DynamicMemoryAttemptStatus::Processing
        );
        assert_eq!(
            database
                .recover_dynamic_memory_attempt(recovery)
                .expect("recovery replay"),
            recovered
        );
        let child_rounds = database
            .list_dynamic_memory_inference_rounds(run_id, child_id)
            .expect("child rounds");
        assert_eq!(child_rounds.len(), 1);
        assert_eq!(child_rounds[0].calls[0].call, admitted_round.calls[0].call);
        let failed = database
            .transition_dynamic_memory_attempt(
                child_id,
                recovered.child.revision,
                DynamicMemoryAttemptStatus::Failed,
                Some(DynamicMemoryAttemptFailureCode::ProviderRejected),
                TimestampMillis::new(14),
            )
            .expect("failure");
        assert_eq!(
            failed.failure,
            Some(DynamicMemoryAttemptFailureCode::ProviderRejected)
        );
        assert_eq!(visible_counts(&database, conversation_id), before);
        assert_eq!(before, (2, 0));
        assert_eq!(parent.status, DynamicMemoryAttemptStatus::Processing);
    }
}
