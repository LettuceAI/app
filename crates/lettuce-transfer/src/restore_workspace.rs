use std::{io::Read, path::Path};

use lettuce_conversations::{ProtectedSnapshotRef, ReplayArtifactRef, TrustedArtifactDescriptor};
use lettuce_platform::{ConfinedInstallStore, InstallPreparation, ObjectKey, PlatformError};
use lettuce_types::ContentHash;
use serde::{Deserialize, Serialize};

use crate::{BackupConversationArtifact, BackupMediaObject, ProviderBackupRestorePlan};

pub const BACKUP_RESTORE_STAGING_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupRestoreStagingReceipt {
    pub version: u32,
    pub source_hash: ContentHash,
    pub media: Vec<ContentHash>,
    pub artifacts: Vec<BackupRestoreArtifactReceipt>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "reference", rename_all = "snake_case")]
pub enum BackupRestoreArtifactReceipt {
    Snapshot(ProtectedSnapshotRef),
    Replay(ReplayArtifactRef),
}

#[derive(Debug)]
pub struct BackupRestoreWorkspace {
    files: ConfinedInstallStore,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BackupRestoreWorkspaceError {
    #[error("restore workspace contains conflicting or incomplete content")]
    Conflict,
    #[error("restore workspace receipt could not be encoded")]
    Serialization,
    #[error("restore workspace storage failed: {0}")]
    Platform(PlatformError),
}

impl BackupRestoreWorkspace {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, BackupRestoreWorkspaceError> {
        Ok(Self {
            files: ConfinedInstallStore::open(root)
                .map_err(BackupRestoreWorkspaceError::Platform)?,
        })
    }

    pub fn stage(
        &self,
        plan: &ProviderBackupRestorePlan,
    ) -> Result<BackupRestoreStagingReceipt, BackupRestoreWorkspaceError> {
        let receipt = staging_receipt(plan);
        for object in &plan.media {
            self.stage_media(object)?;
        }
        for artifact in &plan.artifacts {
            self.stage_artifact(artifact)?;
        }
        let bytes =
            serde_json::to_vec(&receipt).map_err(|_| BackupRestoreWorkspaceError::Serialization)?;
        self.stage_bytes(
            ObjectKey::from_segments(["partial", "restore-receipt.json"])
                .map_err(BackupRestoreWorkspaceError::Platform)?,
            ObjectKey::from_segments(["restore", "receipt.json"])
                .map_err(BackupRestoreWorkspaceError::Platform)?,
            &content_hash(&bytes),
            &bytes,
        )?;
        self.verify_receipt(&receipt)?;
        Ok(receipt)
    }

    fn stage_media(&self, object: &BackupMediaObject) -> Result<(), BackupRestoreWorkspaceError> {
        let name = object.content_hash.as_str();
        self.stage_bytes(
            ObjectKey::from_segments(["partial", "media", &format!("{name}.partial")])
                .map_err(BackupRestoreWorkspaceError::Platform)?,
            ObjectKey::from_segments(["media", "blobs", name])
                .map_err(BackupRestoreWorkspaceError::Platform)?,
            &object.content_hash,
            &object.bytes,
        )
    }

    fn stage_artifact(
        &self,
        artifact: &BackupConversationArtifact,
    ) -> Result<(), BackupRestoreWorkspaceError> {
        let (kind, id, digest) = match &artifact.descriptor {
            TrustedArtifactDescriptor::Snapshot(reference) => (
                "snapshots",
                reference.artifact_id.to_string(),
                &reference.digest,
            ),
            TrustedArtifactDescriptor::Replay(reference) => (
                "replays",
                reference.artifact_id.to_string(),
                &reference.digest,
            ),
        };
        self.stage_bytes(
            ObjectKey::from_segments(["partial", "conversation", kind, &format!("{id}.partial")])
                .map_err(BackupRestoreWorkspaceError::Platform)?,
            ObjectKey::from_segments(["conversation", kind, &id])
                .map_err(BackupRestoreWorkspaceError::Platform)?,
            digest,
            &artifact.bytes,
        )
    }

    fn stage_bytes(
        &self,
        partial: ObjectKey,
        target: ObjectKey,
        expected_hash: &ContentHash,
        expected: &[u8],
    ) -> Result<(), BackupRestoreWorkspaceError> {
        let expected_len =
            u64::try_from(expected.len()).map_err(|_| BackupRestoreWorkspaceError::Conflict)?;
        match self
            .files
            .prepare(partial, target.clone(), expected_len.max(1))
            .map_err(BackupRestoreWorkspaceError::Platform)?
        {
            InstallPreparation::Installed(mut file) => {
                let actual_len = file.len();
                verify_reader(&mut file, actual_len, expected_len, expected_hash)?;
            }
            InstallPreparation::Resume(mut file) => {
                let offset = usize::try_from(file.offset())
                    .map_err(|_| BackupRestoreWorkspaceError::Conflict)?;
                let prefix = expected
                    .get(..offset)
                    .ok_or(BackupRestoreWorkspaceError::Conflict)?;
                file.rewind()
                    .map_err(BackupRestoreWorkspaceError::Platform)?;
                let mut staged_prefix = vec![0_u8; offset];
                file.read_exact(&mut staged_prefix)
                    .map_err(|_| BackupRestoreWorkspaceError::Conflict)?;
                if staged_prefix != prefix {
                    return Err(BackupRestoreWorkspaceError::Conflict);
                }
                file.append(&expected[offset..])
                    .map_err(BackupRestoreWorkspaceError::Platform)?;
                file.rewind()
                    .map_err(BackupRestoreWorkspaceError::Platform)?;
                verify_reader(&mut file, expected_len, expected_len, expected_hash)?;
                match file.commit_new() {
                    Ok(_) => {}
                    Err(PlatformError::Conflict) => {
                        let mut installed = self
                            .files
                            .inspect(&target)
                            .map_err(BackupRestoreWorkspaceError::Platform)?
                            .ok_or(BackupRestoreWorkspaceError::Conflict)?;
                        let actual_len = installed.len();
                        verify_reader(&mut installed, actual_len, expected_len, expected_hash)?;
                    }
                    Err(error) => return Err(BackupRestoreWorkspaceError::Platform(error)),
                }
            }
        }
        let mut installed = self
            .files
            .inspect(&target)
            .map_err(BackupRestoreWorkspaceError::Platform)?
            .ok_or(BackupRestoreWorkspaceError::Conflict)?;
        let actual_len = installed.len();
        verify_reader(&mut installed, actual_len, expected_len, expected_hash)
    }

    fn verify_receipt(
        &self,
        expected: &BackupRestoreStagingReceipt,
    ) -> Result<(), BackupRestoreWorkspaceError> {
        let target = ObjectKey::from_segments(["restore", "receipt.json"])
            .map_err(BackupRestoreWorkspaceError::Platform)?;
        let mut installed = self
            .files
            .inspect(&target)
            .map_err(BackupRestoreWorkspaceError::Platform)?
            .ok_or(BackupRestoreWorkspaceError::Conflict)?;
        let mut bytes = Vec::new();
        installed
            .read_to_end(&mut bytes)
            .map_err(|_| BackupRestoreWorkspaceError::Conflict)?;
        let actual: BackupRestoreStagingReceipt =
            serde_json::from_slice(&bytes).map_err(|_| BackupRestoreWorkspaceError::Conflict)?;
        if actual != *expected {
            return Err(BackupRestoreWorkspaceError::Conflict);
        }
        Ok(())
    }
}

fn staging_receipt(plan: &ProviderBackupRestorePlan) -> BackupRestoreStagingReceipt {
    BackupRestoreStagingReceipt {
        version: BACKUP_RESTORE_STAGING_VERSION,
        source_hash: plan.source_hash.clone(),
        media: plan
            .media
            .iter()
            .map(|object| object.content_hash.clone())
            .collect(),
        artifacts: plan
            .artifacts
            .iter()
            .map(|artifact| match &artifact.descriptor {
                TrustedArtifactDescriptor::Snapshot(reference) => {
                    BackupRestoreArtifactReceipt::Snapshot(reference.clone())
                }
                TrustedArtifactDescriptor::Replay(reference) => {
                    BackupRestoreArtifactReceipt::Replay(reference.clone())
                }
            })
            .collect(),
    }
}

fn verify_reader(
    reader: &mut impl Read,
    actual_len: u64,
    expected_len: u64,
    expected_hash: &ContentHash,
) -> Result<(), BackupRestoreWorkspaceError> {
    if actual_len != expected_len {
        return Err(BackupRestoreWorkspaceError::Conflict);
    }
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|_| BackupRestoreWorkspaceError::Conflict)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let actual_hash = ContentHash::parse(hasher.finalize().to_hex().to_string())
        .expect("BLAKE3 produces a valid content hash");
    if actual_hash == *expected_hash {
        return Ok(());
    }
    Err(BackupRestoreWorkspaceError::Conflict)
}

fn content_hash(bytes: &[u8]) -> ContentHash {
    ContentHash::parse(blake3::hash(bytes).to_hex().to_string())
        .expect("BLAKE3 produces a valid content hash")
}
