use std::str::FromStr;

use lettuce_creation::{
    LorebookKeywordGenerationRun, LorebookKeywordRunRepository, LorebookKeywordRunRepositoryError,
};
use lettuce_types::{
    JobId, ModelProfileId, PromptDocumentId, RequestId, Revision, TimestampMillis,
};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use crate::{Database, decode_versioned, encode_versioned};

const RUN_FORMAT_VERSION: u32 = 1;

fn failure(_: impl std::fmt::Debug) -> LorebookKeywordRunRepositoryError {
    LorebookKeywordRunRepositoryError::Failure
}

fn corrupt(_: impl std::fmt::Debug) -> LorebookKeywordRunRepositoryError {
    LorebookKeywordRunRepositoryError::Corrupt
}

fn parse_id<T: FromStr>(value: &str) -> Result<T, LorebookKeywordRunRepositoryError> {
    value
        .parse()
        .map_err(|_| LorebookKeywordRunRepositoryError::Corrupt)
}

fn load_in(
    transaction: &Transaction<'_>,
    request_id: RequestId,
) -> Result<Option<LorebookKeywordGenerationRun>, LorebookKeywordRunRepositoryError> {
    let row = transaction
        .query_row(
            "SELECT job_id, model_profile_id, prompt_id, prompt_revision, created_at, run_json
             FROM creation_lorebook_keyword_runs WHERE request_id = ?1",
            [request_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                ))
            },
        )
        .optional()
        .map_err(corrupt)?;
    let Some((job_id, model_profile_id, prompt_id, prompt_revision, created_at, run_json)) = row
    else {
        return Ok(None);
    };
    let run = decode_versioned::<LorebookKeywordGenerationRun>(&run_json, RUN_FORMAT_VERSION)
        .map_err(corrupt)?;
    let prompt_revision = u64::try_from(prompt_revision)
        .ok()
        .filter(|revision| *revision > 0)
        .map(Revision::new)
        .ok_or(LorebookKeywordRunRepositoryError::Corrupt)?;
    if run.request_id != request_id
        || run.job_id != parse_id::<JobId>(&job_id)?
        || run.profile.chat_profile.model_profile_id
            != parse_id::<ModelProfileId>(&model_profile_id)?
        || run.prompt_id != parse_id::<PromptDocumentId>(&prompt_id)?
        || run.prompt_revision != prompt_revision
        || run.created_at != TimestampMillis::new(created_at)
        || run.validate().is_err()
    {
        return Err(LorebookKeywordRunRepositoryError::Corrupt);
    }
    Ok(Some(run))
}

impl LorebookKeywordRunRepository for Database {
    fn admit_lorebook_keyword_run(
        &self,
        run: LorebookKeywordGenerationRun,
    ) -> Result<LorebookKeywordGenerationRun, LorebookKeywordRunRepositoryError> {
        run.validate()?;
        let encoded = encode_versioned(&run, RUN_FORMAT_VERSION).map_err(failure)?;
        let mut connection = self.connection().map_err(failure)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(failure)?;
        let inserted = transaction
            .execute(
                "INSERT OR IGNORE INTO creation_lorebook_keyword_runs (
                   request_id, job_id, model_profile_id, prompt_id, prompt_revision, created_at,
                   run_json
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    run.request_id.to_string(),
                    run.job_id.to_string(),
                    run.profile.chat_profile.model_profile_id.to_string(),
                    run.prompt_id.to_string(),
                    i64::try_from(run.prompt_revision.get()).map_err(failure)?,
                    run.created_at.get(),
                    encoded,
                ],
            )
            .map_err(failure)?;
        let stored = load_in(&transaction, run.request_id)?
            .ok_or(LorebookKeywordRunRepositoryError::Failure)?;
        if inserted == 0 && stored != run {
            return Err(LorebookKeywordRunRepositoryError::Conflict);
        }
        transaction.commit().map_err(failure)?;
        Ok(stored)
    }

    fn load_lorebook_keyword_run(
        &self,
        request_id: RequestId,
    ) -> Result<LorebookKeywordGenerationRun, LorebookKeywordRunRepositoryError> {
        let mut connection = self.connection().map_err(failure)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(failure)?;
        let run = load_in(&transaction, request_id)?
            .ok_or(LorebookKeywordRunRepositoryError::NotFound)?;
        transaction.commit().map_err(failure)?;
        Ok(run)
    }
}
