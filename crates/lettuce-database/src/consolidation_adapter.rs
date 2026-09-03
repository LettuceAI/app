use std::str::FromStr;

use lettuce_companions::{
    CompanionConsolidationProposalCheckpoint, CompanionConsolidationRun,
    CompanionConsolidationRunRepository, CompanionConsolidationRunRepositoryError,
};
use lettuce_types::{
    CharacterId, ConversationId, JobId, OperationRecordId, Revision, TimestampMillis,
};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use crate::{Database, decode_versioned, encode_versioned};

const RUN_FORMAT_VERSION: u32 = 1;
const PROPOSAL_FORMAT_VERSION: u32 = 1;

fn failure(_: impl std::fmt::Debug) -> CompanionConsolidationRunRepositoryError {
    CompanionConsolidationRunRepositoryError::Failure
}

fn corrupt(_: impl std::fmt::Debug) -> CompanionConsolidationRunRepositoryError {
    CompanionConsolidationRunRepositoryError::Corrupt
}

fn load_in(
    tx: &Transaction<'_>,
    job_id: JobId,
) -> Result<Option<CompanionConsolidationRun>, CompanionConsolidationRunRepositoryError> {
    let row = tx
        .query_row(
            "SELECT growth_job_id, conversation_id, character_id, operation_id,
                    expected_soul_revision, created_at, run_json,
                    proposal_checkpoint_json, reduced_at
               FROM companion_consolidation_runs WHERE job_id = ?1",
            [job_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, Option<i64>>(8)?,
                ))
            },
        )
        .optional()
        .map_err(corrupt)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let mut run = decode_versioned::<CompanionConsolidationRun>(&row.6, RUN_FORMAT_VERSION)
        .map_err(corrupt)?;
    let checkpoint = row
        .7
        .as_deref()
        .map(|payload| {
            decode_versioned::<CompanionConsolidationProposalCheckpoint>(
                payload,
                PROPOSAL_FORMAT_VERSION,
            )
            .map_err(corrupt)
        })
        .transpose()?;
    let expected_revision = u64::try_from(row.4)
        .ok()
        .filter(|revision| *revision > 0)
        .map(Revision::new)
        .ok_or(CompanionConsolidationRunRepositoryError::Corrupt)?;
    if run.job_id != job_id
        || run.growth_job_id != JobId::from_str(&row.0).map_err(corrupt)?
        || run.conversation_id != ConversationId::from_str(&row.1).map_err(corrupt)?
        || run.character_id != CharacterId::from_str(&row.2).map_err(corrupt)?
        || run.operation_id != OperationRecordId::from_str(&row.3).map_err(corrupt)?
        || run.soul.revision != expected_revision
        || run.created_at != TimestampMillis::new(row.5)
        || checkpoint.as_ref().map(|value| value.reduced_at.get()) != row.8
        || run.proposal_checkpoint.is_some()
    {
        return Err(CompanionConsolidationRunRepositoryError::Corrupt);
    }
    run.proposal_checkpoint = checkpoint;
    run.validate()
        .map_err(|_| CompanionConsolidationRunRepositoryError::Corrupt)?;
    Ok(Some(run))
}

impl CompanionConsolidationRunRepository for Database {
    fn admit_companion_consolidation_run(
        &self,
        run: CompanionConsolidationRun,
    ) -> Result<CompanionConsolidationRun, CompanionConsolidationRunRepositoryError> {
        run.validate()?;
        if run.proposal_checkpoint.is_some() {
            return Err(CompanionConsolidationRunRepositoryError::Invalid);
        }
        let payload = encode_versioned(&run, RUN_FORMAT_VERSION).map_err(failure)?;
        let mut connection = self.connection().map_err(failure)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(failure)?;
        let inserted = tx
            .execute(
                "INSERT OR IGNORE INTO companion_consolidation_runs (
                    job_id, growth_job_id, conversation_id, character_id, operation_id,
                    expected_soul_revision, created_at, run_json
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    run.job_id.to_string(),
                    run.growth_job_id.to_string(),
                    run.conversation_id.to_string(),
                    run.character_id.to_string(),
                    run.operation_id.to_string(),
                    i64::try_from(run.soul.revision.get()).map_err(failure)?,
                    run.created_at.get(),
                    payload,
                ],
            )
            .map_err(failure)?;
        let stored =
            load_in(&tx, run.job_id)?.ok_or(CompanionConsolidationRunRepositoryError::Failure)?;
        if inserted == 0 {
            let mut immutable = stored.clone();
            immutable.proposal_checkpoint = None;
            if immutable != run {
                return Err(CompanionConsolidationRunRepositoryError::Conflict);
            }
        }
        tx.commit().map_err(failure)?;
        Ok(stored)
    }

    fn load_companion_consolidation_run(
        &self,
        job_id: JobId,
    ) -> Result<CompanionConsolidationRun, CompanionConsolidationRunRepositoryError> {
        let mut connection = self.connection().map_err(failure)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(failure)?;
        let run =
            load_in(&tx, job_id)?.ok_or(CompanionConsolidationRunRepositoryError::NotFound)?;
        tx.commit().map_err(failure)?;
        Ok(run)
    }

    fn load_companion_consolidation_run_for_growth(
        &self,
        growth_job_id: JobId,
    ) -> Result<CompanionConsolidationRun, CompanionConsolidationRunRepositoryError> {
        let mut connection = self.connection().map_err(failure)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(failure)?;
        let job_id = tx
            .query_row(
                "SELECT job_id FROM companion_consolidation_runs WHERE growth_job_id = ?1",
                [growth_job_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(corrupt)?
            .ok_or(CompanionConsolidationRunRepositoryError::NotFound)
            .and_then(|id| JobId::from_str(&id).map_err(corrupt))?;
        let run = load_in(&tx, job_id)?.ok_or(CompanionConsolidationRunRepositoryError::Corrupt)?;
        tx.commit().map_err(failure)?;
        Ok(run)
    }

    fn commit_companion_consolidation_proposal(
        &self,
        job_id: JobId,
        checkpoint: CompanionConsolidationProposalCheckpoint,
    ) -> Result<CompanionConsolidationRun, CompanionConsolidationRunRepositoryError> {
        checkpoint.validate()?;
        let payload = encode_versioned(&checkpoint, PROPOSAL_FORMAT_VERSION).map_err(failure)?;
        let mut connection = self.connection().map_err(failure)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(failure)?;
        let current =
            load_in(&tx, job_id)?.ok_or(CompanionConsolidationRunRepositoryError::NotFound)?;
        if let Some(stored) = &current.proposal_checkpoint {
            if stored == &checkpoint {
                tx.commit().map_err(failure)?;
                return Ok(current);
            }
            return Err(CompanionConsolidationRunRepositoryError::Conflict);
        }
        let updated = tx
            .execute(
                "UPDATE companion_consolidation_runs
                    SET proposal_checkpoint_json = ?2, reduced_at = ?3
                  WHERE job_id = ?1 AND proposal_checkpoint_json IS NULL",
                params![job_id.to_string(), payload, checkpoint.reduced_at.get()],
            )
            .map_err(failure)?;
        if updated != 1 {
            return Err(CompanionConsolidationRunRepositoryError::Conflict);
        }
        let stored =
            load_in(&tx, job_id)?.ok_or(CompanionConsolidationRunRepositoryError::Failure)?;
        tx.commit().map_err(failure)?;
        Ok(stored)
    }
}
