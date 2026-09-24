use std::str::FromStr;

use lettuce_speech::{
    CachedSpeechBlob, SpeechCacheRepository, SynthesisRecord, SynthesisRepository,
    SynthesisRepositoryError, SynthesisResult, SynthesisReuseKey, SynthesisState, TtsOutputPolicy,
};
use lettuce_types::{AssetId, AudioProviderId, JobId, MediaBlobId, RequestId, TimestampMillis};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use crate::{Database, decode_versioned, encode_versioned};

const REQUEST_FORMAT_VERSION: u32 = 1;
const RESULT_FORMAT_VERSION: u32 = 1;

fn storage(_: impl std::fmt::Debug) -> SynthesisRepositoryError {
    SynthesisRepositoryError::Storage
}

fn corrupt(_: impl std::fmt::Debug) -> SynthesisRepositoryError {
    SynthesisRepositoryError::InvalidData
}

fn load_in(
    transaction: &Transaction<'_>,
    job_id: JobId,
) -> Result<Option<SynthesisRecord>, SynthesisRepositoryError> {
    let row = transaction
        .query_row(
            "SELECT request_id, provider_id, output_asset_id, output_retention,
                    output_expires_at, admitted_at, request_json, result_json,
                    result_asset_id, completed_at
               FROM speech_syntheses WHERE job_id = ?1",
            [job_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<i64>>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, Option<i64>>(9)?,
                ))
            },
        )
        .optional()
        .map_err(corrupt)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let request = decode_versioned(&row.6, REQUEST_FORMAT_VERSION).map_err(corrupt)?;
    let result = row
        .7
        .as_deref()
        .map(|value| decode_versioned(value, RESULT_FORMAT_VERSION).map_err(corrupt))
        .transpose()?;
    let record = SynthesisRecord {
        job_id,
        request,
        state: match result {
            Some(result) => SynthesisState::Succeeded { result },
            None => SynthesisState::Pending,
        },
    };
    record.validate().map_err(corrupt)?;
    let (retention, expires_at) = output_policy(&record.request.output_policy);
    if record.request.id != RequestId::from_str(&row.0).map_err(corrupt)?
        || record.request.provider.id != AudioProviderId::from_str(&row.1).map_err(corrupt)?
        || record.request.output_asset_id != AssetId::from_str(&row.2).map_err(corrupt)?
        || retention != row.3
        || expires_at != row.4
        || record.request.created_at != TimestampMillis::new(row.5)
        || match (&record.state, row.8.as_deref(), row.9) {
            (SynthesisState::Pending, None, None) => false,
            (SynthesisState::Succeeded { result }, Some(asset), Some(completed_at)) => {
                result.audio_asset_id != AssetId::from_str(asset).map_err(corrupt)?
                    || result.completed_at != TimestampMillis::new(completed_at)
            }
            _ => true,
        }
    {
        return Err(SynthesisRepositoryError::InvalidData);
    }
    Ok(Some(record))
}

fn output_policy(policy: &TtsOutputPolicy) -> (&'static str, Option<i64>) {
    match policy {
        TtsOutputPolicy::Preview { expires_at } => ("temporary", Some(expires_at.get())),
        TtsOutputPolicy::Retained => ("persistent", None),
    }
}

fn insert_pending_row(
    transaction: &Transaction<'_>,
    record: &SynthesisRecord,
) -> Result<usize, SynthesisRepositoryError> {
    let request_json =
        encode_versioned(&record.request, REQUEST_FORMAT_VERSION).map_err(storage)?;
    let (retention, expires_at) = output_policy(&record.request.output_policy);
    transaction
        .execute(
            "INSERT OR IGNORE INTO speech_syntheses (
                job_id, request_id, provider_id, output_asset_id, output_retention,
                output_expires_at, admitted_at, request_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                record.job_id.to_string(),
                record.request.id.to_string(),
                record.request.provider.id.to_string(),
                record.request.output_asset_id.to_string(),
                retention,
                expires_at,
                record.request.created_at.get(),
                request_json,
            ],
        )
        .map_err(storage)
}

fn settle_row(
    transaction: &Transaction<'_>,
    job_id: JobId,
    result: &SynthesisResult,
) -> Result<usize, SynthesisRepositoryError> {
    let result_json = encode_versioned(result, RESULT_FORMAT_VERSION).map_err(storage)?;
    transaction
        .execute(
            "UPDATE speech_syntheses
                SET result_json = ?2, result_asset_id = ?3, completed_at = ?4
              WHERE job_id = ?1 AND result_json IS NULL",
            params![
                job_id.to_string(),
                result_json,
                result.audio_asset_id.to_string(),
                result.completed_at.get(),
            ],
        )
        .map_err(storage)
}

pub(crate) fn list_in(
    transaction: &Transaction<'_>,
) -> Result<Vec<SynthesisRecord>, SynthesisRepositoryError> {
    let job_ids = transaction
        .prepare("SELECT job_id FROM speech_syntheses ORDER BY admitted_at, job_id")
        .and_then(|mut statement| {
            statement
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()
        })
        .map_err(storage)?;
    job_ids
        .iter()
        .map(|job_id| {
            load_in(transaction, JobId::from_str(job_id).map_err(corrupt)?)?
                .ok_or(SynthesisRepositoryError::Storage)
        })
        .collect()
}

pub(crate) fn insert_restored_in(
    transaction: &Transaction<'_>,
    record: &SynthesisRecord,
) -> Result<(), SynthesisRepositoryError> {
    record.validate().map_err(corrupt)?;
    if insert_pending_row(transaction, record)? != 1 {
        return Err(SynthesisRepositoryError::Conflict);
    }
    if let SynthesisState::Succeeded { result } = &record.state
        && settle_row(transaction, record.job_id, result)? != 1
    {
        return Err(SynthesisRepositoryError::Conflict);
    }
    if load_in(transaction, record.job_id)?.as_ref() != Some(record) {
        return Err(SynthesisRepositoryError::InvalidData);
    }
    Ok(())
}

impl SynthesisRepository for Database {
    fn admit(&self, record: SynthesisRecord) -> Result<SynthesisRecord, SynthesisRepositoryError> {
        record.validate().map_err(corrupt)?;
        if !matches!(record.state, SynthesisState::Pending) {
            return Err(SynthesisRepositoryError::InvalidData);
        }
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let inserted = insert_pending_row(&transaction, &record)?;
        let stored =
            load_in(&transaction, record.job_id)?.ok_or(SynthesisRepositoryError::Storage)?;
        if inserted == 0 && stored != record {
            return Err(SynthesisRepositoryError::Conflict);
        }
        transaction.commit().map_err(storage)?;
        Ok(stored)
    }

    fn get(&self, job_id: JobId) -> Result<SynthesisRecord, SynthesisRepositoryError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(storage)?;
        let record = load_in(&transaction, job_id)?.ok_or(SynthesisRepositoryError::NotFound)?;
        transaction.commit().map_err(storage)?;
        Ok(record)
    }

    fn settle(
        &self,
        job_id: JobId,
        result: SynthesisResult,
    ) -> Result<SynthesisRecord, SynthesisRepositoryError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let current = load_in(&transaction, job_id)?.ok_or(SynthesisRepositoryError::NotFound)?;
        result.validate_for(&current.request).map_err(corrupt)?;
        if let SynthesisState::Succeeded { result: stored } = &current.state {
            if stored == &result {
                transaction.commit().map_err(storage)?;
                return Ok(current);
            }
            return Err(SynthesisRepositoryError::Conflict);
        }
        if settle_row(&transaction, job_id, &result)? != 1 {
            return Err(SynthesisRepositoryError::Conflict);
        }
        let stored = load_in(&transaction, job_id)?.ok_or(SynthesisRepositoryError::Storage)?;
        transaction.commit().map_err(storage)?;
        Ok(stored)
    }
}

fn other_asset_references(
    transaction: &Transaction<'_>,
) -> Result<Vec<String>, SynthesisRepositoryError> {
    let references = transaction
        .prepare(
            "SELECT m.name, f.\"from\"
               FROM sqlite_schema AS m
               JOIN pragma_foreign_key_list(m.name) AS f
              WHERE m.type = 'table'
                AND f.\"table\" = 'media_assets'
                AND (f.\"to\" IS NULL OR f.\"to\" = 'id')
              ORDER BY m.name, f.\"from\"",
        )
        .and_then(|mut statement| {
            statement
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<Result<Vec<_>, _>>()
        })
        .map_err(storage)?;
    Ok(references
        .into_iter()
        .filter(|(table, column)| !(table == "speech_syntheses" && column == "result_asset_id"))
        .map(|(table, column)| {
            format!(
                "SELECT \"{}\" FROM \"{}\"",
                column.replace('"', "\"\""),
                table.replace('"', "\"\"")
            )
        })
        .collect())
}

fn cached_blob_filter(transaction: &Transaction<'_>) -> Result<String, SynthesisRepositoryError> {
    let others = other_asset_references(transaction)?;
    let referenced = if others.is_empty() {
        String::new()
    } else {
        format!(" OR a.id IN ({})", others.join(" UNION ALL "))
    };
    Ok(format!(
        "b.state = 'ready'
         AND EXISTS (
             SELECT 1 FROM media_assets AS a
               JOIN speech_syntheses AS s ON s.result_asset_id = a.id
              WHERE a.blob_id = b.id)
         AND NOT EXISTS (
             SELECT 1 FROM media_assets AS a
              WHERE a.blob_id = b.id
                AND (a.kind <> 'synthesized_speech'
                     OR a.retention = 'library'
                     OR NOT EXISTS (
                         SELECT 1 FROM speech_syntheses AS s WHERE s.result_asset_id = a.id){referenced}))"
    ))
}

impl SpeechCacheRepository for Database {
    fn find_reusable(
        &self,
        key: &SynthesisReuseKey,
        now: TimestampMillis,
    ) -> Result<Option<SynthesisRecord>, SynthesisRepositoryError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(storage)?;
        let job_id = transaction
            .query_row(
                "SELECT s.job_id
                   FROM speech_syntheses AS s
                   JOIN media_assets AS a ON a.id = s.result_asset_id
                   JOIN media_blobs AS b ON b.id = a.blob_id
                  WHERE s.provider_id = ?1
                    AND json_extract(s.request_json, '$.value.text') = ?2
                    AND json_extract(s.request_json, '$.value.model_id') = ?3
                    AND json_extract(s.request_json, '$.value.voice_id') = ?4
                    AND COALESCE(json_extract(s.request_json, '$.value.prompt'), '') = ?5
                    AND s.result_json IS NOT NULL
                    AND s.output_retention = 'persistent'
                    AND b.state = 'ready'
                    AND (a.expires_at IS NULL OR a.expires_at > ?6)
                  ORDER BY s.completed_at DESC, s.job_id DESC
                  LIMIT 1",
                params![
                    key.provider_id.to_string(),
                    key.text,
                    key.model_id,
                    key.voice_id,
                    key.prompt.as_deref().unwrap_or(""),
                    now.get(),
                ],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(storage)?;
        let record = match job_id {
            Some(job_id) => load_in(&transaction, JobId::from_str(&job_id).map_err(corrupt)?)?,
            None => None,
        };
        transaction.commit().map_err(storage)?;
        Ok(record)
    }

    fn cached_blobs(&self) -> Result<Vec<CachedSpeechBlob>, SynthesisRepositoryError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(storage)?;
        let filter = cached_blob_filter(&transaction)?;
        let rows = transaction
            .prepare(&format!(
                "SELECT b.id, b.byte_size FROM media_blobs AS b WHERE {filter} ORDER BY b.id"
            ))
            .and_then(|mut statement| {
                statement
                    .query_map([], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
                    })?
                    .collect::<Result<Vec<_>, _>>()
            })
            .map_err(storage)?;
        transaction.commit().map_err(storage)?;
        rows.into_iter()
            .map(|(id, size)| {
                Ok(CachedSpeechBlob {
                    blob_id: MediaBlobId::from_str(&id).map_err(corrupt)?,
                    byte_size: u64::try_from(size).map_err(corrupt)?,
                })
            })
            .collect()
    }

    fn release(
        &self,
        blob_id: MediaBlobId,
        now: TimestampMillis,
    ) -> Result<bool, SynthesisRepositoryError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let filter = cached_blob_filter(&transaction)?;
        let released = transaction
            .execute(
                &format!(
                    "UPDATE media_blobs SET state = 'missing', updated_at = ?2
                      WHERE id = ?1
                        AND id IN (SELECT b.id FROM media_blobs AS b WHERE {filter})"
                ),
                params![blob_id.to_string(), now.get()],
            )
            .map_err(storage)?;
        transaction.commit().map_err(storage)?;
        Ok(released == 1)
    }
}
