use std::collections::HashSet;

use lettuce_conversations::ConversationRepositoryError;
use lettuce_memory::{
    DynamicMemoryRoundCommit, DynamicMemoryRoundCommitError, DynamicMemoryRoundCommitResult,
    DynamicMemoryRoundRepository, MemoryRepositoryError,
};
use rusqlite::TransactionBehavior;

use crate::{Database, memory_adapter, tool_adapter};

fn memory_storage(_: impl std::fmt::Debug) -> MemoryRepositoryError {
    MemoryRepositoryError::Failure("sqlite dynamic-memory round operation failed".to_owned())
}

impl DynamicMemoryRoundRepository for Database {
    fn commit_dynamic_memory_round(
        &self,
        commit: DynamicMemoryRoundCommit,
        at: lettuce_types::TimestampMillis,
    ) -> Result<DynamicMemoryRoundCommitResult, DynamicMemoryRoundCommitError> {
        if commit.execution_transitions.is_empty()
            || commit
                .execution_transitions
                .iter()
                .any(|transition| !transition.next.is_terminal())
        {
            return Err(ConversationRepositoryError::Invalid(
                lettuce_conversations::ValidationError::InvalidValue {
                    field: "dynamic_memory_round.execution_transitions",
                },
            )
            .into());
        }
        if commit
            .change
            .as_ref()
            .is_some_and(|change| change.space_id != commit.space_id)
        {
            return Err(MemoryRepositoryError::Invalid(
                lettuce_memory::MemoryValidationError::InvalidSpaceId,
            )
            .into());
        }
        if commit.change.as_ref().is_some_and(|change| {
            commit.expected_memory_revision.is_some()
                && commit.expected_memory_revision != Some(change.expected_revision)
        }) {
            return Err(MemoryRepositoryError::Invalid(
                lettuce_memory::MemoryValidationError::InvalidRevision,
            )
            .into());
        }
        let mut ids = HashSet::with_capacity(commit.execution_transitions.len());
        if commit
            .execution_transitions
            .iter()
            .any(|transition| !ids.insert(transition.id))
        {
            return Err(ConversationRepositoryError::Invalid(
                lettuce_conversations::ValidationError::Duplicate {
                    field: "dynamic_memory_round.execution_ids",
                },
            )
            .into());
        }
        let mut connection = self.connection().map_err(memory_storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(memory_storage)?;
        let first = tool_adapter::get_in(&transaction, commit.execution_transitions[0].id)?;
        let owner = (first.conversation_id, first.turn_id, first.attempt_id);
        let current_snapshot = memory_adapter::get_in(&transaction, commit.space_id)?
            .ok_or(MemoryRepositoryError::NotFound)?;
        if let Some(expected) = commit.expected_memory_revision
            && current_snapshot.revision != expected
        {
            return Err(MemoryRepositoryError::Conflict.into());
        }
        let mut executions = Vec::with_capacity(commit.execution_transitions.len());
        for transition in &commit.execution_transitions {
            let current = tool_adapter::get_in(&transaction, transition.id)?;
            if owner != (current.conversation_id, current.turn_id, current.attempt_id) {
                return Err(ConversationRepositoryError::Invalid(
                    lettuce_conversations::ValidationError::InvalidReference {
                        field: "dynamic_memory_round.execution_owner",
                    },
                )
                .into());
            }
            executions.push(tool_adapter::transition_in(&transaction, transition, at)?);
        }
        let snapshot = match &commit.change {
            Some(change) => memory_adapter::compare_and_apply_in(&transaction, change)?,
            None => current_snapshot,
        };
        transaction.commit().map_err(memory_storage)?;
        Ok(DynamicMemoryRoundCommitResult {
            snapshot,
            executions,
        })
    }
}

#[cfg(test)]
mod tests {
    use lettuce_conversations::{
        ToolExecution, ToolExecutionRepository, ToolExecutionStatus, ToolExecutionTransition,
        ToolOutput,
    };
    use lettuce_memory::{
        DynamicMemoryRoundCommit, DynamicMemoryRoundRepository, MemoryCategory, MemoryChangeSet,
        MemoryItem, MemoryRepository, MemorySpaceSnapshot, Score,
    };
    use lettuce_types::{
        ConversationId, GenerationAttemptId, GenerationTurnId, MemoryId, MemorySpaceId, Revision,
        TimestampMillis, ToolExecutionId,
    };
    use serde_json::json;

    use crate::Database;

    fn memory(id: MemoryId, text: &str) -> MemoryItem {
        MemoryItem {
            id,
            text: text.to_owned(),
            category: MemoryCategory::Other,
            source_message_id: None,
            source_role: None,
            observed_at: None,
            observed_time_precision: None,
            superseded_by: None,
            superseded_at: None,
            supersedes: Vec::new(),
            token_count: 2,
            is_cold: false,
            is_pinned: false,
            importance: Score::FULL,
            persistence_importance: Score::FULL,
            prompt_importance: Score::FULL,
            volatility: Score::LEGACY_VOLATILITY,
            access_count: 0,
            created_at: TimestampMillis::new(1),
            last_accessed_at: TimestampMillis::new(1),
        }
    }

    fn running_round(database: &Database) -> Vec<ToolExecution> {
        database
            .connection()
            .expect("connection")
            .execute_batch("PRAGMA foreign_keys = OFF")
            .expect("test fixture mode");
        let conversation_id = ConversationId::new();
        let turn_id = GenerationTurnId::new();
        let attempt_id = GenerationAttemptId::new();
        database
            .connection()
            .expect("connection")
            .execute(
                "INSERT INTO generation_attempts (
                    conversation_id, turn_id, id, ordinal, parent_attempt_id,
                    status, job_idempotency_key, job_id, started_at, finished_at,
                    usage_event_id, usage_outcome, failure
                 ) VALUES (?1, ?2, ?3, 0, NULL, 'running', ?4, NULL, 1, NULL,
                           NULL, NULL, NULL)",
                rusqlite::params![
                    conversation_id.to_string(),
                    turn_id.to_string(),
                    attempt_id.to_string(),
                    format!("generation.{turn_id}.{attempt_id}"),
                ],
            )
            .expect("attempt");
        let requested = (0..2)
            .map(|ordinal| ToolExecution {
                id: ToolExecutionId::new(),
                conversation_id,
                turn_id,
                attempt_id,
                ordinal,
                definition_name: "create_memory".to_owned(),
                definition_version: 1,
                provider_call_id: Some(format!("call-{ordinal}")),
                arguments: json!({"text": format!("memory-{ordinal}")}),
                raw_arguments: None,
                provider_replay: None,
                status: ToolExecutionStatus::Requested,
                output: None,
                failure: None,
                revision: Revision::INITIAL,
                requested_at: TimestampMillis::new(1),
                started_at: None,
                finished_at: None,
                updated_at: TimestampMillis::new(1),
            })
            .collect::<Vec<_>>();
        let requested = database
            .append_tool_executions(0, &requested)
            .expect("request round");
        let validated = database
            .transition_tool_execution_batch(
                &requested
                    .iter()
                    .map(|execution| ToolExecutionTransition {
                        id: execution.id,
                        expected_revision: execution.revision,
                        next: ToolExecutionStatus::Validated,
                        output: None,
                        failure: None,
                    })
                    .collect::<Vec<_>>(),
                TimestampMillis::new(2),
            )
            .expect("validate round");
        database
            .transition_tool_execution_batch(
                &validated
                    .iter()
                    .map(|execution| ToolExecutionTransition {
                        id: execution.id,
                        expected_revision: execution.revision,
                        next: ToolExecutionStatus::Running,
                        output: None,
                        failure: None,
                    })
                    .collect::<Vec<_>>(),
                TimestampMillis::new(3),
            )
            .expect("run round")
    }

    fn terminal(executions: &[ToolExecution]) -> Vec<ToolExecutionTransition> {
        executions
            .iter()
            .map(|execution| ToolExecutionTransition {
                id: execution.id,
                expected_revision: execution.revision,
                next: ToolExecutionStatus::Succeeded,
                output: Some(ToolOutput {
                    value: json!({"created": execution.ordinal}),
                    is_error: false,
                }),
                failure: None,
            })
            .collect()
    }

    #[test]
    fn memory_change_and_terminal_outputs_commit_together() {
        let database = Database::open_in_memory().expect("database");
        let space_id = MemorySpaceId::new();
        database
            .create(MemorySpaceSnapshot {
                id: space_id,
                revision: Revision::INITIAL,
                items: vec![],
            })
            .expect("space");
        let running = running_round(&database);
        let result = database
            .commit_dynamic_memory_round(
                DynamicMemoryRoundCommit {
                    space_id,
                    expected_memory_revision: Some(Revision::INITIAL),
                    change: Some(MemoryChangeSet {
                        space_id,
                        expected_revision: Revision::INITIAL,
                        items: vec![memory(MemoryId::new(), "committed")],
                    }),
                    execution_transitions: terminal(&running),
                },
                TimestampMillis::new(4),
            )
            .expect("commit round");
        assert_eq!(result.snapshot.revision, Revision::new(2));
        assert!(
            result
                .executions
                .iter()
                .all(|execution| execution.status == ToolExecutionStatus::Succeeded)
        );
    }

    #[test]
    fn stale_terminal_transition_rolls_back_memory_change_and_siblings() {
        let database = Database::open_in_memory().expect("database");
        let space_id = MemorySpaceId::new();
        let original = database
            .create(MemorySpaceSnapshot {
                id: space_id,
                revision: Revision::INITIAL,
                items: vec![],
            })
            .expect("space");
        let running = running_round(&database);
        let mut transitions = terminal(&running);
        transitions[1].expected_revision = Revision::INITIAL;
        assert!(
            database
                .commit_dynamic_memory_round(
                    DynamicMemoryRoundCommit {
                        space_id,
                        expected_memory_revision: Some(Revision::INITIAL),
                        change: Some(MemoryChangeSet {
                            space_id,
                            expected_revision: Revision::INITIAL,
                            items: vec![memory(MemoryId::new(), "must roll back")],
                        }),
                        execution_transitions: transitions,
                    },
                    TimestampMillis::new(4),
                )
                .is_err()
        );
        assert_eq!(database.get(space_id).expect("space"), Some(original));
        assert_eq!(
            database
                .list_tool_executions(
                    running[0].conversation_id,
                    running[0].turn_id,
                    running[0].attempt_id,
                )
                .expect("executions"),
            running
        );
    }

    #[test]
    fn no_op_settlement_still_checks_planned_memory_revision() {
        let database = Database::open_in_memory().expect("database");
        let space_id = MemorySpaceId::new();
        database
            .create(MemorySpaceSnapshot {
                id: space_id,
                revision: Revision::INITIAL,
                items: vec![],
            })
            .expect("space");
        let running = running_round(&database);
        database
            .connection()
            .expect("connection")
            .execute(
                "UPDATE memory_spaces SET revision = 2 WHERE id = ?1",
                [space_id.to_string()],
            )
            .expect("advance memory");
        assert!(matches!(
            database.commit_dynamic_memory_round(
                DynamicMemoryRoundCommit {
                    space_id,
                    expected_memory_revision: Some(Revision::INITIAL),
                    change: None,
                    execution_transitions: terminal(&running),
                },
                TimestampMillis::new(4),
            ),
            Err(lettuce_memory::DynamicMemoryRoundCommitError::Memory(
                lettuce_memory::MemoryRepositoryError::Conflict
            ))
        ));
        assert!(running.iter().all(|execution| {
            database
                .get_tool_execution(execution.id)
                .is_ok_and(|stored| stored.status == ToolExecutionStatus::Running)
        }));
    }
}
