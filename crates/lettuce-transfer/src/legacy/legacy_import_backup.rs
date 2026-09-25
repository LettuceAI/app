use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BackupSqlValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
}

pub type BackupSqlRow = BTreeMap<String, BackupSqlValue>;

/// Legacy import audit evidence: runs with their sealed assignments, skips,
/// completions and results, plus the imported legacy usage records.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyImportBackup {
    #[serde(default)]
    pub runs: Vec<BackupLegacyImportRun>,
    #[serde(default)]
    pub usage_records: Vec<BackupSqlRow>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupLegacyImportRun {
    pub run: BackupSqlRow,
    pub assignments: Vec<BackupSqlRow>,
    pub skips: Vec<BackupSqlRow>,
    pub secret_completions: Vec<BackupSqlRow>,
    pub media_completions: Vec<BackupSqlRow>,
    pub stage_results: Vec<BackupSqlRow>,
    pub results: Option<BackupSqlRow>,
    pub provider_model_results: Option<BackupSqlRow>,
    pub asr_results: Option<BackupSqlRow>,
}

pub fn backup_sql_text<'a>(row: &'a BackupSqlRow, column: &str) -> Option<&'a str> {
    match row.get(column) {
        Some(BackupSqlValue::Text(value)) => Some(value),
        _ => None,
    }
}

impl LegacyImportBackup {
    pub fn canonicalize_and_validate(&mut self) -> Result<(), LegacyImportBackupError> {
        let mut run_ids = BTreeSet::new();
        for entry in &self.runs {
            let run_id =
                backup_sql_text(&entry.run, "id").ok_or(LegacyImportBackupError::InvalidData)?;
            if !run_ids.insert(run_id) {
                return Err(LegacyImportBackupError::InvalidData);
            }
            for row in entry
                .assignments
                .iter()
                .chain(&entry.skips)
                .chain(&entry.secret_completions)
                .chain(&entry.media_completions)
                .chain(&entry.stage_results)
                .chain(&entry.results)
                .chain(&entry.provider_model_results)
                .chain(&entry.asr_results)
            {
                if backup_sql_text(row, "run_id") != Some(run_id) {
                    return Err(LegacyImportBackupError::InvalidData);
                }
            }
        }
        if self.usage_records.iter().any(|row| {
            backup_sql_text(row, "run_id").is_none() || backup_sql_text(row, "source_id").is_none()
        }) {
            return Err(LegacyImportBackupError::InvalidData);
        }
        self.runs.sort_by(|left, right| {
            backup_sql_text(&left.run, "id").cmp(&backup_sql_text(&right.run, "id"))
        });
        self.usage_records.sort_by(|left, right| {
            (
                backup_sql_text(left, "run_id"),
                backup_sql_text(left, "source_id"),
            )
                .cmp(&(
                    backup_sql_text(right, "run_id"),
                    backup_sql_text(right, "source_id"),
                ))
        });
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LegacyImportBackupError {
    #[error("legacy import backup exceeds its limit")]
    LimitExceeded,
    #[error("legacy import backup is invalid")]
    InvalidData,
}
