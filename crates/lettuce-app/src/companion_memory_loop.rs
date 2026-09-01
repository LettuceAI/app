use lettuce_conversations::{InferencePort, ProviderReplayArtifactPort};
use lettuce_embeddings::MemoryEmbeddingRepository;
use lettuce_jobs::{Claim, handle::JobHandle};
use lettuce_memory::{
    DynamicMemoryInferenceRound, DynamicMemoryRunRepository, DynamicMemoryRunRepositoryError,
    MemoryPolicy, MemoryRepository,
};
use lettuce_types::{
    DynamicMemoryAttemptId, DynamicMemoryRunId, MemoryId, RequestId, TimestampMillis,
};

use crate::{
    CompanionMemoryContinuationCoordinator, CompanionMemoryContinuationError,
    CompanionMemoryContinuationResult, CompanionMemoryRoundExecutionError,
    CompanionMemoryRoundExecutor, MemoryCreateSeed, MemoryEmbeddingEngine,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompanionMemoryLoopResult {
    pub summary: Option<String>,
    pub completed_rounds: u8,
    pub projection_repairs_pending: Vec<MemoryId>,
}

#[derive(Debug)]
pub struct CompanionMemoryLoopCoordinator<'a, E: ?Sized, R: ?Sized, I: ?Sized> {
    engine: &'a E,
    repository: &'a R,
    inference: &'a I,
}

impl<'a, E: ?Sized, R: ?Sized, I: ?Sized> CompanionMemoryLoopCoordinator<'a, E, R, I> {
    #[must_use]
    pub const fn new(engine: &'a E, repository: &'a R, inference: &'a I) -> Self {
        Self {
            engine,
            repository,
            inference,
        }
    }
}

impl<
    E: MemoryEmbeddingEngine + ?Sized,
    R: DynamicMemoryRunRepository
        + MemoryRepository
        + MemoryEmbeddingRepository
        + ProviderReplayArtifactPort
        + ?Sized,
    I: InferencePort + ?Sized,
> CompanionMemoryLoopCoordinator<'_, E, R, I>
{
    #[allow(clippy::too_many_arguments)]
    pub async fn run_until_done<F>(
        &self,
        run_id: DynamicMemoryRunId,
        attempt_id: DynamicMemoryAttemptId,
        policy: &MemoryPolicy,
        duplicate_threshold: lettuce_memory::Score,
        claim: &Claim,
        handle: &JobHandle,
        stream_sink: Option<RequestId>,
        now: TimestampMillis,
        mut seeds_for_round: F,
    ) -> Result<CompanionMemoryLoopResult, CompanionMemoryLoopError>
    where
        F: FnMut(&DynamicMemoryInferenceRound) -> Vec<MemoryCreateSeed>,
    {
        let rounds = self
            .repository
            .list_dynamic_memory_inference_rounds(run_id, attempt_id)?;
        let mut round = rounds
            .last()
            .cloned()
            .ok_or(CompanionMemoryLoopError::MissingRound)?;
        let executor = CompanionMemoryRoundExecutor::new(self.engine, self.repository);
        let continuation =
            CompanionMemoryContinuationCoordinator::new(self.repository, self.inference);
        let mut projection_repairs_pending = Vec::new();

        loop {
            let settlement = self.repository.load_dynamic_memory_round_settlement(
                run_id,
                attempt_id,
                round.ordinal,
            )?;
            if settlement.is_none() {
                let seeds = seeds_for_round(&round);
                let executed = executor.execute_round(
                    run_id,
                    attempt_id,
                    round.ordinal,
                    policy,
                    &seeds,
                    duplicate_threshold,
                    claim,
                    handle,
                    now,
                )?;
                projection_repairs_pending.extend(executed.projection_repairs_pending);
            }

            match continuation
                .continue_after_round(run_id, attempt_id, round.ordinal, handle, stream_sink, now)
                .await?
            {
                CompanionMemoryContinuationResult::Done { summary } => {
                    return Ok(CompanionMemoryLoopResult {
                        summary,
                        completed_rounds: round.ordinal.saturating_add(1),
                        projection_repairs_pending,
                    });
                }
                CompanionMemoryContinuationResult::NextRound { round: next, .. } => {
                    round = *next;
                }
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CompanionMemoryLoopError {
    #[error("background memory run has no admitted inference round")]
    MissingRound,
    #[error("background memory run persistence failed: {0}")]
    Run(#[from] DynamicMemoryRunRepositoryError),
    #[error("background memory round execution failed: {0}")]
    Execution(#[from] CompanionMemoryRoundExecutionError),
    #[error("background memory continuation failed: {0}")]
    Continuation(#[from] CompanionMemoryContinuationError),
}
