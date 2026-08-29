use std::collections::{HashMap, HashSet};

use lettuce_conversations::{ToolExecution, ToolExecutionStatus, ToolOutput};
use lettuce_memory::{
    CreateMemoryPreparation, MemoryBatchResult, MemoryPolicy, MemoryRepository,
    MemoryRepositoryError, MemorySpaceSnapshot, MemoryToolArguments, MemoryToolCall,
    MemoryToolError, MemoryToolOutcome, MemoryToolReducer,
};
use lettuce_types::{MemorySpaceId, ToolExecutionId};

const TOOL_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedMemoryCreate {
    pub execution_id: ToolExecutionId,
    pub preparation: CreateMemoryPreparation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicMemoryRoundResult {
    pub snapshot: MemorySpaceSnapshot,
    pub reduction: MemoryBatchResult,
    pub outputs: Vec<(ToolExecutionId, ToolOutput)>,
}

#[derive(Debug)]
pub struct DynamicMemoryHandler<'a, R: MemoryRepository + ?Sized> {
    repository: &'a R,
}

impl<'a, R: MemoryRepository + ?Sized> DynamicMemoryHandler<'a, R> {
    #[must_use]
    pub const fn new(repository: &'a R) -> Self {
        Self { repository }
    }

    pub fn apply_admitted_round(
        &self,
        space_id: MemorySpaceId,
        policy: &MemoryPolicy,
        executions: &[ToolExecution],
        prepared_creates: &[PreparedMemoryCreate],
    ) -> Result<DynamicMemoryRoundResult, DynamicMemoryHandlerError> {
        let snapshot = self
            .repository
            .get(space_id)?
            .ok_or(MemoryRepositoryError::NotFound)?;
        let calls = prepare_calls(executions, prepared_creates)?;
        let reduction = MemoryToolReducer.reduce(&snapshot, policy, &calls)?;
        let outputs = reduction
            .results
            .iter()
            .map(|result| {
                let is_error = matches!(result.outcome, MemoryToolOutcome::Rejected { .. });
                serde_json::to_value(&result.outcome)
                    .map(|value| (result.execution_id, ToolOutput { value, is_error }))
                    .map_err(|_| DynamicMemoryHandlerError::OutputSerialization)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let stored = match &reduction.change {
            Some(change) => self.repository.compare_and_apply(change.clone())?,
            None => snapshot,
        };
        Ok(DynamicMemoryRoundResult {
            snapshot: stored,
            reduction,
            outputs,
        })
    }
}

fn prepare_calls(
    executions: &[ToolExecution],
    prepared_creates: &[PreparedMemoryCreate],
) -> Result<Vec<MemoryToolCall>, DynamicMemoryHandlerError> {
    if executions.is_empty() {
        return Err(DynamicMemoryHandlerError::EmptyRound);
    }
    let owner = (
        executions[0].conversation_id,
        executions[0].turn_id,
        executions[0].attempt_id,
    );
    let mut previous_ordinal = None;
    let mut execution_ids = HashSet::with_capacity(executions.len());
    let mut preparations = prepared_creates
        .iter()
        .map(|prepared| (prepared.execution_id, prepared.preparation.clone()))
        .collect::<HashMap<_, _>>();
    if preparations.len() != prepared_creates.len() {
        return Err(DynamicMemoryHandlerError::DuplicatePreparation);
    }

    let mut calls = Vec::with_capacity(executions.len());
    for execution in executions {
        execution
            .validate()
            .map_err(|_| DynamicMemoryHandlerError::InvalidExecution)?;
        if execution.status != ToolExecutionStatus::Running
            || execution.definition_version != TOOL_VERSION
            || owner
                != (
                    execution.conversation_id,
                    execution.turn_id,
                    execution.attempt_id,
                )
            || previous_ordinal.is_some_and(|previous| execution.ordinal != previous + 1)
            || !execution_ids.insert(execution.id)
        {
            return Err(DynamicMemoryHandlerError::InvalidExecution);
        }
        let arguments =
            MemoryToolArguments::parse(&execution.definition_name, &execution.arguments)?;
        let create = if matches!(arguments, MemoryToolArguments::CreateMemory { .. }) {
            Some(
                preparations
                    .remove(&execution.id)
                    .ok_or(DynamicMemoryHandlerError::MissingPreparation)?,
            )
        } else {
            None
        };
        calls.push(MemoryToolCall {
            execution_id: execution.id,
            arguments,
            create,
        });
        previous_ordinal = Some(execution.ordinal);
    }
    if !preparations.is_empty() {
        return Err(DynamicMemoryHandlerError::UnknownPreparation);
    }
    Ok(calls)
}

#[derive(Debug, thiserror::Error)]
pub enum DynamicMemoryHandlerError {
    #[error("dynamic-memory round is empty")]
    EmptyRound,
    #[error("dynamic-memory execution is invalid or not running")]
    InvalidExecution,
    #[error("dynamic-memory create preparation is duplicated")]
    DuplicatePreparation,
    #[error("dynamic-memory create execution is not prepared")]
    MissingPreparation,
    #[error("dynamic-memory create preparation has no matching execution")]
    UnknownPreparation,
    #[error("dynamic-memory tool call is invalid: {0}")]
    Tool(#[from] MemoryToolError),
    #[error("dynamic-memory repository failed: {0}")]
    Repository(#[from] MemoryRepositoryError),
    #[error("dynamic-memory output serialization failed")]
    OutputSerialization,
}

#[cfg(test)]
mod tests {
    use lettuce_conversations::{
        ProposedToolCall, ToolExecution, ToolExecutionOwner, ToolExecutionStatus,
    };
    use lettuce_memory::{
        CreateMemoryPreparation, MemoryPolicy, MemoryRepository, MemorySpaceSnapshot, Score,
        dynamic_memory_tool_request,
    };
    use lettuce_types::{
        ConversationId, GenerationAttemptId, GenerationTurnId, MemoryId, MemorySpaceId, Revision,
        TimestampMillis, ToolExecutionId,
    };
    use serde_json::{Value, json};

    use super::{DynamicMemoryHandlerError, PreparedMemoryCreate};
    use crate::AppBackend;

    fn score(value: u16) -> Score {
        match Score::from_basis_points(value) {
            Some(score) => score,
            None => panic!("test score must be valid"),
        }
    }

    fn policy() -> MemoryPolicy {
        MemoryPolicy {
            max_entries: 10,
            hot_token_budget: 100,
            cold_threshold: score(2_000),
            delete_confidence_default: score(5_000),
            max_hard_delete_ratio_per_cycle: score(5_000),
        }
    }

    fn running_execution(
        owner: ToolExecutionOwner,
        ordinal: u16,
        name: &str,
        arguments: Value,
    ) -> ToolExecution {
        let definition = dynamic_memory_tool_request()
            .definitions
            .into_iter()
            .find(|definition| definition.name == name);
        let definition = match definition {
            Some(definition) => definition,
            None => panic!("test definition must exist"),
        };
        let requested = ToolExecution::requested(
            ToolExecutionId::new(),
            owner,
            ordinal,
            &definition,
            ProposedToolCall {
                provider_call_id: Some(format!("call-{ordinal}")),
                name: name.to_owned(),
                arguments,
                raw_arguments: None,
                provider_replay: None,
            },
            TimestampMillis::new(1),
        );
        let requested = match requested {
            Ok(execution) => execution,
            Err(error) => panic!("request failed: {error}"),
        };
        let validated = requested.transition(
            ToolExecutionStatus::Validated,
            None,
            None,
            TimestampMillis::new(2),
        );
        let validated = match validated {
            Ok(execution) => execution,
            Err(error) => panic!("validation failed: {error}"),
        };
        match validated.transition(
            ToolExecutionStatus::Running,
            None,
            None,
            TimestampMillis::new(3),
        ) {
            Ok(execution) => execution,
            Err(error) => panic!("start failed: {error}"),
        }
    }

    fn owner() -> ToolExecutionOwner {
        ToolExecutionOwner {
            conversation_id: ConversationId::new(),
            turn_id: GenerationTurnId::new(),
            attempt_id: GenerationAttemptId::new(),
        }
    }

    #[test]
    fn admitted_round_persists_once_and_returns_settlement_outputs() {
        let backend = AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
        let space_id = MemorySpaceId::new();
        MemoryRepository::create(
            backend.database(),
            MemorySpaceSnapshot {
                id: space_id,
                revision: Revision::INITIAL,
                items: vec![],
            },
        )
        .expect("memory space");
        let owner = owner();
        let create = running_execution(
            owner,
            4,
            "create_memory",
            json!({"text": "Mira prefers tea", "category": "preference"}),
        );
        let done = running_execution(owner, 5, "done", json!({"summary": "updated"}));
        let memory_id = MemoryId::new();
        let result = backend
            .dynamic_memory_handler()
            .apply_admitted_round(
                space_id,
                &policy(),
                &[create.clone(), done.clone()],
                &[PreparedMemoryCreate {
                    execution_id: create.id,
                    preparation: CreateMemoryPreparation {
                        id: memory_id,
                        token_count: 4,
                        created_at: TimestampMillis::new(4),
                        semantic_duplicate: None,
                    },
                }],
            )
            .expect("handle round");
        assert_eq!(result.snapshot.revision, Revision::new(2));
        assert_eq!(result.snapshot.items[0].id, memory_id);
        assert_eq!(
            result.outputs.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
            vec![create.id, done.id]
        );
        assert!(result.outputs.iter().all(|(_, output)| !output.is_error));
        assert_eq!(
            MemoryRepository::get(backend.database(), space_id)
                .expect("stored")
                .map(|snapshot| snapshot.revision),
            Some(Revision::new(2))
        );
    }

    #[test]
    fn invalid_or_unprepared_round_does_not_mutate_memory() {
        let backend = AppBackend::open_in_memory(TimestampMillis::new(1)).expect("backend");
        let space_id = MemorySpaceId::new();
        let original = MemoryRepository::create(
            backend.database(),
            MemorySpaceSnapshot {
                id: space_id,
                revision: Revision::INITIAL,
                items: vec![],
            },
        )
        .expect("memory space");
        let execution = running_execution(
            owner(),
            0,
            "create_memory",
            json!({"text": "Mira prefers tea", "category": "preference"}),
        );
        assert!(matches!(
            backend.dynamic_memory_handler().apply_admitted_round(
                space_id,
                &policy(),
                &[execution],
                &[],
            ),
            Err(DynamicMemoryHandlerError::MissingPreparation)
        ));
        assert_eq!(
            MemoryRepository::get(backend.database(), space_id).expect("stored"),
            Some(original)
        );
    }
}
