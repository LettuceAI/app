use std::str::FromStr;

use lettuce_speech::{
    TranscriptionRecord, TranscriptionRepository, TranscriptionRepositoryError,
    TranscriptionResult, TranscriptionState,
};
use lettuce_types::{AssetId, ContentHash, JobId, RequestId, TimestampMillis};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use crate::{Database, decode_versioned, encode_versioned};

const TRANSCRIPTION_REQUEST_FORMAT_VERSION: u32 = 1;
const TRANSCRIPTION_RESULT_FORMAT_VERSION: u32 = 1;

fn storage(_: impl std::fmt::Debug) -> TranscriptionRepositoryError {
    TranscriptionRepositoryError::Storage
}

fn corrupt(_: impl std::fmt::Debug) -> TranscriptionRepositoryError {
    TranscriptionRepositoryError::InvalidData
}

fn load_in(
    transaction: &Transaction<'_>,
    job_id: JobId,
) -> Result<Option<TranscriptionRecord>, TranscriptionRepositoryError> {
    let row = transaction
        .query_row(
            "SELECT request_id, audio_asset_id, model_id, model_artifact_hash,
                    admitted_at, request_json, result_json, completed_at
               FROM speech_transcriptions WHERE job_id = ?1",
            [job_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<i64>>(7)?,
                ))
            },
        )
        .optional()
        .map_err(corrupt)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let request =
        decode_versioned(&row.5, TRANSCRIPTION_REQUEST_FORMAT_VERSION).map_err(corrupt)?;
    let result = row
        .6
        .as_deref()
        .map(|payload| {
            decode_versioned(payload, TRANSCRIPTION_RESULT_FORMAT_VERSION).map_err(corrupt)
        })
        .transpose()?;
    let record = TranscriptionRecord {
        job_id,
        request,
        state: match result {
            Some(result) => TranscriptionState::Succeeded {
                result: Box::new(result),
            },
            None => TranscriptionState::Pending,
        },
    };
    record.validate().map_err(corrupt)?;
    if record.request.id != RequestId::from_str(&row.0).map_err(corrupt)?
        || record.request.audio_asset_id != AssetId::from_str(&row.1).map_err(corrupt)?
        || record.request.model.id.as_str() != row.2
        || record.request.model.artifact_hash != ContentHash::parse(row.3).map_err(corrupt)?
        || record.request.created_at != TimestampMillis::new(row.4)
        || match (&record.state, row.7) {
            (TranscriptionState::Pending, None) => false,
            (TranscriptionState::Succeeded { result }, Some(completed_at)) => {
                result.completed_at != TimestampMillis::new(completed_at)
            }
            _ => true,
        }
    {
        return Err(TranscriptionRepositoryError::InvalidData);
    }
    Ok(Some(record))
}

impl TranscriptionRepository for Database {
    fn admit(
        &self,
        record: TranscriptionRecord,
    ) -> Result<TranscriptionRecord, TranscriptionRepositoryError> {
        record.validate().map_err(corrupt)?;
        if !matches!(record.state, TranscriptionState::Pending) {
            return Err(TranscriptionRepositoryError::InvalidData);
        }
        let request_json = encode_versioned(&record.request, TRANSCRIPTION_REQUEST_FORMAT_VERSION)
            .map_err(storage)?;
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let inserted = transaction
            .execute(
                "INSERT OR IGNORE INTO speech_transcriptions (
                    job_id, request_id, audio_asset_id, model_id, model_artifact_hash,
                    admitted_at, request_json
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    record.job_id.to_string(),
                    record.request.id.to_string(),
                    record.request.audio_asset_id.to_string(),
                    record.request.model.id.as_str(),
                    record.request.model.artifact_hash.as_str(),
                    record.request.created_at.get(),
                    request_json,
                ],
            )
            .map_err(storage)?;
        let stored =
            load_in(&transaction, record.job_id)?.ok_or(TranscriptionRepositoryError::Storage)?;
        if inserted == 0 && (stored.job_id != record.job_id || stored.request != record.request) {
            return Err(TranscriptionRepositoryError::Conflict);
        }
        transaction.commit().map_err(storage)?;
        Ok(stored)
    }

    fn get(&self, job_id: JobId) -> Result<TranscriptionRecord, TranscriptionRepositoryError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(storage)?;
        let record =
            load_in(&transaction, job_id)?.ok_or(TranscriptionRepositoryError::NotFound)?;
        transaction.commit().map_err(storage)?;
        Ok(record)
    }

    fn settle(
        &self,
        job_id: JobId,
        result: TranscriptionResult,
    ) -> Result<TranscriptionRecord, TranscriptionRepositoryError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let current =
            load_in(&transaction, job_id)?.ok_or(TranscriptionRepositoryError::NotFound)?;
        result.validate_for(&current.request).map_err(corrupt)?;
        if let TranscriptionState::Succeeded { result: stored } = &current.state {
            if stored.as_ref() == &result {
                transaction.commit().map_err(storage)?;
                return Ok(current);
            }
            return Err(TranscriptionRepositoryError::Conflict);
        }
        let result_json =
            encode_versioned(&result, TRANSCRIPTION_RESULT_FORMAT_VERSION).map_err(storage)?;
        let changed = transaction
            .execute(
                "UPDATE speech_transcriptions
                    SET result_json = ?2, completed_at = ?3
                  WHERE job_id = ?1 AND result_json IS NULL",
                params![job_id.to_string(), result_json, result.completed_at.get()],
            )
            .map_err(storage)?;
        if changed != 1 {
            return Err(TranscriptionRepositoryError::Conflict);
        }
        let stored = load_in(&transaction, job_id)?.ok_or(TranscriptionRepositoryError::Storage)?;
        transaction.commit().map_err(storage)?;
        Ok(stored)
    }
}
