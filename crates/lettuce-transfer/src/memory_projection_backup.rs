use std::collections::BTreeSet;

use lettuce_types::{MemoryId, MemorySpaceId, TimestampMillis};
use serde::{Deserialize, Serialize};

pub const MEMORY_PROJECTION_BACKUP_VERSION: u32 = 1;
pub const MAX_BACKUP_MEMORY_PROJECTIONS: usize = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryProjectionBackup {
    pub version: u32,
    pub projections: Vec<BackupMemoryProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupMemoryProjection {
    pub space_id: MemorySpaceId,
    pub memory_id: MemoryId,
    pub source_revision: String,
    pub dimensions: u16,
    pub source_text: String,
    pub state: BackupMemoryProjectionState,
    pub updated_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "status", deny_unknown_fields)]
pub enum BackupMemoryProjectionState {
    Ready { vector_le_hex: String },
    RepairNeeded,
}

impl MemoryProjectionBackup {
    pub fn canonicalize_and_validate(
        &mut self,
        memory: &crate::MemoryBackup,
    ) -> Result<(), MemoryProjectionBackupError> {
        if self.version != MEMORY_PROJECTION_BACKUP_VERSION
            || self.projections.len() > MAX_BACKUP_MEMORY_PROJECTIONS
        {
            return Err(MemoryProjectionBackupError::InvalidData);
        }
        self.projections.sort_by(|left, right| {
            (
                left.space_id,
                left.memory_id,
                left.source_revision.as_str(),
                left.dimensions,
            )
                .cmp(&(
                    right.space_id,
                    right.memory_id,
                    right.source_revision.as_str(),
                    right.dimensions,
                ))
        });
        let spaces = memory
            .spaces
            .iter()
            .map(|space| space.snapshot.id)
            .collect::<BTreeSet<_>>();
        let mut identities = BTreeSet::new();
        for projection in &self.projections {
            let identity = (
                projection.space_id,
                projection.memory_id,
                projection.source_revision.as_str(),
                projection.dimensions,
            );
            if !spaces.contains(&projection.space_id)
                || !identities.insert(identity)
                || projection.source_revision.trim().is_empty()
                || projection.source_revision.len() > 128
                || projection.source_text.trim().is_empty()
                || projection.source_text.len() > 16 * 1024
                || !matches!(projection.dimensions, 64 | 128 | 256 | 512 | 768)
                || !valid_state(projection)
            {
                return Err(MemoryProjectionBackupError::InvalidData);
            }
        }
        Ok(())
    }
}

fn valid_state(projection: &BackupMemoryProjection) -> bool {
    match &projection.state {
        BackupMemoryProjectionState::Ready { vector_le_hex } => {
            vector_le_hex.len() == usize::from(projection.dimensions) * 8
                && vector_le_hex.as_bytes().chunks_exact(8).all(|chunk| {
                    let Ok(chunk) = std::str::from_utf8(chunk) else {
                        return false;
                    };
                    let Ok(bits) = u32::from_str_radix(chunk, 16) else {
                        return false;
                    };
                    f32::from_le_bytes(bits.to_be_bytes()).is_finite()
                })
        }
        BackupMemoryProjectionState::RepairNeeded => true,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum MemoryProjectionBackupError {
    #[error("memory projection backup contains invalid data")]
    InvalidData,
}
