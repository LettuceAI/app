use std::str::FromStr;

use lettuce_companions::{
    CompanionGrowthProposalCheckpoint, CompanionGrowthRun, CompanionGrowthRunRepository,
    CompanionGrowthRunRepositoryError,
};
use lettuce_types::{
    CharacterId, ConversationId, DynamicMemoryAttemptId, DynamicMemoryRunId, JobId,
    OperationRecordId, Revision, TimestampMillis,
};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use crate::{Database, decode_versioned, encode_versioned};

const GROWTH_RUN_FORMAT_VERSION: u32 = 1;
const GROWTH_PROPOSAL_FORMAT_VERSION: u32 = 1;

fn failure(_: impl std::fmt::Debug) -> CompanionGrowthRunRepositoryError {
    CompanionGrowthRunRepositoryError::Failure
}

fn corrupt(_: impl std::fmt::Debug) -> CompanionGrowthRunRepositoryError {
    CompanionGrowthRunRepositoryError::Corrupt
}

fn load_in(
    tx: &Transaction<'_>,
    job_id: JobId,
) -> Result<Option<CompanionGrowthRun>, CompanionGrowthRunRepositoryError> {
    let row = tx
        .query_row(
            "SELECT conversation_id, character_id, memory_run_id, memory_attempt_id,
                    operation_id, expected_soul_revision, created_at, run_json,
                    proposal_checkpoint_json, reduced_at
               FROM companion_growth_runs WHERE job_id = ?1",
            [job_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, String>(7)?,
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
    let mut run = decode_versioned::<CompanionGrowthRun>(&row.7, GROWTH_RUN_FORMAT_VERSION)
        .map_err(corrupt)?;
    let checkpoint = row
        .8
        .as_deref()
        .map(|payload| {
            decode_versioned::<CompanionGrowthProposalCheckpoint>(
                payload,
                GROWTH_PROPOSAL_FORMAT_VERSION,
            )
            .map_err(corrupt)
        })
        .transpose()?;
    let expected_revision = u64::try_from(row.5)
        .ok()
        .filter(|revision| *revision > 0)
        .map(Revision::new)
        .ok_or(CompanionGrowthRunRepositoryError::Corrupt)?;
    if run.job_id != job_id
        || run.conversation_id != ConversationId::from_str(&row.0).map_err(corrupt)?
        || run.character_id != CharacterId::from_str(&row.1).map_err(corrupt)?
        || run.memory_run_id != DynamicMemoryRunId::from_str(&row.2).map_err(corrupt)?
        || run.memory_attempt_id != DynamicMemoryAttemptId::from_str(&row.3).map_err(corrupt)?
        || run.operation_id != OperationRecordId::from_str(&row.4).map_err(corrupt)?
        || run.soul.revision != expected_revision
        || run.created_at != TimestampMillis::new(row.6)
        || checkpoint.as_ref().map(|value| value.reduced_at.get()) != row.9
        || run.proposal_checkpoint.is_some()
    {
        return Err(CompanionGrowthRunRepositoryError::Corrupt);
    }
    run.proposal_checkpoint = checkpoint;
    run.validate()
        .map_err(|_| CompanionGrowthRunRepositoryError::Corrupt)?;
    Ok(Some(run))
}

impl CompanionGrowthRunRepository for Database {
    fn admit_companion_growth_run(
        &self,
        run: CompanionGrowthRun,
    ) -> Result<CompanionGrowthRun, CompanionGrowthRunRepositoryError> {
        run.validate()?;
        if run.proposal_checkpoint.is_some() {
            return Err(CompanionGrowthRunRepositoryError::Invalid);
        }
        let payload = encode_versioned(&run, GROWTH_RUN_FORMAT_VERSION).map_err(failure)?;
        let mut connection = self.connection().map_err(failure)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(failure)?;
        let inserted = tx
            .execute(
                "INSERT OR IGNORE INTO companion_growth_runs (
                    job_id, conversation_id, character_id, memory_run_id, memory_attempt_id,
                    operation_id, expected_soul_revision, created_at, run_json
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    run.job_id.to_string(),
                    run.conversation_id.to_string(),
                    run.character_id.to_string(),
                    run.memory_run_id.to_string(),
                    run.memory_attempt_id.to_string(),
                    run.operation_id.to_string(),
                    i64::try_from(run.soul.revision.get()).map_err(failure)?,
                    run.created_at.get(),
                    payload,
                ],
            )
            .map_err(failure)?;
        let stored = load_in(&tx, run.job_id)?.ok_or(CompanionGrowthRunRepositoryError::Failure)?;
        if inserted == 0 {
            let mut immutable = stored.clone();
            immutable.proposal_checkpoint = None;
            if immutable != run {
                return Err(CompanionGrowthRunRepositoryError::Conflict);
            }
        }
        tx.commit().map_err(failure)?;
        Ok(stored)
    }

    fn load_companion_growth_run(
        &self,
        job_id: JobId,
    ) -> Result<CompanionGrowthRun, CompanionGrowthRunRepositoryError> {
        let mut connection = self.connection().map_err(failure)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(failure)?;
        let run = load_in(&tx, job_id)?.ok_or(CompanionGrowthRunRepositoryError::NotFound)?;
        tx.commit().map_err(failure)?;
        Ok(run)
    }

    fn commit_companion_growth_proposals(
        &self,
        job_id: JobId,
        checkpoint: CompanionGrowthProposalCheckpoint,
    ) -> Result<CompanionGrowthRun, CompanionGrowthRunRepositoryError> {
        checkpoint.validate()?;
        let payload =
            encode_versioned(&checkpoint, GROWTH_PROPOSAL_FORMAT_VERSION).map_err(failure)?;
        let mut connection = self.connection().map_err(failure)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(failure)?;
        let current = load_in(&tx, job_id)?.ok_or(CompanionGrowthRunRepositoryError::NotFound)?;
        if let Some(stored) = &current.proposal_checkpoint {
            if stored == &checkpoint {
                tx.commit().map_err(failure)?;
                return Ok(current);
            }
            return Err(CompanionGrowthRunRepositoryError::Conflict);
        }
        let updated = tx
            .execute(
                "UPDATE companion_growth_runs
                    SET proposal_checkpoint_json = ?2, reduced_at = ?3
                  WHERE job_id = ?1 AND proposal_checkpoint_json IS NULL",
                params![job_id.to_string(), payload, checkpoint.reduced_at.get()],
            )
            .map_err(failure)?;
        if updated != 1 {
            return Err(CompanionGrowthRunRepositoryError::Conflict);
        }
        let stored = load_in(&tx, job_id)?.ok_or(CompanionGrowthRunRepositoryError::Failure)?;
        tx.commit().map_err(failure)?;
        Ok(stored)
    }
}
