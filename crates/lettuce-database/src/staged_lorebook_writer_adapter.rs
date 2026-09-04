use std::str::FromStr;

use lettuce_creation::{
    StagedLorebookWriterAttempt, StagedLorebookWriterRun, StagedLorebookWriterRunRepository,
    StagedLorebookWriterRunRepositoryError,
};
use lettuce_types::{
    CreationWorkflowId, JobId, LorebookEntryId, ModelProfileId, PromptDocumentId, RequestId,
    Revision, TimestampMillis,
};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use crate::{Database, decode_versioned, encode_versioned};

const RUN_FORMAT_VERSION: u32 = 1;

fn failure(_: impl std::fmt::Debug) -> StagedLorebookWriterRunRepositoryError {
    StagedLorebookWriterRunRepositoryError::Failure
}

fn corrupt(_: impl std::fmt::Debug) -> StagedLorebookWriterRunRepositoryError {
    StagedLorebookWriterRunRepositoryError::Corrupt
}

fn parse_id<T: FromStr>(value: &str) -> Result<T, StagedLorebookWriterRunRepositoryError> {
    value
        .parse()
        .map_err(|_| StagedLorebookWriterRunRepositoryError::Corrupt)
}

fn positive_revision(value: i64) -> Result<Revision, StagedLorebookWriterRunRepositoryError> {
    u64::try_from(value)
        .ok()
        .filter(|value| *value > 0)
        .map(Revision::new)
        .ok_or(StagedLorebookWriterRunRepositoryError::Corrupt)
}

fn load_in(
    transaction: &Transaction<'_>,
    request_id: RequestId,
) -> Result<Option<StagedLorebookWriterRun>, StagedLorebookWriterRunRepositoryError> {
    let row = transaction
        .query_row(
            "SELECT job_id, project_request_id, project_id, project_revision, plan_id,
                    model_profile_id, prompt_id, prompt_revision, created_at, run_json
             FROM creation_staged_lorebook_writer_runs WHERE request_id = ?1",
            [request_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, i64>(8)?,
                    row.get::<_, String>(9)?,
                ))
            },
        )
        .optional()
        .map_err(corrupt)?;
    let Some((
        job_id,
        project_request_id,
        project_id,
        project_revision,
        plan_id,
        model_profile_id,
        prompt_id,
        prompt_revision,
        created_at,
        encoded,
    )) = row
    else {
        return Ok(None);
    };
    let run = decode_versioned::<StagedLorebookWriterRun>(&encoded, RUN_FORMAT_VERSION)
        .map_err(corrupt)?;
    if run.request_id != request_id
        || run.job_id != parse_id::<JobId>(&job_id)?
        || run.project_request_id != parse_id::<RequestId>(&project_request_id)?
        || run.project_id != parse_id::<CreationWorkflowId>(&project_id)?
        || run.project_revision != positive_revision(project_revision)?
        || run.plan_id != parse_id::<LorebookEntryId>(&plan_id)?
        || run.profile.chat_profile.model_profile_id
            != parse_id::<ModelProfileId>(&model_profile_id)?
        || run.prompt_id != parse_id::<PromptDocumentId>(&prompt_id)?
        || run.prompt_revision != positive_revision(prompt_revision)?
        || run.created_at != TimestampMillis::new(created_at)
        || run.validate().is_err()
    {
        return Err(StagedLorebookWriterRunRepositoryError::Corrupt);
    }
    Ok(Some(run))
}

impl StagedLorebookWriterRunRepository for Database {
    fn admit_staged_lorebook_writer_run(
        &self,
        run: StagedLorebookWriterRun,
    ) -> Result<StagedLorebookWriterRun, StagedLorebookWriterRunRepositoryError> {
        run.validate()?;
        let encoded = encode_versioned(&run, RUN_FORMAT_VERSION).map_err(failure)?;
        let mut connection = self.connection().map_err(failure)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(failure)?;
        let inserted = transaction
            .execute(
                "INSERT OR IGNORE INTO creation_staged_lorebook_writer_runs (
                   request_id, job_id, project_request_id, project_id, project_revision, plan_id,
                   model_profile_id, prompt_id, prompt_revision, created_at, run_json
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    run.request_id.to_string(),
                    run.job_id.to_string(),
                    run.project_request_id.to_string(),
                    run.project_id.to_string(),
                    i64::try_from(run.project_revision.get()).map_err(failure)?,
                    run.plan_id.to_string(),
                    run.profile.chat_profile.model_profile_id.to_string(),
                    run.prompt_id.to_string(),
                    i64::try_from(run.prompt_revision.get()).map_err(failure)?,
                    run.created_at.get(),
                    encoded,
                ],
            )
            .map_err(failure)?;
        let stored = load_in(&transaction, run.request_id)?
            .ok_or(StagedLorebookWriterRunRepositoryError::Failure)?;
        if inserted == 0 && stored != run {
            return Err(StagedLorebookWriterRunRepositoryError::Conflict);
        }
        transaction.commit().map_err(failure)?;
        Ok(stored)
    }

    fn load_staged_lorebook_writer_run(
        &self,
        request_id: RequestId,
    ) -> Result<StagedLorebookWriterRun, StagedLorebookWriterRunRepositoryError> {
        let mut connection = self.connection().map_err(failure)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(failure)?;
        let run = load_in(&transaction, request_id)?
            .ok_or(StagedLorebookWriterRunRepositoryError::NotFound)?;
        transaction.commit().map_err(failure)?;
        Ok(run)
    }

    fn commit_staged_lorebook_writer_attempt(
        &self,
        request_id: RequestId,
        attempt: StagedLorebookWriterAttempt,
    ) -> Result<StagedLorebookWriterRun, StagedLorebookWriterRunRepositoryError> {
        let mut connection = self.connection().map_err(failure)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(failure)?;
        let mut run = load_in(&transaction, request_id)?
            .ok_or(StagedLorebookWriterRunRepositoryError::NotFound)?;
        attempt.validate(&run)?;
        if let Some(stored) = &run.attempt {
            if stored == &attempt {
                transaction.commit().map_err(failure)?;
                return Ok(run);
            }
            return Err(StagedLorebookWriterRunRepositoryError::Conflict);
        }
        run.attempt = Some(attempt);
        let encoded = encode_versioned(&run, RUN_FORMAT_VERSION).map_err(failure)?;
        if transaction
            .execute(
                "UPDATE creation_staged_lorebook_writer_runs SET run_json = ?2
                 WHERE request_id = ?1",
                params![request_id.to_string(), encoded],
            )
            .map_err(failure)?
            != 1
        {
            return Err(StagedLorebookWriterRunRepositoryError::Conflict);
        }
        transaction.commit().map_err(failure)?;
        Ok(run)
    }
}
