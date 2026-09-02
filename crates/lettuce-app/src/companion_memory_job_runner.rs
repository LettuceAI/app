use lettuce_companions::{CompanionTurnEffect, CompanionTurnEffectRepository};
use lettuce_context::PromptDocument;
use lettuce_conversations::{
    ConversationReader, InferencePort, ProviderReplayArtifactPort, ResolvedInferenceProfile,
};
use lettuce_embeddings::MemoryEmbeddingRepository;
use lettuce_jobs::{Claim, handle::JobHandle};
use lettuce_memory::{DynamicMemoryInferenceRound, DynamicMemoryRunRepository, MemoryRepository};
use lettuce_types::{RequestId, TimestampMillis};

use crate::{
    CompanionMemoryInferenceCoordinator, CompanionMemoryInferenceError,
    CompanionMemoryLoopCoordinator, CompanionMemoryLoopError, CompanionMemoryLoopResult,
    CompanionMemoryTerminalCoordinator, CompanionMemoryTerminalError,
    CompanionMemoryTerminalFailure, CompanionPostTurnMemoryAdmission,
    CompanionPostTurnMemoryRunCoordinator, CompanionPostTurnMemoryRunDispatch,
    CompanionPostTurnMemoryRunError, MemoryCreateSeed, MemoryEmbeddingEngine,
};

#[derive(Debug, Clone, PartialEq)]
pub struct CompanionMemoryJobRunResult {
    pub dispatch: CompanionPostTurnMemoryRunDispatch,
    pub first_round_replayed: bool,
    pub loop_result: CompanionMemoryLoopResult,
    pub effects: Vec<CompanionTurnEffect>,
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
        + MemoryEmbeddingRepository
        + ProviderReplayArtifactPort
        + CompanionTurnEffectRepository
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
        prompt: &PromptDocument,
        previous_summary: &str,
        policy: &lettuce_memory::MemoryPolicy,
        duplicate_threshold: lettuce_memory::Score,
        claim: &Claim,
        handle: &JobHandle,
        stream_sink: Option<RequestId>,
        now: TimestampMillis,
        seeds_for_round: F,
    ) -> Result<CompanionMemoryJobRunResult, CompanionMemoryJobRunError>
    where
        F: FnMut(&DynamicMemoryInferenceRound) -> Vec<MemoryCreateSeed>,
    {
        let mut dispatch =
            CompanionPostTurnMemoryRunCoordinator::new(self.repository, self.conversations)
                .admit_or_recover(
                    admission,
                    profile,
                    time_awareness_enabled,
                    supersession_enabled,
                    handle,
                    now,
                )?;
        let first = match CompanionMemoryInferenceCoordinator::new(
            self.repository,
            self.conversations,
            self.inference,
        )
        .run_first_round(
            dispatch.run.id,
            dispatch.attempt.id,
            prompt,
            previous_summary,
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
        let loop_result =
            match CompanionMemoryLoopCoordinator::new(self.engine, self.repository, self.inference)
                .run_until_done(
                    dispatch.run.id,
                    dispatch.attempt.id,
                    policy,
                    duplicate_threshold,
                    claim,
                    handle,
                    stream_sink,
                    now,
                    seeds_for_round,
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
            loop_result,
            effects: terminal.effects,
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
}

impl CompanionMemoryJobRunError {
    #[must_use]
    pub fn terminal_failure(&self) -> Option<CompanionMemoryTerminalFailure> {
        match self {
            Self::Inference(error) => {
                Some(CompanionMemoryTerminalFailure::from_inference_error(error))
            }
            Self::Loop(error) => Some(CompanionMemoryTerminalFailure::from_loop_error(error)),
            Self::Admission(_) | Self::Terminal(_) => None,
        }
    }
}
