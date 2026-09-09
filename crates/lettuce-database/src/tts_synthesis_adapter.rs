use std::str::FromStr;

use lettuce_speech::{
    SynthesisRecord, SynthesisRepository, SynthesisRepositoryError, SynthesisResult,
    SynthesisState, TtsOutputPolicy,
};
use lettuce_types::{AssetId, AudioProviderId, JobId, RequestId, TimestampMillis};
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

impl SynthesisRepository for Database {
    fn admit(&self, record: SynthesisRecord) -> Result<SynthesisRecord, SynthesisRepositoryError> {
        record.validate().map_err(corrupt)?;
        if !matches!(record.state, SynthesisState::Pending) {
            return Err(SynthesisRepositoryError::InvalidData);
        }
        let request_json =
            encode_versioned(&record.request, REQUEST_FORMAT_VERSION).map_err(storage)?;
        let (retention, expires_at) = output_policy(&record.request.output_policy);
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let inserted = transaction
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
            .map_err(storage)?;
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
        let result_json = encode_versioned(&result, RESULT_FORMAT_VERSION).map_err(storage)?;
        let changed = transaction
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
            .map_err(storage)?;
        if changed != 1 {
            return Err(SynthesisRepositoryError::Conflict);
        }
        let stored = load_in(&transaction, job_id)?.ok_or(SynthesisRepositoryError::Storage)?;
        transaction.commit().map_err(storage)?;
        Ok(stored)
    }
}
