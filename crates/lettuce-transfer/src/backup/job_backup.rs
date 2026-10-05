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
    #[serde(default)]
    pub local_model_jobs: Vec<BackupLocalModelJob>,
    #[serde(default)]
    pub local_model_operations: Vec<BackupLocalModelOperation>,
    #[serde(default)]
    pub job_details: Vec<BackupJobDetail>,
    #[serde(default)]
    pub job_operations: Vec<BackupJobOperation>,
    #[serde(default)]
    pub api_operation_receipts: Vec<BackupApiOperationReceipt>,
    #[serde(default)]
    pub hugging_face_refusals: Vec<BackupHuggingFaceRefusal>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupApiOperationReceipt {
    pub command: String,
    pub client_operation_id: String,
    pub request_digest: String,
    pub result: serde_json::Value,
    pub committed_at: lettuce_types::TimestampMillis,
}

/// What a local model job (a download, a pull, a folder move) works on,
/// produced and failed with, as the app stored it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupLocalModelJob {
    pub job_id: JobId,
    pub detail: serde_json::Value,
    pub result: Option<serde_json::Value>,
    pub failure: Option<serde_json::Value>,
}

/// A client operation key and the job it started.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupLocalModelOperation {
    pub operation_key: String,
    pub request_digest: String,
    pub job_id: JobId,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupJobDetail {
    pub job_id: JobId,
    pub detail: serde_json::Value,
    pub result: Option<serde_json::Value>,
    pub failure: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupJobOperation {
    pub operation_key: String,
    pub request_digest: String,
    pub job_id: JobId,
}

/// The Hugging Face repository a failed install was refused by.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupHuggingFaceRefusal {
    pub job_id: JobId,
    pub repository: String,
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
        self.api_operation_receipts.sort_by(|a, b| (&a.command, &a.client_operation_id).cmp(&(&b.command, &b.client_operation_id)));
        let mut receipt_keys = BTreeSet::new();
        for receipt in &self.api_operation_receipts {
            if receipt.command.trim().is_empty() || receipt.client_operation_id.trim().is_empty()
                || receipt.request_digest.trim().is_empty()
                || !receipt_keys.insert((&receipt.command, &receipt.client_operation_id)) {
                return Err(JobBackupError::InvalidData);
            }
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
        self.local_model_jobs.sort_by_key(|job| job.job_id);
        self.local_model_operations
            .sort_by(|a, b| a.operation_key.cmp(&b.operation_key));
        self.hugging_face_refusals
            .sort_by_key(|refusal| refusal.job_id);
        let object =
            |value: Option<&serde_json::Value>| value.is_none_or(serde_json::Value::is_object);
        let mut described = BTreeSet::new();
        for job in &self.local_model_jobs {
            if !job_ids.contains(&job.job_id)
                || !described.insert(job.job_id)
                || !job.detail.is_object()
                || !object(job.result.as_ref())
                || !object(job.failure.as_ref())
            {
                return Err(JobBackupError::InvalidData);
            }
        }
        let mut keys = BTreeSet::new();
        for operation in &self.local_model_operations {
            if !job_ids.contains(&operation.job_id)
                || operation.operation_key.trim().is_empty()
                || operation.request_digest.is_empty()
                || !keys.insert(operation.operation_key.as_str())
            {
                return Err(JobBackupError::InvalidData);
            }
        }
        self.job_details.sort_by_key(|job| job.job_id);
        self.job_operations
            .sort_by(|a, b| a.operation_key.cmp(&b.operation_key));
        let mut described = BTreeSet::new();
        for job in &self.job_details {
            if !job_ids.contains(&job.job_id)
                || !described.insert(job.job_id)
                || !job.detail.is_object()
                || !object(job.result.as_ref())
                || !object(job.failure.as_ref())
            {
                return Err(JobBackupError::InvalidData);
            }
        }
        let mut keys = BTreeSet::new();
        for operation in &self.job_operations {
            if !described.contains(&operation.job_id)
                || operation.operation_key.trim().is_empty()
                || operation.request_digest.is_empty()
                || !keys.insert(operation.operation_key.as_str())
            {
                return Err(JobBackupError::InvalidData);
            }
        }
        let mut refused = BTreeSet::new();
        for refusal in &self.hugging_face_refusals {
            if !job_ids.contains(&refusal.job_id)
                || refusal.repository.trim().is_empty()
                || !refused.insert(refusal.job_id)
            {
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
            job_details: Vec::new(),
            job_operations: Vec::new(),
            api_operation_receipts: Vec::new(),
            local_model_jobs: Vec::new(),
            local_model_operations: Vec::new(),
            hugging_face_refusals: Vec::new(),
        };
        assert!(backup.canonicalize_and_validate().is_ok());
        backup
            .local_model_operations
            .push(BackupLocalModelOperation {
                operation_key: "hf_download:op".to_owned(),
                request_digest: "digest".to_owned(),
                job_id: JobId::new(),
            });
        assert_eq!(
            backup.canonicalize_and_validate(),
            Err(JobBackupError::InvalidData),
            "an operation must name a job of the backup"
        );
    }
}
