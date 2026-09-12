use lettuce_companions::{CompanionTurnEffect, CompanionTurnEffectRepository};
use lettuce_context::PromptDocument;
use lettuce_conversations::{
    ConversationReader, InferencePort, ProviderReplayArtifactPort, ResolvedInferenceProfile,
};
use lettuce_embeddings::MemoryEmbeddingRepository;
use lettuce_jobs::{Claim, handle::JobHandle};
use lettuce_memory::{
    DynamicMemoryInferenceRound, DynamicMemoryRunRepository, DynamicMemoryStructuredFallbackFormat,
    MemoryItem, MemoryRepository, MemorySummaryRepository,
};
use lettuce_settings::GlobalSettingsStore;
use lettuce_types::{RequestId, TimestampMillis};
use lettuce_usage::JobUsageLedger;

use crate::{
    CompanionMemoryInferenceCoordinator, CompanionMemoryInferenceError,
    CompanionMemoryLoopCoordinator, CompanionMemoryLoopError, CompanionMemoryLoopResult,
    CompanionMemoryRoundExecutionError, CompanionMemorySummaryCoordinator,
    CompanionMemoryTerminalCoordinator, CompanionMemoryTerminalError,
    CompanionMemoryTerminalFailure, CompanionPostTurnMemoryAdmission,
    CompanionPostTurnMemoryRunCoordinator, CompanionPostTurnMemoryRunDispatch,
    CompanionPostTurnMemoryRunError, MemoryCreateSeed, MemoryEmbeddingEngine,
};

#[derive(Debug, Clone, PartialEq)]
pub struct CompanionMemoryJobRunResult {
    pub dispatch: CompanionPostTurnMemoryRunDispatch,
    pub first_round_replayed: bool,
    pub summary_replayed: bool,
    pub loop_result: CompanionMemoryLoopResult,
    pub effects: Vec<CompanionTurnEffect>,
    pub fresh_memories: Vec<MemoryItem>,
}

#[derive(Debug)]
pub struct CompanionMemoryJobRunner<'a, E: ?Sized, R: ?Sized, C: ?Sized, I: ?Sized> {
    engine: &'a E,
    repository: &'a R,
    conversations: &'a C,
    inference: &'a I,
}

impl<'a, E: ?Sized, R: ?Sized, C: ?Sized, I: ?Sized> CompanionMemoryJobRunner<'a, E, R, C, I> {
    #[must_use]
    pub const fn new(
        engine: &'a E,
        repository: &'a R,
        conversations: &'a C,
        inference: &'a I,
    ) -> Self {
        Self {
            engine,
            repository,
            conversations,
            inference,
        }
    }
}

impl<
    E: MemoryEmbeddingEngine + ?Sized,
    R: DynamicMemoryRunRepository
        + MemoryRepository
        + MemorySummaryRepository
        + MemoryEmbeddingRepository
        + ProviderReplayArtifactPort
        + JobUsageLedger
        + CompanionTurnEffectRepository
        + GlobalSettingsStore
        + crate::runtime_text::RuntimeTextSource
        + ?Sized,
    C: ConversationReader + ?Sized,
    I: InferencePort + ?Sized,
> CompanionMemoryJobRunner<'_, E, R, C, I>
{
    #[allow(clippy::too_many_arguments)]
    pub async fn run<F>(
        &self,
        admission: &CompanionPostTurnMemoryAdmission,
        profile: ResolvedInferenceProfile,
        time_awareness_enabled: bool,
        supersession_enabled: bool,
        structured_fallback_format: DynamicMemoryStructuredFallbackFormat,
        summary_prompt: &PromptDocument,
        memory_prompt: &PromptDocument,
        policy: &lettuce_memory::MemoryPolicy,
        duplicate_threshold: lettuce_memory::Score,
        claim: &Claim,
        handle: &JobHandle,
        stream_sink: Option<RequestId>,
        now: TimestampMillis,
        mut seeds_for_round: F,
    ) -> Result<CompanionMemoryJobRunResult, CompanionMemoryJobRunError>
    where
        F: FnMut(&DynamicMemoryInferenceRound) -> Vec<MemoryCreateSeed>,
    {
        let group = self
            .conversations
            .get(admission.batch.conversation_id)
            .map_err(CompanionMemoryJobRunError::Conversation)?
            .conversation
            .kind
            .is_group();
        let settings = GlobalSettingsStore::load(self.repository)
            .map_err(CompanionMemoryJobRunError::Settings)?
            .settings;
        let loop_policy = crate::CompanionMemoryLoopPolicy::from_settings(if group {
            settings.effective_group_dynamic_memory()
        } else {
            &settings.dynamic_memory
        });
        let mut dispatch =
            CompanionPostTurnMemoryRunCoordinator::new(self.repository, self.conversations)
                .admit_or_recover(
                    admission,
                    profile,
                    time_awareness_enabled,
                    supersession_enabled,
                    structured_fallback_format,
                    handle,
                    now,
                )?;
        let summary = match CompanionMemorySummaryCoordinator::new(
            self.engine,
            self.repository,
            self.conversations,
            self.inference,
        )
        .run(
            dispatch.run.id,
            dispatch.attempt.id,
            summary_prompt,
            handle,
            stream_sink,
            now,
        )
        .await
        {
            Ok(summary) => summary,
            Err(error) => {
                CompanionMemoryTerminalCoordinator::new(self.repository).settle_failure(
                    dispatch.run.id,
                    dispatch.attempt.id,
                    &admission.batch,
                    handle,
                    CompanionMemoryTerminalFailure::from_inference_error(&error),
                    now,
                )?;
                return Err(CompanionMemoryJobRunError::Inference(error));
            }
        };
        let first = match CompanionMemoryInferenceCoordinator::new(
            self.repository,
            self.conversations,
            self.inference,
        )
        .run_first_round(
            dispatch.run.id,
            dispatch.attempt.id,
            memory_prompt,
            &summary.checkpoint.summary.text,
            policy,
            handle,
            stream_sink,
            now,
        )
        .await
        {
            Ok(first) => first,
            Err(error) => {
                CompanionMemoryTerminalCoordinator::new(self.repository).settle_failure(
                    dispatch.run.id,
                    dispatch.attempt.id,
                    &admission.batch,
                    handle,
                    CompanionMemoryTerminalFailure::from_inference_error(&error),
                    now,
                )?;
                return Err(CompanionMemoryJobRunError::Inference(error));
            }
        };
        let mut loop_result =
            match CompanionMemoryLoopCoordinator::new(self.engine, self.repository, self.inference)
                .run_until_done(
                    dispatch.run.id,
                    dispatch.attempt.id,
                    policy,
                    loop_policy,
                    duplicate_threshold,
                    claim,
                    handle,
                    stream_sink,
                    now,
                    &mut seeds_for_round,
                )
                .await
            {
                Ok(result) => result,
                Err(error) => {
                    CompanionMemoryTerminalCoordinator::new(self.repository).settle_failure(
                        dispatch.run.id,
                        dispatch.attempt.id,
                        &admission.batch,
                        handle,
                        CompanionMemoryTerminalFailure::from_loop_error(&error),
                        now,
                    )?;
                    return Err(CompanionMemoryJobRunError::Loop(error));
                }
            };
        let repaired =
            match crate::CompanionMemoryRepairCoordinator::new(self.repository, self.inference)
                .repair_round(&dispatch.run, dispatch.attempt.id, handle, stream_sink, now)
                .await
            {
                Ok(round) => round,
                Err(crate::CompanionMemoryRepairError::Cancelled) => {
                    CompanionMemoryTerminalCoordinator::new(self.repository).settle_failure(
                        dispatch.run.id,
                        dispatch.attempt.id,
                        &admission.batch,
                        handle,
                        CompanionMemoryTerminalFailure::Cancelled,
                        now,
                    )?;
                    return Err(CompanionMemoryJobRunError::Repair(
                        crate::CompanionMemoryRepairError::Cancelled,
                    ));
                }
                Err(error) => {
                    tracing::warn!(
                        run_id = %dispatch.run.id,
                        %error,
                        "memory category repair did not run; keeping the cycle"
                    );
                    None
                }
            };
        if let Some(round) = repaired {
            let seeds = seeds_for_round(&round);
            match crate::CompanionMemoryRoundExecutor::new(self.engine, self.repository)
                .execute_round(
                    dispatch.run.id,
                    dispatch.attempt.id,
                    round.ordinal,
                    policy,
                    &seeds,
                    duplicate_threshold,
                    claim,
                    handle,
                    now,
                ) {
                Ok(executed) => {
                    loop_result
                        .projection_repairs_pending
                        .extend(executed.projection_repairs_pending);
                    loop_result.completed_rounds = loop_result.completed_rounds.saturating_add(1);
                }
                Err(CompanionMemoryRoundExecutionError::Cancelled) => {
                    let error = CompanionMemoryLoopError::Execution(
                        CompanionMemoryRoundExecutionError::Cancelled,
                    );
                    CompanionMemoryTerminalCoordinator::new(self.repository).settle_failure(
                        dispatch.run.id,
                        dispatch.attempt.id,
                        &admission.batch,
                        handle,
                        CompanionMemoryTerminalFailure::from_loop_error(&error),
                        now,
                    )?;
                    return Err(CompanionMemoryJobRunError::Loop(error));
                }
                Err(error) => {
                    tracing::warn!(
                        run_id = %dispatch.run.id,
                        %error,
                        "memory category repair round was not applied; keeping the cycle"
                    );
                }
            }
        }
        self.finish_cycle(&dispatch, &admission.batch, policy, handle, now)?;
        let terminal = CompanionMemoryTerminalCoordinator::new(self.repository).settle_success(
            dispatch.run.id,
            dispatch.attempt.id,
            &admission.batch,
            handle,
            now,
        )?;
        dispatch.attempt = terminal.attempt;
        Ok(CompanionMemoryJobRunResult {
            dispatch,
            first_round_replayed: first.replayed,
            summary_replayed: summary.replayed,
            loop_result,
            effects: terminal.effects,
            fresh_memories: terminal.fresh_memories,
        })
    }

    /// Legacy trimmed to `max_entries` and demoted to the hot budget once per
    /// cycle after the loop and the repair pass; replaying a finished cycle
    /// finds nothing left to change.
    fn finish_cycle(
        &self,
        dispatch: &CompanionPostTurnMemoryRunDispatch,
        batch: &crate::CompanionPostTurnMemoryBatch,
        policy: &lettuce_memory::MemoryPolicy,
        handle: &JobHandle,
        now: TimestampMillis,
    ) -> Result<(), CompanionMemoryJobRunError> {
        let snapshot = self
            .repository
            .get(dispatch.run.space_id)
            .map_err(|error| CompanionMemoryJobRunError::Terminal(error.into()))?
            .ok_or(CompanionMemoryJobRunError::Terminal(
                CompanionMemoryTerminalError::InvalidOwnership,
            ))?;
        let finish = match lettuce_memory::MemoryToolReducer.finish_cycle(&snapshot, policy) {
            Ok(finish) => finish,
            Err(error) => {
                let error = CompanionMemoryLoopError::Execution(
                    CompanionMemoryRoundExecutionError::Tool(error),
                );
                CompanionMemoryTerminalCoordinator::new(self.repository).settle_failure(
                    dispatch.run.id,
                    dispatch.attempt.id,
                    batch,
                    handle,
                    CompanionMemoryTerminalFailure::from_loop_error(&error),
                    now,
                )?;
                return Err(CompanionMemoryJobRunError::Loop(error));
            }
        };
        if let Some(change) = finish.change {
            tracing::info!(
                run_id = %dispatch.run.id,
                trimmed = finish.trimmed_ids.len(),
                demoted = finish.demoted_ids.len(),
                "applied the cycle-end memory capacity and hot budget"
            );
            self.repository
                .compare_and_apply(change)
                .map_err(|error| CompanionMemoryJobRunError::Terminal(error.into()))?;
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CompanionMemoryJobRunError {
    #[error("background memory run admission failed: {0}")]
    Admission(#[from] CompanionPostTurnMemoryRunError),
    #[error("background memory first-round inference failed: {0}")]
    Inference(CompanionMemoryInferenceError),
    #[error("background memory loop failed: {0}")]
    Loop(CompanionMemoryLoopError),
    #[error("background memory terminal settlement failed: {0}")]
    Terminal(#[from] CompanionMemoryTerminalError),
    #[error("background memory conversation lookup failed: {0}")]
    Conversation(lettuce_conversations::ConversationRepositoryError),
    #[error("background memory settings are unavailable: {0}")]
    Settings(lettuce_settings::GlobalSettingsStoreError),
    #[error("background memory category repair failed: {0}")]
    Repair(crate::CompanionMemoryRepairError),
}

impl CompanionMemoryJobRunError {
    #[must_use]
    pub fn terminal_failure(&self) -> Option<CompanionMemoryTerminalFailure> {
        match self {
            Self::Inference(error) => {
                Some(CompanionMemoryTerminalFailure::from_inference_error(error))
            }
            Self::Loop(error) => Some(CompanionMemoryTerminalFailure::from_loop_error(error)),
            Self::Repair(_) => Some(CompanionMemoryTerminalFailure::Cancelled),
            Self::Admission(_) | Self::Terminal(_) | Self::Conversation(_) | Self::Settings(_) => {
                None
            }
        }
    }
}
