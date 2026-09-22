//! Playground history rows (generated and imported entries with their
//! images) as a backup section.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{BackupSqlRow, BackupSqlValue, backup_sql_text};

pub const MAX_BACKUP_PLAYGROUND_ROWS: usize = 2_000_000;

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlaygroundHistoryBackup {
    #[serde(default)]
    pub entries: Vec<BackupSqlRow>,
    #[serde(default)]
    pub images: Vec<BackupSqlRow>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PlaygroundHistoryBackupError {
    #[error("playground history backup exceeds its limit")]
    LimitExceeded,
    #[error("playground history backup is invalid")]
    InvalidData,
}

fn ordinal(row: &BackupSqlRow) -> Option<i64> {
    match row.get("ordinal") {
        Some(BackupSqlValue::Integer(value)) => Some(*value),
        _ => None,
    }
}

impl PlaygroundHistoryBackup {
    /// Every image belongs to an entry once per ordinal; rows are sorted by
    /// entry and ordinal.
    pub fn canonicalize_and_validate(&mut self) -> Result<(), PlaygroundHistoryBackupError> {
        if self.entries.len().saturating_add(self.images.len()) > MAX_BACKUP_PLAYGROUND_ROWS {
            return Err(PlaygroundHistoryBackupError::LimitExceeded);
        }
        let mut ids = BTreeSet::new();
        for entry in &self.entries {
            let id =
                backup_sql_text(entry, "id").ok_or(PlaygroundHistoryBackupError::InvalidData)?;
            if !ids.insert(id) {
                return Err(PlaygroundHistoryBackupError::InvalidData);
            }
        }
        let mut images = BTreeSet::new();
        for image in &self.images {
            let key = backup_sql_text(image, "history_id")
                .filter(|history_id| ids.contains(history_id))
                .zip(ordinal(image))
                .ok_or(PlaygroundHistoryBackupError::InvalidData)?;
            if !images.insert(key) {
                return Err(PlaygroundHistoryBackupError::InvalidData);
            }
        }
        self.entries
            .sort_by(|left, right| backup_sql_text(left, "id").cmp(&backup_sql_text(right, "id")));
        self.images.sort_by(|left, right| {
            (backup_sql_text(left, "history_id"), ordinal(left))
                .cmp(&(backup_sql_text(right, "history_id"), ordinal(right)))
        });
        Ok(())
    }
}
