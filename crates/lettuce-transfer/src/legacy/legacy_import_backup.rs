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
    #[serde(default)]
    pub preserved_rows: Vec<BackupSqlRow>,
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
                .chain(&entry.preserved_rows)
            {
                if backup_sql_text(row, "run_id") != Some(run_id) {
                    return Err(LegacyImportBackupError::InvalidData);
                }
            }
            let mut paths = BTreeSet::new();
            let mut assets = BTreeSet::new();
            let mut media_assignments = BTreeMap::new();
            for assignment in &entry.assignments {
                if backup_sql_text(assignment, "source_kind") != Some("media") {
                    continue;
                }
                let key = backup_sql_text(assignment, "source_key")
                    .ok_or(LegacyImportBackupError::InvalidData)?;
                let asset = backup_sql_text(assignment, "destination_id")
                    .ok_or(LegacyImportBackupError::InvalidData)?;
                if media_assignments.insert((key, asset), assignment).is_some() {
                    return Err(LegacyImportBackupError::InvalidData);
                }
            }
            for proof in &entry.media_completions {
                let text = |column| {
                    backup_sql_text(proof, column).ok_or(LegacyImportBackupError::InvalidData)
                };
                let path = text("relative_path")?;
                let asset = text("destination_asset_id")?;
                let blob = text("blob_id")?;
                let hash = text("content_hash")?;
                let bytes = match proof.get("byte_len") {
                    Some(BackupSqlValue::Integer(bytes)) if *bytes >= 0 => *bytes,
                    _ => return Err(LegacyImportBackupError::InvalidData),
                };
                let assignment = media_assignments
                    .get(&(path, asset))
                    .ok_or(LegacyImportBackupError::InvalidData)?;
                if proof.len() != 7
                    || !matches!(proof.get("completed_at"), Some(BackupSqlValue::Integer(_)))
                    || !paths.insert(path)
                    || !assets.insert(asset)
                    || asset.parse::<lettuce_types::AssetId>().is_err()
                    || blob.parse::<lettuce_types::MediaBlobId>().is_err()
                    || lettuce_types::ContentHash::parse(hash).is_err()
                    || backup_sql_text(assignment, "source_detail") != Some("")
                    || backup_sql_text(assignment, "expected_content_hash") != Some(hash)
                    || assignment.get("expected_byte_len") != Some(&BackupSqlValue::Integer(bytes))
                {
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

    pub fn validate_media_snapshots(
        &self,
        assets: &[lettuce_media::MediaAsset],
        blobs: &[lettuce_media::MediaBlob],
    ) -> Result<(), LegacyImportBackupError> {
        let assets = assets
            .iter()
            .map(|asset| (asset.id.to_string(), asset))
            .collect::<BTreeMap<_, _>>();
        let blobs = blobs
            .iter()
            .map(|blob| (blob.id.to_string(), blob))
            .collect::<BTreeMap<_, _>>();
        for proof in self.runs.iter().flat_map(|entry| &entry.media_completions) {
            let asset = backup_sql_text(proof, "destination_asset_id")
                .ok_or(LegacyImportBackupError::InvalidData)?;
            let blob =
                backup_sql_text(proof, "blob_id").ok_or(LegacyImportBackupError::InvalidData)?;
            let hash = backup_sql_text(proof, "content_hash")
                .ok_or(LegacyImportBackupError::InvalidData)?;
            if assets
                .get(asset)
                .is_some_and(|asset| asset.blob_id.to_string() != blob)
                || blobs.get(blob).is_some_and(|blob| {
                    blob.content_hash.as_str() != hash
                        || i64::try_from(blob.byte_size)
                            .ok()
                            .map(BackupSqlValue::Integer)
                            .as_ref()
                            != proof.get("byte_len")
                })
            {
                return Err(LegacyImportBackupError::InvalidData);
            }
        }
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
