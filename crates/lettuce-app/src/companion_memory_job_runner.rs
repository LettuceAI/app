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
    CompanionMemorySummaryCoordinator, CompanionMemoryTerminalCoordinator,
    CompanionMemoryTerminalError, CompanionMemoryTerminalFailure, CompanionPostTurnMemoryAdmission,
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
            let executed = crate::CompanionMemoryRoundExecutor::new(self.engine, self.repository)
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
                )
                .map_err(|error| {
                    CompanionMemoryJobRunError::Loop(CompanionMemoryLoopError::Execution(error))
                })?;
            loop_result
                .projection_repairs_pending
                .extend(executed.projection_repairs_pending);
            loop_result.completed_rounds = loop_result.completed_rounds.saturating_add(1);
        }
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
