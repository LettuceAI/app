use std::collections::{BTreeMap, BTreeSet};

use lettuce_jobs::{InMemoryJobStore, StoredJobRecord};
use lettuce_types::{JobId, UsageEventId};
use serde::{Deserialize, Serialize};

pub const JOB_BACKUP_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobBackup {
    pub version: u32,
    pub jobs: Vec<StoredJobRecord>,
    pub inference: Vec<BackupJobInference>,
    #[serde(default)]
    pub speech_transcriptions: Vec<lettuce_speech::TranscriptionRecord>,
    #[serde(default)]
    pub speech_syntheses: Vec<lettuce_speech::SynthesisRecord>,
    #[serde(default)]
    pub image_generations: Vec<lettuce_image_generation::ImageGenerationRecord>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupJobInference {
    pub evidence: lettuce_usage::JobInferenceUsage,
    pub cost_basis: Option<lettuce_usage::UsageCostBasis>,
}

impl JobBackup {
    pub fn canonicalize_and_validate(&mut self) -> Result<(), JobBackupError> {
        if self.version != JOB_BACKUP_VERSION {
            return Err(JobBackupError::InvalidData);
        }
        self.jobs
            .sort_by_key(|job| (job.snapshot.created_at, job.snapshot.id));
        InMemoryJobStore::restore(self.jobs.clone()).map_err(|_| JobBackupError::InvalidData)?;
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
        self.speech_transcriptions
            .sort_by_key(|record| (record.request.created_at, record.job_id));
        self.speech_syntheses
            .sort_by_key(|record| (record.request.created_at, record.job_id));
        self.image_generations
            .sort_by_key(|record| (record.request.created_at, record.job_id));
        let job_ids = self
            .jobs
            .iter()
            .map(|job| job.snapshot.id)
            .collect::<BTreeSet<_>>();
        let mut bound_jobs = BTreeSet::new();
        for (job_id, valid) in self
            .speech_transcriptions
            .iter()
            .map(|record| (record.job_id, record.validate().is_ok()))
            .chain(
                self.speech_syntheses
                    .iter()
                    .map(|record| (record.job_id, record.validate().is_ok())),
            )
            .chain(
                self.image_generations
                    .iter()
                    .map(|record| (record.job_id, record.validate().is_ok())),
            )
        {
            if !valid || !job_ids.contains(&job_id) || !bound_jobs.insert(job_id) {
                return Err(JobBackupError::InvalidData);
            }
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
            speech_transcriptions: Vec::new(),
            speech_syntheses: Vec::new(),
            image_generations: Vec::new(),
        };
        assert!(backup.canonicalize_and_validate().is_ok());
    }
}
