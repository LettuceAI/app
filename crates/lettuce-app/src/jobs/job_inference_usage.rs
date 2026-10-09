use lettuce_conversations::{
    InferenceOutcome, InferencePort, InferenceRequest, PortError, ProviderReplayArtifactPort,
};
use lettuce_types::{JobId, TimestampMillis, UsageEventId};
use lettuce_usage::{JobInferenceUsage, JobInferenceUsageResult, JobUsageLedger};

pub(crate) fn inference_usage_snapshot(
    profile: &lettuce_conversations::ResolvedInferenceProfile,
    context: &lettuce_conversations::ProviderNeutralContext,
) -> lettuce_conversations::UsageRecordSnapshot {
    let mut snapshot = context
        .attributions
        .usage_snapshot
        .clone()
        .unwrap_or_default();
    snapshot.model_name = Some(profile.chat_profile.model_display_name.clone());
    snapshot.provider_kind = Some(profile.chat_profile.provider_kind.clone());
    snapshot.provider_label = profile.chat_profile.provider_label.clone();
    snapshot
}

pub(crate) fn settle_usage_snapshot(
    mut snapshot: lettuce_conversations::UsageRecordSnapshot,
    outcome: &InferenceOutcome,
) -> lettuce_conversations::UsageRecordSnapshot {
    snapshot.provider_response_id = outcome.provider_response_id.clone();
    snapshot.finish_reason = outcome
        .provider_finish_reason
        .as_deref()
        .map(|reason| {
            match reason.to_ascii_lowercase().as_str() {
                "stop" | "end_turn" | "stop_sequence" => "stop",
                "length" | "max_tokens" => "length",
                "content_filter" | "safety" | "recitation" | "language" | "prohibited_content"
                | "spii" => "content_filter",
                "tool_calls" | "function_call" | "tool_use" => "tool_calls",
                "aborted" => "aborted",
                "error" => "error",
                _ => "other",
            }
            .into()
        })
        .or_else(|| match outcome.finish_reason {
            lettuce_conversations::FinishReason::Cancelled => Some("aborted".into()),
            lettuce_conversations::FinishReason::Error => Some("error".into()),
            lettuce_conversations::FinishReason::Stop
            | lettuce_conversations::FinishReason::Length => None,
        });
    snapshot
}

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
    Box::pin(run_job_inference_with_id(
        repository,
        inference,
        job_id,
        request,
        now,
        UsageEventId::new(),
    ))
    .await
}

pub(crate) async fn run_job_inference_with_id<
    R: JobUsageLedger + ProviderReplayArtifactPort + ?Sized,
    I: InferencePort + ?Sized,
>(
    repository: &R,
    inference: &I,
    job_id: JobId,
    request: InferenceRequest,
    now: TimestampMillis,
    id: UsageEventId,
) -> Result<InferenceOutcome, JobInferenceError> {
    let profile = &request.profile.chat_profile;
    let snapshot = inference_usage_snapshot(&request.profile, &request.context);
    repository
        .admit_job_usage(JobInferenceUsage {
            snapshot: Some(snapshot.clone()),
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
            snapshot: Some(Box::new(settle_usage_snapshot(snapshot, outcome))),
            usage: outcome.usage.clone(),
            provider_response_id: outcome.provider_response_id.clone(),
        },
        Err(error) => JobInferenceUsageResult::Failure {
            cancelled: *error == PortError::Cancelled,
            snapshot: Box::new(lettuce_conversations::UsageRecordSnapshot {
                finish_reason: Some(
                    if *error == PortError::Cancelled {
                        "aborted"
                    } else {
                        "error"
                    }
                    .into(),
                ),
                error_message: Some(error.to_string()),
                ..snapshot
            }),
        },
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_finish_reason_normalizes_reported_values_and_preserves_unknown() {
        let mut outcome = InferenceOutcome {
            provider_response_id: Some("response".into()),
            candidates: Vec::new(),
            usage: None,
            finish_reason: lettuce_conversations::FinishReason::Stop,
            provider_finish_reason: Some("end_turn".into()),
            provider_request_id: None,
            warning_codes: Vec::new(),
        };
        let original = lettuce_conversations::UsageRecordSnapshot {
            model_name: Some("Recorded model".into()),
            provider_label: Some("Recorded account".into()),
            ..Default::default()
        };
        let snapshot = settle_usage_snapshot(original.clone(), &outcome);
        assert_eq!(snapshot.finish_reason.as_deref(), Some("stop"));
        assert_eq!(snapshot.provider_response_id.as_deref(), Some("response"));
        assert_eq!(snapshot.model_name, original.model_name);
        outcome.provider_finish_reason = Some("custom-provider-finish".into());
        assert_eq!(
            settle_usage_snapshot(original.clone(), &outcome)
                .finish_reason
                .as_deref(),
            Some("other")
        );
        outcome.provider_finish_reason = None;
        assert_eq!(
            settle_usage_snapshot(original.clone(), &outcome).finish_reason,
            None
        );
        outcome.finish_reason = lettuce_conversations::FinishReason::Cancelled;
        assert_eq!(
            settle_usage_snapshot(original, &outcome)
                .finish_reason
                .as_deref(),
            Some("aborted")
        );
    }
}
