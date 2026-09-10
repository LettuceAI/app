use std::collections::{BTreeMap, BTreeSet};

use lettuce_jobs::{InMemoryJobStore, StoredJobRecord};
use lettuce_types::{JobId, UsageEventId};
use serde::{Deserialize, Serialize};

pub const JOB_BACKUP_VERSION: u32 = 1;
pub const MAX_BACKUP_JOBS: usize = 200_000;
pub const MAX_BACKUP_JOB_EVENTS: usize = 2_000_000;
pub const MAX_BACKUP_JOB_INFERENCE_EVENTS: usize = 1_000_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobBackup {
    pub version: u32,
    pub jobs: Vec<StoredJobRecord>,
    pub inference: Vec<BackupJobInference>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupJobInference {
    pub evidence: lettuce_usage::JobInferenceUsage,
    pub cost_basis: Option<lettuce_usage::UsageCostBasis>,
}

impl JobBackup {
    pub fn canonicalize_and_validate(&mut self) -> Result<(), JobBackupError> {
        if self.version != JOB_BACKUP_VERSION || self.jobs.len() > MAX_BACKUP_JOBS {
            return Err(if self.jobs.len() > MAX_BACKUP_JOBS {
                JobBackupError::LimitExceeded
            } else {
                JobBackupError::InvalidData
            });
        }
        self.jobs
            .sort_by_key(|job| (job.snapshot.created_at, job.snapshot.id));
        InMemoryJobStore::restore(self.jobs.clone()).map_err(|_| JobBackupError::InvalidData)?;
        let event_count = self.jobs.iter().try_fold(0_usize, |count, job| {
            count
                .checked_add(job.events.len())
                .ok_or(JobBackupError::LimitExceeded)
        })?;
        self.inference
            .sort_by_key(|entry| (entry.evidence.admitted_at, entry.evidence.id));
        let mut evidence_ids = BTreeSet::new();
        for entry in &self.inference {
            if entry.evidence.model_revision.get() == 0
                || entry.evidence.provider_account_revision.get() == 0
                || !evidence_ids.insert(entry.evidence.id)
                || entry
                    .cost_basis
                    .as_ref()
                    .is_some_and(|basis| basis.calculate_job(&entry.evidence).is_err())
            {
                return Err(JobBackupError::InvalidData);
            }
        }
        if event_count > MAX_BACKUP_JOB_EVENTS
            || self.inference.len() > MAX_BACKUP_JOB_INFERENCE_EVENTS
        {
            return Err(JobBackupError::LimitExceeded);
        }
        Ok(())
    }

    pub(crate) fn inference_owners(&self) -> BTreeMap<UsageEventId, JobId> {
        self.inference
            .iter()
            .map(|entry| (entry.evidence.id, entry.evidence.job_id))
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum JobBackupError {
    #[error("job backup exceeds its limit")]
    LimitExceeded,
    #[error("job backup is invalid")]
    InvalidData,
}

#[cfg(test)]
mod tests {
    use lettuce_types::{
        GenerationAttemptId, JobId, ModelProfileId, ProviderAccountId, Revision, TimestampMillis,
        UsageEventId,
    };
    use lettuce_usage::{JobInferenceUsage, JobInferenceUsageResult};

    use super::*;

    #[test]
    fn inference_evidence_survives_scheduler_retention() {
        let mut backup = JobBackup {
            version: JOB_BACKUP_VERSION,
            jobs: Vec::new(),
            inference: vec![BackupJobInference {
                evidence: JobInferenceUsage {
                    id: UsageEventId::new(),
                    job_id: JobId::new(),
                    logical_attempt_id: GenerationAttemptId::new(),
                    model_profile_id: ModelProfileId::new(),
                    model_revision: Revision::INITIAL,
                    provider_account_id: ProviderAccountId::new(),
                    provider_account_revision: Revision::INITIAL,
                    admitted_at: TimestampMillis::new(1),
                    result: Some(JobInferenceUsageResult::InferenceFailed),
                },
                cost_basis: None,
            }],
        };
        assert!(backup.canonicalize_and_validate().is_ok());
    }
}
