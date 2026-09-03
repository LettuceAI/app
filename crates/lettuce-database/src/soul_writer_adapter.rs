use std::str::FromStr;

use lettuce_companions::{
    CompanionSoulWriterRoundCheckpoint, CompanionSoulWriterRun, CompanionSoulWriterRunRepository,
    CompanionSoulWriterRunRepositoryError,
};
use lettuce_types::{JobId, PromptDocumentId, RequestId, Revision, TimestampMillis};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use crate::{Database, decode_versioned, encode_versioned};

const RUN_FORMAT_VERSION: u32 = 1;
const ROUNDS_FORMAT_VERSION: u32 = 1;

fn failure(_: impl std::fmt::Debug) -> CompanionSoulWriterRunRepositoryError {
    CompanionSoulWriterRunRepositoryError::Failure
}

fn corrupt(_: impl std::fmt::Debug) -> CompanionSoulWriterRunRepositoryError {
    CompanionSoulWriterRunRepositoryError::Corrupt
}

fn load_in(
    tx: &Transaction<'_>,
    request_id: RequestId,
) -> Result<Option<CompanionSoulWriterRun>, CompanionSoulWriterRunRepositoryError> {
    let row = tx
        .query_row(
            "SELECT job_id, prompt_id, prompt_revision, created_at, run_json, rounds_json
               FROM companion_soul_writer_runs WHERE request_id = ?1",
            [request_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                ))
            },
        )
        .optional()
        .map_err(corrupt)?;
    let Some((job_id, prompt_id, prompt_revision, created_at, run_json, rounds_json)) = row else {
        return Ok(None);
    };
    let mut run = decode_versioned::<CompanionSoulWriterRun>(&run_json, RUN_FORMAT_VERSION)
        .map_err(corrupt)?;
    let rounds = decode_versioned::<Vec<CompanionSoulWriterRoundCheckpoint>>(
        &rounds_json,
        ROUNDS_FORMAT_VERSION,
    )
    .map_err(corrupt)?;
    let prompt_revision = u64::try_from(prompt_revision)
        .ok()
        .filter(|revision| *revision > 0)
        .map(Revision::new)
        .ok_or(CompanionSoulWriterRunRepositoryError::Corrupt)?;
    if run.request_id != request_id
        || run.job_id != JobId::from_str(&job_id).map_err(corrupt)?
        || run.prompt_id != PromptDocumentId::from_str(&prompt_id).map_err(corrupt)?
        || run.prompt_revision != prompt_revision
        || run.created_at != TimestampMillis::new(created_at)
        || !run.rounds.is_empty()
    {
        return Err(CompanionSoulWriterRunRepositoryError::Corrupt);
    }
    run.rounds = rounds;
    run.validate()
        .map_err(|_| CompanionSoulWriterRunRepositoryError::Corrupt)?;
    Ok(Some(run))
}

impl CompanionSoulWriterRunRepository for Database {
    fn admit_companion_soul_writer_run(
        &self,
        run: CompanionSoulWriterRun,
    ) -> Result<CompanionSoulWriterRun, CompanionSoulWriterRunRepositoryError> {
        run.validate()?;
        if !run.rounds.is_empty() {
            return Err(CompanionSoulWriterRunRepositoryError::Invalid);
        }
        let run_json = encode_versioned(&run, RUN_FORMAT_VERSION).map_err(failure)?;
        let rounds_json = encode_versioned(&run.rounds, ROUNDS_FORMAT_VERSION).map_err(failure)?;
        let mut connection = self.connection().map_err(failure)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(failure)?;
        let inserted = tx
            .execute(
                "INSERT OR IGNORE INTO companion_soul_writer_runs (
                    request_id, job_id, prompt_id, prompt_revision, created_at, run_json, rounds_json
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    run.request_id.to_string(),
                    run.job_id.to_string(),
                    run.prompt_id.to_string(),
                    i64::try_from(run.prompt_revision.get()).map_err(failure)?,
                    run.created_at.get(),
                    run_json,
                    rounds_json,
                ],
            )
            .map_err(failure)?;
        let stored =
            load_in(&tx, run.request_id)?.ok_or(CompanionSoulWriterRunRepositoryError::Failure)?;
        if inserted == 0 && stored != run {
            return Err(CompanionSoulWriterRunRepositoryError::Conflict);
        }
        tx.commit().map_err(failure)?;
        Ok(stored)
    }

    fn load_companion_soul_writer_run(
        &self,
        request_id: RequestId,
    ) -> Result<CompanionSoulWriterRun, CompanionSoulWriterRunRepositoryError> {
        let mut connection = self.connection().map_err(failure)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(failure)?;
        let run =
            load_in(&tx, request_id)?.ok_or(CompanionSoulWriterRunRepositoryError::NotFound)?;
        tx.commit().map_err(failure)?;
        Ok(run)
    }

    fn commit_companion_soul_writer_round(
        &self,
        request_id: RequestId,
        checkpoint: CompanionSoulWriterRoundCheckpoint,
    ) -> Result<CompanionSoulWriterRun, CompanionSoulWriterRunRepositoryError> {
        let mut connection = self.connection().map_err(failure)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(failure)?;
        let mut run =
            load_in(&tx, request_id)?.ok_or(CompanionSoulWriterRunRepositoryError::NotFound)?;
        if let Some(stored) = run.rounds.get(checkpoint.ordinal as usize) {
            if stored == &checkpoint {
                tx.commit().map_err(failure)?;
                return Ok(run);
            }
            return Err(CompanionSoulWriterRunRepositoryError::Conflict);
        }
        if checkpoint.ordinal as usize != run.rounds.len()
            || run.rounds.last().is_some_and(|round| round.completed)
        {
            return Err(CompanionSoulWriterRunRepositoryError::Conflict);
        }
        run.rounds.push(checkpoint);
        run.validate()?;
        let rounds_json = encode_versioned(&run.rounds, ROUNDS_FORMAT_VERSION).map_err(failure)?;
        let updated = tx
            .execute(
                "UPDATE companion_soul_writer_runs SET rounds_json = ?2
                  WHERE request_id = ?1",
                params![request_id.to_string(), rounds_json],
            )
            .map_err(failure)?;
        if updated != 1 {
            return Err(CompanionSoulWriterRunRepositoryError::Conflict);
        }
        tx.commit().map_err(failure)?;
        Ok(run)
    }
}
