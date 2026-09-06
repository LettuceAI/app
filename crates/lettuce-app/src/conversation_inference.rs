use lettuce_conversations::{
    ConversationReader, ConversationRepositoryError, GenerationAttemptStatus, GenerationTurnStatus,
    InferenceOutcome, InferencePort, InferenceRequest, PortError, ProviderReplayArtifactPort,
    ValidationError,
};
use lettuce_jobs::handle::JobHandle;
use lettuce_types::{ConversationId, TimestampMillis};
use lettuce_usage::JobUsageLedger;

use crate::job_inference_usage::JobInferenceError;

#[derive(Debug)]
pub struct ConversationInitialInferenceCoordinator<'a, R: ?Sized, I: ?Sized> {
    repository: &'a R,
    inference: &'a I,
}

impl<
    'a,
    R: ConversationReader + JobUsageLedger + ProviderReplayArtifactPort + ?Sized,
    I: InferencePort + ?Sized,
> ConversationInitialInferenceCoordinator<'a, R, I>
{
    #[must_use]
    pub const fn new(repository: &'a R, inference: &'a I) -> Self {
        Self {
            repository,
            inference,
        }
    }

    pub async fn run(
        &self,
        conversation_id: ConversationId,
        handle: &JobHandle,
        request: InferenceRequest,
        now: TimestampMillis,
    ) -> Result<InferenceOutcome, ConversationInitialInferenceError> {
        request.validate()?;
        let turn = self.repository.get_turn(request.turn_id)?;
        let attempt = turn
            .attempts
            .iter()
            .find(|attempt| attempt.id == request.attempt_id)
            .ok_or(ConversationInitialInferenceError::InvalidOwnership)?;
        if turn.conversation_id != conversation_id
            || turn.operation != request.operation
            || turn.status != GenerationTurnStatus::Running
            || attempt.status != GenerationAttemptStatus::Running
            || attempt.job_id != Some(handle.id())
            || request.cancellation != Some(handle.id())
        {
            return Err(ConversationInitialInferenceError::InvalidOwnership);
        }
        if let Some(model) = &turn.resolved_model {
            let profile = &request.profile.chat_profile;
            if model.source_id != profile.model_profile_id
                || model.source_revision != profile.model_revision
                || model.provider_account_id != profile.provider_account_id
                || model.provider_account_revision != profile.provider_account_revision
                || model.provider_protocol != profile.provider_protocol
                || model.external_model_id != profile.external_model_id
            {
                return Err(ConversationInitialInferenceError::InvalidModel);
            }
        }
        if handle.cancellation_token().is_cancelled() {
            return Err(ConversationInitialInferenceError::Cancelled);
        }
        let outcome = crate::job_inference_usage::run_job_inference(
            self.repository,
            self.inference,
            handle.id(),
            request,
            now,
        )
        .await
        .map_err(|error| match error {
            JobInferenceError::Provider(PortError::Cancelled) => {
                ConversationInitialInferenceError::Cancelled
            }
            JobInferenceError::Provider(error) => {
                ConversationInitialInferenceError::Inference(error)
            }
            JobInferenceError::Evidence => {
                ConversationInitialInferenceError::Repository(ConversationRepositoryError::Storage)
            }
        })?;
        if handle.cancellation_token().is_cancelled() {
            cleanup_provider_replays(self.repository, &outcome)?;
            return Err(ConversationInitialInferenceError::Cancelled);
        }
        Ok(outcome)
    }
}

fn cleanup_provider_replays<R: ProviderReplayArtifactPort + ?Sized>(
    repository: &R,
    outcome: &InferenceOutcome,
) -> Result<(), lettuce_conversations::ArtifactError> {
    let mut ids = std::collections::BTreeSet::new();
    for candidate in &outcome.candidates {
        if let Some(replay) = &candidate.provider_replay {
            ids.insert(replay.artifact_id);
        }
        for call in &candidate.tool_calls {
            if let Some(replay) = &call.provider_replay {
                ids.insert(replay.artifact_id);
            }
        }
    }
    for id in ids {
        repository.cleanup_orphan_provider_replay(id)?;
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum ConversationInitialInferenceError {
    #[error("conversation inference ownership is invalid")]
    InvalidOwnership,
    #[error("conversation inference model identity is invalid")]
    InvalidModel,
    #[error("conversation inference was cancelled")]
    Cancelled,
    #[error("conversation inference request is invalid: {0}")]
    Validation(#[from] ValidationError),
    #[error("conversation inference repository failed: {0}")]
    Repository(#[from] ConversationRepositoryError),
    #[error("conversation inference replay cleanup failed: {0}")]
    Artifact(#[from] lettuce_conversations::ArtifactError),
    #[error("conversation inference provider failed: {0}")]
    Inference(PortError),
}
