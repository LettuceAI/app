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

/// Rebinds retained paths inside the same transaction as the device root/profile update.
/// Identity and bytes remain unchanged; files must already have moved successfully.
pub(crate) fn relocate_whisper_models_in(
    transaction: &Transaction<'_>,
    relocate: &dyn Fn(&str) -> Option<String>,
) -> Result<u32, WhisperModelRepositoryError> {
    let mut changed = 0;
    for mut manifest in load_all_in(transaction, None)? {
        let old_path = manifest.model.path.to_str()
            .ok_or(WhisperModelRepositoryError::InvalidData)?;
        let Some(new_path) = relocate(old_path) else { continue; };
        if new_path == old_path { continue; }
        manifest.model.path = new_path.into();
        manifest.verify_contents().map_err(corrupt)?;
        let payload = encode_versioned(&manifest, WHISPER_MANIFEST_FORMAT_VERSION)
            .map_err(storage)?;
        transaction.execute(
            "UPDATE installed_whisper_models SET model_path=?2, manifest_json=?3 WHERE model_id=?1",
            params![manifest.model_id, manifest.model.path.to_str(), payload],
        ).map_err(storage)?;
        changed += 1;
    }
    Ok(changed)
}

impl WhisperModelRepository for Database {
    fn admit_whisper_model(
        &self,
        manifest: InstalledWhisperManifest,
    ) -> Result<InstalledWhisperManifest, WhisperModelRepositoryError> {
        manifest.verify_contents().map_err(corrupt)?;
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

    fn remove_whisper_model(
        &self,
        expected: &InstalledWhisperManifest,
    ) -> Result<bool, WhisperModelRepositoryError> {
        expected.validate().map_err(corrupt)?;
        let payload =
            encode_versioned(expected, WHISPER_MANIFEST_FORMAT_VERSION).map_err(storage)?;
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let current = load_all_in(&transaction, Some(&expected.model_id))?
            .into_iter()
            .next();
        let Some(current) = current else {
            transaction.commit().map_err(storage)?;
            return Ok(false);
        };
        if current != *expected {
            return Err(WhisperModelRepositoryError::Conflict);
        }
        let removed = transaction
            .execute(
                "DELETE FROM installed_whisper_models
                 WHERE model_id = ?1 AND manifest_json = ?2",
                params![expected.model_id, payload],
            )
            .map_err(storage)?;
        if removed != 1 {
            return Err(WhisperModelRepositoryError::Conflict);
        }
        transaction.commit().map_err(storage)?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lettuce_models::ModelPathRelocation;

    #[test]
    fn relocation_preserves_identity_and_trigger_rejects_content_changes() {
        let root = std::env::temp_dir().join(format!("whisper-rebind-{}", lettuce_types::JobId::new()));
        let before = root.join("before");
        let after = root.join("after");
        std::fs::create_dir_all(&before).expect("before");
        std::fs::write(before.join("ggml-tiny.en.bin"), b"retained whisper model").expect("file");
        let original = InstalledWhisperManifest::inspect_legacy(
            &root, &before.join("ggml-tiny.en.bin"), TimestampMillis::new(10),
        ).expect("manifest");
        let database = Database::open_in_memory().expect("database");
        database.admit_whisper_model(original.clone()).expect("admit");
        std::fs::rename(&before, &after).expect("move");
        let relocate = |path: &str| Path::new(path).strip_prefix(&before).ok()
            .map(|relative| after.join(relative).to_string_lossy().into_owned());
        assert_eq!(database.relocate_model_paths(&relocate, TimestampMillis::new(20)).expect("rebind"), 1);
        let rebound = database.get_whisper_model(&original.model_id).expect("get").expect("model");
        let mut expected = original.clone();
        expected.model.path = after.join("ggml-tiny.en.bin");
        assert_eq!(rebound, expected);
        rebound.verify_contents().expect("unchanged contents");
        assert_eq!(database.relocate_model_paths(&relocate, TimestampMillis::new(21)).expect("replay"), 0);
        let connection = database.connection().expect("connection");
        for sql in [
            "UPDATE installed_whisper_models SET source_revision='changed'",
            "UPDATE installed_whisper_models SET byte_size=byte_size+1",
            "UPDATE installed_whisper_models SET admitted_at=admitted_at+1",
            "UPDATE installed_whisper_models SET model_path='/unpaired'",
            "UPDATE installed_whisper_models SET manifest_json=json_set(manifest_json, '$.value.english_only', 0)",
        ] {
            assert!(connection.execute(sql, []).is_err(), "allowed {sql}");
        }
        drop(connection);
        assert_eq!(database.get_whisper_model(&original.model_id).expect("unchanged"), Some(expected));
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn failed_content_verification_does_not_rebind_any_manifest() {
        let root = std::env::temp_dir().join(format!("whisper-rebind-{}", lettuce_types::JobId::new()));
        std::fs::create_dir_all(&root).expect("root");
        let folder = root.join("tiny.en");
        std::fs::create_dir_all(&folder).expect("folder");
        let path = folder.join("ggml-tiny.en.bin");
        std::fs::write(&path, b"original").expect("file");
        let manifest = InstalledWhisperManifest::inspect_legacy(&root, &path, TimestampMillis::new(10)).expect("manifest");
        let database = Database::open_in_memory().expect("database");
        database.admit_whisper_model(manifest.clone()).expect("admit");
        let target = root.join("changed.bin");
        std::fs::write(&target, b"tampered").expect("tamper");
        assert!(database.relocate_model_paths(&|_| Some(target.to_string_lossy().into_owned()), TimestampMillis::new(20)).is_err());
        assert_eq!(database.get_whisper_model(&manifest.model_id).expect("unchanged"), Some(manifest));
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}
