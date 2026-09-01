use std::collections::{HashMap, HashSet};

use lettuce_embeddings::MemoryEmbeddingRepository;
use lettuce_jobs::{Claim, handle::JobHandle};
use lettuce_memory::{
    DynamicMemoryAttemptStatus, DynamicMemoryBackgroundRoundCommit,
    DynamicMemoryBackgroundRoundSettlement, DynamicMemoryInferenceRound,
    DynamicMemoryRunRepository, DynamicMemoryRunRepositoryError, MemoryPolicy, MemoryRepository,
    MemoryRepositoryError, MemoryToolArguments, MemoryToolCall, MemoryToolError, MemoryToolReducer,
};
use lettuce_types::{DynamicMemoryAttemptId, DynamicMemoryRunId, MemoryId, TimestampMillis};

use crate::{
    DynamicMemoryCreatePreparer, DynamicMemoryPreparationError, MemoryCreateSeed,
    MemoryEmbeddingEngine, PreparedMemoryCreate, persist_created_projections,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompanionMemoryRoundExecutionResult {
    pub settlement: DynamicMemoryBackgroundRoundSettlement,
    pub replayed: bool,
    pub projection_repairs_pending: Vec<MemoryId>,
}

#[derive(Debug)]
pub struct CompanionMemoryRoundExecutor<'a, E: ?Sized, R: ?Sized> {
    engine: &'a E,
    repository: &'a R,
}

impl<'a, E: MemoryEmbeddingEngine + ?Sized, R: ?Sized> CompanionMemoryRoundExecutor<'a, E, R> {
    #[must_use]
    pub const fn new(engine: &'a E, repository: &'a R) -> Self {
        Self { engine, repository }
    }
}

impl<
    E: MemoryEmbeddingEngine + ?Sized,
    R: DynamicMemoryRunRepository + MemoryRepository + MemoryEmbeddingRepository + ?Sized,
> CompanionMemoryRoundExecutor<'_, E, R>
{
    #[allow(clippy::too_many_arguments)]
    pub fn execute_round(
        &self,
        run_id: DynamicMemoryRunId,
        attempt_id: DynamicMemoryAttemptId,
        round_ordinal: u8,
        policy: &MemoryPolicy,
        seeds: &[MemoryCreateSeed],
        duplicate_threshold: lettuce_memory::Score,
        claim: &Claim,
        handle: &JobHandle,
        now: TimestampMillis,
    ) -> Result<CompanionMemoryRoundExecutionResult, CompanionMemoryRoundExecutionError> {
        let run = self.repository.load_dynamic_memory_run(run_id)?;
        let attempt = self.repository.load_dynamic_memory_attempt(attempt_id)?;
        if attempt.run_id != run.id || attempt.job_id != handle.id() {
            return Err(CompanionMemoryRoundExecutionError::InvalidOwnership);
        }
        if let Some(settlement) = self.repository.load_dynamic_memory_round_settlement(
            run_id,
            attempt_id,
            round_ordinal,
        )? {
            return Ok(CompanionMemoryRoundExecutionResult {
                settlement,
                replayed: true,
                projection_repairs_pending: Vec::new(),
            });
        }
        if handle.cancellation_token().is_cancelled() {
            self.cancel(&attempt, now)?;
            return Err(CompanionMemoryRoundExecutionError::Cancelled);
        }
        let round = self
            .repository
            .list_dynamic_memory_inference_rounds(run_id, attempt_id)?
            .get(usize::from(round_ordinal))
            .cloned()
            .ok_or(CompanionMemoryRoundExecutionError::InvalidOwnership)?;
        if attempt.status != DynamicMemoryAttemptStatus::Processing
            || round.run_id != run.id
            || round.attempt_id != attempt.id
            || round.ordinal != round_ordinal
            || round.calls.is_empty()
        {
            return Err(CompanionMemoryRoundExecutionError::InvalidOwnership);
        }
        let snapshot = self
            .repository
            .get(run.space_id)?
            .ok_or(CompanionMemoryRoundExecutionError::InvalidOwnership)?;
        let prepared = DynamicMemoryCreatePreparer::new(self.engine, self.repository)
            .prepare_background_calls(
                run.space_id,
                &round.calls,
                seeds,
                duplicate_threshold,
                claim,
                handle,
            );
        let prepared = match prepared {
            Ok(prepared) => prepared,
            Err(DynamicMemoryPreparationError::Cancelled) => {
                self.cancel(&attempt, now)?;
                return Err(CompanionMemoryRoundExecutionError::Cancelled);
            }
            Err(error) => return Err(error.into()),
        };
        if handle.cancellation_token().is_cancelled() {
            self.cancel(&attempt, now)?;
            return Err(CompanionMemoryRoundExecutionError::Cancelled);
        }
        let calls = prepare_background_calls(&round, &prepared)?;
        let reduction = MemoryToolReducer.reduce(&snapshot, policy, &calls)?;
        let settlement = self.repository.commit_dynamic_memory_background_round(
            DynamicMemoryBackgroundRoundCommit {
                run_id,
                attempt_id,
                round_ordinal,
                space_id: run.space_id,
                expected_memory_revision: snapshot.revision,
                change: reduction.change.clone(),
                results: reduction.results.clone(),
            },
            now,
        )?;
        let stored = self
            .repository
            .get(run.space_id)?
            .ok_or(CompanionMemoryRoundExecutionError::InvalidOwnership)?;
        let projection_repairs_pending =
            persist_created_projections(self.repository, &stored, &reduction, &prepared);
        Ok(CompanionMemoryRoundExecutionResult {
            settlement,
            replayed: false,
            projection_repairs_pending,
        })
    }

    fn cancel(
        &self,
        attempt: &lettuce_memory::DynamicMemoryAttempt,
        now: TimestampMillis,
    ) -> Result<(), CompanionMemoryRoundExecutionError> {
        self.repository.transition_dynamic_memory_attempt(
            attempt.id,
            attempt.revision,
            DynamicMemoryAttemptStatus::Cancelled,
            None,
            now,
        )?;
        Ok(())
    }
}

fn prepare_background_calls(
    round: &DynamicMemoryInferenceRound,
    prepared_creates: &[PreparedMemoryCreate],
) -> Result<Vec<MemoryToolCall>, CompanionMemoryRoundExecutionError> {
    let mut preparations = prepared_creates
        .iter()
        .map(|prepared| (prepared.execution_id, prepared.preparation.clone()))
        .collect::<HashMap<_, _>>();
    if preparations.len() != prepared_creates.len() {
        return Err(CompanionMemoryRoundExecutionError::InvalidPreparation);
    }
    let mut ids = HashSet::with_capacity(round.calls.len());
    let mut results = Vec::with_capacity(round.calls.len());
    for call in &round.calls {
        if !ids.insert(call.id) {
            return Err(CompanionMemoryRoundExecutionError::InvalidOwnership);
        }
        let arguments = MemoryToolArguments::parse(&call.call.name, &call.call.arguments)?;
        let create = if matches!(arguments, MemoryToolArguments::CreateMemory { .. }) {
            Some(
                preparations
                    .remove(&call.id)
                    .ok_or(CompanionMemoryRoundExecutionError::InvalidPreparation)?,
            )
        } else {
            None
        };
        results.push(MemoryToolCall {
            execution_id: call.id,
            arguments,
            create,
        });
    }
    if !preparations.is_empty() {
        return Err(CompanionMemoryRoundExecutionError::InvalidPreparation);
    }
    Ok(results)
}

#[derive(Debug, thiserror::Error)]
pub enum CompanionMemoryRoundExecutionError {
    #[error("background memory round ownership is invalid")]
    InvalidOwnership,
    #[error("background memory create preparation is invalid")]
    InvalidPreparation,
    #[error("background memory round was cancelled")]
    Cancelled,
    #[error("background memory run persistence failed: {0}")]
    Run(#[from] DynamicMemoryRunRepositoryError),
    #[error("background memory persistence failed: {0}")]
    Memory(#[from] MemoryRepositoryError),
    #[error("background memory preparation failed: {0}")]
    Preparation(#[from] DynamicMemoryPreparationError),
    #[error("background memory tool reduction failed: {0}")]
    Tool(#[from] MemoryToolError),
}

#[cfg(test)]
mod tests {
    use lettuce_conversations::ProposedToolCall;
    use lettuce_memory::{
        CreateMemoryPreparation, DynamicMemoryRoundFinishReason, DynamicMemoryToolCallEvidence,
        MemoryCategory, MemoryPolicy, MemorySpaceSnapshot, MemoryToolOutcome, MemoryToolReducer,
        NewDynamicMemoryToolCall, Score,
    };
    use lettuce_types::{
        DynamicMemoryAttemptId, DynamicMemoryRunId, MemoryId, MemorySpaceId, MessageId, Revision,
        TimestampMillis, ToolExecutionId,
    };
    use serde_json::json;

    use super::*;

    fn evidence(
        run_id: DynamicMemoryRunId,
        attempt_id: DynamicMemoryAttemptId,
        ordinal: u16,
        name: &str,
        arguments: serde_json::Value,
    ) -> DynamicMemoryToolCallEvidence {
        let admitted_at = TimestampMillis::new(1);
        let call = NewDynamicMemoryToolCall {
            id: ToolExecutionId::new(),
            definition_version: 1,
            call: ProposedToolCall {
                provider_call_id: Some(format!("call-{ordinal}")),
                name: name.into(),
                arguments,
                raw_arguments: None,
                provider_replay: None,
            },
        };
        DynamicMemoryToolCallEvidence {
            id: call.id,
            run_id,
            attempt_id,
            round_ordinal: 0,
            ordinal,
            definition_version: call.definition_version,
            call: call.call,
            admitted_at,
        }
    }

    #[test]
    fn background_adapter_preserves_order_done_and_source_attribution() {
        let run_id = DynamicMemoryRunId::new();
        let attempt_id = DynamicMemoryAttemptId::new();
        let source_id = MessageId::new();
        let memory_id = MemoryId::new();
        let calls = vec![
            evidence(
                run_id,
                attempt_id,
                0,
                "create_memory",
                json!({
                    "text":"The user prefers tea",
                    "category":"preference",
                    "source_message_id":source_id.to_string()
                }),
            ),
            evidence(run_id, attempt_id, 1, "done", json!({"summary":"complete"})),
            evidence(
                run_id,
                attempt_id,
                2,
                "pin_memory",
                json!({"id":memory_id.to_string()}),
            ),
        ];
        let round = DynamicMemoryInferenceRound {
            run_id,
            attempt_id,
            ordinal: 0,
            first_call_ordinal: 0,
            request_context: lettuce_conversations::ProviderNeutralContext {
                messages: Vec::new(),
                attributions: Default::default(),
                budget: Default::default(),
            },
            parts: Vec::new(),
            provider_replay: None,
            usage: None,
            finish_reason: DynamicMemoryRoundFinishReason::Stop,
            provider_request_id: None,
            calls: calls.clone(),
            admitted_at: TimestampMillis::new(1),
        };
        let prepared = vec![PreparedMemoryCreate {
            execution_id: calls[0].id,
            preparation: CreateMemoryPreparation {
                id: memory_id,
                token_count: 4,
                created_at: TimestampMillis::new(1),
                semantic_duplicate: None,
            },
            projection: None,
        }];
        let reduced = MemoryToolReducer
            .reduce(
                &MemorySpaceSnapshot {
                    id: MemorySpaceId::new(),
                    revision: Revision::INITIAL,
                    items: Vec::new(),
                },
                &MemoryPolicy {
                    max_entries: 100,
                    hot_token_budget: 1_000,
                    cold_threshold: Score::ZERO,
                    delete_confidence_default: Score::FULL,
                    max_hard_delete_ratio_per_cycle: Score::FULL,
                },
                &prepare_background_calls(&round, &prepared).expect("calls"),
            )
            .expect("reduction");
        assert!(matches!(
            reduced.results[0].outcome,
            MemoryToolOutcome::Created { id } if id == memory_id
        ));
        assert!(matches!(
            reduced.results[1].outcome,
            MemoryToolOutcome::Done { .. }
        ));
        assert_eq!(
            reduced.results[2].outcome,
            MemoryToolOutcome::StoppedAfterDone
        );
        let created = reduced
            .change
            .expect("change")
            .items
            .into_iter()
            .find(|item| item.category == MemoryCategory::Preference)
            .expect("created memory");
        assert_eq!(created.source_message_id, Some(source_id));
    }
}
