use crate::UsageLedgerError;
use lettuce_conversations::InferenceUsage;
use lettuce_types::{
    GenerationAttemptId, JobId, ModelProfileId, ProviderAccountId, Revision, TimestampMillis,
    UsageEventId,
};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct JobInferenceUsage {
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
    Response { usage: Option<InferenceUsage> },
    InferenceFailed,
    Cancelled,
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
