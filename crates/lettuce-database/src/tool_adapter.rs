use std::{collections::HashSet, str::FromStr};

use lettuce_conversations::{
    ConversationRepositoryError, ReplayRetention, ToolExecution, ToolExecutionRepository,
    ToolExecutionStatus, ToolExecutionTransition, ToolFailure, ToolFailureCode, ToolOutput,
};
use lettuce_types::{
    ConversationId, GenerationAttemptId, GenerationTurnId, Revision, TimestampMillis,
    ToolExecutionId,
};
use rusqlite::{OptionalExtension, Row, Transaction, TransactionBehavior, params};

use super::{Database, conversation_query, decode_versioned, encode_versioned};

const TOOL_JSON_VERSION: u32 = 1;

fn storage<T>(_: T) -> ConversationRepositoryError {
    ConversationRepositoryError::Storage
}

fn status_name(status: ToolExecutionStatus) -> &'static str {
    match status {
        ToolExecutionStatus::Requested => "requested",
        ToolExecutionStatus::Validated => "validated",
        ToolExecutionStatus::Running => "running",
        ToolExecutionStatus::Succeeded => "succeeded",
        ToolExecutionStatus::Rejected => "rejected",
        ToolExecutionStatus::Failed => "failed",
        ToolExecutionStatus::Cancelled => "cancelled",
        ToolExecutionStatus::Interrupted => "interrupted",
    }
}

fn parse_status(value: &str) -> Result<ToolExecutionStatus, ConversationRepositoryError> {
    Ok(match value {
        "requested" => ToolExecutionStatus::Requested,
        "validated" => ToolExecutionStatus::Validated,
        "running" => ToolExecutionStatus::Running,
        "succeeded" => ToolExecutionStatus::Succeeded,
        "rejected" => ToolExecutionStatus::Rejected,
        "failed" => ToolExecutionStatus::Failed,
        "cancelled" => ToolExecutionStatus::Cancelled,
        "interrupted" => ToolExecutionStatus::Interrupted,
        _ => return Err(ConversationRepositoryError::Storage),
    })
}

fn failure_name(code: ToolFailureCode) -> &'static str {
    match code {
        ToolFailureCode::InvalidArguments => "invalid_arguments",
        ToolFailureCode::UndeclaredTool => "undeclared_tool",
        ToolFailureCode::PermissionDenied => "permission_denied",
        ToolFailureCode::HandlerFailed => "handler_failed",
        ToolFailureCode::ResultInvalid => "result_invalid",
        ToolFailureCode::Internal => "internal",
    }
}

fn parse_failure(value: &str) -> Result<ToolFailureCode, ConversationRepositoryError> {
    Ok(match value {
        "invalid_arguments" => ToolFailureCode::InvalidArguments,
        "undeclared_tool" => ToolFailureCode::UndeclaredTool,
        "permission_denied" => ToolFailureCode::PermissionDenied,
        "handler_failed" => ToolFailureCode::HandlerFailed,
        "result_invalid" => ToolFailureCode::ResultInvalid,
        "internal" => ToolFailureCode::Internal,
        _ => return Err(ConversationRepositoryError::Storage),
    })
}

fn sql_u64(value: u64) -> Result<i64, ConversationRepositoryError> {
    i64::try_from(value).map_err(storage)
}

const SELECT_EXECUTION: &str = "
    SELECT e.id, e.conversation_id, e.turn_id, e.attempt_id, e.ordinal,
           e.definition_name, e.definition_version, e.provider_call_id,
           e.arguments_json, e.raw_arguments, e.status, e.output_json,
           e.failure_code, e.failure_message, e.revision, e.requested_at,
           e.started_at, e.finished_at, e.updated_at,
           e.provider_replay_artifact_id, e.provider_replay_retention
      FROM tool_executions e
    ";

fn hydrate(
    transaction: &Transaction<'_>,
    row: &Row<'_>,
) -> Result<ToolExecution, ConversationRepositoryError> {
    let provider_replay = conversation_query::replay_ref(
        transaction,
        row.get(19).map_err(storage)?,
        row.get(20).map_err(storage)?,
    )?;
    let output = row
        .get::<_, Option<String>>(11)
        .map_err(storage)?
        .map(|value| decode_versioned::<ToolOutput>(&value, TOOL_JSON_VERSION).map_err(storage))
        .transpose()?;
    let failure_code = row
        .get::<_, Option<String>>(12)
        .map_err(storage)?
        .map(|value| parse_failure(&value))
        .transpose()?;
    let failure_message = row.get::<_, Option<String>>(13).map_err(storage)?;
    let execution = ToolExecution {
        id: ToolExecutionId::from_str(&row.get::<_, String>(0).map_err(storage)?)
            .map_err(storage)?,
        conversation_id: row
            .get::<_, String>(1)
            .map_err(storage)?
            .parse()
            .map_err(storage)?,
        turn_id: row
            .get::<_, String>(2)
            .map_err(storage)?
            .parse()
            .map_err(storage)?,
        attempt_id: row
            .get::<_, String>(3)
            .map_err(storage)?
            .parse()
            .map_err(storage)?,
        ordinal: u16::try_from(row.get::<_, i64>(4).map_err(storage)?).map_err(storage)?,
        definition_name: row.get(5).map_err(storage)?,
        definition_version: u32::try_from(row.get::<_, i64>(6).map_err(storage)?)
            .map_err(storage)?,
        provider_call_id: row.get(7).map_err(storage)?,
        arguments: decode_versioned(
            &row.get::<_, String>(8).map_err(storage)?,
            TOOL_JSON_VERSION,
        )
        .map_err(storage)?,
        raw_arguments: row.get(9).map_err(storage)?,
        provider_replay,
        status: parse_status(&row.get::<_, String>(10).map_err(storage)?)?,
        output,
        failure: failure_code.map(|code| ToolFailure {
            code,
            message: failure_message,
        }),
        revision: Revision::new(
            u64::try_from(row.get::<_, i64>(14).map_err(storage)?).map_err(storage)?,
        ),
        requested_at: TimestampMillis::new(row.get(15).map_err(storage)?),
        started_at: row
            .get::<_, Option<i64>>(16)
            .map_err(storage)?
            .map(TimestampMillis::new),
        finished_at: row
            .get::<_, Option<i64>>(17)
            .map_err(storage)?
            .map(TimestampMillis::new),
        updated_at: TimestampMillis::new(row.get(18).map_err(storage)?),
    };
    execution.validate()?;
    Ok(execution)
}

pub(super) fn get_in(
    transaction: &Transaction<'_>,
    id: ToolExecutionId,
) -> Result<ToolExecution, ConversationRepositoryError> {
    transaction
        .query_row(
            &format!("{SELECT_EXECUTION} WHERE e.id = ?1"),
            params![id.to_string()],
            |row| hydrate(transaction, row).map_err(|_| rusqlite::Error::InvalidQuery),
        )
        .optional()
        .map_err(storage)?
        .ok_or(ConversationRepositoryError::NotFound)
}

pub(super) fn transition_in(
    transaction: &Transaction<'_>,
    transition: &ToolExecutionTransition,
    at: TimestampMillis,
) -> Result<ToolExecution, ConversationRepositoryError> {
    let current = get_in(transaction, transition.id)?;
    if current.revision != transition.expected_revision {
        return Err(ConversationRepositoryError::StaleRevision {
            expected: transition.expected_revision,
            actual: current.revision,
        });
    }
    let updated = current.transition(
        transition.next,
        transition.output.clone(),
        transition.failure.clone(),
        at,
    )?;
    let output_json = updated
        .output
        .as_ref()
        .map(|value| encode_versioned(value, TOOL_JSON_VERSION).map_err(storage))
        .transpose()?;
    let failure_code = updated
        .failure
        .as_ref()
        .map(|failure| failure_name(failure.code));
    let failure_message = updated
        .failure
        .as_ref()
        .and_then(|failure| failure.message.as_deref());
    let changed = transaction
        .execute(
            "UPDATE tool_executions
                SET status = ?2, output_json = ?3, failure_code = ?4,
                    failure_message = ?5, revision = ?6, started_at = ?7,
                    finished_at = ?8, updated_at = ?9
              WHERE id = ?1 AND revision = ?10",
            params![
                transition.id.to_string(),
                status_name(updated.status),
                output_json,
                failure_code,
                failure_message,
                sql_u64(updated.revision.get())?,
                updated.started_at.map(TimestampMillis::get),
                updated.finished_at.map(TimestampMillis::get),
                updated.updated_at.get(),
                sql_u64(transition.expected_revision.get())?,
            ],
        )
        .map_err(storage)?;
    if changed != 1 {
        return Err(ConversationRepositoryError::Conflict);
    }
    get_in(transaction, transition.id)
}

fn validate_new(execution: &ToolExecution) -> Result<(), ConversationRepositoryError> {
    execution.validate()?;
    if execution.status != ToolExecutionStatus::Requested || execution.revision != Revision::INITIAL
    {
        return Err(ConversationRepositoryError::Invalid(
            lettuce_conversations::ValidationError::Invariant {
                field: "tool_execution.new",
            },
        ));
    }
    if execution
        .provider_replay
        .as_ref()
        .is_some_and(|reference| reference.retention != ReplayRetention::Conversation)
    {
        return Err(ConversationRepositoryError::Invalid(
            lettuce_conversations::ValidationError::InvalidReference {
                field: "tool_execution.provider_replay",
            },
        ));
    }
    Ok(())
}

fn validate_new_batch(executions: &[ToolExecution]) -> Result<(), ConversationRepositoryError> {
    let Some(first) = executions.first() else {
        return Err(ConversationRepositoryError::Invalid(
            lettuce_conversations::ValidationError::InvalidValue {
                field: "tool_executions",
            },
        ));
    };
    if executions.len() > lettuce_conversations::MAX_TOOL_CALLS_PER_RESPONSE {
        return Err(ConversationRepositoryError::Invalid(
            lettuce_conversations::ValidationError::TooMany {
                field: "tool_executions",
                max: lettuce_conversations::MAX_TOOL_CALLS_PER_RESPONSE,
            },
        ));
    }
    let mut ids = std::collections::HashSet::new();
    let mut provider_call_ids = std::collections::HashSet::new();
    for (offset, execution) in executions.iter().enumerate() {
        validate_new(execution)?;
        let expected_ordinal = usize::from(first.ordinal).checked_add(offset).ok_or(
            ConversationRepositoryError::Invalid(
                lettuce_conversations::ValidationError::OutOfBounds {
                    field: "tool_executions.ordinal",
                },
            ),
        )?;
        if execution.conversation_id != first.conversation_id
            || execution.turn_id != first.turn_id
            || execution.attempt_id != first.attempt_id
            || execution.requested_at != first.requested_at
            || usize::from(execution.ordinal) != expected_ordinal
        {
            return Err(ConversationRepositoryError::Invalid(
                lettuce_conversations::ValidationError::Invariant {
                    field: "tool_executions.batch",
                },
            ));
        }
        if !ids.insert(execution.id)
            || execution
                .provider_call_id
                .as_deref()
                .is_some_and(|id| !provider_call_ids.insert(id))
        {
            return Err(ConversationRepositoryError::Invalid(
                lettuce_conversations::ValidationError::Duplicate {
                    field: "tool_executions.identity",
                },
            ));
        }
    }
    Ok(())
}

fn insert_in(
    transaction: &Transaction<'_>,
    execution: &ToolExecution,
) -> Result<ToolExecution, ConversationRepositoryError> {
    let arguments = encode_versioned(&execution.arguments, TOOL_JSON_VERSION).map_err(storage)?;
    let (replay_id, replay_retention) = execution
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
            "INSERT INTO tool_executions (
                conversation_id, turn_id, attempt_id, id, ordinal,
                definition_name, definition_version, provider_call_id,
                arguments_json, raw_arguments, provider_replay_artifact_id,
                provider_replay_retention, status, output_json, failure_code,
                failure_message, revision, requested_at, started_at,
                finished_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                       ?11, ?12, 'requested', NULL, NULL, NULL, 1, ?13,
                       NULL, NULL, ?13)",
            params![
                execution.conversation_id.to_string(),
                execution.turn_id.to_string(),
                execution.attempt_id.to_string(),
                execution.id.to_string(),
                i64::from(execution.ordinal),
                execution.definition_name,
                i64::from(execution.definition_version),
                execution.provider_call_id,
                arguments,
                execution.raw_arguments,
                replay_id,
                replay_retention,
                execution.requested_at.get(),
            ],
        )
        .map_err(|error| match error.sqlite_error_code() {
            Some(rusqlite::ErrorCode::ConstraintViolation) => ConversationRepositoryError::Conflict,
            _ => ConversationRepositoryError::Storage,
        })?;
    get_in(transaction, execution.id)
}

impl ToolExecutionRepository for Database {
    fn append_tool_executions(
        &self,
        expected_next_ordinal: u16,
        executions: &[ToolExecution],
    ) -> Result<Vec<ToolExecution>, ConversationRepositoryError> {
        validate_new_batch(executions)?;
        if executions[0].ordinal != expected_next_ordinal {
            return Err(ConversationRepositoryError::Invalid(
                lettuce_conversations::ValidationError::Invariant {
                    field: "tool_executions.expected_ordinal",
                },
            ));
        }
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let actual_next_ordinal = transaction
            .query_row(
                "SELECT coalesce(max(ordinal) + 1, 0)
                   FROM tool_executions
                  WHERE conversation_id = ?1 AND turn_id = ?2 AND attempt_id = ?3",
                params![
                    executions[0].conversation_id.to_string(),
                    executions[0].turn_id.to_string(),
                    executions[0].attempt_id.to_string(),
                ],
                |row| row.get::<_, i64>(0),
            )
            .map_err(storage)?;
        if actual_next_ordinal != i64::from(expected_next_ordinal) {
            return Err(ConversationRepositoryError::Conflict);
        }
        let mut stored = Vec::with_capacity(executions.len());
        for execution in executions {
            stored.push(insert_in(&transaction, execution)?);
        }
        transaction.commit().map_err(storage)?;
        Ok(stored)
    }

    fn get_tool_execution(
        &self,
        id: ToolExecutionId,
    ) -> Result<ToolExecution, ConversationRepositoryError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(storage)?;
        let execution = get_in(&transaction, id)?;
        transaction.commit().map_err(storage)?;
        Ok(execution)
    }

    fn list_tool_executions(
        &self,
        conversation_id: ConversationId,
        turn_id: GenerationTurnId,
        attempt_id: GenerationAttemptId,
    ) -> Result<Vec<ToolExecution>, ConversationRepositoryError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(storage)?;
        let sql = format!(
            "{SELECT_EXECUTION} WHERE e.conversation_id = ?1 AND e.turn_id = ?2 AND e.attempt_id = ?3 ORDER BY e.ordinal"
        );
        let executions = {
            let mut statement = transaction.prepare(&sql).map_err(storage)?;
            let mut rows = statement
                .query(params![
                    conversation_id.to_string(),
                    turn_id.to_string(),
                    attempt_id.to_string(),
                ])
                .map_err(storage)?;
            let mut executions = Vec::new();
            while let Some(row) = rows.next().map_err(storage)? {
                executions.push(hydrate(&transaction, row)?);
            }
            executions
        };
        transaction.commit().map_err(storage)?;
        Ok(executions)
    }

    fn transition_tool_execution(
        &self,
        id: ToolExecutionId,
        expected_revision: Revision,
        next: ToolExecutionStatus,
        output: Option<ToolOutput>,
        failure: Option<ToolFailure>,
        at: TimestampMillis,
    ) -> Result<ToolExecution, ConversationRepositoryError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let stored = transition_in(
            &transaction,
            &ToolExecutionTransition {
                id,
                expected_revision,
                next,
                output,
                failure,
            },
            at,
        )?;
        transaction.commit().map_err(storage)?;
        Ok(stored)
    }

    fn transition_tool_execution_batch(
        &self,
        transitions: &[ToolExecutionTransition],
        at: TimestampMillis,
    ) -> Result<Vec<ToolExecution>, ConversationRepositoryError> {
        if transitions.is_empty() {
            return Err(ConversationRepositoryError::Invalid(
                lettuce_conversations::ValidationError::InvalidValue {
                    field: "tool_execution_transitions",
                },
            ));
        }
        let mut ids = HashSet::with_capacity(transitions.len());
        if transitions
            .iter()
            .any(|transition| !ids.insert(transition.id))
        {
            return Err(ConversationRepositoryError::Invalid(
                lettuce_conversations::ValidationError::Duplicate {
                    field: "tool_execution_transitions.ids",
                },
            ));
        }
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let first = get_in(&transaction, transitions[0].id)?;
        let owner = (first.conversation_id, first.turn_id, first.attempt_id);
        let mut stored = Vec::with_capacity(transitions.len());
        for transition in transitions {
            let current = get_in(&transaction, transition.id)?;
            if owner != (current.conversation_id, current.turn_id, current.attempt_id) {
                return Err(ConversationRepositoryError::Invalid(
                    lettuce_conversations::ValidationError::InvalidReference {
                        field: "tool_execution_transitions.owner",
                    },
                ));
            }
            stored.push(transition_in(&transaction, transition, at)?);
        }
        transaction.commit().map_err(storage)?;
        Ok(stored)
    }
}

#[cfg(test)]
mod tests {
    use lettuce_conversations::{
        ArtifactCodec, ArtifactRetention, ConversationArtifactStore, ProtectedArtifactBytes,
        ReplayArtifactDraft, ToolExecution, ToolExecutionRepository, ToolExecutionStatus,
        ToolExecutionTransition, ToolOutput,
    };
    use lettuce_types::{
        ConversationId, GenerationAttemptId, GenerationTurnId, ReplayArtifactId, Revision,
        TimestampMillis, ToolExecutionId,
    };
    use serde_json::json;

    use super::Database;

    fn requested(
        conversation_id: ConversationId,
        turn_id: GenerationTurnId,
        attempt_id: GenerationAttemptId,
        ordinal: u16,
        provider_call_id: &str,
    ) -> ToolExecution {
        ToolExecution {
            id: ToolExecutionId::new(),
            conversation_id,
            turn_id,
            attempt_id,
            ordinal,
            definition_name: "create_memory".to_owned(),
            definition_version: 1,
            provider_call_id: Some(provider_call_id.to_owned()),
            arguments: json!({"content": "remember this"}),
            raw_arguments: Some(r#"{"content":"remember this"}"#.to_owned()),
            provider_replay: None,
            status: ToolExecutionStatus::Requested,
            output: None,
            failure: None,
            revision: Revision::INITIAL,
            requested_at: TimestampMillis::new(10),
            started_at: None,
            finished_at: None,
            updated_at: TimestampMillis::new(10),
        }
    }

    fn seed_running_attempt(
        database: &Database,
        conversation_id: ConversationId,
        turn_id: GenerationTurnId,
        attempt_id: GenerationAttemptId,
    ) {
        database
            .connection()
            .expect("connection")
            .execute(
                "INSERT INTO generation_attempts (
                    conversation_id, turn_id, id, ordinal, parent_attempt_id,
                    status, job_idempotency_key, job_id, started_at, finished_at,
                    usage_event_id, usage_outcome, failure
                 ) VALUES (?1, ?2, ?3, 0, NULL, 'running', ?4, NULL, 9, NULL,
                           NULL, NULL, NULL)",
                rusqlite::params![
                    conversation_id.to_string(),
                    turn_id.to_string(),
                    attempt_id.to_string(),
                    format!("generation.{turn_id}.{attempt_id}"),
                ],
            )
            .expect("running attempt");
    }

    fn insert_one(
        database: &Database,
        execution: &ToolExecution,
    ) -> Result<ToolExecution, lettuce_conversations::ConversationRepositoryError> {
        database
            .append_tool_executions(execution.ordinal, std::slice::from_ref(execution))?
            .pop()
            .ok_or(lettuce_conversations::ConversationRepositoryError::Storage)
    }

    #[test]
    fn persists_and_cas_transitions_a_tool_execution() {
        let database = Database::open_in_memory().expect("database");
        database
            .connection()
            .expect("connection")
            .execute_batch("PRAGMA foreign_keys = OFF")
            .expect("test fixture mode");
        let conversation_id = ConversationId::new();
        let turn_id = GenerationTurnId::new();
        let attempt_id = GenerationAttemptId::new();
        seed_running_attempt(&database, conversation_id, turn_id, attempt_id);
        let execution = requested(conversation_id, turn_id, attempt_id, 0, "call-1");
        let stored = insert_one(&database, &execution).expect("insert execution");
        assert_eq!(stored, execution);

        let validated = database
            .transition_tool_execution(
                execution.id,
                Revision::INITIAL,
                ToolExecutionStatus::Validated,
                None,
                None,
                TimestampMillis::new(11),
            )
            .expect("validate");
        let running = database
            .transition_tool_execution(
                execution.id,
                validated.revision,
                ToolExecutionStatus::Running,
                None,
                None,
                TimestampMillis::new(12),
            )
            .expect("start");
        assert!(
            database
                .connection()
                .expect("connection")
                .execute(
                    "UPDATE generation_attempts
                    SET status = 'interrupted', finished_at = 13,
                        usage_event_id = 'usage-1', usage_outcome = 'interrupted'
                  WHERE id = ?1",
                    [attempt_id.to_string()],
                )
                .is_err()
        );
        let succeeded = database
            .transition_tool_execution(
                execution.id,
                running.revision,
                ToolExecutionStatus::Succeeded,
                Some(ToolOutput {
                    value: json!({"memory_id": "memory-1"}),
                    is_error: false,
                }),
                None,
                TimestampMillis::new(13),
            )
            .expect("settle");
        assert_eq!(succeeded.status, ToolExecutionStatus::Succeeded);
        assert_eq!(
            database
                .list_tool_executions(conversation_id, turn_id, attempt_id)
                .expect("attempt executions"),
            vec![succeeded.clone()]
        );
        assert_eq!(
            database
                .get_tool_execution(execution.id)
                .expect("stored execution"),
            succeeded
        );

        assert!(matches!(
            database.transition_tool_execution(
                execution.id,
                Revision::INITIAL,
                ToolExecutionStatus::Validated,
                None,
                None,
                TimestampMillis::new(14),
            ),
            Err(lettuce_conversations::ConversationRepositoryError::StaleRevision { .. })
        ));
    }

    #[test]
    fn batch_transition_rolls_back_every_execution_on_one_stale_revision() {
        let database = Database::open_in_memory().expect("database");
        database
            .connection()
            .expect("connection")
            .execute_batch("PRAGMA foreign_keys = OFF")
            .expect("test fixture mode");
        let conversation_id = ConversationId::new();
        let turn_id = GenerationTurnId::new();
        let attempt_id = GenerationAttemptId::new();
        seed_running_attempt(&database, conversation_id, turn_id, attempt_id);
        let calls = vec![
            requested(conversation_id, turn_id, attempt_id, 0, "call-1"),
            requested(conversation_id, turn_id, attempt_id, 1, "call-2"),
        ];
        let stored = database
            .append_tool_executions(0, &calls)
            .expect("insert round");
        let validated = database
            .transition_tool_execution_batch(
                &stored
                    .iter()
                    .map(|execution| ToolExecutionTransition {
                        id: execution.id,
                        expected_revision: execution.revision,
                        next: ToolExecutionStatus::Validated,
                        output: None,
                        failure: None,
                    })
                    .collect::<Vec<_>>(),
                TimestampMillis::new(11),
            )
            .expect("validate round");
        let transitions = vec![
            ToolExecutionTransition {
                id: validated[0].id,
                expected_revision: validated[0].revision,
                next: ToolExecutionStatus::Running,
                output: None,
                failure: None,
            },
            ToolExecutionTransition {
                id: validated[1].id,
                expected_revision: Revision::INITIAL,
                next: ToolExecutionStatus::Running,
                output: None,
                failure: None,
            },
        ];
        assert!(matches!(
            database.transition_tool_execution_batch(&transitions, TimestampMillis::new(12)),
            Err(lettuce_conversations::ConversationRepositoryError::StaleRevision { .. })
        ));
        assert_eq!(
            database
                .list_tool_executions(conversation_id, turn_id, attempt_id)
                .expect("stored"),
            validated
        );
    }

    #[test]
    fn rejects_provider_call_id_collisions_within_an_attempt() {
        let database = Database::open_in_memory().expect("database");
        database
            .connection()
            .expect("connection")
            .execute_batch("PRAGMA foreign_keys = OFF")
            .expect("test fixture mode");
        let conversation_id = ConversationId::new();
        let turn_id = GenerationTurnId::new();
        let attempt_id = GenerationAttemptId::new();
        seed_running_attempt(&database, conversation_id, turn_id, attempt_id);
        insert_one(
            &database,
            &requested(conversation_id, turn_id, attempt_id, 0, "same-call"),
        )
        .expect("first call");
        assert!(matches!(
            insert_one(
                &database,
                &requested(conversation_id, turn_id, attempt_id, 1, "same-call")
            ),
            Err(lettuce_conversations::ConversationRepositoryError::Conflict)
        ));
    }

    #[test]
    fn rolls_back_the_whole_tool_call_batch_on_a_collision() {
        let database = Database::open_in_memory().expect("database");
        database
            .connection()
            .expect("connection")
            .execute_batch("PRAGMA foreign_keys = OFF")
            .expect("test fixture mode");
        let conversation_id = ConversationId::new();
        let turn_id = GenerationTurnId::new();
        let attempt_id = GenerationAttemptId::new();
        seed_running_attempt(&database, conversation_id, turn_id, attempt_id);
        let existing = requested(conversation_id, turn_id, attempt_id, 0, "same-call");
        insert_one(&database, &existing).expect("existing call");
        let calls = vec![
            requested(conversation_id, turn_id, attempt_id, 1, "new-call"),
            requested(conversation_id, turn_id, attempt_id, 2, "same-call"),
        ];

        assert!(matches!(
            database.append_tool_executions(1, &calls),
            Err(lettuce_conversations::ConversationRepositoryError::Conflict)
        ));
        assert_eq!(
            database
                .list_tool_executions(conversation_id, turn_id, attempt_id)
                .expect("attempt executions"),
            vec![existing]
        );
    }

    #[test]
    fn continuation_rounds_compare_and_append_after_prior_calls() {
        let database = Database::open_in_memory().expect("database");
        database
            .connection()
            .expect("connection")
            .execute_batch("PRAGMA foreign_keys = OFF")
            .expect("test fixture mode");
        let conversation_id = ConversationId::new();
        let turn_id = GenerationTurnId::new();
        let attempt_id = GenerationAttemptId::new();
        seed_running_attempt(&database, conversation_id, turn_id, attempt_id);
        let first = requested(conversation_id, turn_id, attempt_id, 0, "call-1");
        insert_one(&database, &first).expect("first round");
        let next = vec![
            requested(conversation_id, turn_id, attempt_id, 1, "call-2"),
            requested(conversation_id, turn_id, attempt_id, 2, "call-3"),
        ];

        assert_eq!(
            database
                .append_tool_executions(1, &next)
                .expect("continuation round"),
            next
        );
        assert!(matches!(
            database.append_tool_executions(
                1,
                std::slice::from_ref(&requested(
                    conversation_id,
                    turn_id,
                    attempt_id,
                    1,
                    "stale-call",
                )),
            ),
            Err(lettuce_conversations::ConversationRepositoryError::Conflict)
        ));
        assert_eq!(
            database
                .list_tool_executions(conversation_id, turn_id, attempt_id)
                .expect("attempt executions")
                .len(),
            3
        );
    }

    #[test]
    fn rejects_an_execution_without_its_owned_attempt() {
        let database = Database::open_in_memory().expect("database");
        let execution = requested(
            ConversationId::new(),
            GenerationTurnId::new(),
            GenerationAttemptId::new(),
            0,
            "orphan-call",
        );
        assert!(matches!(
            insert_one(&database, &execution),
            Err(lettuce_conversations::ConversationRepositoryError::Conflict)
        ));
    }

    #[test]
    fn tool_replay_reference_prevents_orphan_cleanup() {
        let database = Database::open_in_memory().expect("database");
        let bytes = ProtectedArtifactBytes::new(br#"{"thoughtSignature":"opaque"}"#.to_vec())
            .expect("bytes");
        let artifact_id = ReplayArtifactId::new();
        let replay = database
            .put_replay(ReplayArtifactDraft {
                artifact_id,
                digest: bytes.digest(),
                schema_version: 1,
                byte_size: u64::try_from(bytes.len()).expect("size"),
                codec: ArtifactCodec::Json,
                retention: ArtifactRetention::Conversation,
                bytes,
            })
            .expect("replay");
        database
            .connection()
            .expect("connection")
            .execute_batch("PRAGMA foreign_keys = OFF")
            .expect("test fixture mode");
        let conversation_id = ConversationId::new();
        let turn_id = GenerationTurnId::new();
        let attempt_id = GenerationAttemptId::new();
        seed_running_attempt(&database, conversation_id, turn_id, attempt_id);
        let mut execution = requested(conversation_id, turn_id, attempt_id, 0, "call-with-replay");
        execution.provider_replay = Some(replay.clone());
        insert_one(&database, &execution).expect("execution");
        database
            .cleanup_orphan_replay(artifact_id)
            .expect("referenced cleanup is a no-op");
        database.verify_replay(&replay).expect("replay retained");
    }
}
