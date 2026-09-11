use std::fmt;

use lettuce_types::{ContentHash, OperationId, TimestampMillis};

use crate::{
    BackupRestoreStagingReceipt, LegacyBackupCompatibilityPlan, LegacyBackupRestoreStagingReceipt,
    ProviderBackupRestorePlan,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupRestoreSourceVersion {
    LegacyV1,
    CurrentV2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupRestoreAdmissionStatus {
    Admitted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupRestoreCounts {
    pub document_count: u64,
    pub media_count: u64,
    pub secret_count: u64,
    pub artifact_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupRestoreAdmissionRequest {
    pub id: OperationId,
    pub source_version: BackupRestoreSourceVersion,
    pub source_hash: ContentHash,
    pub plan_fingerprint: ContentHash,
    pub staging_receipt_fingerprint: ContentHash,
    pub counts: BackupRestoreCounts,
    pub admitted_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupRestoreAdmission {
    pub request: BackupRestoreAdmissionRequest,
    pub status: BackupRestoreAdmissionStatus,
    pub replayed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupRestoreAdmissionError {
    InvalidInput,
    Conflict,
    Storage,
}

impl fmt::Display for BackupRestoreAdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput => formatter.write_str("backup restore admission is invalid"),
            Self::Conflict => formatter.write_str("backup restore admission conflicts"),
            Self::Storage => formatter.write_str("backup restore admission storage is unavailable"),
        }
    }
}

impl std::error::Error for BackupRestoreAdmissionError {}

pub trait BackupRestoreAdmissionRepository: Send + Sync {
    fn admit_backup_restore(
        &self,
        request: BackupRestoreAdmissionRequest,
    ) -> Result<BackupRestoreAdmission, BackupRestoreAdmissionError>;
}

pub fn current_backup_restore_admission(
    id: OperationId,
    plan: &ProviderBackupRestorePlan,
    receipt: &BackupRestoreStagingReceipt,
    admitted_at: TimestampMillis,
) -> Result<BackupRestoreAdmissionRequest, BackupRestoreAdmissionError> {
    if *receipt != crate::restore_workspace::staging_receipt(plan) {
        return Err(BackupRestoreAdmissionError::InvalidInput);
    }
    Ok(BackupRestoreAdmissionRequest {
        id,
        source_version: BackupRestoreSourceVersion::CurrentV2,
        source_hash: plan.source_hash.clone(),
        plan_fingerprint: domain_hash(b"lettuce.backup.restore-plan.v2", &plan.source_hash),
        staging_receipt_fingerprint: receipt_hash(receipt)?,
        counts: BackupRestoreCounts {
            document_count: count(crate::PROVIDER_BACKUP_FIXED_SECTIONS)?,
            media_count: count(plan.media.len())?,
            secret_count: count(plan.secrets.len())?,
            artifact_count: count(plan.artifacts.len())?,
        },
        admitted_at,
    })
}

pub fn legacy_backup_restore_admission(
    id: OperationId,
    plan: &LegacyBackupCompatibilityPlan,
    receipt: &LegacyBackupRestoreStagingReceipt,
    admitted_at: TimestampMillis,
) -> Result<BackupRestoreAdmissionRequest, BackupRestoreAdmissionError> {
    plan.verify_seal()
        .map_err(|_| BackupRestoreAdmissionError::InvalidInput)?;
    if receipt.version != crate::BACKUP_RESTORE_STAGING_VERSION
        || receipt.source_hash != plan.inventory().source_hash
        || receipt.compatibility_fingerprint != plan.fingerprint
        || receipt.media.len() != plan.coverage.media.len()
        || !receipt
            .media
            .iter()
            .zip(&plan.coverage.media)
            .all(|(left, right)| {
                left.root == right.root
                    && left.relative_segments == right.relative_segments
                    && left.byte_count == right.byte_count
                    && left.content_hash == right.content_hash
            })
    {
        return Err(BackupRestoreAdmissionError::InvalidInput);
    }
    Ok(BackupRestoreAdmissionRequest {
        id,
        source_version: BackupRestoreSourceVersion::LegacyV1,
        source_hash: plan.inventory().source_hash.clone(),
        plan_fingerprint: plan.fingerprint.clone(),
        staging_receipt_fingerprint: receipt_hash(receipt)?,
        counts: BackupRestoreCounts {
            document_count: plan.coverage.present_document_count,
            media_count: plan.coverage.media_object_count,
            secret_count: count(plan.secret_count())?,
            artifact_count: 0,
        },
        admitted_at,
    })
}

fn count(value: usize) -> Result<u64, BackupRestoreAdmissionError> {
    u64::try_from(value).map_err(|_| BackupRestoreAdmissionError::InvalidInput)
}

fn receipt_hash(value: &impl serde::Serialize) -> Result<ContentHash, BackupRestoreAdmissionError> {
    let bytes = serde_json::to_vec(value).map_err(|_| BackupRestoreAdmissionError::InvalidInput)?;
    Ok(
        ContentHash::parse(blake3::hash(&bytes).to_hex().to_string())
            .expect("BLAKE3 produces a valid content hash"),
    )
}

fn domain_hash(domain: &[u8], hash: &ContentHash) -> ContentHash {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(hash.as_str().as_bytes());
    ContentHash::parse(hasher.finalize().to_hex().to_string())
        .expect("BLAKE3 produces a valid content hash")
}
