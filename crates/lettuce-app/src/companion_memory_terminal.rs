use std::collections::HashSet;

use lettuce_companions::{CompanionTurnEffect, CompanionTurnEffectRepository, MAX_GROWTH_MEMORIES};
use lettuce_conversations::{MessageRole, PortError, ProviderFailureKind};
use lettuce_jobs::handle::JobHandle;
use lettuce_memory::{
    DynamicMemoryAttempt, DynamicMemoryAttemptFailureCode, DynamicMemoryAttemptStatus,
    DynamicMemoryRunRepository, DynamicMemoryRunRepositoryError, MemoryItem, MemoryRepository,
    MemoryRepositoryError, MemorySpaceSnapshot,
};
use lettuce_settings::GlobalSettingsStore;
use lettuce_types::{DynamicMemoryAttemptId, DynamicMemoryRunId, TimestampMillis};

use crate::{
    CompanionMemoryContinuationError, CompanionMemoryInferenceError, CompanionMemoryLoopError,
    CompanionMemoryRoundExecutionError, CompanionPostTurnEffectCoordinator,
    CompanionPostTurnEffectError, CompanionPostTurnFailure, CompanionPostTurnMemoryBatch,
};

#[derive(Debug, Clone, PartialEq)]
pub struct CompanionMemoryTerminalResult {
    pub attempt: DynamicMemoryAttempt,
    pub effects: Vec<CompanionTurnEffect>,
    pub fresh_memories: Vec<MemoryItem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompanionMemoryTerminalFailure {
    ProviderUnavailable,
    ProviderRejected,
    EmptyResponse,
    RoundLimit,
    Tool,
    Cancelled,
    Recovery,
}

impl CompanionMemoryTerminalFailure {
    #[must_use]
    pub fn from_inference_error(error: &CompanionMemoryInferenceError) -> Self {
        match error {
            CompanionMemoryInferenceError::Cancelled
            | CompanionMemoryInferenceError::Inference(PortError::Cancelled) => Self::Cancelled,
            CompanionMemoryInferenceError::NoToolCalls
            | CompanionMemoryInferenceError::Inference(PortError::Empty) => Self::EmptyResponse,
            CompanionMemoryInferenceError::Inference(PortError::Unavailable) => {
                Self::ProviderUnavailable
            }
            CompanionMemoryInferenceError::Inference(PortError::Rejected) => Self::ProviderRejected,
            CompanionMemoryInferenceError::Inference(PortError::Provider(failure)) => {
                match failure.kind {
                    ProviderFailureKind::Unavailable => Self::ProviderUnavailable,
                    ProviderFailureKind::CredentialRejected
                    | ProviderFailureKind::RequestRejected => Self::ProviderRejected,
                }
            }
            CompanionMemoryInferenceError::MultipleCandidates
            | CompanionMemoryInferenceError::MixedToolAndContent
            | CompanionMemoryInferenceError::InvalidSignedReplay
            | CompanionMemoryInferenceError::UndeclaredTool => Self::ProviderRejected,
            CompanionMemoryInferenceError::InvalidOwnership
            | CompanionMemoryInferenceError::InvalidPrompt
            | CompanionMemoryInferenceError::InvalidSource
            | CompanionMemoryInferenceError::ContextTooLarge
            | CompanionMemoryInferenceError::Conversation(_)
            | CompanionMemoryInferenceError::Memory(_)
            | CompanionMemoryInferenceError::Run(_)
            | CompanionMemoryInferenceError::Prompt(_)
            | CompanionMemoryInferenceError::ReplayCleanup => Self::Recovery,
        }
    }

    #[must_use]
    pub fn from_loop_error(error: &CompanionMemoryLoopError) -> Self {
        match error {
            CompanionMemoryLoopError::Execution(CompanionMemoryRoundExecutionError::Cancelled)
            | CompanionMemoryLoopError::Continuation(
                CompanionMemoryContinuationError::Cancelled
                | CompanionMemoryContinuationError::Inference(PortError::Cancelled),
            ) => Self::Cancelled,
            CompanionMemoryLoopError::Continuation(
                CompanionMemoryContinuationError::RoundLimit
                | CompanionMemoryContinuationError::CallLimit,
            ) => Self::RoundLimit,
            CompanionMemoryLoopError::Continuation(
                CompanionMemoryContinuationError::Inference(PortError::Empty),
            ) => Self::EmptyResponse,
            CompanionMemoryLoopError::Continuation(
                CompanionMemoryContinuationError::Inference(PortError::Rejected),
            ) => Self::ProviderRejected,
            CompanionMemoryLoopError::Continuation(
                CompanionMemoryContinuationError::Inference(PortError::Unavailable),
            ) => Self::ProviderUnavailable,
            CompanionMemoryLoopError::Continuation(
                CompanionMemoryContinuationError::Inference(PortError::Provider(failure)),
            ) => match failure.kind {
                ProviderFailureKind::Unavailable => Self::ProviderUnavailable,
                ProviderFailureKind::CredentialRejected | ProviderFailureKind::RequestRejected => {
                    Self::ProviderRejected
                }
            },
            CompanionMemoryLoopError::Execution(
                CompanionMemoryRoundExecutionError::InvalidPreparation
                | CompanionMemoryRoundExecutionError::Preparation(_)
                | CompanionMemoryRoundExecutionError::Tool(_),
            ) => Self::Tool,
            CompanionMemoryLoopError::Continuation(
                CompanionMemoryContinuationError::InvalidOutcome(_),
            ) => Self::ProviderRejected,
            CompanionMemoryLoopError::MissingRound
            | CompanionMemoryLoopError::Run(_)
            | CompanionMemoryLoopError::Execution(_)
            | CompanionMemoryLoopError::Continuation(_) => Self::Recovery,
        }
    }

    const fn effect_failure(self) -> CompanionPostTurnFailure {
        match self {
            Self::ProviderUnavailable | Self::ProviderRejected | Self::EmptyResponse => {
                CompanionPostTurnFailure::Provider
            }
            Self::RoundLimit | Self::Tool => CompanionPostTurnFailure::Tool,
            Self::Cancelled => CompanionPostTurnFailure::Cancelled,
            Self::Recovery => CompanionPostTurnFailure::Recovery,
        }
    }

    const fn attempt_terminal(
        self,
    ) -> (
        DynamicMemoryAttemptStatus,
        Option<DynamicMemoryAttemptFailureCode>,
    ) {
        match self {
            Self::ProviderUnavailable => (
                DynamicMemoryAttemptStatus::Failed,
                Some(DynamicMemoryAttemptFailureCode::ProviderUnavailable),
            ),
            Self::ProviderRejected => (
                DynamicMemoryAttemptStatus::Failed,
                Some(DynamicMemoryAttemptFailureCode::ProviderRejected),
            ),
            Self::EmptyResponse => (
                DynamicMemoryAttemptStatus::Failed,
                Some(DynamicMemoryAttemptFailureCode::EmptyResponse),
            ),
            Self::RoundLimit => (
                DynamicMemoryAttemptStatus::Failed,
                Some(DynamicMemoryAttemptFailureCode::RoundLimit),
            ),
            Self::Tool | Self::Recovery => (
                DynamicMemoryAttemptStatus::Failed,
                Some(DynamicMemoryAttemptFailureCode::Internal),
            ),
            Self::Cancelled => (DynamicMemoryAttemptStatus::Cancelled, None),
        }
    }
}

#[derive(Debug)]
pub struct CompanionMemoryTerminalCoordinator<'a, R: ?Sized> {
    repository: &'a R,
}

impl<'a, R: ?Sized> CompanionMemoryTerminalCoordinator<'a, R> {
    #[must_use]
    pub const fn new(repository: &'a R) -> Self {
        Self { repository }
    }
}

impl<
    R: DynamicMemoryRunRepository
        + MemoryRepository
        + CompanionTurnEffectRepository
        + GlobalSettingsStore
        + ?Sized,
> CompanionMemoryTerminalCoordinator<'_, R>
{
    pub fn settle_success(
        &self,
        run_id: DynamicMemoryRunId,
        attempt_id: DynamicMemoryAttemptId,
        batch: &CompanionPostTurnMemoryBatch,
        handle: &JobHandle,
        now: TimestampMillis,
    ) -> Result<CompanionMemoryTerminalResult, CompanionMemoryTerminalError> {
        let (run, attempt) = self.load_owner(run_id, attempt_id, batch, handle)?;
        if !matches!(
            attempt.status,
            DynamicMemoryAttemptStatus::Processing | DynamicMemoryAttemptStatus::Succeeded
        ) {
            return Err(CompanionMemoryTerminalError::InvalidOwnership);
        }
        let after = self
            .repository
            .get(run.space_id)?
            .ok_or(CompanionMemoryTerminalError::InvalidOwnership)?;
        let fresh_memories = fresh_growth_memories(&run.starting_memory, &after);
        let effects = if batch.settle_effects {
            CompanionPostTurnEffectCoordinator::new(self.repository).settle_ready(
                &batch.terminal_effects(),
                &run.starting_memory,
                &after,
                now,
            )?
        } else {
            batch.effects.clone()
        };
        let first_settlement = attempt.status == DynamicMemoryAttemptStatus::Processing;
        let attempt = if first_settlement {
            self.repository.transition_dynamic_memory_attempt(
                attempt.id,
                attempt.revision,
                DynamicMemoryAttemptStatus::Succeeded,
                None,
                now,
            )?
        } else {
            attempt
        };
        if first_settlement && batch.update_dynamic_memory_model_on_success {
            let selected = batch
                .selected_model_profile_id
                .ok_or(CompanionMemoryTerminalError::InvalidOwnership)?;
            match self.repository.load() {
                Ok(settings) => {
                    if let Err(error) = self
                        .repository
                        .set_dynamic_memory_model_profile(Some(selected), settings.revision)
                    {
                        tracing::warn!(
                            ?error,
                            "failed to update dynamic memory model after success"
                        );
                    }
                }
                Err(error) => {
                    tracing::warn!(?error, "failed to load settings after memory success");
                }
            }
        }
        Ok(CompanionMemoryTerminalResult {
            attempt,
            effects,
            fresh_memories,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn settle_failure(
        &self,
        run_id: DynamicMemoryRunId,
        attempt_id: DynamicMemoryAttemptId,
        batch: &CompanionPostTurnMemoryBatch,
        handle: &JobHandle,
        failure: CompanionMemoryTerminalFailure,
        now: TimestampMillis,
    ) -> Result<CompanionMemoryTerminalResult, CompanionMemoryTerminalError> {
        let (_, attempt) = self.load_owner(run_id, attempt_id, batch, handle)?;
        let (status, failure_code) = failure.attempt_terminal();
        if attempt.status != DynamicMemoryAttemptStatus::Processing
            && (attempt.status != status || attempt.failure != failure_code)
        {
            return Err(CompanionMemoryTerminalError::InvalidOwnership);
        }
        let effects = if batch.settle_effects {
            let effect_coordinator = CompanionPostTurnEffectCoordinator::new(self.repository);
            batch
                .effects
                .iter()
                .map(|effect| {
                    effect_coordinator.settle_failed(effect, failure.effect_failure(), now)
                })
                .collect::<Result<Vec<_>, _>>()?
        } else {
            batch.effects.clone()
        };
        let attempt = if attempt.status == DynamicMemoryAttemptStatus::Processing {
            self.repository.transition_dynamic_memory_attempt(
                attempt.id,
                attempt.revision,
                status,
                failure_code,
                now,
            )?
        } else {
            attempt
        };
        Ok(CompanionMemoryTerminalResult {
            attempt,
            effects,
            fresh_memories: Vec::new(),
        })
    }

    fn load_owner(
        &self,
        run_id: DynamicMemoryRunId,
        attempt_id: DynamicMemoryAttemptId,
        batch: &CompanionPostTurnMemoryBatch,
        handle: &JobHandle,
    ) -> Result<
        (lettuce_memory::DynamicMemoryRun, DynamicMemoryAttempt),
        CompanionMemoryTerminalError,
    > {
        let run = self.repository.load_dynamic_memory_run(run_id)?;
        let attempt = self.repository.load_dynamic_memory_attempt(attempt_id)?;
        let expected_sources = batch
            .effects
            .iter()
            .flat_map(|effect| {
                effect
                    .user_message_id
                    .map(|id| (id, MessageRole::User))
                    .into_iter()
                    .chain(std::iter::once((
                        effect.assistant_message_id,
                        MessageRole::Assistant,
                    )))
            })
            .collect::<Vec<_>>();
        if attempt.run_id != run.id
            || attempt.job_id != handle.id()
            || run.conversation_id != batch.conversation_id
            || batch.effects.is_empty()
            || batch
                .effects
                .iter()
                .any(|effect| effect.conversation_id != run.conversation_id)
            || run
                .source_messages
                .iter()
                .map(|source| (source.message_id, source.role))
                .ne(expected_sources)
        {
            return Err(CompanionMemoryTerminalError::InvalidOwnership);
        }
        Ok((run, attempt))
    }
}

fn fresh_growth_memories(
    before: &MemorySpaceSnapshot,
    after: &MemorySpaceSnapshot,
) -> Vec<MemoryItem> {
    let previous = before
        .items
        .iter()
        .map(|memory| memory.id)
        .collect::<HashSet<_>>();
    after
        .items
        .iter()
        .filter(|memory| !previous.contains(&memory.id) && !memory.text.trim().is_empty())
        .take(MAX_GROWTH_MEMORIES)
        .cloned()
        .collect()
}

#[derive(Debug, thiserror::Error)]
pub enum CompanionMemoryTerminalError {
    #[error("background memory terminal ownership is invalid")]
    InvalidOwnership,
    #[error("background memory run persistence failed: {0}")]
    Run(#[from] DynamicMemoryRunRepositoryError),
    #[error("background memory persistence failed: {0}")]
    Memory(#[from] MemoryRepositoryError),
    #[error("background memory effect settlement failed: {0}")]
    Effect(#[from] CompanionPostTurnEffectError),
}

#[cfg(test)]
mod tests {
    use lettuce_memory::{MemoryCategory, Score};
    use lettuce_types::{MemoryId, MemorySpaceId, Revision};

    use super::*;

    fn memory(text: &str) -> MemoryItem {
        MemoryItem {
            id: MemoryId::new(),
            text: text.to_owned(),
            category: MemoryCategory::Other,
            source_message_id: None,
            source_role: None,
            observed_at: None,
            observed_time_precision: None,
            superseded_by: None,
            superseded_at: None,
            supersedes: Vec::new(),
            token_count: 1,
            is_cold: false,
            is_pinned: false,
            importance: Score::ZERO,
            persistence_importance: Score::ZERO,
            prompt_importance: Score::ZERO,
            volatility: Score::ZERO,
            access_count: 0,
            created_at: TimestampMillis::new(1),
            last_accessed_at: TimestampMillis::new(1),
        }
    }

    #[test]
    fn loop_failures_use_existing_terminal_categories() {
        assert_eq!(
            CompanionMemoryTerminalFailure::from_loop_error(
                &CompanionMemoryLoopError::Continuation(
                    CompanionMemoryContinuationError::Inference(PortError::Unavailable),
                ),
            ),
            CompanionMemoryTerminalFailure::ProviderUnavailable
        );
        assert_eq!(
            CompanionMemoryTerminalFailure::from_loop_error(
                &CompanionMemoryLoopError::Continuation(
                    CompanionMemoryContinuationError::RoundLimit,
                ),
            ),
            CompanionMemoryTerminalFailure::RoundLimit
        );
        assert_eq!(
            CompanionMemoryTerminalFailure::from_loop_error(&CompanionMemoryLoopError::Execution(
                CompanionMemoryRoundExecutionError::Cancelled,
            ),),
            CompanionMemoryTerminalFailure::Cancelled
        );
        assert_eq!(
            CompanionMemoryTerminalFailure::from_loop_error(
                &CompanionMemoryLoopError::MissingRound
            ),
            CompanionMemoryTerminalFailure::Recovery
        );
    }

    #[test]
    fn successful_terminal_selects_legacy_bounded_fresh_memory_prefix() {
        let space_id = MemorySpaceId::new();
        let existing = memory("existing");
        let before = MemorySpaceSnapshot {
            id: space_id,
            revision: Revision::INITIAL,
            items: vec![existing.clone()],
        };
        let mut items = vec![existing, memory("   ")];
        items.extend((0..18).map(|index| memory(&format!("new {index}"))));
        let after = MemorySpaceSnapshot {
            id: space_id,
            revision: Revision::new(2),
            items,
        };

        let selected = fresh_growth_memories(&before, &after);

        assert_eq!(selected.len(), MAX_GROWTH_MEMORIES);
        assert_eq!(
            selected.first().map(|item| item.text.as_str()),
            Some("new 0")
        );
        assert_eq!(
            selected.last().map(|item| item.text.as_str()),
            Some("new 15")
        );
    }
}
