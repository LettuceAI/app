use crate::UsageLedgerError;
use lettuce_conversations::InferenceUsage;
use lettuce_types::{
    GenerationAttemptId, JobId, ModelProfileId, ProviderAccountId, Revision, TimestampMillis,
    UsageEventId,
};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct JobInferenceUsage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<lettuce_conversations::UsageRecordSnapshot>,
    pub id: UsageEventId,
    pub job_id: JobId,
    pub logical_attempt_id: GenerationAttemptId,
    pub model_profile_id: ModelProfileId,
    pub model_revision: Revision,
    pub provider_account_id: ProviderAccountId,
    pub provider_account_revision: Revision,
    pub admitted_at: TimestampMillis,
    pub result: Option<JobInferenceUsageResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum JobInferenceUsageResult {
    Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        snapshot: Option<Box<lettuce_conversations::UsageRecordSnapshot>>,
        usage: Option<InferenceUsage>,
        #[serde(default)]
        provider_response_id: Option<String>,
    },
    InferenceFailed,
    Cancelled,
}

impl JobInferenceUsage {
    pub fn validate_snapshot(&self) -> Result<(), UsageLedgerError> {
        if self.snapshot.as_ref().is_some_and(|snapshot| {
            snapshot.finish_reason.is_some()
                || snapshot.error_message.is_some()
                || snapshot.provider_response_id.is_some()
        }) {
            return Err(UsageLedgerError::Invalid);
        }
        if let Some(result) = &self.result {
            self.validate_result_snapshot(result)?;
        }
        Ok(())
    }

    pub fn validate_result_snapshot(
        &self,
        result: &JobInferenceUsageResult,
    ) -> Result<(), UsageLedgerError> {
        if self.snapshot.is_some()
            && matches!(
                result,
                JobInferenceUsageResult::Response { snapshot: None, .. }
            )
        {
            return Err(UsageLedgerError::Invalid);
        }
        if let JobInferenceUsageResult::Response {
            snapshot: Some(snapshot),
            provider_response_id,
            ..
        } = result
        {
            if &snapshot.provider_response_id != provider_response_id {
                return Err(UsageLedgerError::Invalid);
            }
            if let Some(admitted) = &self.snapshot {
                let mut immutable = snapshot.as_ref().clone();
                immutable.finish_reason = None;
                immutable.error_message = None;
                immutable.provider_response_id = None;
                if &immutable != admitted {
                    return Err(UsageLedgerError::Invalid);
                }
            }
        }
        Ok(())
    }
}

pub trait JobUsageLedger: Send + Sync {
    fn admit_job_usage(&self, record: JobInferenceUsage) -> Result<(), UsageLedgerError>;
    fn settle_job_usage(
        &self,
        id: UsageEventId,
        result: JobInferenceUsageResult,
    ) -> Result<(), UsageLedgerError>;
    fn job_usage(&self, job_id: JobId) -> Result<Vec<JobInferenceUsage>, UsageLedgerError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_cannot_discard_an_admitted_snapshot() {
        let mut record = JobInferenceUsage {
            snapshot: Some(lettuce_conversations::UsageRecordSnapshot::default()),
            id: UsageEventId::new(),
            job_id: JobId::new(),
            logical_attempt_id: GenerationAttemptId::new(),
            model_profile_id: ModelProfileId::new(),
            model_revision: Revision::INITIAL,
            provider_account_id: ProviderAccountId::new(),
            provider_account_revision: Revision::INITIAL,
            admitted_at: TimestampMillis::new(1),
            result: Some(JobInferenceUsageResult::Response {
                snapshot: None,
                usage: None,
                provider_response_id: None,
            }),
        };
        assert_eq!(record.validate_snapshot(), Err(UsageLedgerError::Invalid));
        record.snapshot = None;
        assert_eq!(record.validate_snapshot(), Ok(()));
    }
}
