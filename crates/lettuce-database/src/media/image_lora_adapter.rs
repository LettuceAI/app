use lettuce_image_generation::sd_runtime::lora_library::{
    LoraArchitectureSource, LoraKeywordSource, LoraLibraryRepository, LoraLibraryRepositoryError,
    LoraRecord,
};
use lettuce_types::TimestampMillis;
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::Database;

const COLUMNS: &str = "path, filename, bytes_on_disk, modified_at, sha256, keywords, \
                       keyword_source, architecture, architecture_source";

fn storage(_: impl std::fmt::Debug) -> LoraLibraryRepositoryError {
    LoraLibraryRepositoryError::Storage
}

const fn keyword_source_name(source: LoraKeywordSource) -> &'static str {
    match source {
        LoraKeywordSource::None => "none",
        LoraKeywordSource::Metadata => "metadata",
        LoraKeywordSource::Civitai => "civitai",
        LoraKeywordSource::Manual => "manual",
    }
}

const fn architecture_source_name(source: LoraArchitectureSource) -> &'static str {
    match source {
        LoraArchitectureSource::None => "none",
        LoraArchitectureSource::Metadata => "metadata",
        LoraArchitectureSource::Civitai => "civitai",
    }
}

fn to_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn from_row(row: &Row<'_>) -> rusqlite::Result<Result<LoraRecord, LoraLibraryRepositoryError>> {
    let keywords: String = row.get(5)?;
    let keyword_source: String = row.get(6)?;
    let architecture_source: String = row.get(8)?;
    Ok((|| {
        Ok(LoraRecord {
            path: row.get(0).map_err(storage)?,
            filename: row.get(1).map_err(storage)?,
            bytes_on_disk: row.get::<_, i64>(2).map_err(storage)?.max(0).unsigned_abs(),
            modified_at: row.get::<_, i64>(3).map_err(storage)?.max(0).unsigned_abs(),
            sha256: row.get(4).map_err(storage)?,
            keywords: lettuce_image_generation::sd_runtime::lora_library::normalize_lora_keywords(
                serde_json::from_str::<Vec<String>>(&keywords)
                    .map_err(|_| LoraLibraryRepositoryError::InvalidData)?,
            ),
            keyword_source: match keyword_source.as_str() {
                "none" => LoraKeywordSource::None,
                "metadata" => LoraKeywordSource::Metadata,
                "civitai" => LoraKeywordSource::Civitai,
                "manual" => LoraKeywordSource::Manual,
                _ => return Err(LoraLibraryRepositoryError::InvalidData),
            },
            architecture: row.get(7).map_err(storage)?,
            architecture_source: match architecture_source.as_str() {
                "none" => LoraArchitectureSource::None,
                "metadata" => LoraArchitectureSource::Metadata,
                "civitai" => LoraArchitectureSource::Civitai,
                _ => return Err(LoraLibraryRepositoryError::InvalidData),
            },
        })
    })())
}

fn load(
    connection: &Connection,
    path: &str,
) -> Result<Option<LoraRecord>, LoraLibraryRepositoryError> {
    connection
        .query_row(
            &format!("SELECT {COLUMNS} FROM image_loras WHERE path = ?1"),
            [path],
            from_row,
        )
        .optional()
        .map_err(storage)?
        .transpose()
}

impl LoraLibraryRepository for Database {
    fn lora(&self, path: &str) -> Result<Option<LoraRecord>, LoraLibraryRepositoryError> {
        let connection = self.connection().map_err(storage)?;
        load(&connection, path)
    }

    fn lora_by_hash(&self, sha256: &str) -> Result<Option<LoraRecord>, LoraLibraryRepositoryError> {
        let connection = self.connection().map_err(storage)?;
        connection
            .query_row(
                &format!(
                    "SELECT {COLUMNS} FROM image_loras \
                     WHERE sha256 = ?1 AND (keywords != '[]' OR architecture IS NOT NULL) \
                     ORDER BY updated_at DESC LIMIT 1"
                ),
                [sha256],
                from_row,
            )
            .optional()
            .map_err(storage)?
            .transpose()
    }

    fn record_lora_file(
        &self,
        path: &str,
        filename: &str,
        bytes_on_disk: u64,
        modified_at: u64,
        now: TimestampMillis,
    ) -> Result<LoraRecord, LoraLibraryRepositoryError> {
        let connection = self.connection().map_err(storage)?;
        let changed = load(&connection, path)?.is_some_and(|entry| {
            entry.bytes_on_disk != bytes_on_disk || entry.modified_at != modified_at
        });
        connection
            .execute(
                "INSERT INTO image_loras (
                    path, filename, bytes_on_disk, modified_at, sha256, keywords, keyword_source,
                    architecture, architecture_source, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, NULL, '[]', 'none', NULL, 'none', ?5, ?5)
                 ON CONFLICT(path) DO UPDATE SET
                    filename = excluded.filename,
                    bytes_on_disk = excluded.bytes_on_disk,
                    modified_at = excluded.modified_at,
                    sha256 = CASE WHEN ?6 THEN NULL ELSE image_loras.sha256 END,
                    keywords = CASE WHEN ?6 THEN '[]' ELSE image_loras.keywords END,
                    keyword_source = CASE WHEN ?6 THEN 'none' ELSE image_loras.keyword_source END,
                    architecture = CASE WHEN ?6 THEN NULL ELSE image_loras.architecture END,
                    architecture_source =
                        CASE WHEN ?6 THEN 'none' ELSE image_loras.architecture_source END,
                    updated_at = ?5",
                params![
                    path,
                    filename,
                    to_i64(bytes_on_disk),
                    to_i64(modified_at),
                    now.get(),
                    changed,
                ],
            )
            .map_err(storage)?;
        load(&connection, path)?.ok_or(LoraLibraryRepositoryError::Storage)
    }

    fn save_lora(
        &self,
        record: &LoraRecord,
        now: TimestampMillis,
    ) -> Result<(), LoraLibraryRepositoryError> {
        let keywords = serde_json::to_string(&record.keywords).map_err(storage)?;
        let connection = self.connection().map_err(storage)?;
        connection
            .execute(
                "INSERT INTO image_loras (
                    path, filename, bytes_on_disk, modified_at, sha256, keywords, keyword_source,
                    architecture, architecture_source, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)
                 ON CONFLICT(path) DO UPDATE SET
                    filename = excluded.filename,
                    bytes_on_disk = excluded.bytes_on_disk,
                    modified_at = excluded.modified_at,
                    sha256 = excluded.sha256,
                    keywords = excluded.keywords,
                    keyword_source = excluded.keyword_source,
                    architecture = excluded.architecture,
                    architecture_source = excluded.architecture_source,
                    updated_at = excluded.updated_at",
                params![
                    record.path,
                    record.filename,
                    to_i64(record.bytes_on_disk),
                    to_i64(record.modified_at),
                    record.sha256,
                    keywords,
                    keyword_source_name(record.keyword_source),
                    record.architecture,
                    architecture_source_name(record.architecture_source),
                    now.get(),
                ],
            )
            .map_err(storage)?;
        Ok(())
    }

    fn delete_lora(&self, path: &str) -> Result<(), LoraLibraryRepositoryError> {
        let connection = self.connection().map_err(storage)?;
        connection
            .execute("DELETE FROM image_loras WHERE path = ?1", [path])
            .map_err(storage)?;
        Ok(())
    }

    /// Legacy searched the model's settings for the quoted path.
    fn lora_model_references(&self, path: &str) -> Result<u64, LoraLibraryRepositoryError> {
        let escaped = path
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        let pattern = format!("%\"{escaped}\"%");
        let connection = self.connection().map_err(storage)?;
        let references: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM model_profiles model
                   JOIN provider_accounts account ON account.id = model.provider_account_id
                  WHERE account.provider_kind = 'sdcpp'
                    AND model.config_json LIKE ?1 ESCAPE '\\'",
                [pattern],
                |row| row.get(0),
            )
            .map_err(storage)?;
        Ok(references.max(0).unsigned_abs())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_reset_on_file_changes_and_find_copies_by_hash() {
        let database = Database::open_in_memory().expect("database");
        let first = database
            .record_lora_file(
                "styles/a.safetensors",
                "a.safetensors",
                10,
                100,
                TimestampMillis::new(1),
            )
            .expect("record");
        assert_eq!(first.keyword_source, LoraKeywordSource::None);
        let discovered = LoraRecord {
            sha256: Some("a".repeat(64)),
            keywords: vec!["Trigger".to_owned()],
            keyword_source: LoraKeywordSource::Civitai,
            architecture: Some("z-image".to_owned()),
            architecture_source: LoraArchitectureSource::Civitai,
            ..first
        };
        database
            .save_lora(&discovered, TimestampMillis::new(2))
            .expect("save");
        let same = database
            .record_lora_file(
                "styles/a.safetensors",
                "a.safetensors",
                10,
                100,
                TimestampMillis::new(3),
            )
            .expect("same file");
        assert_eq!(same, discovered);
        assert_eq!(
            database.lora_by_hash(&"a".repeat(64)).expect("hash"),
            Some(discovered.clone())
        );
        let changed = database
            .record_lora_file(
                "styles/a.safetensors",
                "a.safetensors",
                11,
                100,
                TimestampMillis::new(4),
            )
            .expect("changed file");
        assert_eq!(changed.sha256, None);
        assert!(changed.keywords.is_empty());
        assert_eq!(changed.architecture, None);
        assert_eq!(
            database.lora_model_references("styles/a.safetensors"),
            Ok(0)
        );
        database
            .delete_lora("styles/a.safetensors")
            .expect("delete");
        assert_eq!(database.lora("styles/a.safetensors"), Ok(None));
    }
}
