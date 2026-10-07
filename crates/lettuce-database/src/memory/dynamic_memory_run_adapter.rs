use std::str::FromStr;

use lettuce_conversations::{InferenceUsage, MessageRenderSource, ProposedToolCall};
use lettuce_memory::{
    DynamicMemoryAttempt, DynamicMemoryAttemptFailureCode, DynamicMemoryAttemptRecovery,
    DynamicMemoryAttemptStatus, DynamicMemoryBackgroundRoundCommit,
    DynamicMemoryBackgroundRoundSettlement, DynamicMemoryInferenceRound,
    DynamicMemoryRoundFinishReason, DynamicMemoryRoundKind, DynamicMemoryRun,
    DynamicMemoryRunAttemptAdmission, DynamicMemoryRunRepository, DynamicMemoryRunRepositoryError,
    DynamicMemorySourceMessage, DynamicMemoryStructuredFallbackFormat,
    DynamicMemorySummaryCheckpoint, DynamicMemorySummaryCommit, DynamicMemorySummaryWindow,
    DynamicMemoryToolCallEvidence, MemoryRepositoryError, MemorySummary, MemoryToolResult,
    NewDynamicMemoryAttemptRecovery, NewDynamicMemoryInferenceRound, NewDynamicMemoryRunAttempt,
};
use lettuce_types::{DynamicMemoryAttemptId, DynamicMemoryRunId, JobId, Revision, TimestampMillis};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};

use crate::{
    Database, conversation::conversation_query, decode_versioned, encode_versioned,
    memory::memory_adapter,
};

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

fn optional_count(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<Option<u64>> {
    row.get::<_, Option<i64>>(index)?
        .map(u64::try_from)
        .transpose()
        .map_err(|_| rusqlite::Error::InvalidQuery)
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

fn fallback_format_name(format: DynamicMemoryStructuredFallbackFormat) -> &'static str {
    match format {
        DynamicMemoryStructuredFallbackFormat::Json => "json",
        DynamicMemoryStructuredFallbackFormat::Xml => "xml",
    }
}

fn parse_fallback_format(value: &str) -> rusqlite::Result<DynamicMemoryStructuredFallbackFormat> {
    match value {
        "json" => Ok(DynamicMemoryStructuredFallbackFormat::Json),
        "xml" => Ok(DynamicMemoryStructuredFallbackFormat::Xml),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

pub(crate) fn load_run_in(
    connection: &Connection,
    id: DynamicMemoryRunId,
) -> Result<DynamicMemoryRun, DynamicMemoryRunRepositoryError> {
    let mut run = connection
        .query_row(
            "SELECT conversation_id,space_id,time_awareness_enabled,supersession_enabled,structured_fallback_format,summary_message_interval,summary_window_start,summary_window_end,starting_memory_json,profile_json,tool_request_json,created_at,branch_id \
             FROM dynamic_memory_runs WHERE id=?1",
            [id.to_string()],
            |row| {
                Ok(DynamicMemoryRun {
                    id,
                    conversation_id: parse_id(row.get(0)?)?,
                    branch_id: parse_id(row.get(12)?)?,
                    space_id: parse_id(row.get(1)?)?,
                    time_awareness_enabled: row.get(2)?,
                    supersession_enabled: row.get(3)?,
                    structured_fallback_format: parse_fallback_format(&row.get::<_, String>(4)?)?,
                    summary_window: DynamicMemorySummaryWindow {
                        message_interval: u32::try_from(row.get::<_, i64>(5)?)
                            .map_err(|_| rusqlite::Error::InvalidQuery)?,
                        start: u64::try_from(row.get::<_, i64>(6)?)
                            .map_err(|_| rusqlite::Error::InvalidQuery)?,
                        end: u64::try_from(row.get::<_, i64>(7)?)
                            .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    },
                    starting_memory: decode_versioned(&row.get::<_, String>(8)?, JSON_VERSION)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    source_messages: Vec::new(),
                    profile: decode_versioned(&row.get::<_, String>(9)?, JSON_VERSION)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    tool_request: decode_versioned(&row.get::<_, String>(10)?, JSON_VERSION)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    created_at: TimestampMillis::new(row.get(11)?),
                })
            },
        )
        .optional()
        .map_err(storage)?
        .ok_or(DynamicMemoryRunRepositoryError::NotFound)?;
    let mut statement = connection
        .prepare(
            "SELECT message_id,role,revision_id,candidate_id,effective_time FROM dynamic_memory_run_source_messages \
             WHERE run_id=?1 ORDER BY ordinal",
        )
        .map_err(storage)?;
    run.source_messages = statement
        .query_map([id.to_string()], |row| {
            Ok(DynamicMemorySourceMessage {
                message_id: parse_id(row.get(0)?)?,
                role: match row.get::<_, String>(1)?.as_str() {
                    "user" => lettuce_conversations::MessageRole::User,
                    "assistant" => lettuce_conversations::MessageRole::Assistant,
                    _ => return Err(rusqlite::Error::InvalidQuery),
                },
                render_source: match (
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ) {
                    (Some(id), None) => MessageRenderSource::Revision(parse_id(id)?),
                    (None, Some(id)) => MessageRenderSource::Candidate(parse_id(id)?),
                    _ => return Err(rusqlite::Error::InvalidQuery),
                },
                effective_time: TimestampMillis::new(row.get(4)?),
            })
        })
        .map_err(storage)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(storage)?;
    run.validate()
        .map_err(|_| DynamicMemoryRunRepositoryError::Invalid)?;
    Ok(run)
}

pub(crate) fn load_attempt_in(
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

/// Makes a finished run's summary checkpoint the space's summary. A run whose
/// tools phase failed still publishes its summary, while the summary cursor
/// only follows succeeded runs (`memory_adapter::summary_cursor_in`). Nothing is
/// written when a suffix rewind of the conversation landed after the
/// checkpoint, or when a newer checkpoint already wrote the space's summary.
fn publish_summary_checkpoint_in(
    transaction: &Transaction<'_>,
    run_id: DynamicMemoryRunId,
) -> Result<(), DynamicMemoryRunRepositoryError> {
    let Some(checkpoint) = load_summary_checkpoint_in(transaction, run_id)? else {
        return Ok(());
    };
    let superseded = transaction
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM dynamic_memory_suffix_rewinds rewind
                  JOIN dynamic_memory_runs run ON run.id = ?1
                 WHERE rewind.conversation_id = run.conversation_id
                   AND rewind.space_id = run.space_id
                   AND rewind.applied_at >= ?3
             ) OR EXISTS(
                SELECT 1 FROM memory_summaries WHERE space_id = ?2 AND updated_at > ?3
             )",
            params![
                run_id.to_string(),
                checkpoint.summary.space_id.to_string(),
                checkpoint.settled_at.get(),
            ],
            |row| row.get::<_, bool>(0),
        )
        .map_err(storage)?;
    if superseded {
        return Ok(());
    }
    memory_adapter::replace_summary_in(
        transaction,
        checkpoint.summary.space_id,
        Some(&checkpoint.summary),
    )
    .map(|_| ())
    .map_err(|error| match error {
        lettuce_memory::MemoryRepositoryError::NotFound => {
            DynamicMemoryRunRepositoryError::NotFound
        }
        _ => DynamicMemoryRunRepositoryError::Storage,
    })
}

pub(crate) fn load_summary_checkpoint_in(
    connection: &Connection,
    run_id: DynamicMemoryRunId,
) -> Result<Option<DynamicMemorySummaryCheckpoint>, DynamicMemoryRunRepositoryError> {
    let row = connection
        .query_row(
            "SELECT attempt_id,space_id,expected_memory_revision,resulting_memory_revision,
                    summary_text,token_count,request_context_json,input_tokens,output_tokens,
                    provider_request_id,settled_at,cached_input_tokens,reasoning_tokens,cache_write_tokens,web_search_requests,provider_reported_cost,image_tokens,audio_tokens,total_tokens
               FROM dynamic_memory_summary_checkpoints WHERE run_id=?1",
            [run_id.to_string()],
            |row| {
                Ok((
                    parse_id(row.get::<_, String>(0)?)?,
                    parse_id(row.get::<_, String>(1)?)?,
                    revision(row.get(2)?)?,
                    revision(row.get(3)?)?,
                    row.get::<_, String>(4)?,
                    u32::try_from(row.get::<_, i64>(5)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    decode_versioned::<lettuce_conversations::ProviderNeutralContext>(
                        &row.get::<_, String>(6)?,
                        JSON_VERSION,
                    )
                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    row.get::<_, Option<i64>>(7)?,
                    row.get::<_, Option<i64>>(8)?,
                    row.get::<_, Option<String>>(9)?,
                    TimestampMillis::new(row.get(10)?),
                    row.get::<_, Option<i64>>(11)?
                        .map(u64::try_from)
                        .transpose()
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    row.get::<_, Option<i64>>(12)?
                        .map(u64::try_from)
                        .transpose()
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    row.get::<_, Option<i64>>(13)?.map(u64::try_from).transpose().map_err(|_| rusqlite::Error::InvalidQuery)?,
                    row.get::<_, Option<i64>>(14)?.map(u64::try_from).transpose().map_err(|_| rusqlite::Error::InvalidQuery)?,
                    row.get::<_, Option<f64>>(15)?.map(lettuce_conversations::ProviderReportedCost::try_from).transpose().map_err(|_| rusqlite::Error::InvalidQuery)?,
                    optional_count(row, 16)?,
                    optional_count(row, 17)?,
                    optional_count(row, 18)?,
                ))
            },
        )
        .optional()
        .map_err(storage)?;
    let Some((
        attempt_id,
        space_id,
        expected_memory_revision,
        resulting_memory_revision,
        text,
        token_count,
        request_context,
        input_tokens,
        output_tokens,
        provider_request_id,
        settled_at,
        cached_input_tokens,
        reasoning_tokens,
        cache_write_tokens,
        web_search_requests,
        provider_reported_cost,
        image_tokens,
        audio_tokens,
        total_tokens,
    )) = row
    else {
        return Ok(None);
    };
    let run = load_run_in(connection, run_id)?;
    if run.space_id != space_id
        || resulting_memory_revision.get() != expected_memory_revision.get().saturating_add(1)
    {
        return Err(DynamicMemoryRunRepositoryError::Invalid);
    }
    let usage = match (input_tokens, output_tokens) {
        (Some(input), Some(output)) => Some(InferenceUsage {
            provider_reported_cost,
            cache_write_tokens,
            web_search_requests,
            cached_input_tokens,
            reasoning_tokens,
            image_tokens,
            audio_tokens,
            total_tokens,
            input_tokens: u64::try_from(input).map_err(storage)?,
            output_tokens: u64::try_from(output).map_err(storage)?,
        }),
        (None, None) => None,
        _ => return Err(DynamicMemoryRunRepositoryError::Invalid),
    };
    let summary = MemorySummary {
        space_id,
        branch_id: run.branch_id,
        text,
        token_count,
        window_start: run.summary_window.start,
        window_end: run.summary_window.end,
        source_message_ids: run
            .source_messages
            .iter()
            .map(|source| source.message_id)
            .collect(),
        updated_at: settled_at,
    };
    summary
        .validate()
        .map_err(|_| DynamicMemoryRunRepositoryError::Invalid)?;
    request_context
        .validate()
        .map_err(|_| DynamicMemoryRunRepositoryError::Invalid)?;
    Ok(Some(DynamicMemorySummaryCheckpoint {
        run_id,
        attempt_id,
        summary,
        expected_memory_revision,
        resulting_memory_revision,
        request_context,
        usage,
        provider_request_id,
        settled_at,
    }))
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

pub(crate) fn list_rounds_in(
    transaction: &Transaction<'_>,
    run_id: DynamicMemoryRunId,
    attempt_id: DynamicMemoryAttemptId,
) -> Result<Vec<DynamicMemoryInferenceRound>, DynamicMemoryRunRepositoryError> {
    let calls = list_calls_in(transaction, run_id, attempt_id)?;
    let mut statement = transaction
        .prepare(
            "SELECT ordinal,first_call_ordinal,call_count,request_context_json,parts_json,provider_replay_artifact_id,\
                    provider_replay_retention,input_tokens,output_tokens,finish_reason,\
                    provider_request_id,admitted_at,cached_input_tokens,reasoning_tokens,cache_write_tokens,web_search_requests,provider_reported_cost,kind,image_tokens,audio_tokens,total_tokens \
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
                request_context: decode_versioned(&row.get::<_, String>(3)?, JSON_VERSION)
                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                parts: decode_versioned(&row.get::<_, String>(4)?, JSON_VERSION)
                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                provider_replay: conversation_query::replay_ref(
                    transaction,
                    row.get(5)?,
                    row.get(6)?,
                )
                .map_err(|_| rusqlite::Error::InvalidQuery)?,
                usage: match (row.get::<_, Option<i64>>(7)?, row.get::<_, Option<i64>>(8)?) {
                    (Some(input), Some(output)) => Some(InferenceUsage {
                        provider_reported_cost: row
                            .get::<_, Option<f64>>(16)?
                            .map(lettuce_conversations::ProviderReportedCost::try_from)
                            .transpose()
                            .map_err(|_| rusqlite::Error::InvalidQuery)?,
                        cache_write_tokens: row
                            .get::<_, Option<i64>>(14)?
                            .map(u64::try_from)
                            .transpose()
                            .map_err(|_| rusqlite::Error::InvalidQuery)?,
                        web_search_requests: row
                            .get::<_, Option<i64>>(15)?
                            .map(u64::try_from)
                            .transpose()
                            .map_err(|_| rusqlite::Error::InvalidQuery)?,
                        cached_input_tokens: row
                            .get::<_, Option<i64>>(12)?
                            .map(u64::try_from)
                            .transpose()
                            .map_err(|_| rusqlite::Error::InvalidQuery)?,
                        reasoning_tokens: row
                            .get::<_, Option<i64>>(13)?
                            .map(u64::try_from)
                            .transpose()
                            .map_err(|_| rusqlite::Error::InvalidQuery)?,
                        image_tokens: optional_count(row, 18)?,
                        audio_tokens: optional_count(row, 19)?,
                        total_tokens: optional_count(row, 20)?,
                        input_tokens: u64::try_from(input)
                            .map_err(|_| rusqlite::Error::InvalidQuery)?,
                        output_tokens: u64::try_from(output)
                            .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    }),
                    (None, None) => None,
                    _ => return Err(rusqlite::Error::InvalidQuery),
                },
                finish_reason: match row.get::<_, String>(9)?.as_str() {
                    "stop" => DynamicMemoryRoundFinishReason::Stop,
                    "length" => DynamicMemoryRoundFinishReason::Length,
                    _ => return Err(rusqlite::Error::InvalidQuery),
                },
                kind: match row.get::<_, String>(17)?.as_str() {
                    "manager" => DynamicMemoryRoundKind::Manager,
                    "repair" => DynamicMemoryRoundKind::Repair,
                    _ => return Err(rusqlite::Error::InvalidQuery),
                },
                provider_request_id: row.get(10)?,
                calls: round_calls,
                admitted_at: TimestampMillis::new(row.get(11)?),
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

fn background_change_digest(
    change: &Option<lettuce_memory::MemoryChangeSet>,
) -> Result<String, DynamicMemoryRunRepositoryError> {
    let encoded = encode_versioned(change, JSON_VERSION).map_err(storage)?;
    Ok(blake3::hash(encoded.as_bytes()).to_hex().to_string())
}

pub(crate) fn load_background_settlement_in(
    connection: &Connection,
    run_id: DynamicMemoryRunId,
    attempt_id: DynamicMemoryAttemptId,
    round_ordinal: u8,
) -> Result<Option<(DynamicMemoryBackgroundRoundSettlement, String)>, DynamicMemoryRunRepositoryError>
{
    let row = connection
        .query_row(
            "SELECT space_id,expected_memory_revision,resulting_memory_revision,change_digest,settled_at \
             FROM dynamic_memory_background_round_settlements \
             WHERE run_id=?1 AND attempt_id=?2 AND round_ordinal=?3",
            params![run_id.to_string(), attempt_id.to_string(), i64::from(round_ordinal)],
            |row| {
                Ok((
                    parse_id(row.get::<_, String>(0)?)?,
                    revision(row.get(1)?)?,
                    revision(row.get(2)?)?,
                    row.get::<_, String>(3)?,
                    TimestampMillis::new(row.get(4)?),
                ))
            },
        )
        .optional()
        .map_err(storage)?;
    let Some((space_id, expected_memory_revision, resulting_memory_revision, digest, settled_at)) =
        row
    else {
        return Ok(None);
    };
    let mut statement = connection
        .prepare(
            "SELECT call_id,outcome_json FROM dynamic_memory_background_tool_results \
             WHERE run_id=?1 AND attempt_id=?2 AND round_ordinal=?3 ORDER BY ordinal",
        )
        .map_err(storage)?;
    let results = statement
        .query_map(
            params![
                run_id.to_string(),
                attempt_id.to_string(),
                i64::from(round_ordinal)
            ],
            |row| {
                Ok(MemoryToolResult {
                    execution_id: parse_id(row.get(0)?)?,
                    outcome: decode_versioned(&row.get::<_, String>(1)?, JSON_VERSION)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                })
            },
        )
        .map_err(storage)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(storage)?;
    Ok(Some((
        DynamicMemoryBackgroundRoundSettlement {
            run_id,
            attempt_id,
            round_ordinal,
            space_id,
            expected_memory_revision,
            resulting_memory_revision,
            results,
            settled_at,
        },
        digest,
    )))
}

fn copy_background_settlement_in(
    transaction: &Transaction<'_>,
    settlement: &DynamicMemoryBackgroundRoundSettlement,
    change_digest: &str,
    child_attempt_id: DynamicMemoryAttemptId,
) -> Result<(), DynamicMemoryRunRepositoryError> {
    transaction
        .execute(
            "INSERT INTO dynamic_memory_background_round_settlements
               (run_id,attempt_id,round_ordinal,space_id,expected_memory_revision,
                resulting_memory_revision,change_digest,settled_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                settlement.run_id.to_string(),
                child_attempt_id.to_string(),
                i64::from(settlement.round_ordinal),
                settlement.space_id.to_string(),
                sql_u64(settlement.expected_memory_revision.get())?,
                sql_u64(settlement.resulting_memory_revision.get())?,
                change_digest,
                settlement.settled_at.get(),
            ],
        )
        .map_err(storage)?;
    for (ordinal, result) in settlement.results.iter().enumerate() {
        transaction
            .execute(
                "INSERT INTO dynamic_memory_background_tool_results
                   (run_id,attempt_id,round_ordinal,call_id,ordinal,outcome_json,settled_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7)",
                params![
                    settlement.run_id.to_string(),
                    child_attempt_id.to_string(),
                    i64::from(settlement.round_ordinal),
                    result.execution_id.to_string(),
                    i64::try_from(ordinal).map_err(storage)?,
                    encode_versioned(&result.outcome, JSON_VERSION).map_err(storage)?,
                    settlement.settled_at.get(),
                ],
            )
            .map_err(storage)?;
    }
    Ok(())
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
             (run_id,attempt_id,ordinal,first_call_ordinal,call_count,request_context_json,parts_json,\
              provider_replay_artifact_id,provider_replay_retention,input_tokens,output_tokens,\
              finish_reason,provider_request_id,admitted_at,cached_input_tokens,reasoning_tokens,cache_write_tokens,web_search_requests,provider_reported_cost,kind,image_tokens,audio_tokens,total_tokens) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23)",
            params![
                round.run_id.to_string(),
                round.attempt_id.to_string(),
                i64::from(round.ordinal),
                i64::from(round.first_call_ordinal),
                i64::try_from(round.calls.len()).map_err(storage)?,
                encode_versioned(&round.request_context, JSON_VERSION).map_err(storage)?,
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
                round.usage.as_ref().and_then(|u| u.cached_input_tokens).map(sql_u64).transpose()?,
                round.usage.as_ref().and_then(|u| u.reasoning_tokens).map(sql_u64).transpose()?,
                round.usage.as_ref().and_then(|u| u.cache_write_tokens).map(sql_u64).transpose()?,
                round.usage.as_ref().and_then(|u| u.web_search_requests).map(sql_u64).transpose()?,
                round.usage.as_ref().and_then(|u| u.provider_reported_cost).map(lettuce_conversations::ProviderReportedCost::get),
                match round.kind {
                    DynamicMemoryRoundKind::Manager => "manager",
                    DynamicMemoryRoundKind::Repair => "repair",
                },
                round.usage.as_ref().and_then(|u| u.image_tokens).map(sql_u64).transpose()?,
                round.usage.as_ref().and_then(|u| u.audio_tokens).map(sql_u64).transpose()?,
                round.usage.as_ref().and_then(|u| u.total_tokens).map(sql_u64).transpose()?,
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

/// Writes a backed-up dynamic-memory run. Each source row is inserted while
/// its message briefly renders the source the run recorded, and attempts walk
/// `created -> processing -> terminal` up to their stored status; an attempt
/// still open at backup time stays open for its restored job to resume.
pub(crate) fn insert_restored_run_in(
    transaction: &Transaction<'_>,
    backup: &lettuce_transfer::BackupDynamicMemoryRun,
) -> Result<(), DynamicMemoryRunRepositoryError> {
    let run = &backup.run;
    transaction
        .execute(
            "INSERT INTO dynamic_memory_runs \
             (id,conversation_id,space_id,time_awareness_enabled,supersession_enabled,structured_fallback_format,summary_message_interval,summary_window_start,summary_window_end,starting_memory_json,profile_json,tool_request_json,created_at,branch_id) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
            params![
                run.id.to_string(),
                run.conversation_id.to_string(),
                run.space_id.to_string(),
                run.time_awareness_enabled,
                run.supersession_enabled,
                fallback_format_name(run.structured_fallback_format),
                i64::from(run.summary_window.message_interval),
                sql_u64(run.summary_window.start)?,
                sql_u64(run.summary_window.end)?,
                encode_versioned(&run.starting_memory, JSON_VERSION).map_err(storage)?,
                encode_versioned(&run.profile, JSON_VERSION).map_err(storage)?,
                encode_versioned(&run.tool_request, JSON_VERSION).map_err(storage)?,
                run.created_at.get(),
                run.branch_id.to_string(),
            ],
        )
        .map_err(storage)?;
    for (ordinal, source) in run.source_messages.iter().enumerate() {
        let (revision_id, candidate_id) = match source.render_source {
            MessageRenderSource::Revision(id) => (Some(id.to_string()), None),
            MessageRenderSource::Candidate(id) => (None, Some(id.to_string())),
        };
        let original: (Option<String>, Option<String>, String) = transaction
            .query_row(
                "SELECT active_revision_id, active_candidate_id, visibility FROM conversation_messages WHERE conversation_id = ?1 AND id = ?2",
                params![run.conversation_id.to_string(), source.message_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(storage)?;
        let swapped =
            original.0 != revision_id || original.1 != candidate_id || original.2 != "visible";
        let render = |active_revision: &Option<String>,
                      active_candidate: &Option<String>,
                      visibility: &str| {
            transaction
                .execute(
                    "UPDATE conversation_messages SET active_revision_id = ?1, active_candidate_id = ?2, visibility = ?3 WHERE conversation_id = ?4 AND id = ?5",
                    params![
                        active_revision,
                        active_candidate,
                        visibility,
                        run.conversation_id.to_string(),
                        source.message_id.to_string(),
                    ],
                )
                .map_err(storage)
        };
        if swapped {
            render(&revision_id, &candidate_id, "visible")?;
        }
        transaction
            .execute(
                "INSERT INTO dynamic_memory_run_source_messages \
                 (run_id,conversation_id,message_id,role,revision_id,candidate_id,effective_time,ordinal) \
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                params![
                    run.id.to_string(),
                    run.conversation_id.to_string(),
                    source.message_id.to_string(),
                    match source.role {
                        lettuce_conversations::MessageRole::User => "user",
                        lettuce_conversations::MessageRole::Assistant => "assistant",
                        _ => return Err(DynamicMemoryRunRepositoryError::Invalid),
                    },
                    revision_id,
                    candidate_id,
                    source.effective_time.get(),
                    i64::try_from(ordinal).map_err(storage)?,
                ],
            )
            .map_err(storage)?;
        if swapped {
            render(&original.0, &original.1, &original.2)?;
        }
    }
    for entry in &backup.attempts {
        let attempt = &entry.attempt;
        let exported = attempt.revision.get();
        let terminal = attempt.status.is_terminal();
        let direct_processing =
            attempt.started_at.is_some() && exported == if terminal { 2 } else { 1 };
        match (direct_processing, attempt.started_at) {
            (true, Some(started_at)) => {
                transaction
                    .execute(
                        "INSERT INTO dynamic_memory_run_attempts \
                         (run_id,id,ordinal,retry_parent_id,job_id,status,failure,revision,created_at,\
                          started_at,finished_at,updated_at) \
                         VALUES (?1,?2,?3,?4,?5,'processing',NULL,1,?6,?7,NULL,?7)",
                        params![
                            attempt.run_id.to_string(),
                            attempt.id.to_string(),
                            i64::from(attempt.ordinal),
                            attempt.retry_parent_id.map(|id| id.to_string()),
                            attempt.job_id.to_string(),
                            attempt.created_at.get(),
                            started_at.get(),
                        ],
                    )
                    .map_err(storage)?;
            }
            _ => {
                transaction
                    .execute(
                        "INSERT INTO dynamic_memory_run_attempts \
                         (run_id,id,ordinal,retry_parent_id,job_id,status,failure,revision,created_at,\
                          started_at,finished_at,updated_at) \
                         VALUES (?1,?2,?3,?4,?5,'created',NULL,1,?6,NULL,NULL,?6)",
                        params![
                            attempt.run_id.to_string(),
                            attempt.id.to_string(),
                            i64::from(attempt.ordinal),
                            attempt.retry_parent_id.map(|id| id.to_string()),
                            attempt.job_id.to_string(),
                            attempt.created_at.get(),
                        ],
                    )
                    .map_err(storage)?;
            }
        }
        let mut revision = 1_u64;
        if let (false, Some(started_at)) = (direct_processing, attempt.started_at) {
            revision += 1;
            transaction
                .execute(
                    "UPDATE dynamic_memory_run_attempts SET status = 'processing', revision = ?1, started_at = ?2, updated_at = ?2 WHERE id = ?3",
                    params![sql_u64(revision)?, started_at.get(), attempt.id.to_string()],
                )
                .map_err(storage)?;
        }
        for round in &entry.rounds {
            insert_round_in(transaction, &round.round)?;
            if let (Some(settlement), Some(digest)) =
                (&round.settlement, &round.settlement_change_digest)
            {
                copy_background_settlement_in(transaction, settlement, digest, attempt.id)?;
            }
        }
        if terminal {
            revision += 1;
            transaction
                .execute(
                    "UPDATE dynamic_memory_run_attempts SET status = ?1, failure = ?2, revision = ?3, finished_at = ?4, updated_at = ?5 WHERE id = ?6",
                    params![
                        status_name(attempt.status),
                        attempt.failure.map(failure_name),
                        sql_u64(revision)?,
                        attempt.finished_at.map(TimestampMillis::get),
                        attempt.updated_at.get(),
                        attempt.id.to_string(),
                    ],
                )
                .map_err(storage)?;
        }
        if revision != exported {
            return Err(DynamicMemoryRunRepositoryError::Invalid);
        }
    }
    if let Some(checkpoint) = &backup.summary_checkpoint {
        let usage = checkpoint.usage.as_ref();
        transaction
            .execute(
                "INSERT INTO dynamic_memory_summary_checkpoints (
                    run_id,attempt_id,space_id,expected_memory_revision,
                    resulting_memory_revision,summary_text,token_count,
                    request_context_json,input_tokens,output_tokens,
                    provider_request_id,settled_at,cached_input_tokens,reasoning_tokens,cache_write_tokens,web_search_requests,provider_reported_cost,image_tokens,audio_tokens,total_tokens
                 ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20)",
                params![
                    checkpoint.run_id.to_string(),
                    checkpoint.attempt_id.to_string(),
                    checkpoint.summary.space_id.to_string(),
                    sql_u64(checkpoint.expected_memory_revision.get())?,
                    sql_u64(checkpoint.resulting_memory_revision.get())?,
                    checkpoint.summary.text,
                    i64::from(checkpoint.summary.token_count),
                    encode_versioned(&checkpoint.request_context, JSON_VERSION).map_err(storage)?,
                    usage.map(|usage| sql_u64(usage.input_tokens)).transpose()?,
                    usage.map(|usage| sql_u64(usage.output_tokens)).transpose()?,
                    checkpoint.provider_request_id,
                    checkpoint.settled_at.get(),
                    usage.and_then(|usage| usage.cached_input_tokens).map(sql_u64).transpose()?,
                    usage.and_then(|usage| usage.reasoning_tokens).map(sql_u64).transpose()?,
                    usage.and_then(|usage| usage.cache_write_tokens).map(sql_u64).transpose()?,
                    usage.and_then(|usage| usage.web_search_requests).map(sql_u64).transpose()?,
                    usage
                        .and_then(|usage| usage.provider_reported_cost)
                        .map(lettuce_conversations::ProviderReportedCost::get),
                    usage.and_then(|usage| usage.image_tokens).map(sql_u64).transpose()?,
                    usage.and_then(|usage| usage.audio_tokens).map(sql_u64).transpose()?,
                    usage.and_then(|usage| usage.total_tokens).map(sql_u64).transpose()?,
                ],
            )
            .map_err(storage)?;
    }
    Ok(())
}

impl DynamicMemoryRunRepository for Database {
    fn list_dynamic_memory_runs(
        &self,
        conversation_id: lettuce_types::ConversationId,
    ) -> Result<Vec<DynamicMemoryRun>, DynamicMemoryRunRepositoryError> {
        let connection = self.connection().map_err(storage)?;
        let ids = {
            let mut statement = connection
                .prepare(
                    "SELECT id FROM dynamic_memory_runs
                     WHERE conversation_id=?1
                     ORDER BY summary_window_start, summary_window_end, created_at, id",
                )
                .map_err(storage)?;
            statement
                .query_map(params![conversation_id.to_string()], |row| {
                    parse_id(row.get(0)?)
                })
                .map_err(storage)?
                .collect::<rusqlite::Result<Vec<DynamicMemoryRunId>>>()
                .map_err(storage)?
        };
        ids.into_iter()
            .map(|id| load_run_in(&connection, id))
            .collect()
    }

    fn admit_dynamic_memory_run_attempt(
        &self,
        mut input: NewDynamicMemoryRunAttempt,
    ) -> Result<DynamicMemoryRunAttemptAdmission, DynamicMemoryRunRepositoryError> {
        input
            .validate_cycle_start()
            .map_err(|_| DynamicMemoryRunRepositoryError::Invalid)?;
        let cycle_start_change = input.cycle_start_change.take();
        let requested_run = DynamicMemoryRun {
            id: input.run_id,
            conversation_id: input.conversation_id,
            branch_id: input.branch_id,
            space_id: input.space_id,
            starting_memory: input.starting_memory,
            source_messages: input.source_messages,
            profile: input.profile,
            time_awareness_enabled: input.time_awareness_enabled,
            supersession_enabled: input.supersession_enabled,
            structured_fallback_format: input.structured_fallback_format,
            summary_window: input.summary_window,
            tool_request: input.tool_request,
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
        let branch_active: bool = transaction.query_row("SELECT EXISTS(SELECT 1 FROM conversation_branches WHERE conversation_id = ?1 AND id = ?2 AND status = 'active')",params![input.conversation_id.to_string(),input.branch_id.to_string()],|row| row.get(0)).map_err(storage)?;
        if !branch_active {
            return Err(DynamicMemoryRunRepositoryError::Conflict);
        }
        if let Some(change) = &cycle_start_change {
            memory_adapter::compare_and_apply_in(&transaction, change).map_err(
                |error| match error {
                    MemoryRepositoryError::Conflict => DynamicMemoryRunRepositoryError::Conflict,
                    MemoryRepositoryError::NotFound => DynamicMemoryRunRepositoryError::NotFound,
                    MemoryRepositoryError::Invalid(_) => DynamicMemoryRunRepositoryError::Invalid,
                    MemoryRepositoryError::AlreadyExists | MemoryRepositoryError::Failure(_) => {
                        DynamicMemoryRunRepositoryError::Storage
                    }
                },
            )?;
        }
        if memory_adapter::get_in(&transaction, requested_run.space_id)
            .map_err(|_| DynamicMemoryRunRepositoryError::Storage)?
            .as_ref()
            != Some(&requested_run.starting_memory)
        {
            return Err(DynamicMemoryRunRepositoryError::Conflict);
        }
        transaction
            .execute(
                "INSERT INTO dynamic_memory_runs \
                 (id,conversation_id,space_id,time_awareness_enabled,supersession_enabled,structured_fallback_format,summary_message_interval,summary_window_start,summary_window_end,starting_memory_json,profile_json,tool_request_json,created_at,branch_id) \
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
                params![
                    requested_run.id.to_string(),
                    requested_run.conversation_id.to_string(),
                    requested_run.space_id.to_string(),
                    requested_run.time_awareness_enabled,
                    requested_run.supersession_enabled,
                    fallback_format_name(requested_run.structured_fallback_format),
                    i64::from(requested_run.summary_window.message_interval),
                    sql_u64(requested_run.summary_window.start)?,
                    sql_u64(requested_run.summary_window.end)?,
                    encode_versioned(&requested_run.starting_memory, JSON_VERSION)
                        .map_err(storage)?,
                    encode_versioned(&requested_run.profile, JSON_VERSION).map_err(storage)?,
                    encode_versioned(&requested_run.tool_request, JSON_VERSION).map_err(storage)?,
                    requested_run.created_at.get(),
                    requested_run.branch_id.to_string(),
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
                     (run_id,conversation_id,message_id,role,revision_id,candidate_id,effective_time,ordinal) \
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                    params![
                        requested_run.id.to_string(),
                        requested_run.conversation_id.to_string(),
                        source.message_id.to_string(),
                        match source.role {
                            lettuce_conversations::MessageRole::User => "user",
                            lettuce_conversations::MessageRole::Assistant => "assistant",
                            _ => return Err(DynamicMemoryRunRepositoryError::Conflict),
                        },
                        revision_id,
                        candidate_id,
                        source.effective_time.get(),
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
        if matches!(
            updated.status,
            DynamicMemoryAttemptStatus::Succeeded | DynamicMemoryAttemptStatus::Failed
        ) {
            publish_summary_checkpoint_in(&transaction, updated.run_id)?;
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
                let settlements_match = parent_rounds.iter().all(|round| {
                    let parent = load_background_settlement_in(
                        &transaction,
                        input.run_id,
                        parent.id,
                        round.ordinal,
                    );
                    let child_settlement = load_background_settlement_in(
                        &transaction,
                        input.run_id,
                        child.id,
                        round.ordinal,
                    );
                    match (parent, child_settlement) {
                        (Ok(None), Ok(None)) => true,
                        (
                            Ok(Some((parent, parent_digest))),
                            Ok(Some((child_settlement, child_digest))),
                        ) => {
                            let mut expected = parent;
                            expected.attempt_id = child.id;
                            expected == child_settlement && parent_digest == child_digest
                        }
                        _ => false,
                    }
                });
                if parent.status != DynamicMemoryAttemptStatus::Interrupted
                    || parent.finished_at != Some(input.now)
                    || child.run_id != input.run_id
                    || child.ordinal != expected_ordinal
                    || child.retry_parent_id != Some(parent.id)
                    || child.job_id != input.job_id
                    || child.status != DynamicMemoryAttemptStatus::Processing
                    || child.created_at != input.now
                    || list_rounds_in(&transaction, input.run_id, child.id)? != expected_rounds
                    || !settlements_match
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
            let settlement = load_background_settlement_in(
                &transaction,
                input.run_id,
                parent.id,
                round.ordinal,
            )?;
            round.attempt_id = input.child_attempt_id;
            for call in &mut round.calls {
                call.attempt_id = input.child_attempt_id;
            }
            insert_round_in(&transaction, &round)?;
            if let Some((settlement, digest)) = settlement {
                copy_background_settlement_in(
                    &transaction,
                    &settlement,
                    &digest,
                    input.child_attempt_id,
                )?;
            }
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
            kind: round.kind,
            first_call_ordinal: expected_next_call_ordinal,
            request_context: round.request_context,
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

    fn load_dynamic_memory_round_settlement(
        &self,
        run_id: DynamicMemoryRunId,
        attempt_id: DynamicMemoryAttemptId,
        round_ordinal: u8,
    ) -> Result<Option<DynamicMemoryBackgroundRoundSettlement>, DynamicMemoryRunRepositoryError>
    {
        let connection = self.connection().map_err(storage)?;
        let attempt = load_attempt_in(&connection, attempt_id)?;
        if attempt.run_id != run_id {
            return Err(DynamicMemoryRunRepositoryError::Conflict);
        }
        load_background_settlement_in(&connection, run_id, attempt_id, round_ordinal)
            .map(|stored| stored.map(|(settlement, _)| settlement))
    }

    fn commit_dynamic_memory_background_round(
        &self,
        commit: DynamicMemoryBackgroundRoundCommit,
        at: TimestampMillis,
    ) -> Result<DynamicMemoryBackgroundRoundSettlement, DynamicMemoryRunRepositoryError> {
        if commit.results.is_empty()
            || commit.change.as_ref().is_some_and(|change| {
                change.space_id != commit.space_id
                    || change.expected_revision != commit.expected_memory_revision
                    || change.validate().is_err()
            })
        {
            return Err(DynamicMemoryRunRepositoryError::Invalid);
        }
        let digest = background_change_digest(&commit.change)?;
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        if let Some((stored, stored_digest)) = load_background_settlement_in(
            &transaction,
            commit.run_id,
            commit.attempt_id,
            commit.round_ordinal,
        )? {
            if stored.space_id == commit.space_id
                && stored.expected_memory_revision == commit.expected_memory_revision
                && stored.results == commit.results
                && stored_digest == digest
            {
                transaction.commit().map_err(storage)?;
                return Ok(stored);
            }
            return Err(DynamicMemoryRunRepositoryError::Conflict);
        }
        let run = load_run_in(&transaction, commit.run_id)?;
        let attempt = load_attempt_in(&transaction, commit.attempt_id)?;
        let rounds = list_rounds_in(&transaction, commit.run_id, commit.attempt_id)?;
        let round = rounds
            .get(usize::from(commit.round_ordinal))
            .ok_or(DynamicMemoryRunRepositoryError::NotFound)?;
        if run.space_id != commit.space_id
            || attempt.run_id != run.id
            || attempt.status != DynamicMemoryAttemptStatus::Processing
            || round.ordinal != commit.round_ordinal
            || round.calls.len() != commit.results.len()
            || round
                .calls
                .iter()
                .zip(&commit.results)
                .any(|(call, result)| call.id != result.execution_id)
        {
            return Err(DynamicMemoryRunRepositoryError::Conflict);
        }
        let current = crate::memory::memory_adapter::get_in(&transaction, commit.space_id)
            .map_err(storage)?
            .ok_or(DynamicMemoryRunRepositoryError::NotFound)?;
        if current.revision != commit.expected_memory_revision {
            return Err(DynamicMemoryRunRepositoryError::Conflict);
        }
        let resulting = match &commit.change {
            Some(change) => {
                crate::memory::memory_adapter::compare_and_apply_in(&transaction, change)
                    .map_err(storage)?
            }
            None => current,
        };
        transaction
            .execute(
                "INSERT INTO dynamic_memory_background_round_settlements \
                 (run_id,attempt_id,round_ordinal,space_id,expected_memory_revision,\
                  resulting_memory_revision,change_digest,settled_at) \
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                params![
                    commit.run_id.to_string(),
                    commit.attempt_id.to_string(),
                    i64::from(commit.round_ordinal),
                    commit.space_id.to_string(),
                    sql_u64(commit.expected_memory_revision.get())?,
                    sql_u64(resulting.revision.get())?,
                    digest,
                    at.get(),
                ],
            )
            .map_err(storage)?;
        for (call, result) in round.calls.iter().zip(&commit.results) {
            transaction
                .execute(
                    "INSERT INTO dynamic_memory_background_tool_results \
                     (run_id,attempt_id,round_ordinal,call_id,ordinal,outcome_json,settled_at) \
                     VALUES (?1,?2,?3,?4,?5,?6,?7)",
                    params![
                        commit.run_id.to_string(),
                        commit.attempt_id.to_string(),
                        i64::from(commit.round_ordinal),
                        result.execution_id.to_string(),
                        i64::from(call.ordinal),
                        encode_versioned(&result.outcome, JSON_VERSION).map_err(storage)?,
                        at.get(),
                    ],
                )
                .map_err(storage)?;
        }
        let stored = load_background_settlement_in(
            &transaction,
            commit.run_id,
            commit.attempt_id,
            commit.round_ordinal,
        )?
        .map(|(settlement, _)| settlement)
        .ok_or(DynamicMemoryRunRepositoryError::Storage)?;
        transaction.commit().map_err(storage)?;
        Ok(stored)
    }

    fn load_dynamic_memory_summary_checkpoint(
        &self,
        run_id: DynamicMemoryRunId,
    ) -> Result<Option<DynamicMemorySummaryCheckpoint>, DynamicMemoryRunRepositoryError> {
        let connection = self.connection().map_err(storage)?;
        load_summary_checkpoint_in(&connection, run_id)
    }

    fn commit_dynamic_memory_summary(
        &self,
        commit: DynamicMemorySummaryCommit,
        at: TimestampMillis,
    ) -> Result<DynamicMemorySummaryCheckpoint, DynamicMemoryRunRepositoryError> {
        commit
            .request_context
            .validate()
            .map_err(|_| DynamicMemoryRunRepositoryError::Invalid)?;
        if commit
            .provider_request_id
            .as_ref()
            .is_some_and(|id| id.trim().is_empty() || id.len() > 256)
        {
            return Err(DynamicMemoryRunRepositoryError::Invalid);
        }
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        if let Some(stored) = load_summary_checkpoint_in(&transaction, commit.run_id)? {
            if stored.attempt_id == commit.attempt_id
                && stored.expected_memory_revision == commit.expected_memory_revision
                && stored.summary.text == commit.text
                && stored.summary.token_count == commit.token_count
                && stored.request_context == commit.request_context
                && stored.usage == commit.usage
                && stored.provider_request_id == commit.provider_request_id
                && stored.settled_at == at
            {
                transaction.commit().map_err(storage)?;
                return Ok(stored);
            }
            return Err(DynamicMemoryRunRepositoryError::Conflict);
        }
        let run = load_run_in(&transaction, commit.run_id)?;
        let attempt = load_attempt_in(&transaction, commit.attempt_id)?;
        if attempt.run_id != run.id
            || attempt.status != DynamicMemoryAttemptStatus::Processing
            || transaction
                .query_row(
                    "SELECT EXISTS(
                        SELECT 1 FROM dynamic_memory_inference_rounds WHERE run_id=?1
                    )",
                    [run.id.to_string()],
                    |row| row.get::<_, bool>(0),
                )
                .map_err(storage)?
        {
            return Err(DynamicMemoryRunRepositoryError::Conflict);
        }
        let summary = MemorySummary {
            space_id: run.space_id,
            branch_id: run.branch_id,
            text: commit.text,
            token_count: commit.token_count,
            window_start: run.summary_window.start,
            window_end: run.summary_window.end,
            source_message_ids: run
                .source_messages
                .iter()
                .map(|source| source.message_id)
                .collect(),
            updated_at: at,
        };
        summary
            .validate()
            .map_err(|_| DynamicMemoryRunRepositoryError::Invalid)?;
        let applied = memory_adapter::advance_revision_in(
            &transaction,
            run.space_id,
            commit.expected_memory_revision,
        )
        .map_err(|error| match error {
            lettuce_memory::MemoryRepositoryError::Conflict => {
                DynamicMemoryRunRepositoryError::Conflict
            }
            lettuce_memory::MemoryRepositoryError::NotFound => {
                DynamicMemoryRunRepositoryError::NotFound
            }
            _ => DynamicMemoryRunRepositoryError::Storage,
        })?;
        let (input_tokens, output_tokens) = commit
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
                "INSERT INTO dynamic_memory_summary_checkpoints (
                    run_id,attempt_id,space_id,expected_memory_revision,
                    resulting_memory_revision,summary_text,token_count,
                    request_context_json,input_tokens,output_tokens,
                    provider_request_id,settled_at,cached_input_tokens,reasoning_tokens,cache_write_tokens,web_search_requests,provider_reported_cost,image_tokens,audio_tokens,total_tokens
                 ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20)",
                params![
                    run.id.to_string(),
                    attempt.id.to_string(),
                    run.space_id.to_string(),
                    sql_u64(commit.expected_memory_revision.get())?,
                    sql_u64(applied.revision.get())?,
                    summary.text,
                    i64::from(summary.token_count),
                    encode_versioned(&commit.request_context, JSON_VERSION).map_err(storage)?,
                    input_tokens,
                    output_tokens,
                    commit.provider_request_id,
                    at.get(),
                    commit
                        .usage
                        .as_ref()
                        .and_then(|u| u.cached_input_tokens)
                        .map(sql_u64)
                        .transpose()?,
                    commit
                        .usage
                        .as_ref()
                        .and_then(|u| u.reasoning_tokens)
                        .map(sql_u64)
                        .transpose()?,
                    commit.usage.as_ref().and_then(|u| u.cache_write_tokens).map(sql_u64).transpose()?,
                    commit.usage.as_ref().and_then(|u| u.web_search_requests).map(sql_u64).transpose()?,
                    commit.usage.as_ref().and_then(|u| u.provider_reported_cost).map(lettuce_conversations::ProviderReportedCost::get),
                    commit.usage.as_ref().and_then(|u| u.image_tokens).map(sql_u64).transpose()?,
                    commit.usage.as_ref().and_then(|u| u.audio_tokens).map(sql_u64).transpose()?,
                    commit.usage.as_ref().and_then(|u| u.total_tokens).map(sql_u64).transpose()?,
                ],
            )
            .map_err(storage)?;
        let stored = load_summary_checkpoint_in(&transaction, run.id)?
            .ok_or(DynamicMemoryRunRepositoryError::Storage)?;
        transaction.commit().map_err(storage)?;
        Ok(stored)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use lettuce_companions::{
        CompanionTurnEffectRepository, CompanionTurnEffectSeed, CompanionTurnEffectStatus,
    };
    use lettuce_conversations::{
        InferenceUsage, MessagePart, MessageRenderSource, MessageRole, OutputPolicy,
        ProviderContextPart, ProviderNeutralContext, ProviderNeutralMessage,
        ResolvedInferenceProfile, SafetyContext, ToolPolicy,
    };
    use lettuce_memory::{
        DynamicMemoryApprovalRepository, DynamicMemoryAttemptFailureCode,
        DynamicMemoryAttemptStatus, DynamicMemoryBackgroundRoundCommit,
        DynamicMemoryRoundFinishReason, DynamicMemoryRoundKind, DynamicMemoryRunRepository,
        DynamicMemoryRunRepositoryError, DynamicMemorySourceMessage,
        DynamicMemoryStructuredFallbackFormat, DynamicMemorySuffixRewind,
        DynamicMemorySuffixRewindError, DynamicMemorySuffixRewindRepository,
        DynamicMemorySummaryCommit, MemoryCategory, MemoryChangeSet, MemoryItem, MemoryRepository,
        MemorySummaryRepository, MemoryToolOutcome, MemoryToolResult,
        NewDynamicMemoryAttemptRecovery, NewDynamicMemoryInferenceRound,
        NewDynamicMemoryRunAttempt, NewDynamicMemoryToolCall, Score,
    };
    use lettuce_models::{
        CapabilityStatus, ChatParameterResolutionInput, ChatRequirements, ExpectedModelIdentity,
        ModelCapabilities, ModelKind, ModelProfile, ModelProfileConfig, ProviderAccount,
        ProviderConfig, ProviderProtocol,
    };
    use lettuce_settings::SecretOwnerId;
    use lettuce_types::{
        CharacterId, ConversationBranchId, ConversationId, ConversationParticipantId,
        DynamicMemoryAttemptId, DynamicMemoryRunId, GenerationTurnId, JobId, MemoryId,
        MemorySpaceId, MessageId, MessageRevisionId, ModelProfileId, OperationId,
        ProviderAccountId, Revision, TimestampMillis, ToolExecutionId,
    };
    use rusqlite::{TransactionBehavior, params};
    use serde_json::json;

    use crate::Database;

    pub(crate) fn profile() -> ResolvedInferenceProfile {
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
                llama_cpp: Default::default(),
                stable_diffusion: Default::default(),
                feature_parameters: Default::default(),
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

    #[test]
    fn branch_memory_bindings_keep_sibling_spaces_independent() {
        let database = Database::open_in_memory().expect("database");
        let (conversation_id, root_space_id, sources) = conversation_fixture(&database);
        let root_branch_id: String = database
            .connection()
            .expect("connection")
            .query_row(
                "SELECT active_branch_id FROM conversations WHERE id = ?1",
                [conversation_id.to_string()],
                |row| row.get(0),
            )
            .expect("root branch");
        let child_branch_id = ConversationBranchId::new();
        let child_space_id = MemorySpaceId::new();
        let mut connection = database.connection().expect("connection");
        let transaction = connection.transaction().expect("transaction");
        transaction
            .execute(
                "INSERT INTO conversation_branches (conversation_id,id,parent_branch_id,fork_message_id,status,revision,created_at,updated_at) VALUES (?1,?2,?3,?4,'active',1,2,2)",
                params![conversation_id.to_string(), child_branch_id.to_string(), root_branch_id, sources[0].message_id.to_string()],
            )
            .expect("child branch");
        transaction
            .execute(
                "INSERT INTO memory_spaces (id,revision) VALUES (?1,1)",
                [child_space_id.to_string()],
            )
            .expect("child space");
        assert!(transaction
            .execute(
                "INSERT INTO conversation_memory_spaces (conversation_id,space_id) VALUES (?1,?2)",
                params![conversation_id.to_string(), child_space_id.to_string()],
            )
            .is_err());
        transaction
            .execute(
                "INSERT INTO conversation_memory_spaces (conversation_id,branch_id,space_id) VALUES (?1,?2,?3)",
                params![conversation_id.to_string(), child_branch_id.to_string(), child_space_id.to_string()],
            )
            .expect("child binding");
        assert_eq!(
            transaction
                .query_row(
                    "SELECT space_id FROM conversation_memory_spaces WHERE conversation_id = ?1 AND branch_id = ?2 AND pooled = 0",
                    params![conversation_id.to_string(), root_branch_id],
                    |row| row.get::<_, String>(0),
                )
                .expect("root binding"),
            root_space_id.to_string()
        );
        assert!(transaction
            .execute(
                "INSERT INTO conversation_memory_spaces (conversation_id,branch_id,space_id) VALUES (?1,?2,?3)",
                params![conversation_id.to_string(), child_branch_id.to_string(), root_space_id.to_string()],
            )
            .is_err());
        transaction.commit().expect("commit bindings");
    }

    #[test]
    fn explicit_branch_memory_reads_do_not_follow_the_active_selection() {
        let database = Database::open_in_memory().expect("database");
        let (conversation_id, parent_space, messages) = conversation_fixture(&database);
        let parent_branch = fixture_branch(&database, conversation_id);
        let child_branch = ConversationBranchId::new();
        let mut connection = database.connection().expect("connection");
        let transaction = connection.transaction().expect("transaction");
        transaction.execute("INSERT INTO conversation_branches (conversation_id,id,parent_branch_id,fork_message_id,status,revision,created_at,updated_at) VALUES (?1,?2,?3,?4,'active',1,2,2)", params![conversation_id.to_string(),child_branch.to_string(),parent_branch.to_string(),messages[0].message_id.to_string()]).expect("branch");
        let child_space = super::memory_adapter::create_conversation_space_in(
            &transaction,
            conversation_id,
            child_branch,
        )
        .expect("child space");
        transaction
            .execute(
                "UPDATE conversations SET active_branch_id = ?2 WHERE id = ?1",
                params![conversation_id.to_string(), child_branch.to_string()],
            )
            .expect("select child");
        transaction.commit().expect("commit");
        drop(connection);
        assert_eq!(
            database
                .get_for_branch(conversation_id, parent_branch)
                .expect("parent")
                .expect("space")
                .id,
            parent_space
        );
        assert_eq!(
            database
                .get_for_branch(conversation_id, child_branch)
                .expect("child")
                .expect("space")
                .id,
            child_space
        );
        assert!(
            database
                .get_for_branch(conversation_id, ConversationBranchId::new())
                .expect("missing branch")
                .is_none()
        );
    }

    #[test]
    fn branch_memory_binding_rejects_a_branch_from_another_conversation() {
        let database = Database::open_in_memory().expect("database");
        let (conversation_id, _, _) = conversation_fixture(&database);
        let (other_conversation_id, _, _) = conversation_fixture(&database);
        let mut connection = database.connection().expect("connection");
        let transaction = connection.transaction().expect("transaction");
        let foreign_branch: String = transaction
            .query_row(
                "SELECT active_branch_id FROM conversations WHERE id = ?1",
                [other_conversation_id.to_string()],
                |row| row.get(0),
            )
            .expect("foreign branch");
        let space_id = MemorySpaceId::new();
        transaction
            .execute(
                "INSERT INTO memory_spaces (id,revision) VALUES (?1,1)",
                [space_id.to_string()],
            )
            .expect("space");
        transaction
            .execute(
                "INSERT INTO conversation_memory_spaces (conversation_id,branch_id,space_id) VALUES (?1,?2,?3)",
                params![conversation_id.to_string(), foreign_branch, space_id.to_string()],
            )
            .expect("deferred binding");
        assert!(transaction.commit().is_err());
        drop(connection);
        assert!(
            database
                .get(space_id)
                .expect("read rolled-back space")
                .is_none()
        );
    }

    fn fixture_branch(
        database: &Database,
        conversation_id: ConversationId,
    ) -> ConversationBranchId {
        let id: String = database
            .connection()
            .expect("connection")
            .query_row(
                "SELECT active_branch_id FROM conversations WHERE id = ?1",
                [conversation_id.to_string()],
                |row| row.get(0),
            )
            .expect("branch");
        id.parse().expect("branch id")
    }

    fn conversation_fixture(
        database: &Database,
    ) -> (
        ConversationId,
        MemorySpaceId,
        Vec<DynamicMemorySourceMessage>,
    ) {
        conversation_fixture_with_message_count(database, 2)
    }

    fn conversation_fixture_with_message_count(
        database: &Database,
        count: usize,
    ) -> (
        ConversationId,
        MemorySpaceId,
        Vec<DynamicMemorySourceMessage>,
    ) {
        let conversation_id = ConversationId::new();
        let branch_id = ConversationBranchId::new();
        let space_id = MemorySpaceId::new();
        let messages = (0..count).map(|_| MessageId::new()).collect::<Vec<_>>();
        let revisions = (0..count)
            .map(|_| MessageRevisionId::new())
            .collect::<Vec<_>>();
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
                        participants[offset % 2].to_string(),
                        if offset % 2 == 0 { "user" } else { "assistant" },
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
        if count > 2 {
            transaction
                .execute(
                    "UPDATE conversation_branches SET head_message_id = ?2 WHERE id = ?1",
                    params![
                        branch_id.to_string(),
                        messages.last().expect("head").to_string()
                    ],
                )
                .expect("head");
            transaction
                .execute(
                    "UPDATE conversations SET next_timeline_ordinal = ?2 WHERE id = ?1",
                    params![
                        conversation_id.to_string(),
                        i64::try_from(count + 1).expect("next ordinal")
                    ],
                )
                .expect("next ordinal");
        }
        transaction
            .execute(
                "INSERT INTO memory_spaces (id,revision) VALUES (?1,1)",
                [space_id.to_string()],
            )
            .expect("space");
        transaction
            .execute(
                "INSERT INTO conversation_memory_spaces (conversation_id,branch_id,space_id) VALUES (?1,?3,?2)",
                params![conversation_id.to_string(), space_id.to_string(), branch_id.to_string()],
            )
            .expect("binding");
        transaction.commit().expect("commit");
        (
            conversation_id,
            space_id,
            messages
                .into_iter()
                .zip(revisions)
                .enumerate()
                .map(
                    |(ordinal, (message_id, revision_id))| DynamicMemorySourceMessage {
                        message_id,
                        role: if ordinal % 2 == 0 {
                            lettuce_conversations::MessageRole::User
                        } else {
                            lettuce_conversations::MessageRole::Assistant
                        },
                        render_source: MessageRenderSource::Revision(revision_id),
                        effective_time: TimestampMillis::new(i64::from(ordinal as u16) + 1),
                    },
                )
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

    fn memory_item(id: MemoryId, text: &str, at: i64) -> MemoryItem {
        MemoryItem {
            id,
            short_id: lettuce_memory::MemoryShortId::derived(id),
            text: text.into(),
            category: MemoryCategory::Other,
            source_message_id: None,
            source_role: None,
            observed_at: None,
            observed_time_precision: None,
            superseded_by: None,
            superseded_at: None,
            supersedes: Vec::new(),
            token_count: 3,
            is_cold: false,
            is_pinned: false,
            importance: Score::FULL,
            persistence_importance: Score::FULL,
            prompt_importance: Score::FULL,
            volatility: Score::LEGACY_VOLATILITY,
            access_count: 0,
            created_at: TimestampMillis::new(at),
            last_accessed_at: TimestampMillis::new(at),
        }
    }

    #[test]
    fn copied_conversation_seed_respects_companion_memory_sharing() {
        for shared in [false, true] {
            let database = Database::open_in_memory().expect("database");
            let (source, source_space, source_messages) = conversation_fixture(&database);
            let (target, target_space, target_messages) = conversation_fixture(&database);
            let source_branch = fixture_branch(&database, source);
            let target_branch = fixture_branch(&database, target);
            database
                .compare_and_apply(lettuce_memory::MemoryChangeSet {
                    space_id: source_space,
                    expected_revision: Revision::INITIAL,
                    items: vec![memory_item(MemoryId::new(), "Source memory", 1)],
                })
                .expect("source memory");
            let character = lettuce_types::CharacterId::new();
            let pool = lettuce_types::MemorySpaceId::new();
            let config = lettuce_companions::CompanionSoulConfig {
                share_memory_across_chats: shared,
                ..Default::default()
            };
            let defaults = lettuce_characters::CharacterDefaults {
                interaction_mode: lettuce_characters::InteractionMode::Companion,
                companion_soul: Some(config),
                ..Default::default()
            };
            let ids = source_messages
                .iter()
                .zip(&target_messages)
                .map(|(source, target)| (source.message_id, target.message_id))
                .collect();
            let mut connection = database.connection().expect("connection");
            let transaction = connection.transaction().expect("transaction");
            transaction.execute("INSERT INTO characters (id,status,name,normalized_name,profile_json,provenance_json,defaults_json,interaction_mode,memory_policy,voice_autoplay,presentation_json,revision,created_at,updated_at) VALUES (?1,'active','Pool','pool','{}','{}',?2,'companion','manual',0,'{}',1,1,1)", params![character.to_string(), crate::encode_versioned(&defaults, 1).expect("defaults")]).expect("character");
            transaction
                .execute(
                    "INSERT INTO memory_spaces (id,revision) VALUES (?1,1)",
                    [pool.to_string()],
                )
                .expect("pool space");
            transaction
                .execute(
                    "INSERT INTO companion_memory_pools (character_id,space_id) VALUES (?1,?2)",
                    params![character.to_string(), pool.to_string()],
                )
                .expect("pool");
            transaction.execute("INSERT INTO conversation_memory_spaces (conversation_id,branch_id,space_id,pooled) VALUES (?1,?2,?3,1)", params![target.to_string(), target_branch.to_string(), pool.to_string()]).expect("pool binding");
            crate::memory::memory_branch_adapter::seed_new_conversation_space_in(
                &transaction,
                source,
                source_branch,
                (target, target_branch),
                Some(source_messages[1].message_id),
                &ids,
            )
            .expect("copy seed");
            transaction.commit().expect("commit");
            drop(connection);
            assert_eq!(
                database
                    .get(target_space)
                    .expect("target memory")
                    .expect("own space")
                    .items
                    .len(),
                usize::from(!shared)
            );
            assert!(
                database
                    .get(pool)
                    .expect("pool memory")
                    .expect("pool space")
                    .items
                    .is_empty()
            );
            assert_eq!(
                database
                    .get(source_space)
                    .expect("source memory")
                    .expect("source space")
                    .items
                    .len(),
                1
            );
        }
    }

    #[test]
    fn branch_memory_purge_leaves_the_companion_pool_untouched() {
        let database = Database::open_in_memory().expect("database");
        let (conversation, own, _) = conversation_fixture(&database);
        let branch = fixture_branch(&database, conversation);
        let character = CharacterId::new();
        let defaults = lettuce_characters::CharacterDefaults {
            companion_soul: Some(lettuce_companions::CompanionSoulConfig::default()),
            ..Default::default()
        };
        let pool = {
            let mut connection = database.connection().expect("connection");
            let transaction = connection.transaction().expect("transaction");
            transaction.execute("INSERT INTO characters (id,status,name,normalized_name,profile_json,provenance_json,defaults_json,interaction_mode,memory_policy,voice_autoplay,presentation_json,revision,created_at,updated_at) VALUES (?1,'active','Pool','pool','{}','{}',?2,'companion','manual',0,'{}',1,1,1)",params![character.to_string(),crate::encode_versioned(&defaults,1).expect("defaults")]).expect("character");
            let pool = crate::memory::memory_adapter::join_companion_pool_in(
                &transaction,
                conversation,
                branch,
                character,
            )
            .expect("pool");
            transaction.commit().expect("commit");
            pool
        };
        for space_id in [own, pool] {
            database
                .compare_and_apply(MemoryChangeSet {
                    space_id,
                    expected_revision: Revision::INITIAL,
                    items: vec![memory_item(MemoryId::new(), "Kept memory", 1)],
                })
                .expect("memory");
        }
        let before = database.get(pool).expect("pool").expect("pool space");
        let mut connection = database.connection().expect("connection");
        let transaction = connection.transaction().expect("transaction");
        crate::purge::purge_branch_memory_in(
            &transaction,
            conversation,
            branch,
            TimestampMillis::new(10),
        )
        .expect("purge own branch memory");
        transaction.commit().expect("commit");
        drop(connection);
        assert!(database.get(own).expect("own memory").is_none());
        assert_eq!(database.get(pool).expect("pool memory"), Some(before));
        assert_eq!(database.connection().expect("connection").query_row("SELECT count(*) FROM conversation_memory_spaces WHERE conversation_id = ?1 AND pooled = 1",[conversation.to_string()],|row| row.get::<_,i64>(0)).expect("pool binding"),1);
    }

    #[test]
    fn copied_summary_with_any_missing_source_is_discarded_and_cursor_restarts() {
        for kept in 0..2 {
            let database = Database::open_in_memory().expect("database");
            let (source, source_space, source_messages) = conversation_fixture(&database);
            let source_branch = fixture_branch(&database, source);
            let (target, target_space, target_messages) = conversation_fixture(&database);
            let target_branch = fixture_branch(&database, target);
            let attempt = checkpointed_window(
                &database,
                source,
                source_space,
                &source_messages,
                "Complete source summary",
                10,
                0,
            );
            finish(&database, &attempt, true, 12);
            let ids = source_messages
                .iter()
                .zip(&target_messages)
                .take(kept)
                .map(|(source, target)| (source.message_id, target.message_id))
                .collect();
            let mut connection = database.connection().expect("connection");
            let transaction = connection.transaction().expect("transaction");
            super::super::memory_branch_adapter::seed_new_conversation_space_in(
                &transaction,
                source,
                source_branch,
                (target, target_branch),
                Some(source_messages[1].message_id),
                &ids,
            )
            .expect("copy without incomplete summary");
            assert!(
                crate::memory::memory_adapter::get_summary_in(&transaction, target_space)
                    .expect("summary")
                    .is_none()
            );
            assert_eq!(
                crate::memory::memory_adapter::summary_cursor_in(
                    &transaction,
                    target_space,
                    target,
                    target_branch
                )
                .expect("cursor"),
                0
            );
            transaction.commit().expect("commit");
            drop(connection);
            assert!(
                database
                    .get_summary(source_space)
                    .expect("source summary")
                    .is_some()
            );
        }
    }

    #[test]
    fn copied_conversation_seed_remaps_items_and_summary_sources() {
        let database = Database::open_in_memory().expect("database");
        let (source, source_space, source_messages) =
            conversation_fixture_with_message_count(&database, 3);
        let source_branch = fixture_branch(&database, source);
        let (target, target_space, target_messages) = conversation_fixture(&database);
        let target_branch = fixture_branch(&database, target);
        let mut copied = memory_item(MemoryId::new(), "Copied source", 1);
        copied.source_message_id = Some(source_messages[0].message_id);
        copied.source_role = Some(source_messages[0].role);
        copied.observed_at = Some(source_messages[0].effective_time);
        copied.observed_time_precision = Some("turn".into());
        let mut uncopied = memory_item(MemoryId::new(), "Uncopied source", 2);
        uncopied.source_message_id = Some(source_messages[2].message_id);
        uncopied.source_role = Some(source_messages[2].role);
        uncopied.observed_at = Some(source_messages[2].effective_time);
        uncopied.observed_time_precision = Some("turn".into());
        let mut user_dated = memory_item(MemoryId::new(), "User dated", 3);
        user_dated.observed_at = Some(TimestampMillis::new(5));
        user_dated.observed_time_precision = Some("user".into());
        database
            .compare_and_apply(lettuce_memory::MemoryChangeSet {
                space_id: source_space,
                expected_revision: Revision::INITIAL,
                items: vec![copied.clone(), uncopied.clone(), user_dated.clone()],
            })
            .expect("source items");
        let attempt = checkpointed_window(
            &database,
            source,
            source_space,
            &source_messages[..2],
            "Source summary",
            10,
            0,
        );
        finish(&database, &attempt, true, 12);
        let ids = source_messages[..2]
            .iter()
            .zip(&target_messages)
            .map(|(source, target)| (source.message_id, target.message_id))
            .collect();
        let mut connection = database.connection().expect("connection");
        let transaction = connection.transaction().expect("transaction");
        super::super::memory_branch_adapter::seed_new_conversation_space_in(
            &transaction,
            source,
            source_branch,
            (target, target_branch),
            Some(source_messages[1].message_id),
            &ids,
        )
        .expect("copy memory");
        transaction.commit().expect("commit");
        drop(connection);
        let seed = database
            .get(target_space)
            .expect("memory")
            .expect("target space");
        let copied_seed = seed
            .items
            .iter()
            .find(|item| item.text == copied.text)
            .expect("copied item");
        assert_ne!(copied_seed.id, copied.id);
        assert_eq!(
            copied_seed.source_message_id,
            Some(target_messages[0].message_id)
        );
        let uncopied_seed = seed
            .items
            .iter()
            .find(|item| item.text == uncopied.text)
            .expect("uncopied item");
        assert_eq!(uncopied_seed.source_message_id, None);
        assert_eq!(uncopied_seed.source_role, None);
        assert_eq!(uncopied_seed.observed_at, None);
        assert_eq!(uncopied_seed.observed_time_precision, None);
        let user_dated_seed = seed
            .items
            .iter()
            .find(|item| item.text == user_dated.text)
            .expect("user-dated item");
        assert_eq!(user_dated_seed.observed_at, user_dated.observed_at);
        assert_eq!(
            user_dated_seed.observed_time_precision,
            user_dated.observed_time_precision
        );
        let summary = database
            .get_summary(target_space)
            .expect("summary")
            .expect("copied summary");
        assert_eq!(summary.text, "Source summary");
        assert_eq!(
            summary.source_message_ids,
            target_messages
                .iter()
                .map(|message| message.message_id)
                .collect::<Vec<_>>()
        );
        assert_eq!(summary.branch_id, target_branch);
    }

    #[test]
    fn user_observed_time_round_trips_through_the_sync_item_codec() {
        let source = Database::open_in_memory().expect("source database");
        let (conversation_id, space_id, _) = conversation_fixture(&source);
        let branch_id = fixture_branch(&source, conversation_id);
        let mut memory = memory_item(MemoryId::new(), "User anniversary", 1);
        memory.observed_at = Some(TimestampMillis::new(5));
        memory.observed_time_precision = Some("user".into());
        source
            .compare_and_apply(lettuce_memory::MemoryChangeSet {
                space_id,
                expected_revision: Revision::INITIAL,
                items: vec![memory.clone()],
            })
            .expect("source memory");
        let mut connection = source.connection().expect("source connection");
        let transaction = connection.transaction().expect("source transaction");
        let exchanged = crate::sync::memory_sync_adapter::sync_load_memory_item(
            &transaction,
            &format!(
                "conversation:{conversation_id}:branch:{branch_id}/{}",
                memory.id
            ),
        )
        .expect("encode memory")
        .expect("source item");
        let bytes = serde_json::to_vec(&exchanged).expect("sync payload");
        let decoded = serde_json::from_slice(&bytes).expect("decode sync payload");
        transaction.commit().expect("source commit");
        let target = Database::open_in_memory().expect("target database");
        let (target_conversation, target_space, _) = conversation_fixture(&target);
        let target_branch = fixture_branch(&target, target_conversation);
        let mut connection = target.connection().expect("target connection");
        let transaction = connection.transaction().expect("target transaction");
        assert!(
            crate::sync::memory_sync_adapter::sync_put_memory_item(
                &transaction,
                &format!(
                    "conversation:{target_conversation}:branch:{target_branch}/{}",
                    memory.id
                ),
                &decoded,
            )
            .expect("materialize sync memory")
        );
        transaction.commit().expect("target commit");
        drop(connection);
        let received = target
            .get(target_space)
            .expect("read memory")
            .expect("space");
        assert_eq!(received.items, vec![memory]);
    }

    #[test]
    fn branch_seed_uses_unsettled_snapshot_and_settled_current_state() {
        for succeeded in [false, true] {
            let database = Database::open_in_memory().expect("database");
            let (conversation_id, space_id, messages) = conversation_fixture(&database);
            let parent_branch = fixture_branch(&database, conversation_id);
            let original_id = MemoryId::new();
            database
                .compare_and_apply(lettuce_memory::MemoryChangeSet {
                    space_id,
                    expected_revision: Revision::INITIAL,
                    items: vec![memory_item(original_id, "Starting state", 1)],
                })
                .expect("initial memory");
            let attempt = checkpointed_run(
                &database,
                conversation_id,
                space_id,
                &messages,
                "Summary",
                10,
            );
            let current = database.get(space_id).expect("memory").expect("space");
            database
                .compare_and_apply(lettuce_memory::MemoryChangeSet {
                    space_id,
                    expected_revision: current.revision,
                    items: vec![memory_item(MemoryId::new(), "Changed during rounds", 12)],
                })
                .expect("round changes");
            if succeeded {
                finish(&database, &attempt, true, 14);
            }
            let child_branch = ConversationBranchId::new();
            let mut connection = database.connection().expect("connection");
            let transaction = connection.transaction().expect("transaction");
            transaction.execute(
                "INSERT INTO conversation_branches (conversation_id,id,parent_branch_id,fork_message_id,status,revision,created_at,updated_at) VALUES (?1,?2,?3,?4,'active',1,20,20)",
                params![conversation_id.to_string(), child_branch.to_string(), parent_branch.to_string(), messages[1].message_id.to_string()],
            ).expect("branch");
            super::super::memory_branch_adapter::seed_branch_space_in(
                &transaction,
                conversation_id,
                parent_branch,
                child_branch,
                messages[1].message_id,
                false,
            )
            .expect("seed");
            let child_space: String = transaction.query_row(
                "SELECT space_id FROM conversation_memory_spaces WHERE conversation_id = ?1 AND branch_id = ?2 AND pooled = 0",
                params![conversation_id.to_string(),child_branch.to_string()], |row| row.get(0),
            ).expect("child space");
            let child_space = child_space.parse().expect("space id");
            let child = super::memory_adapter::get_in(&transaction, child_space)
                .expect("child memory")
                .expect("space");
            assert_eq!(
                child.items[0].text,
                if succeeded {
                    "Changed during rounds"
                } else {
                    "Starting state"
                }
            );
            assert_ne!(child.items[0].id, original_id);
            transaction.commit().expect("commit");
            drop(connection);
            let summary = database.get_summary(child_space).expect("summary");
            assert_eq!(summary.is_some(), succeeded);
            if let Some(summary) = summary {
                assert_eq!(summary.window_end, 2);
                assert_eq!(
                    summary.source_message_ids,
                    messages
                        .iter()
                        .map(|source| source.message_id)
                        .collect::<Vec<_>>()
                );
            }
        }
    }

    #[test]
    fn fork_inside_second_of_three_cycles_uses_its_start_and_prior_checkpoint() {
        let database = Database::open_in_memory().expect("database");
        let (conversation_id, parent_space, messages) =
            conversation_fixture_with_message_count(&database, 6);
        let parent_branch = fixture_branch(&database, conversation_id);
        for cycle in 0..3 {
            let attempt = checkpointed_window(
                &database,
                conversation_id,
                parent_space,
                &messages[cycle * 2..cycle * 2 + 2],
                &format!("Summary {}", cycle + 1),
                10 + i64::try_from(cycle).expect("cycle") * 10,
                u64::try_from(cycle * 2).expect("start"),
            );
            let current = database.get(parent_space).expect("memory").expect("space");
            database
                .compare_and_apply(lettuce_memory::MemoryChangeSet {
                    space_id: parent_space,
                    expected_revision: current.revision,
                    items: vec![memory_item(
                        MemoryId::new(),
                        &format!("After cycle {}", cycle + 1),
                        1,
                    )],
                })
                .expect("cycle state");
            finish(
                &database,
                &attempt,
                true,
                12 + i64::try_from(cycle).expect("cycle") * 10,
            );
        }
        for (point, expected_text, expected_cursor) in
            [(2, "After cycle 1", 2), (5, "After cycle 3", 6)]
        {
            let child_branch = ConversationBranchId::new();
            let mut connection = database.connection().expect("connection");
            let transaction = connection.transaction().expect("transaction");
            transaction.execute("INSERT INTO conversation_branches (conversation_id,id,parent_branch_id,fork_message_id,status,revision,created_at,updated_at) VALUES (?1,?2,?3,?4,'active',1,50,50)", params![conversation_id.to_string(),child_branch.to_string(),parent_branch.to_string(),messages[point].message_id.to_string()]).expect("child branch");
            super::super::memory_branch_adapter::seed_branch_space_in(
                &transaction,
                conversation_id,
                parent_branch,
                child_branch,
                messages[point].message_id,
                false,
            )
            .expect("seed");
            let space: String = transaction.query_row("SELECT space_id FROM conversation_memory_spaces WHERE conversation_id = ?1 AND branch_id = ?2 AND pooled = 0",params![conversation_id.to_string(),child_branch.to_string()], |row| row.get(0)).expect("space");
            let child_space = space.parse().expect("space id");
            transaction.commit().expect("commit");
            drop(connection);
            let child = database
                .get(child_space)
                .expect("child memory")
                .expect("space");
            assert_eq!(child.items[0].text, expected_text);
            let summary = database
                .get_summary(child_space)
                .expect("summary")
                .expect("checkpoint summary");
            assert_eq!(summary.window_end, expected_cursor);
            assert_eq!(
                summary.source_message_ids,
                messages[usize::try_from(expected_cursor - 2).expect("start")
                    ..usize::try_from(expected_cursor).expect("end")]
                    .iter()
                    .map(|source| source.message_id)
                    .collect::<Vec<_>>()
            );
            database
                .compare_and_apply(lettuce_memory::MemoryChangeSet {
                    space_id: child_space,
                    expected_revision: child.revision,
                    items: vec![memory_item(
                        MemoryId::new(),
                        "Child changed independently",
                        60,
                    )],
                })
                .expect("child change");
            assert_eq!(
                database
                    .get(parent_space)
                    .expect("parent")
                    .expect("space")
                    .items[0]
                    .text,
                "After cycle 3"
            );
        }
    }

    #[test]
    fn branch_seed_copies_imported_items_and_matching_projections_with_new_ids() {
        let database = Database::open_in_memory().expect("database");
        let (conversation_id, space_id, messages) = conversation_fixture(&database);
        let parent_branch = fixture_branch(&database, conversation_id);
        let original_id = MemoryId::new();
        let mut item = memory_item(original_id, "Imported memory", 1);
        item.source_message_id = Some(messages[0].message_id);
        database
            .compare_and_apply(lettuce_memory::MemoryChangeSet {
                space_id,
                expected_revision: Revision::INITIAL,
                items: vec![item],
            })
            .expect("imported memory");
        let child_branch = ConversationBranchId::new();
        let mut connection = database.connection().expect("connection");
        let transaction = connection.transaction().expect("transaction");
        transaction.execute(
            "INSERT INTO memory_embedding_projections (space_id,memory_id,source_revision,dimensions,source_text,status,vector,updated_at) VALUES (?1,?2,'model',64,'Imported memory','ready',?3,1)",
            params![space_id.to_string(),original_id.to_string(),vec![0_u8;256]],
        ).expect("projection");
        transaction.execute(
            "INSERT INTO conversation_branches (conversation_id,id,parent_branch_id,fork_message_id,status,revision,created_at,updated_at) VALUES (?1,?2,?3,?4,'active',1,20,20)",
            params![conversation_id.to_string(),child_branch.to_string(),parent_branch.to_string(),messages[0].message_id.to_string()],
        ).expect("branch");
        super::super::memory_branch_adapter::seed_branch_space_in(
            &transaction,
            conversation_id,
            parent_branch,
            child_branch,
            messages[0].message_id,
            true,
        )
        .expect("seed");
        let child_id = MemoryId::from_uuid(uuid::Uuid::new_v5(
            &child_branch.as_uuid(),
            original_id.as_uuid().as_bytes(),
        ));
        let projection: (String, Vec<u8>) = transaction
            .query_row(
                "SELECT source_text,vector FROM memory_embedding_projections WHERE memory_id = ?1",
                [child_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("copied projection");
        assert_eq!(projection, ("Imported memory".to_owned(), vec![0_u8; 256]));
        let source: String = transaction
            .query_row(
                "SELECT source_message_id FROM memory_items WHERE id = ?1",
                [child_id.to_string()],
                |row| row.get(0),
            )
            .expect("source");
        assert_eq!(source, messages[0].message_id.to_string());
    }

    fn seed_child(
        database: &Database,
        conversation_id: ConversationId,
        parent: ConversationBranchId,
        fork_message: MessageId,
        at: i64,
    ) -> (ConversationBranchId, MemorySpaceId) {
        let child = ConversationBranchId::new();
        let mut connection = database.connection().expect("connection");
        let transaction = connection.transaction().expect("transaction");
        transaction.execute(
            "INSERT INTO conversation_branches (conversation_id,id,parent_branch_id,fork_message_id,status,revision,created_at,updated_at) VALUES (?1,?2,?3,?4,'active',1,?5,?5)",
            params![conversation_id.to_string(), child.to_string(), parent.to_string(), fork_message.to_string(), at],
        ).expect("branch");
        super::super::memory_branch_adapter::seed_branch_space_in(
            &transaction,
            conversation_id,
            parent,
            child,
            fork_message,
            false,
        )
        .expect("seed");
        let space: String = transaction.query_row(
            "SELECT space_id FROM conversation_memory_spaces WHERE conversation_id = ?1 AND branch_id = ?2 AND pooled = 0",
            params![conversation_id.to_string(), child.to_string()], |row| row.get(0),
        ).expect("child space");
        transaction.commit().expect("commit");
        (child, space.parse().expect("space id"))
    }

    fn set_items(database: &Database, space_id: MemorySpaceId, text: &str) {
        let current = database.get(space_id).expect("memory").expect("space");
        database
            .compare_and_apply(lettuce_memory::MemoryChangeSet {
                space_id,
                expected_revision: current.revision,
                items: vec![memory_item(MemoryId::new(), text, 1)],
            })
            .expect("items");
    }

    fn branch_message(
        database: &Database,
        conversation_id: ConversationId,
        branch_id: ConversationBranchId,
        parent: MessageId,
        ordinal: i64,
    ) -> MessageId {
        branch_source(database, conversation_id, branch_id, parent, ordinal).message_id
    }

    fn branch_source(
        database: &Database,
        conversation_id: ConversationId,
        branch_id: ConversationBranchId,
        parent: MessageId,
        ordinal: i64,
    ) -> DynamicMemorySourceMessage {
        let message_id = MessageId::new();
        let revision_id = MessageRevisionId::new();
        let mut connection = database.connection().expect("connection");
        let connection = connection.transaction().expect("transaction");
        let participant: String = connection
            .query_row(
                "SELECT id FROM conversation_participants WHERE conversation_id = ?1 AND role = 'user'",
                [conversation_id.to_string()],
                |row| row.get(0),
            )
            .expect("user participant");
        connection
            .execute(
                "INSERT INTO conversation_messages \
             (conversation_id,id,branch_id,parent_message_id,author_participant_id,role,\
              timeline_ordinal,logical_time,effective_time,visibility,pinned,scene_edited,\
              active_revision_id,active_candidate_id,revision,created_at,updated_at) \
             VALUES (?1,?2,?3,?4,?5,'user',?6,?6,?6,'visible',0,0,?7,NULL,1,?6,?6)",
                params![
                    conversation_id.to_string(),
                    message_id.to_string(),
                    branch_id.to_string(),
                    parent.to_string(),
                    participant,
                    ordinal,
                    revision_id.to_string()
                ],
            )
            .expect("branch message");
        connection.execute(
            "INSERT INTO conversation_message_revisions \
             (conversation_id,id,message_id,branch_id,sequence,parts_json,authored_at,\
              source_turn_id,provider_replay_artifact_id,provider_replay_retention) \
             VALUES (?1,?2,?3,?4,1,?5,?6,NULL,NULL,NULL)",
            params![conversation_id.to_string(), revision_id.to_string(), message_id.to_string(), branch_id.to_string(),
                json!({"format_version":1,"value":[{"kind":"text","details":{"text":"branch message"}}]}).to_string(), ordinal],
        ).expect("branch revision");
        connection.commit().expect("commit");
        DynamicMemorySourceMessage {
            message_id,
            role: lettuce_conversations::MessageRole::User,
            render_source: MessageRenderSource::Revision(revision_id),
            effective_time: TimestampMillis::new(ordinal),
        }
    }

    #[test]
    fn grandchild_seed_ignores_runs_the_parent_never_inherited() {
        let database = Database::open_in_memory().expect("database");
        let (conversation_id, root_space, messages) =
            conversation_fixture_with_message_count(&database, 6);
        let root = fixture_branch(&database, conversation_id);
        let first = checkpointed_window(
            &database,
            conversation_id,
            root_space,
            &messages[0..2],
            "First summary",
            10,
            0,
        );
        set_items(&database, root_space, "After cycle 1");
        finish(&database, &first, true, 12);
        let (child, child_space) =
            seed_child(&database, conversation_id, root, messages[3].message_id, 30);
        let second = checkpointed_window(
            &database,
            conversation_id,
            root_space,
            &messages[2..4],
            "Second summary",
            40,
            2,
        );
        set_items(&database, root_space, "After cycle 2");
        finish(&database, &second, true, 42);
        let child_message = branch_message(
            &database,
            conversation_id,
            child,
            messages[3].message_id,
            45,
        );
        let (_, grandchild_space) =
            seed_child(&database, conversation_id, child, child_message, 50);
        let grandchild = database
            .get(grandchild_space)
            .expect("memory")
            .expect("space");
        assert_eq!(grandchild.items[0].text, "After cycle 1");
        let summary = database
            .get_summary(grandchild_space)
            .expect("summary")
            .expect("inherited summary");
        assert_eq!(summary.text, "First summary");
        assert_eq!(summary.window_end, 2);
        assert_eq!(
            database
                .get_summary(child_space)
                .expect("summary")
                .expect("child")
                .text,
            "First summary"
        );
    }

    #[test]
    fn rewinding_a_branch_first_run_restores_the_inherited_summary() {
        let database = Database::open_in_memory().expect("database");
        let (conversation_id, root_space, messages) = conversation_fixture(&database);
        let root = fixture_branch(&database, conversation_id);
        let first = checkpointed_run(
            &database,
            conversation_id,
            root_space,
            &messages,
            "Root summary",
            10,
        );
        set_items(&database, root_space, "Root memory");
        finish(&database, &first, true, 12);
        let (child, child_space) =
            seed_child(&database, conversation_id, root, messages[1].message_id, 20);
        let first_source = branch_source(
            &database,
            conversation_id,
            child,
            messages[1].message_id,
            21,
        );
        let second_source = branch_source(
            &database,
            conversation_id,
            child,
            first_source.message_id,
            22,
        );
        database
            .connection()
            .expect("connection")
            .execute(
                "UPDATE conversations SET active_branch_id = ?2 WHERE id = ?1",
                params![conversation_id.to_string(), child.to_string()],
            )
            .expect("select child");
        let own = checkpointed_window(
            &database,
            conversation_id,
            child_space,
            &[first_source, second_source],
            "Child summary",
            30,
            2,
        );
        finish(&database, &own, true, 32);
        let memory = database.get(child_space).expect("memory").expect("space");
        database
            .rewind_dynamic_memory_suffix(DynamicMemorySuffixRewind {
                operation_id: OperationId::new(),
                conversation_id,
                invalid_run_id: Some(own.run_id),
                expected_memory_revision: memory.revision,
                invalidated_effect_ids: Vec::new(),
                at: TimestampMillis::new(40),
            })
            .expect("rewind");
        let summary = database
            .get_summary(child_space)
            .expect("summary")
            .expect("inherited summary");
        assert_eq!(summary.text, "Root summary");
        assert_eq!(summary.window_end, 2);
        assert_eq!(
            database
                .summary_cursor(child_space, conversation_id, child)
                .expect("cursor"),
            2
        );
    }

    #[test]
    fn a_pooled_fork_continues_at_its_parent_cursor() {
        let database = Database::open_in_memory().expect("database");
        let (conversation_id, space_id, messages) =
            conversation_fixture_with_message_count(&database, 4);
        let root = fixture_branch(&database, conversation_id);
        let child = ConversationBranchId::new();
        {
            let mut connection = database.connection().expect("connection");
            let transaction = connection.transaction().expect("transaction");
            let character = lettuce_types::CharacterId::new();
            transaction.execute("INSERT INTO characters (id,status,name,normalized_name,profile_json,provenance_json,defaults_json,interaction_mode,memory_policy,voice_autoplay,presentation_json,revision,created_at,updated_at) VALUES (?1,'active','Pool','pool','{}','{}','{}','companion','dynamic',0,'{}',1,1,1)", [character.to_string()]).expect("pool character");
            transaction
                .execute(
                    "DELETE FROM conversation_memory_spaces WHERE conversation_id = ?1",
                    [conversation_id.to_string()],
                )
                .expect("replace fixture binding");
            transaction
                .execute(
                    "INSERT INTO companion_memory_pools (character_id,space_id) VALUES (?1,?2)",
                    params![character.to_string(), space_id.to_string()],
                )
                .expect("pool");
            transaction.execute("INSERT INTO conversation_memory_spaces (conversation_id,branch_id,space_id,pooled) VALUES (?1,?2,?3,1)", params![conversation_id.to_string(),root.to_string(),space_id.to_string()]).expect("pool binding");
            transaction.commit().expect("pool fixture");
        }
        let first = checkpointed_window(
            &database,
            conversation_id,
            space_id,
            &messages[0..2],
            "Pool summary",
            10,
            0,
        );
        finish(&database, &first, true, 12);
        database.connection().expect("connection").execute(
            "INSERT INTO conversation_branches (conversation_id,id,parent_branch_id,fork_message_id,status,revision,created_at,updated_at) VALUES (?1,?2,?3,?4,'active',1,20,20)",
            params![conversation_id.to_string(), child.to_string(), root.to_string(), messages[3].message_id.to_string()],
        ).expect("child branch");
        assert_eq!(
            database
                .summary_cursor(space_id, conversation_id, root)
                .expect("root cursor"),
            2
        );
        assert_eq!(
            database
                .summary_cursor(space_id, conversation_id, child)
                .expect("child cursor"),
            2
        );
    }

    #[test]
    fn branch_seed_keeps_an_imported_summary_without_runs() {
        let database = Database::open_in_memory().expect("database");
        let (conversation_id, space_id, messages) = conversation_fixture(&database);
        let parent = fixture_branch(&database, conversation_id);
        set_items(&database, space_id, "Imported memory");
        {
            let mut connection = database.connection().expect("connection");
            let transaction = connection.transaction().expect("transaction");
            super::memory_adapter::replace_summary_in(
                &transaction,
                space_id,
                Some(&lettuce_memory::MemorySummary {
                    space_id,
                    branch_id: parent,
                    text: "Imported summary".into(),
                    token_count: 3,
                    window_start: 0,
                    window_end: 2,
                    source_message_ids: messages.iter().map(|source| source.message_id).collect(),
                    updated_at: TimestampMillis::new(5),
                }),
            )
            .expect("imported summary");
            transaction.commit().expect("commit");
        }
        let (_, child_space) = seed_child(
            &database,
            conversation_id,
            parent,
            messages[1].message_id,
            20,
        );
        let summary = database
            .get_summary(child_space)
            .expect("summary")
            .expect("copied summary");
        assert_eq!(summary.text, "Imported summary");
        assert_eq!(summary.window_end, 2);
        assert_eq!(summary.source_message_ids.len(), 2);
    }

    #[test]
    fn an_interrupted_run_counts_as_settled_for_later_forks() {
        let database = Database::open_in_memory().expect("database");
        let (conversation_id, space_id, messages) = conversation_fixture(&database);
        let parent = fixture_branch(&database, conversation_id);
        set_items(&database, space_id, "Before the run");
        let attempt = checkpointed_run(
            &database,
            conversation_id,
            space_id,
            &messages,
            "Summary",
            10,
        );
        database
            .transition_dynamic_memory_attempt(
                attempt.id,
                attempt.revision,
                DynamicMemoryAttemptStatus::Interrupted,
                None,
                TimestampMillis::new(11),
            )
            .expect("interrupted");
        set_items(&database, space_id, "Changed after the run");
        let (_, child_space) = seed_child(
            &database,
            conversation_id,
            parent,
            messages[1].message_id,
            20,
        );
        assert_eq!(
            database
                .get(child_space)
                .expect("memory")
                .expect("space")
                .items[0]
                .text,
            "Changed after the run"
        );
    }

    #[test]
    fn branch_approval_schema_keeps_sibling_baselines_separate() {
        let database = Database::open_in_memory().expect("database");
        let (conversation_id, _, messages) = conversation_fixture(&database);
        let root = fixture_branch(&database, conversation_id);
        let child = ConversationBranchId::new();
        let connection = database.connection().expect("connection");
        connection.execute("INSERT INTO conversation_branches (conversation_id,id,parent_branch_id,fork_message_id,status,revision,created_at,updated_at) VALUES (?1,?2,?3,?4,'active',1,20,20)", params![conversation_id.to_string(),child.to_string(),root.to_string(),messages[0].message_id.to_string()]).expect("child branch");
        for (branch, baseline) in [(root, 4), (child, 2)] {
            connection.execute("INSERT INTO dynamic_memory_pending_approvals (conversation_id,branch_id,prompted_message_count,pending,skipped,updated_at) VALUES (?1,?2,?3,1,0,20)",params![conversation_id.to_string(),branch.to_string(),baseline]).expect("branch approval");
        }
        let approvals: i64 = connection
            .query_row(
                "SELECT count(*) FROM dynamic_memory_pending_approvals WHERE conversation_id = ?1",
                [conversation_id.to_string()],
                |row| row.get(0),
            )
            .expect("approval count");
        assert_eq!(approvals, 2);
    }

    #[test]
    fn memory_item_delete_stamp_uses_the_owning_branch() {
        let database = Database::open_in_memory().expect("database");
        let (conversation_id, space_id, _) = conversation_fixture(&database);
        let branch_id = fixture_branch(&database, conversation_id);
        let item = memory_item(MemoryId::new(), "deleted branch memory", 10);
        let inserted = database
            .compare_and_apply(MemoryChangeSet {
                space_id,
                expected_revision: Revision::INITIAL,
                items: vec![item.clone()],
            })
            .expect("insert item");
        database
            .compare_and_apply(MemoryChangeSet {
                space_id,
                expected_revision: inserted.revision,
                items: Vec::new(),
            })
            .expect("delete item");
        let owner = format!(
            "conversation:{conversation_id}:branch:{branch_id}/{}",
            item.id
        );
        let stamped: bool = database.connection().expect("connection").query_row(
            "SELECT EXISTS(SELECT 1 FROM sync_deleted_entities WHERE entity_kind = 'memory_item' AND entity_id = ?1)",
            [owner], |row| row.get(0),
        ).expect("delete stamp");
        assert!(stamped);
    }

    #[test]
    fn synced_cursor_schema_keeps_sibling_windows_separate() {
        let database = Database::open_in_memory().expect("database");
        let (conversation_id, _, messages) = conversation_fixture(&database);
        let root = fixture_branch(&database, conversation_id);
        let child = ConversationBranchId::new();
        let connection = database.connection().expect("connection");
        connection.execute("INSERT INTO conversation_branches (conversation_id,id,parent_branch_id,fork_message_id,status,revision,created_at,updated_at) VALUES (?1,?2,?3,?4,'active',1,20,20)", params![conversation_id.to_string(),child.to_string(),root.to_string(),messages[0].message_id.to_string()]).expect("child branch");
        for (branch, cursor) in [(root, 4), (child, 2)] {
            connection.execute("INSERT INTO memory_synced_cursors (conversation_id,branch_id,window_end) VALUES (?1,?2,?3)", params![conversation_id.to_string(),branch.to_string(),cursor]).expect("branch cursor");
        }
        let cursors: i64 = connection
            .query_row(
                "SELECT count(*) FROM memory_synced_cursors WHERE conversation_id = ?1",
                [conversation_id.to_string()],
                |row| row.get(0),
            )
            .expect("cursor count");
        assert_eq!(cursors, 2);
    }

    #[test]
    fn ask_first_prompt_baseline_survives_restart() {
        let path = std::env::temp_dir().join(format!(
            "lettuce-memory-approval-{}.sqlite3",
            DynamicMemoryRunId::new()
        ));
        let database = Database::open(&path).expect("database");
        let (conversation_id, _, messages) = conversation_fixture(&database);
        let branch_id = fixture_branch(&database, conversation_id);
        let first = database
            .prompt_dynamic_memory_if_due(
                conversation_id,
                branch_id,
                4,
                3,
                TimestampMillis::new(10),
            )
            .expect("first prompt")
            .expect("approval");
        assert_eq!(first.prompted_message_count, 4);
        assert!(
            database
                .prompt_dynamic_memory_if_due(
                    conversation_id,
                    branch_id,
                    4,
                    3,
                    TimestampMillis::new(99),
                )
                .expect("exact replay")
                .is_none()
        );
        let child_branch = ConversationBranchId::new();
        database.connection().expect("connection").execute(
            "INSERT INTO conversation_branches (conversation_id,id,parent_branch_id,fork_message_id,status,revision,created_at,updated_at) VALUES (?1,?2,?3,?4,'active',1,20,20)",
            params![conversation_id.to_string(), child_branch.to_string(), branch_id.to_string(), messages[0].message_id.to_string()],
        ).expect("child branch");
        let child_approval = database
            .prompt_dynamic_memory_if_due(
                conversation_id,
                child_branch,
                2,
                1,
                TimestampMillis::new(12),
            )
            .expect("child prompt")
            .expect("child approval");
        database
            .connection()
            .expect("connection")
            .execute(
                "UPDATE conversations SET active_branch_id = ?2 WHERE id = ?1",
                params![conversation_id.to_string(), child_branch.to_string()],
            )
            .expect("select child");
        drop(database);

        let reopened = Database::open(&path).expect("reopen");
        assert_eq!(
            reopened
                .get_dynamic_memory_pending_approval(conversation_id, branch_id)
                .expect("stored approval"),
            Some(first.clone())
        );
        let skipped = reopened
            .skip_dynamic_memory_pending_approval(
                conversation_id,
                branch_id,
                TimestampMillis::new(20),
            )
            .expect("skip")
            .expect("skipped approval");
        assert!(!skipped.pending);
        assert!(skipped.skipped);
        assert_eq!(skipped.prompted_message_count, first.prompted_message_count);
        assert!(
            reopened
                .prompt_dynamic_memory_if_due(
                    conversation_id,
                    branch_id,
                    6,
                    3,
                    TimestampMillis::new(100),
                )
                .expect("below next interval")
                .is_none()
        );
        let next = reopened
            .prompt_dynamic_memory_if_due(
                conversation_id,
                branch_id,
                7,
                3,
                TimestampMillis::new(101),
            )
            .expect("next prompt")
            .expect("next approval");
        assert_eq!(next.prompted_message_count, 7);
        assert!(next.pending);
        assert!(next.skipped);
        reopened
            .clear_dynamic_memory_pending_approval(conversation_id, branch_id)
            .expect("clear");
        assert_eq!(
            reopened
                .get_dynamic_memory_pending_approval(conversation_id, branch_id)
                .expect("cleared"),
            None
        );
        assert_eq!(
            reopened
                .get_dynamic_memory_pending_approval(conversation_id, child_branch)
                .expect("child approval after parent clear"),
            Some(child_approval)
        );
        drop(reopened);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn summary_checkpoint_applies_once_and_replays_exactly() {
        let database = Database::open_in_memory().expect("database");
        let (conversation_id, space_id, messages) = conversation_fixture(&database);
        let run_id = DynamicMemoryRunId::new();
        let attempt_id = DynamicMemoryAttemptId::new();
        let admitted = database
            .admit_dynamic_memory_run_attempt(NewDynamicMemoryRunAttempt {
                run_id,
                attempt_id,
                conversation_id,
                branch_id: fixture_branch(&database, conversation_id),
                space_id,
                starting_memory: database.get(space_id).expect("memory").expect("space"),
                cycle_start_change: None,
                source_messages: messages.clone(),
                profile: profile(),
                time_awareness_enabled: true,
                supersession_enabled: true,
                structured_fallback_format: DynamicMemoryStructuredFallbackFormat::Xml,
                summary_window: lettuce_memory::DynamicMemorySummaryWindow {
                    message_interval: 2,
                    start: 0,
                    end: 2,
                },
                tool_request: lettuce_memory::dynamic_memory_tool_request_for_run(
                    lettuce_memory::DynamicMemoryToolOptions {
                        group: false,
                        supersession_enabled: true,
                        require_source_message_id: true,
                    },
                    &|key| key.to_owned(),
                ),
                job_id: JobId::new(),
                now: TimestampMillis::new(10),
            })
            .expect("run");
        let stored_branch: String = database
            .connection()
            .expect("connection")
            .query_row(
                "SELECT branch_id FROM dynamic_memory_runs WHERE id = ?1",
                [run_id.to_string()],
                |row| row.get(0),
            )
            .expect("run branch");
        let own_branch: String = database
            .connection()
            .expect("connection")
            .query_row(
                "SELECT branch_id FROM conversation_memory_spaces WHERE space_id = ?1",
                [space_id.to_string()],
                |row| row.get(0),
            )
            .expect("own branch");
        assert_eq!(stored_branch, own_branch);
        database
            .transition_dynamic_memory_attempt(
                attempt_id,
                admitted.attempt.revision,
                DynamicMemoryAttemptStatus::Processing,
                None,
                TimestampMillis::new(11),
            )
            .expect("processing");
        let commit = DynamicMemorySummaryCommit {
            run_id,
            attempt_id,
            expected_memory_revision: Revision::INITIAL,
            text: "The user prefers tea.".into(),
            token_count: 5,
            request_context: ProviderNeutralContext {
                messages: vec![ProviderNeutralMessage {
                    role: MessageRole::User,
                    parts: vec![ProviderContextPart::Text {
                        text: "frozen summary request".into(),
                    }],
                }],
                attributions: Default::default(),
                budget: Default::default(),
            },
            usage: Some(InferenceUsage {
                image_tokens: Some(2),
                audio_tokens: Some(0),
                total_tokens: Some(40),
                provider_reported_cost: lettuce_conversations::ProviderReportedCost::new(0.0125),
                cache_write_tokens: Some(3),
                web_search_requests: Some(0),
                cached_input_tokens: Some(0),
                reasoning_tokens: Some(1),
                input_tokens: 20,
                output_tokens: 5,
            }),
            provider_request_id: Some("summary-request".into()),
        };
        let checkpoint = database
            .commit_dynamic_memory_summary(commit.clone(), TimestampMillis::new(12))
            .expect("checkpoint");
        assert_eq!(checkpoint.resulting_memory_revision, Revision::new(2));
        assert_eq!(
            checkpoint.summary.source_message_ids,
            messages
                .iter()
                .map(|source| source.message_id)
                .collect::<Vec<_>>()
        );
        assert_eq!(database.get_summary(space_id).expect("summary"), None);
        assert_eq!(
            database
                .summary_cursor(
                    space_id,
                    conversation_id,
                    fixture_branch(&database, conversation_id)
                )
                .expect("cursor before tools"),
            0
        );
        assert_eq!(
            database
                .commit_dynamic_memory_summary(commit.clone(), TimestampMillis::new(12))
                .expect("exact replay"),
            checkpoint
        );
        let mut changed = commit;
        changed.text = "Different summary.".into();
        assert_eq!(
            database.commit_dynamic_memory_summary(changed, TimestampMillis::new(12)),
            Err(DynamicMemoryRunRepositoryError::Conflict)
        );
        assert_eq!(
            database
                .get(space_id)
                .expect("memory")
                .expect("space")
                .revision,
            Revision::new(2)
        );
    }

    fn checkpointed_run(
        database: &Database,
        conversation_id: ConversationId,
        space_id: MemorySpaceId,
        messages: &[DynamicMemorySourceMessage],
        text: &str,
        at: i64,
    ) -> lettuce_memory::DynamicMemoryAttempt {
        checkpointed_window(database, conversation_id, space_id, messages, text, at, 0)
    }

    fn memory_job(database: &Database, conversation_id: ConversationId) -> JobId {
        lettuce_jobs::JobStore::create_or_get(
            database,
            lettuce_jobs::JobSpec::new(
                lettuce_jobs::JobKind::MemoryExtraction,
                lettuce_jobs::JobSubject::new(
                    lettuce_jobs::SubjectKind::Conversation,
                    conversation_id.to_string(),
                )
                .expect("subject"),
                lettuce_jobs::OutcomeRef::Conversation(conversation_id),
            )
            .with_resources(vec![lettuce_jobs::ResourceClass::Cpu]),
        )
        .expect("memory job")
        .job
        .id
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn checkpointed_window(
        database: &Database,
        conversation_id: ConversationId,
        space_id: MemorySpaceId,
        messages: &[DynamicMemorySourceMessage],
        text: &str,
        at: i64,
        start: u64,
    ) -> lettuce_memory::DynamicMemoryAttempt {
        let run_id = DynamicMemoryRunId::new();
        let attempt_id = DynamicMemoryAttemptId::new();
        let admitted = database
            .admit_dynamic_memory_run_attempt(NewDynamicMemoryRunAttempt {
                run_id,
                attempt_id,
                conversation_id,
                branch_id: fixture_branch(database, conversation_id),
                space_id,
                starting_memory: database.get(space_id).expect("memory").expect("space"),
                cycle_start_change: None,
                source_messages: messages.to_vec(),
                profile: profile(),
                time_awareness_enabled: false,
                supersession_enabled: false,
                structured_fallback_format: DynamicMemoryStructuredFallbackFormat::Xml,
                summary_window: lettuce_memory::DynamicMemorySummaryWindow {
                    message_interval: 2,
                    start,
                    end: start + u64::try_from(messages.len()).expect("source count"),
                },
                tool_request: lettuce_memory::dynamic_memory_tool_request_for_run(
                    lettuce_memory::DynamicMemoryToolOptions {
                        group: false,
                        supersession_enabled: false,
                        require_source_message_id: false,
                    },
                    &|key| key.to_owned(),
                ),
                job_id: memory_job(database, conversation_id),
                now: TimestampMillis::new(at),
            })
            .expect("run");
        let processing = database
            .transition_dynamic_memory_attempt(
                attempt_id,
                admitted.attempt.revision,
                DynamicMemoryAttemptStatus::Processing,
                None,
                TimestampMillis::new(at),
            )
            .expect("processing");
        let memory = database.get(space_id).expect("memory").expect("space");
        database
            .commit_dynamic_memory_summary(
                DynamicMemorySummaryCommit {
                    run_id,
                    attempt_id,
                    expected_memory_revision: memory.revision,
                    text: text.into(),
                    token_count: 5,
                    request_context: ProviderNeutralContext {
                        messages: vec![ProviderNeutralMessage {
                            role: MessageRole::User,
                            parts: vec![ProviderContextPart::Text {
                                text: "summary request".into(),
                            }],
                        }],
                        attributions: Default::default(),
                        budget: Default::default(),
                    },
                    usage: None,
                    provider_request_id: None,
                },
                TimestampMillis::new(at + 1),
            )
            .expect("checkpoint");
        processing
    }

    pub(crate) fn finish(
        database: &Database,
        attempt: &lettuce_memory::DynamicMemoryAttempt,
        succeeded: bool,
        at: i64,
    ) {
        database
            .transition_dynamic_memory_attempt(
                attempt.id,
                attempt.revision,
                if succeeded {
                    DynamicMemoryAttemptStatus::Succeeded
                } else {
                    DynamicMemoryAttemptStatus::Failed
                },
                (!succeeded).then_some(DynamicMemoryAttemptFailureCode::ProviderUnavailable),
                TimestampMillis::new(at),
            )
            .expect("finish");
    }

    #[test]
    fn a_failed_tools_phase_keeps_the_summary_but_not_the_cursor() {
        let database = Database::open_in_memory().expect("database");
        let (conversation_id, space_id, messages) = conversation_fixture(&database);
        let failed = checkpointed_run(
            &database,
            conversation_id,
            space_id,
            &messages,
            "The user prefers tea.",
            10,
        );
        finish(&database, &failed, false, 13);
        assert_eq!(
            database
                .get_summary(space_id)
                .expect("summary")
                .expect("summary after failed tools")
                .text,
            "The user prefers tea."
        );
        assert_eq!(
            database
                .summary_cursor(
                    space_id,
                    conversation_id,
                    fixture_branch(&database, conversation_id)
                )
                .expect("cursor after failed tools"),
            0
        );
        let retried = checkpointed_run(
            &database,
            conversation_id,
            space_id,
            &messages,
            "The user prefers green tea.",
            20,
        );
        finish(&database, &retried, true, 23);
        assert_eq!(
            database
                .get_summary(space_id)
                .expect("summary")
                .expect("summary after success")
                .text,
            "The user prefers green tea."
        );
        assert_eq!(
            database
                .summary_cursor(
                    space_id,
                    conversation_id,
                    fixture_branch(&database, conversation_id)
                )
                .expect("cursor after success"),
            2
        );
    }

    #[test]
    fn an_older_run_finishing_last_keeps_the_newer_summary() {
        let database = Database::open_in_memory().expect("database");
        let (conversation_id, space_id, messages) = conversation_fixture(&database);
        let older = checkpointed_run(
            &database,
            conversation_id,
            space_id,
            &messages,
            "Older.",
            10,
        );
        let newer = checkpointed_run(
            &database,
            conversation_id,
            space_id,
            &messages,
            "Newer.",
            20,
        );
        finish(&database, &newer, true, 23);
        finish(&database, &older, true, 24);
        assert_eq!(
            database
                .get_summary(space_id)
                .expect("summary")
                .expect("summary")
                .text,
            "Newer."
        );
    }

    #[test]
    fn a_rewind_before_success_keeps_the_rewound_summary() {
        let database = Database::open_in_memory().expect("database");
        let (conversation_id, space_id, messages) = conversation_fixture(&database);
        let attempt = checkpointed_run(
            &database,
            conversation_id,
            space_id,
            &messages,
            "Stale.",
            10,
        );
        let memory = database.get(space_id).expect("memory").expect("space");
        database
            .rewind_dynamic_memory_suffix(DynamicMemorySuffixRewind {
                operation_id: OperationId::new(),
                conversation_id,
                invalid_run_id: Some(attempt.run_id),
                expected_memory_revision: memory.revision,
                invalidated_effect_ids: Vec::new(),
                at: TimestampMillis::new(12),
            })
            .expect("rewind");
        finish(&database, &attempt, true, 13);
        assert_eq!(database.get_summary(space_id).expect("summary"), None);
        assert_eq!(
            database
                .summary_cursor(
                    space_id,
                    conversation_id,
                    fixture_branch(&database, conversation_id)
                )
                .expect("cursor"),
            0
        );
    }

    #[test]
    fn run_admission_applies_the_cycle_start_change_atomically() {
        let database = Database::open_in_memory().expect("database");
        let (conversation_id, space_id, messages) = conversation_fixture(&database);
        let before = database.get(space_id).expect("memory").expect("space");
        let memory_id = MemoryId::new();
        let decayed = vec![MemoryItem {
            id: memory_id,
            short_id: lettuce_memory::MemoryShortId::derived(memory_id),
            text: "The user prefers tea".into(),
            category: MemoryCategory::Preference,
            source_message_id: None,
            source_role: None,
            observed_at: None,
            observed_time_precision: None,
            superseded_by: None,
            superseded_at: None,
            supersedes: Vec::new(),
            token_count: 4,
            is_cold: false,
            is_pinned: false,
            importance: Score::from_basis_points(4_200).expect("score"),
            persistence_importance: Score::FULL,
            prompt_importance: Score::FULL,
            volatility: Score::LEGACY_VOLATILITY,
            access_count: 0,
            created_at: TimestampMillis::new(5),
            last_accessed_at: TimestampMillis::new(5),
        }];
        let change = MemoryChangeSet {
            space_id,
            expected_revision: before.revision,
            items: decayed.clone(),
        };
        let starting_memory = lettuce_memory::MemorySpaceSnapshot {
            id: space_id,
            revision: before.revision.next().expect("revision"),
            items: decayed,
        };
        let admission =
            |run_id, attempt_id, change: Option<MemoryChangeSet>| NewDynamicMemoryRunAttempt {
                run_id,
                attempt_id,
                conversation_id,
                branch_id: fixture_branch(&database, conversation_id),
                space_id,
                starting_memory: starting_memory.clone(),
                cycle_start_change: change,
                source_messages: messages.clone(),
                profile: profile(),
                time_awareness_enabled: false,
                supersession_enabled: false,
                structured_fallback_format: DynamicMemoryStructuredFallbackFormat::Xml,
                summary_window: lettuce_memory::DynamicMemorySummaryWindow {
                    message_interval: 2,
                    start: 0,
                    end: 2,
                },
                tool_request: lettuce_memory::dynamic_memory_tool_request_for_run(
                    lettuce_memory::DynamicMemoryToolOptions {
                        group: false,
                        supersession_enabled: false,
                        require_source_message_id: false,
                    },
                    &|key| key.to_owned(),
                ),
                job_id: JobId::new(),
                now: TimestampMillis::new(10),
            };
        let mut stale = change.clone();
        stale.expected_revision = Revision::new(7);
        let stale_run = DynamicMemoryRunId::new();
        assert_eq!(
            database
                .admit_dynamic_memory_run_attempt(admission(
                    stale_run,
                    DynamicMemoryAttemptId::new(),
                    Some(stale),
                ))
                .err(),
            Some(DynamicMemoryRunRepositoryError::Invalid)
        );
        assert_eq!(
            database.get(space_id).expect("memory").expect("space"),
            before
        );
        let admitted = database
            .admit_dynamic_memory_run_attempt(admission(
                DynamicMemoryRunId::new(),
                DynamicMemoryAttemptId::new(),
                Some(change),
            ))
            .expect("admission with the cycle start change");
        let after = database.get(space_id).expect("memory").expect("space");
        assert_eq!(after, starting_memory);
        assert_eq!(admitted.run.starting_memory, starting_memory);
        assert_eq!(
            database.load_dynamic_memory_run(stale_run).err(),
            Some(DynamicMemoryRunRepositoryError::NotFound)
        );
    }

    #[test]
    fn suffix_rewind_restores_the_prior_run_boundary_once() {
        let database = Database::open_in_memory().expect("database");
        let (conversation_id, space_id, messages) = conversation_fixture(&database);
        let root_branch = fixture_branch(&database, conversation_id);

        let first_run_id = DynamicMemoryRunId::new();
        let first_attempt_id = DynamicMemoryAttemptId::new();
        let first = database
            .admit_dynamic_memory_run_attempt(NewDynamicMemoryRunAttempt {
                run_id: first_run_id,
                attempt_id: first_attempt_id,
                conversation_id,
                branch_id: root_branch,
                space_id,
                starting_memory: database.get(space_id).expect("memory").expect("space"),
                cycle_start_change: None,
                source_messages: messages.clone(),
                profile: profile(),
                time_awareness_enabled: true,
                supersession_enabled: true,
                structured_fallback_format: DynamicMemoryStructuredFallbackFormat::Xml,
                summary_window: lettuce_memory::DynamicMemorySummaryWindow {
                    message_interval: 2,
                    start: 0,
                    end: 2,
                },
                tool_request: lettuce_memory::dynamic_memory_tool_request_for_run(
                    lettuce_memory::DynamicMemoryToolOptions {
                        group: false,
                        supersession_enabled: true,
                        require_source_message_id: true,
                    },
                    &|key| key.to_owned(),
                ),
                job_id: JobId::new(),
                now: TimestampMillis::new(10),
            })
            .expect("first run");
        let first_processing = database
            .transition_dynamic_memory_attempt(
                first_attempt_id,
                first.attempt.revision,
                DynamicMemoryAttemptStatus::Processing,
                None,
                TimestampMillis::new(11),
            )
            .expect("first processing");
        let first_summary = database
            .commit_dynamic_memory_summary(
                DynamicMemorySummaryCommit {
                    run_id: first_run_id,
                    attempt_id: first_attempt_id,
                    expected_memory_revision: Revision::INITIAL,
                    text: "First summary".into(),
                    token_count: 2,
                    request_context: ProviderNeutralContext {
                        messages: Vec::new(),
                        attributions: Default::default(),
                        budget: Default::default(),
                    },
                    usage: None,
                    provider_request_id: None,
                },
                TimestampMillis::new(12),
            )
            .expect("first summary");
        database
            .transition_dynamic_memory_attempt(
                first_attempt_id,
                first_processing.revision,
                DynamicMemoryAttemptStatus::Succeeded,
                None,
                TimestampMillis::new(12),
            )
            .expect("first succeeded");

        let kept_item = memory_item(MemoryId::new(), "kept memory", 13);
        let before_second = database
            .compare_and_apply(MemoryChangeSet {
                space_id,
                expected_revision: first_summary.resulting_memory_revision,
                items: vec![kept_item.clone()],
            })
            .expect("kept memory");
        let second_run_id = DynamicMemoryRunId::new();
        let second_attempt_id = DynamicMemoryAttemptId::new();
        let second = database
            .admit_dynamic_memory_run_attempt(NewDynamicMemoryRunAttempt {
                run_id: second_run_id,
                attempt_id: second_attempt_id,
                conversation_id,
                branch_id: root_branch,
                space_id,
                starting_memory: before_second.clone(),
                cycle_start_change: None,
                source_messages: messages,
                profile: profile(),
                time_awareness_enabled: true,
                supersession_enabled: true,
                structured_fallback_format: DynamicMemoryStructuredFallbackFormat::Xml,
                summary_window: lettuce_memory::DynamicMemorySummaryWindow {
                    message_interval: 2,
                    start: 2,
                    end: 4,
                },
                tool_request: lettuce_memory::dynamic_memory_tool_request_for_run(
                    lettuce_memory::DynamicMemoryToolOptions {
                        group: false,
                        supersession_enabled: true,
                        require_source_message_id: true,
                    },
                    &|key| key.to_owned(),
                ),
                job_id: JobId::new(),
                now: TimestampMillis::new(14),
            })
            .expect("second run");
        database
            .transition_dynamic_memory_attempt(
                second_attempt_id,
                second.attempt.revision,
                DynamicMemoryAttemptStatus::Processing,
                None,
                TimestampMillis::new(15),
            )
            .expect("second processing");
        let edited_item = memory_item(MemoryId::new(), "edited outside the tools", 16);
        let after_effect = database
            .compare_and_apply(MemoryChangeSet {
                space_id,
                expected_revision: before_second.revision,
                items: vec![edited_item.clone()],
            })
            .expect("suffix memory");
        let second_summary = database
            .commit_dynamic_memory_summary(
                DynamicMemorySummaryCommit {
                    run_id: second_run_id,
                    attempt_id: second_attempt_id,
                    expected_memory_revision: after_effect.revision,
                    text: "Second summary".into(),
                    token_count: 2,
                    request_context: ProviderNeutralContext {
                        messages: Vec::new(),
                        attributions: Default::default(),
                        budget: Default::default(),
                    },
                    usage: None,
                    provider_request_id: None,
                },
                TimestampMillis::new(17),
            )
            .expect("second summary");

        let sibling_branch = ConversationBranchId::new();
        let sibling_space = {
            let mut connection = database.connection().expect("connection");
            let transaction = connection.transaction().expect("transaction");
            let anchor: String = transaction.query_row("SELECT id FROM conversation_messages WHERE conversation_id = ?1 ORDER BY logical_time LIMIT 1", [conversation_id.to_string()], |row| row.get(0)).expect("anchor");
            transaction.execute("INSERT INTO conversation_branches (conversation_id,id,parent_branch_id,fork_message_id,status,revision,created_at,updated_at) VALUES (?1,?2,?3,?4,'active',1,18,18)", params![conversation_id.to_string(),sibling_branch.to_string(),root_branch.to_string(),anchor]).expect("sibling branch");
            let space = crate::memory::memory_adapter::create_conversation_space_in(
                &transaction,
                conversation_id,
                sibling_branch,
            )
            .expect("sibling space");
            transaction
                .execute(
                    "UPDATE conversations SET active_branch_id = ?2 WHERE id = ?1",
                    params![conversation_id.to_string(), sibling_branch.to_string()],
                )
                .expect("select sibling");
            transaction.execute("INSERT INTO memory_synced_cursors (conversation_id,branch_id,window_end) VALUES (?1,?2,9)", params![conversation_id.to_string(),sibling_branch.to_string()]).expect("sibling cursor");
            transaction.commit().expect("commit sibling");
            space
        };
        let sibling_before = database
            .get(sibling_space)
            .expect("sibling memory")
            .expect("space");
        let rewind = DynamicMemorySuffixRewind {
            operation_id: OperationId::new(),
            conversation_id,
            invalid_run_id: Some(second_run_id),
            expected_memory_revision: second_summary.resulting_memory_revision,
            invalidated_effect_ids: Vec::new(),
            at: TimestampMillis::new(18),
        };
        let receipt = database
            .rewind_dynamic_memory_suffix(rewind.clone())
            .expect("rewind");
        assert_eq!(
            database
                .get_dynamic_memory_suffix_rewind(rewind.operation_id)
                .expect("receipt lookup"),
            Some(receipt.clone())
        );
        assert_eq!(
            receipt.memory.revision,
            second_summary.resulting_memory_revision
        );
        assert_eq!(receipt.memory.items, vec![edited_item]);
        assert_ne!(receipt.memory.items, vec![kept_item]);
        assert_eq!(receipt.summary, Some(first_summary.summary.clone()));
        assert_eq!(
            database.get_summary(space_id).expect("summary"),
            receipt.summary
        );
        assert_eq!(
            lettuce_memory::MemorySummaryRepository::summary_cursor(
                &database,
                space_id,
                conversation_id,
                root_branch,
            )
            .expect("cursor"),
            2
        );
        assert_eq!(
            database
                .rewind_dynamic_memory_suffix(rewind.clone())
                .expect("exact replay"),
            receipt
        );

        assert_eq!(
            database.get(sibling_space).expect("sibling memory"),
            Some(sibling_before)
        );
        let sibling_cursor: i64 = database.connection().expect("connection").query_row("SELECT window_end FROM memory_synced_cursors WHERE conversation_id = ?1 AND branch_id = ?2", params![conversation_id.to_string(),sibling_branch.to_string()], |row| row.get(0)).expect("sibling cursor retained");
        assert_eq!(sibling_cursor, 9);
        let mut changed = rewind;
        changed.at = TimestampMillis::new(19);
        assert_eq!(
            database.rewind_dynamic_memory_suffix(changed),
            Err(DynamicMemorySuffixRewindError::Conflict)
        );
        assert_eq!(
            database.get(space_id).expect("memory").expect("space"),
            receipt.memory
        );
    }

    #[test]
    fn suffix_rewind_invalidates_processing_effects_without_rewriting_them() {
        let database = Database::open_in_memory().expect("database");
        let (conversation_id, space_id, messages) = conversation_fixture(&database);
        let turn_id = GenerationTurnId::new();
        let target_message_id = MessageId::new();
        let branch_id = {
            let connection = database.connection().expect("connection");
            connection
                .query_row(
                    "SELECT active_branch_id FROM conversations WHERE id=?1",
                    [conversation_id.to_string()],
                    |row| row.get::<_, String>(0),
                )
                .expect("branch")
        };
        let mut connection = database.connection().expect("connection");
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("transaction");
        transaction
            .execute(
                "INSERT INTO conversation_turns
                    (conversation_id,id,branch_id,operation,input_kind,user_message_id,
                     idempotency_key,status,target_kind,target_message_id,target_parent_message_id,
                     swap_roles,revision,created_at,updated_at)
                 VALUES (?1,?2,?3,'send','user_message',?4,?5,'created','new_assistant',?6,?4,
                         0,1,20,20)",
                params![
                    conversation_id.to_string(),
                    turn_id.to_string(),
                    branch_id,
                    messages[0].message_id.to_string(),
                    format!("effect-{turn_id}"),
                    target_message_id.to_string(),
                ],
            )
            .expect("turn");
        crate::conversation::state_adapter::insert_effect_draft_in(
            &transaction,
            conversation_id,
            turn_id,
            Some(messages[0].message_id),
            &CompanionTurnEffectSeed::default(),
            TimestampMillis::new(20),
        )
        .expect("effect draft");
        crate::conversation::state_adapter::finalize_turn_effect_in(
            &transaction,
            conversation_id,
            turn_id,
            messages[1].message_id,
            TimestampMillis::new(21),
        )
        .expect("effect");
        transaction.commit().expect("commit");
        drop(connection);

        let effect = database
            .get_for_message(conversation_id, messages[1].message_id)
            .expect("effect")
            .expect("stored effect");
        assert_eq!(effect.status, CompanionTurnEffectStatus::Processing);
        assert_eq!(
            database
                .list_processing_for_conversation(conversation_id, 10)
                .expect("conversation pending effects"),
            std::slice::from_ref(&effect)
        );
        let rewind = database
            .rewind_dynamic_memory_suffix(DynamicMemorySuffixRewind {
                operation_id: OperationId::new(),
                conversation_id,
                invalid_run_id: None,
                expected_memory_revision: Revision::INITIAL,
                invalidated_effect_ids: vec![effect.id],
                at: TimestampMillis::new(22),
            })
            .expect("invalidate effect");
        assert_eq!(rewind.memory.revision, Revision::INITIAL);
        assert_eq!(rewind.memory.id, space_id);
        assert_eq!(rewind.invalidated_effect_ids, vec![effect.id]);
        assert_eq!(
            database
                .get_for_message(conversation_id, messages[1].message_id)
                .expect("effect")
                .expect("stored effect")
                .status,
            CompanionTurnEffectStatus::Invalidated
        );
        assert!(
            database
                .list_processing(10)
                .expect("pending effects")
                .is_empty()
        );
        assert!(
            database
                .list_processing_for_conversation(conversation_id, 10)
                .expect("conversation pending effects")
                .is_empty()
        );
    }

    #[test]
    fn background_round_memory_and_results_commit_once() {
        let database = Database::open_in_memory().expect("database");
        let (conversation_id, space_id, messages) = conversation_fixture(&database);
        let root_branch = fixture_branch(&database, conversation_id);
        let sibling_branch = ConversationBranchId::new();
        {
            let mut connection = database.connection().expect("connection");
            let transaction = connection.transaction().expect("transaction");
            let character = lettuce_types::CharacterId::new();
            transaction.execute("INSERT INTO characters (id,status,name,normalized_name,profile_json,provenance_json,defaults_json,interaction_mode,memory_policy,voice_autoplay,presentation_json,revision,created_at,updated_at) VALUES (?1,'active','Pool','pool','{}','{}','{}','companion','dynamic',0,'{}',1,1,1)", [character.to_string()]).expect("pool character");
            transaction
                .execute(
                    "DELETE FROM conversation_memory_spaces WHERE conversation_id = ?1",
                    [conversation_id.to_string()],
                )
                .expect("replace fixture binding");
            transaction
                .execute(
                    "INSERT INTO companion_memory_pools (character_id,space_id) VALUES (?1,?2)",
                    params![character.to_string(), space_id.to_string()],
                )
                .expect("pool");
            transaction.execute("INSERT INTO conversation_memory_spaces (conversation_id,branch_id,space_id,pooled) VALUES (?1,?2,?3,1)", params![conversation_id.to_string(),root_branch.to_string(),space_id.to_string()]).expect("pool binding");
            transaction.execute("INSERT INTO conversation_branches (conversation_id,id,parent_branch_id,fork_message_id,status,revision,created_at,updated_at) VALUES (?1,?2,?3,?4,'active',1,18,18)", params![conversation_id.to_string(),sibling_branch.to_string(),root_branch.to_string(),messages[0].message_id.to_string()]).expect("sibling");
            transaction.commit().expect("pool fixture");
        }
        let run_id = DynamicMemoryRunId::new();
        let attempt_id = DynamicMemoryAttemptId::new();
        let admitted = database
            .admit_dynamic_memory_run_attempt(NewDynamicMemoryRunAttempt {
                run_id,
                attempt_id,
                conversation_id,
                branch_id: fixture_branch(&database, conversation_id),
                space_id,
                starting_memory: database.get(space_id).expect("memory").expect("space"),
                cycle_start_change: None,
                source_messages: messages.clone(),
                profile: profile(),
                time_awareness_enabled: true,
                supersession_enabled: true,
                structured_fallback_format: DynamicMemoryStructuredFallbackFormat::Xml,
                summary_window: lettuce_memory::DynamicMemorySummaryWindow {
                    message_interval: 2,
                    start: 0,
                    end: u64::try_from(messages.len()).expect("messages"),
                },
                tool_request: lettuce_memory::dynamic_memory_tool_request_for_run(
                    lettuce_memory::DynamicMemoryToolOptions {
                        group: false,
                        supersession_enabled: true,
                        require_source_message_id: true,
                    },
                    &|key| key.to_owned(),
                ),
                job_id: JobId::new(),
                now: TimestampMillis::new(10),
            })
            .expect("run");
        database
            .transition_dynamic_memory_attempt(
                attempt_id,
                admitted.attempt.revision,
                DynamicMemoryAttemptStatus::Processing,
                None,
                TimestampMillis::new(11),
            )
            .expect("processing");
        let create_id = ToolExecutionId::new();
        let done_id = ToolExecutionId::new();
        let admitted_round = database
            .admit_dynamic_memory_inference_round(
                run_id,
                attempt_id,
                0,
                0,
                NewDynamicMemoryInferenceRound {
                    ordinal: 0,
                    request_context: lettuce_conversations::ProviderNeutralContext {
                        messages: vec![ProviderNeutralMessage {
                            role: MessageRole::User,
                            parts: vec![ProviderContextPart::Text {
                                text: "frozen first request".into(),
                            }],
                        }],
                        attributions: Default::default(),
                        budget: Default::default(),
                    },
                    parts: Vec::new(),
                    provider_replay: None,
                    usage: None,
                    finish_reason: DynamicMemoryRoundFinishReason::Stop,
                    kind: DynamicMemoryRoundKind::Manager,
                    provider_request_id: None,
                    calls: vec![
                        NewDynamicMemoryToolCall {
                            id: create_id,
                            definition_version: 1,
                            call: lettuce_conversations::ProposedToolCall {
                                provider_call_id: Some("create".into()),
                                name: "create_memory".into(),
                                arguments: json!({
                                    "text":"The user prefers tea",
                                    "category":"preference",
                                    "source_message_id":messages[0].message_id.to_string()
                                }),
                                raw_arguments: None,
                                provider_replay: None,
                            },
                        },
                        NewDynamicMemoryToolCall {
                            id: done_id,
                            definition_version: 1,
                            call: lettuce_conversations::ProposedToolCall {
                                provider_call_id: Some("done".into()),
                                name: "done".into(),
                                arguments: json!({"summary":"stored preference"}),
                                raw_arguments: None,
                                provider_replay: None,
                            },
                        },
                    ],
                    admitted_at: TimestampMillis::new(12),
                },
            )
            .expect("round");
        assert!(matches!(
            &admitted_round.request_context.messages[0].parts[..],
            [ProviderContextPart::Text { text }] if text == "frozen first request"
        ));
        let memory_id = MemoryId::new();
        let commit = DynamicMemoryBackgroundRoundCommit {
            run_id,
            attempt_id,
            round_ordinal: 0,
            space_id,
            expected_memory_revision: Revision::INITIAL,
            change: Some(MemoryChangeSet {
                space_id,
                expected_revision: Revision::INITIAL,
                items: vec![MemoryItem {
                    id: memory_id,
                    short_id: lettuce_memory::MemoryShortId::derived(memory_id),
                    text: "The user prefers tea".into(),
                    category: MemoryCategory::Preference,
                    source_message_id: Some(messages[0].message_id),
                    source_role: None,
                    observed_at: None,
                    observed_time_precision: None,
                    superseded_by: None,
                    superseded_at: None,
                    supersedes: Vec::new(),
                    token_count: 4,
                    is_cold: false,
                    is_pinned: false,
                    importance: Score::FULL,
                    persistence_importance: Score::FULL,
                    prompt_importance: Score::FULL,
                    volatility: Score::LEGACY_VOLATILITY,
                    access_count: 0,
                    created_at: TimestampMillis::new(12),
                    last_accessed_at: TimestampMillis::new(12),
                }],
            }),
            results: vec![
                MemoryToolResult {
                    execution_id: create_id,
                    outcome: MemoryToolOutcome::Created {
                        id: memory_id,
                        short_id: lettuce_memory::MemoryShortId::derived(memory_id),
                        memories: vec![lettuce_memory::ListedMemory {
                            short_id: lettuce_memory::MemoryShortId::derived(memory_id),
                            text: "The user prefers tea".into(),
                        }],
                    },
                },
                MemoryToolResult {
                    execution_id: done_id,
                    outcome: MemoryToolOutcome::Done {
                        summary: Some("stored preference".into()),
                    },
                },
            ],
        };
        let mut stale = commit.clone();
        stale.expected_memory_revision = Revision::new(2);
        stale.change.as_mut().expect("change").expected_revision = Revision::new(2);
        assert_eq!(
            database.commit_dynamic_memory_background_round(stale, TimestampMillis::new(13)),
            Err(DynamicMemoryRunRepositoryError::Conflict)
        );
        assert_eq!(
            database
                .get(space_id)
                .expect("memory")
                .expect("space")
                .revision,
            Revision::INITIAL
        );
        assert_eq!(
            database
                .load_dynamic_memory_round_settlement(run_id, attempt_id, 0)
                .expect("settlement"),
            None
        );
        let settled = database
            .commit_dynamic_memory_background_round(commit.clone(), TimestampMillis::new(13))
            .expect("settle");
        assert_eq!(settled.resulting_memory_revision, Revision::new(2));
        assert_eq!(settled.results, commit.results);
        assert_eq!(
            database
                .commit_dynamic_memory_background_round(commit, TimestampMillis::new(99))
                .expect("exact replay"),
            settled
        );
        let stored = database.get(space_id).expect("memory").expect("space");
        assert_eq!(stored.revision, Revision::new(2));
        assert_eq!(
            stored.items[0].source_message_id,
            Some(messages[0].message_id)
        );
        assert_eq!(visible_counts(&database, conversation_id), (2, 0));

        let other_chat_memory = memory_item(MemoryId::new(), "other chat memory", 20);
        let pooled = database
            .compare_and_apply(MemoryChangeSet {
                space_id,
                expected_revision: stored.revision,
                items: vec![stored.items[0].clone(), other_chat_memory.clone()],
            })
            .expect("other chat memory");
        let sibling_run_id = DynamicMemoryRunId::new();
        let sibling_attempt_id = DynamicMemoryAttemptId::new();
        let sibling_run = database
            .admit_dynamic_memory_run_attempt(NewDynamicMemoryRunAttempt {
                run_id: sibling_run_id,
                attempt_id: sibling_attempt_id,
                conversation_id,
                branch_id: sibling_branch,
                space_id,
                starting_memory: pooled.clone(),
                cycle_start_change: None,
                source_messages: messages.clone(),
                profile: profile(),
                time_awareness_enabled: true,
                supersession_enabled: true,
                structured_fallback_format: DynamicMemoryStructuredFallbackFormat::Xml,
                summary_window: lettuce_memory::DynamicMemorySummaryWindow {
                    message_interval: 2,
                    start: 0,
                    end: 2,
                },
                tool_request: lettuce_memory::dynamic_memory_tool_request_for_run(
                    lettuce_memory::DynamicMemoryToolOptions {
                        group: false,
                        supersession_enabled: true,
                        require_source_message_id: true,
                    },
                    &|key| key.to_owned(),
                ),
                job_id: JobId::new(),
                now: TimestampMillis::new(22),
            })
            .expect("sibling run");
        database
            .transition_dynamic_memory_attempt(
                sibling_attempt_id,
                sibling_run.attempt.revision,
                DynamicMemoryAttemptStatus::Processing,
                None,
                TimestampMillis::new(23),
            )
            .expect("sibling processing");
        let sibling_summary = database
            .commit_dynamic_memory_summary(
                DynamicMemorySummaryCommit {
                    run_id: sibling_run_id,
                    attempt_id: sibling_attempt_id,
                    expected_memory_revision: pooled.revision,
                    text: "Sibling summary".into(),
                    token_count: 2,
                    request_context: ProviderNeutralContext {
                        messages: Vec::new(),
                        attributions: Default::default(),
                        budget: Default::default(),
                    },
                    usage: None,
                    provider_request_id: None,
                },
                TimestampMillis::new(26),
            )
            .expect("sibling summary");
        let sibling_memory_id = MemoryId::new();
        let sibling_call_id = ToolExecutionId::new();
        database
            .admit_dynamic_memory_inference_round(
                sibling_run_id,
                sibling_attempt_id,
                0,
                0,
                NewDynamicMemoryInferenceRound {
                    ordinal: 0,
                    request_context: ProviderNeutralContext {
                        messages: Vec::new(),
                        attributions: Default::default(),
                        budget: Default::default(),
                    },
                    parts: Vec::new(),
                    provider_replay: None,
                    usage: None,
                    finish_reason: DynamicMemoryRoundFinishReason::Stop,
                    kind: DynamicMemoryRoundKind::Manager,
                    provider_request_id: None,
                    calls: vec![NewDynamicMemoryToolCall {
                        id: sibling_call_id,
                        definition_version: 1,
                        call: lettuce_conversations::ProposedToolCall {
                            provider_call_id: Some("sibling-create".into()),
                            name: "create_memory".into(),
                            arguments: json!({"text":"sibling memory","category":"preference"}),
                            raw_arguments: None,
                            provider_replay: None,
                        },
                    }],
                    admitted_at: TimestampMillis::new(24),
                },
            )
            .expect("sibling round");
        let sibling_item = memory_item(sibling_memory_id, "sibling memory", 24);
        let mut siblings_items = pooled.items.clone();
        siblings_items.push(sibling_item.clone());
        let sibling_settled = database
            .commit_dynamic_memory_background_round(
                DynamicMemoryBackgroundRoundCommit {
                    run_id: sibling_run_id,
                    attempt_id: sibling_attempt_id,
                    round_ordinal: 0,
                    space_id,
                    expected_memory_revision: sibling_summary.resulting_memory_revision,
                    change: Some(MemoryChangeSet {
                        space_id,
                        expected_revision: sibling_summary.resulting_memory_revision,
                        items: siblings_items,
                    }),
                    results: vec![MemoryToolResult {
                        execution_id: sibling_call_id,
                        outcome: MemoryToolOutcome::Created {
                            id: sibling_memory_id,
                            short_id: sibling_item.short_id,
                            memories: Vec::new(),
                        },
                    }],
                },
                TimestampMillis::new(25),
            )
            .expect("sibling creates memory");
        let sibling_attempt = database
            .load_dynamic_memory_attempt(sibling_attempt_id)
            .expect("sibling attempt");
        database
            .transition_dynamic_memory_attempt(
                sibling_attempt_id,
                sibling_attempt.revision,
                DynamicMemoryAttemptStatus::Succeeded,
                None,
                TimestampMillis::new(27),
            )
            .expect("sibling succeeded");
        let undone = database
            .rewind_dynamic_memory_suffix(DynamicMemorySuffixRewind {
                operation_id: OperationId::new(),
                conversation_id,
                invalid_run_id: Some(run_id),
                expected_memory_revision: sibling_settled.resulting_memory_revision,
                invalidated_effect_ids: Vec::new(),
                at: TimestampMillis::new(28),
            })
            .expect("own-space rewind");
        assert_eq!(undone.memory.items, vec![other_chat_memory, sibling_item]);
        assert_eq!(undone.summary, Some(sibling_summary.summary));
        let mut connection = database.connection().expect("connection");
        let transaction = connection.transaction().expect("transaction");
        assert_eq!(
            crate::memory::memory_adapter::run_cursor_in(
                &transaction,
                space_id,
                conversation_id,
                sibling_branch
            )
            .expect("sibling run cursor"),
            2
        );
        transaction.commit().expect("read cursor");
        drop(connection);
        assert!(
            database
                .load_dynamic_memory_round_settlement(sibling_run_id, sibling_attempt_id, 0)
                .expect("sibling settlement retained")
                .is_some()
        );
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
            branch_id: fixture_branch(&database, conversation_id),
            space_id,
            starting_memory: database.get(space_id).expect("memory").expect("space"),
            cycle_start_change: None,
            source_messages: messages.clone(),
            profile: profile(),
            time_awareness_enabled: true,
            supersession_enabled: true,
            structured_fallback_format: DynamicMemoryStructuredFallbackFormat::Xml,
            summary_window: lettuce_memory::DynamicMemorySummaryWindow {
                message_interval: 2,
                start: 0,
                end: u64::try_from(messages.len()).expect("messages"),
            },
            tool_request: lettuce_memory::dynamic_memory_tool_request_for_run(
                lettuce_memory::DynamicMemoryToolOptions {
                    group: false,
                    supersession_enabled: true,
                    require_source_message_id: true,
                },
                &|key| key.to_owned(),
            ),
            job_id: JobId::new(),
            now: TimestampMillis::new(10),
        };
        let child_branch_id = ConversationBranchId::new();
        database.connection().expect("connection").execute(
            "INSERT INTO conversation_branches (conversation_id,id,parent_branch_id,fork_message_id,status,revision,created_at,updated_at) VALUES (?1,?2,?3,?4,'active',1,2,2)",
            params![conversation_id.to_string(), child_branch_id.to_string(), admission.branch_id.to_string(), messages[0].message_id.to_string()],
        ).expect("child branch");
        let mut sibling_space = admission.clone();
        sibling_space.branch_id = child_branch_id;
        assert_eq!(
            database.admit_dynamic_memory_run_attempt(sibling_space),
            Err(DynamicMemoryRunRepositoryError::Conflict)
        );
        assert_eq!(
            database.load_dynamic_memory_run(run_id),
            Err(DynamicMemoryRunRepositoryError::NotFound)
        );
        let mut foreign_branch = admission.clone();
        foreign_branch.branch_id = ConversationBranchId::new();
        assert_eq!(
            database.admit_dynamic_memory_run_attempt(foreign_branch),
            Err(DynamicMemoryRunRepositoryError::Conflict)
        );
        let mut stale_start = admission.clone();
        stale_start.run_id = DynamicMemoryRunId::new();
        stale_start.attempt_id = DynamicMemoryAttemptId::new();
        stale_start.starting_memory.revision = Revision::new(2);
        assert_eq!(
            database.admit_dynamic_memory_run_attempt(stale_start),
            Err(DynamicMemoryRunRepositoryError::Conflict)
        );
        let admitted = database
            .admit_dynamic_memory_run_attempt(admission.clone())
            .expect("admit");
        assert!(admitted.run.time_awareness_enabled);
        assert!(admitted.run.supersession_enabled);
        assert_eq!(admitted.run.tool_request, admission.tool_request);
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
            request_context: lettuce_conversations::ProviderNeutralContext {
                messages: Vec::new(),
                attributions: Default::default(),
                budget: Default::default(),
            },
            parts: vec![MessagePart::ReasoningSummary {
                text: "Found a durable preference".into(),
            }],
            provider_replay: None,
            usage: Some(InferenceUsage {
                provider_reported_cost: None,
                cache_write_tokens: Some(1),
                web_search_requests: None,
                cached_input_tokens: Some(2),
                reasoning_tokens: Some(3),
                image_tokens: Some(4),
                audio_tokens: Some(5),
                total_tokens: Some(60),
                input_tokens: 30,
                output_tokens: 20,
            }),
            finish_reason: DynamicMemoryRoundFinishReason::Stop,
            kind: DynamicMemoryRoundKind::Manager,
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
        let stored_usage = database
            .list_dynamic_memory_inference_rounds(run_id, parent_id)
            .expect("rounds")[0]
            .usage
            .clone()
            .expect("round usage");
        assert_eq!(
            (
                stored_usage.image_tokens,
                stored_usage.audio_tokens,
                stored_usage.total_tokens
            ),
            (Some(4), Some(5), Some(60))
        );
        let memory_id = MemoryId::new();
        let parent_settlement = database
            .commit_dynamic_memory_background_round(
                DynamicMemoryBackgroundRoundCommit {
                    run_id,
                    attempt_id: parent_id,
                    round_ordinal: 0,
                    space_id,
                    expected_memory_revision: Revision::INITIAL,
                    change: Some(MemoryChangeSet {
                        space_id,
                        expected_revision: Revision::INITIAL,
                        items: vec![MemoryItem {
                            id: memory_id,
                            short_id: lettuce_memory::MemoryShortId::derived(memory_id),
                            text: "The user prefers tea".into(),
                            category: MemoryCategory::Preference,
                            source_message_id: Some(messages[0].message_id),
                            source_role: None,
                            observed_at: None,
                            observed_time_precision: None,
                            superseded_by: None,
                            superseded_at: None,
                            supersedes: Vec::new(),
                            token_count: 4,
                            is_cold: false,
                            is_pinned: false,
                            importance: Score::FULL,
                            persistence_importance: Score::FULL,
                            prompt_importance: Score::FULL,
                            volatility: Score::LEGACY_VOLATILITY,
                            access_count: 0,
                            created_at: TimestampMillis::new(12),
                            last_accessed_at: TimestampMillis::new(12),
                        }],
                    }),
                    results: vec![MemoryToolResult {
                        execution_id: call_id,
                        outcome: MemoryToolOutcome::Created {
                            id: memory_id,
                            short_id: lettuce_memory::MemoryShortId::derived(memory_id),
                            memories: vec![lettuce_memory::ListedMemory {
                                short_id: lettuce_memory::MemoryShortId::derived(memory_id),
                                text: "The user prefers tea".into(),
                            }],
                        },
                    }],
                },
                TimestampMillis::new(12),
            )
            .expect("parent settlement");
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
        let child_settlement = database
            .load_dynamic_memory_round_settlement(run_id, child_id, 0)
            .expect("child settlement")
            .expect("copied checkpoint");
        let mut expected_child_settlement = parent_settlement;
        expected_child_settlement.attempt_id = child_id;
        assert_eq!(child_settlement, expected_child_settlement);
        assert_eq!(
            database
                .get(space_id)
                .expect("memory")
                .expect("space")
                .revision,
            Revision::new(2)
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
