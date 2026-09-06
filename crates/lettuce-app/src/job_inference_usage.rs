use lettuce_conversations::{
    InferenceOutcome, InferencePort, InferenceRequest, PortError, ProviderReplayArtifactPort,
};
use lettuce_types::{JobId, TimestampMillis, UsageEventId};
use lettuce_usage::{JobInferenceUsage, JobInferenceUsageResult, JobUsageLedger};

#[derive(Debug)]
pub(crate) enum JobInferenceError {
    Evidence,
    Provider(PortError),
}

impl From<JobInferenceError> for PortError {
    fn from(error: JobInferenceError) -> Self {
        match error {
            JobInferenceError::Evidence => Self::Unavailable,
            JobInferenceError::Provider(error) => error,
        }
    }
}

pub(crate) async fn run_job_inference<
    R: JobUsageLedger + ProviderReplayArtifactPort + ?Sized,
    I: InferencePort + ?Sized,
>(
    repository: &R,
    inference: &I,
    job_id: JobId,
    request: InferenceRequest,
    now: TimestampMillis,
) -> Result<InferenceOutcome, JobInferenceError> {
    let profile = &request.profile.chat_profile;
    let id = UsageEventId::new();
    repository
        .admit_job_usage(JobInferenceUsage {
            id,
            job_id,
            logical_attempt_id: request.attempt_id,
            model_profile_id: profile.model_profile_id,
            model_revision: profile.model_revision,
            provider_account_id: profile.provider_account_id,
            provider_account_revision: profile.provider_account_revision,
            admitted_at: now,
            result: None,
        })
        .map_err(|_| JobInferenceError::Evidence)?;
    let outcome = inference.run(request).await;
    let result = match &outcome {
        Ok(outcome) => JobInferenceUsageResult::Response {
            usage: outcome.usage.clone(),
            provider_response_id: outcome.provider_response_id.clone(),
        },
        Err(PortError::Cancelled) => JobInferenceUsageResult::Cancelled,
        Err(_) => JobInferenceUsageResult::InferenceFailed,
    };
    if repository.settle_job_usage(id, result).is_err() {
        if let Ok(outcome) = &outcome {
            crate::cleanup_outcome_replays(repository, outcome)
                .map_err(|_| JobInferenceError::Evidence)?;
        }
        return Err(JobInferenceError::Evidence);
    }
    outcome.map_err(JobInferenceError::Provider)
}
