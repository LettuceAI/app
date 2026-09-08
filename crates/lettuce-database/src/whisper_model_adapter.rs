use std::path::Path;

use lettuce_model_hub::{
    InstalledWhisperManifest, WhisperModelRepository, WhisperModelRepositoryError,
};
use lettuce_types::{ContentHash, TimestampMillis};
use rusqlite::{Transaction, TransactionBehavior, params};

use crate::{Database, decode_versioned, encode_versioned};

const WHISPER_MANIFEST_FORMAT_VERSION: u32 = 1;

fn storage(_: impl std::fmt::Debug) -> WhisperModelRepositoryError {
    WhisperModelRepositoryError::Storage
}

fn corrupt(_: impl std::fmt::Debug) -> WhisperModelRepositoryError {
    WhisperModelRepositoryError::InvalidData
}

fn load_all_in(
    transaction: &Transaction<'_>,
    model_id: Option<&str>,
) -> Result<Vec<InstalledWhisperManifest>, WhisperModelRepositoryError> {
    let sql = if model_id.is_some() {
        "SELECT model_id, source_revision, model_path, byte_size, blake3,
                english_only, quantized, admitted_at, manifest_json
           FROM installed_whisper_models WHERE model_id = ?1 ORDER BY model_id"
    } else {
        "SELECT model_id, source_revision, model_path, byte_size, blake3,
                english_only, quantized, admitted_at, manifest_json
           FROM installed_whisper_models ORDER BY model_id"
    };
    let mut statement = transaction.prepare(sql).map_err(storage)?;
    let map_row = |row: &rusqlite::Row<'_>| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, bool>(5)?,
            row.get::<_, bool>(6)?,
            row.get::<_, i64>(7)?,
            row.get::<_, String>(8)?,
        ))
    };
    let rows = match model_id {
        Some(model_id) => statement
            .query_map([model_id], map_row)
            .map_err(storage)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(corrupt)?,
        None => statement
            .query_map([], map_row)
            .map_err(storage)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(corrupt)?,
    };
    rows.into_iter()
        .map(|row| {
            let manifest = decode_versioned::<InstalledWhisperManifest>(
                &row.8,
                WHISPER_MANIFEST_FORMAT_VERSION,
            )
            .map_err(corrupt)?;
            manifest.validate().map_err(corrupt)?;
            let byte_size = u64::try_from(row.3).map_err(corrupt)?;
            if manifest.model_id != row.0
                || manifest.source_revision != row.1
                || manifest.model.path != Path::new(&row.2)
                || manifest.model.byte_size != byte_size
                || manifest.model.blake3 != ContentHash::parse(row.4).map_err(corrupt)?
                || manifest.english_only != row.5
                || manifest.quantized != row.6
                || manifest.admitted_at != TimestampMillis::new(row.7)
            {
                return Err(WhisperModelRepositoryError::InvalidData);
            }
            Ok(manifest)
        })
        .collect()
}

impl WhisperModelRepository for Database {
    fn admit_whisper_model(
        &self,
        manifest: InstalledWhisperManifest,
    ) -> Result<InstalledWhisperManifest, WhisperModelRepositoryError> {
        manifest.verify().map_err(corrupt)?;
        let payload =
            encode_versioned(&manifest, WHISPER_MANIFEST_FORMAT_VERSION).map_err(storage)?;
        let path = manifest
            .model
            .path
            .to_str()
            .ok_or(WhisperModelRepositoryError::InvalidData)?;
        let byte_size = i64::try_from(manifest.model.byte_size).map_err(corrupt)?;
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let inserted = transaction
            .execute(
                "INSERT OR IGNORE INTO installed_whisper_models (
                    model_id, source_revision, model_path, byte_size, blake3,
                    english_only, quantized, admitted_at, manifest_json
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    manifest.model_id,
                    manifest.source_revision,
                    path,
                    byte_size,
                    manifest.model.blake3.as_str(),
                    manifest.english_only,
                    manifest.quantized,
                    manifest.admitted_at.get(),
                    payload,
                ],
            )
            .map_err(storage)?;
        let stored = load_all_in(&transaction, Some(&manifest.model_id))?
            .into_iter()
            .next()
            .ok_or(WhisperModelRepositoryError::Storage)?;
        if inserted == 0 && stored != manifest {
            return Err(WhisperModelRepositoryError::Conflict);
        }
        transaction.commit().map_err(storage)?;
        Ok(stored)
    }

    fn get_whisper_model(
        &self,
        model_id: &str,
    ) -> Result<Option<InstalledWhisperManifest>, WhisperModelRepositoryError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(storage)?;
        let model = load_all_in(&transaction, Some(model_id))?
            .into_iter()
            .next();
        transaction.commit().map_err(storage)?;
        Ok(model)
    }

    fn list_whisper_models(
        &self,
    ) -> Result<Vec<InstalledWhisperManifest>, WhisperModelRepositoryError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(storage)?;
        let models = load_all_in(&transaction, None)?;
        transaction.commit().map_err(storage)?;
        Ok(models)
    }
}
