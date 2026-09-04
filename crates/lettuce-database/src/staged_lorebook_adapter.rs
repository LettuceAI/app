use std::str::FromStr;

use lettuce_creation::{
    StagedLorebookPlanningRun, StagedLorebookRepository, StagedLorebookRepositoryError,
    StagedLorebookStage,
};
use lettuce_types::{
    CreationWorkflowId, JobId, ModelProfileId, PromptDocumentId, RequestId, Revision,
    TimestampMillis,
};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use crate::{Database, decode_versioned, encode_versioned};

const RUN_FORMAT_VERSION: u32 = 1;

fn failure(_: impl std::fmt::Debug) -> StagedLorebookRepositoryError {
    StagedLorebookRepositoryError::Failure
}

fn corrupt(_: impl std::fmt::Debug) -> StagedLorebookRepositoryError {
    StagedLorebookRepositoryError::Corrupt
}

fn parse_id<T: FromStr>(value: &str) -> Result<T, StagedLorebookRepositoryError> {
    value
        .parse()
        .map_err(|_| StagedLorebookRepositoryError::Corrupt)
}

fn stage(value: StagedLorebookStage) -> &'static str {
    match value {
        StagedLorebookStage::Created => "created",
        StagedLorebookStage::Planning => "planning",
        StagedLorebookStage::AwaitingOutlineApproval => "awaiting_outline_approval",
    }
}

fn load_in(
    transaction: &Transaction<'_>,
    request_id: RequestId,
) -> Result<Option<StagedLorebookPlanningRun>, StagedLorebookRepositoryError> {
    let row = transaction
        .query_row(
            "SELECT project_id, job_id, model_profile_id, prompt_id, prompt_revision, stage,
                    revision, created_at, updated_at, run_json
             FROM creation_staged_lorebook_runs WHERE request_id = ?1",
            [request_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, i64>(8)?,
                    row.get::<_, String>(9)?,
                ))
            },
        )
        .optional()
        .map_err(corrupt)?;
    let Some((
        project_id,
        job_id,
        model_id,
        prompt_id,
        prompt_revision,
        stored_stage,
        revision,
        created_at,
        updated_at,
        encoded,
    )) = row
    else {
        return Ok(None);
    };
    let run = decode_versioned::<StagedLorebookPlanningRun>(&encoded, RUN_FORMAT_VERSION)
        .map_err(corrupt)?;
    let prompt_revision = positive_revision(prompt_revision)?;
    let revision = positive_revision(revision)?;
    if run.request_id != request_id
        || run.project.id != parse_id::<CreationWorkflowId>(&project_id)?
        || run.job_id != parse_id::<JobId>(&job_id)?
        || run.planner_profile.chat_profile.model_profile_id
            != parse_id::<ModelProfileId>(&model_id)?
        || run.planner_prompt_id != parse_id::<PromptDocumentId>(&prompt_id)?
        || run.planner_prompt_revision != prompt_revision
        || stage(run.project.stage) != stored_stage
        || run.project.revision != revision
        || run.project.created_at != TimestampMillis::new(created_at)
        || run.project.updated_at != TimestampMillis::new(updated_at)
        || run.validate().is_err()
    {
        return Err(StagedLorebookRepositoryError::Corrupt);
    }
    Ok(Some(run))
}

fn positive_revision(value: i64) -> Result<Revision, StagedLorebookRepositoryError> {
    u64::try_from(value)
        .ok()
        .filter(|value| *value > 0)
        .map(Revision::new)
        .ok_or(StagedLorebookRepositoryError::Corrupt)
}

impl StagedLorebookRepository for Database {
    fn admit_staged_lorebook(
        &self,
        run: StagedLorebookPlanningRun,
    ) -> Result<StagedLorebookPlanningRun, StagedLorebookRepositoryError> {
        run.validate()?;
        if run.project.stage != StagedLorebookStage::Created
            || run.project.revision != Revision::new(1)
        {
            return Err(StagedLorebookRepositoryError::Invalid);
        }
        let encoded = encode_versioned(&run, RUN_FORMAT_VERSION).map_err(failure)?;
        let mut connection = self.connection().map_err(failure)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(failure)?;
        let inserted = transaction
            .execute(
                "INSERT OR IGNORE INTO creation_staged_lorebook_runs
             (request_id, project_id, job_id, model_profile_id, prompt_id, prompt_revision, stage,
              revision, created_at, updated_at, run_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    run.request_id.to_string(),
                    run.project.id.to_string(),
                    run.job_id.to_string(),
                    run.planner_profile
                        .chat_profile
                        .model_profile_id
                        .to_string(),
                    run.planner_prompt_id.to_string(),
                    i64::try_from(run.planner_prompt_revision.get()).map_err(failure)?,
                    stage(run.project.stage),
                    i64::try_from(run.project.revision.get()).map_err(failure)?,
                    run.project.created_at.get(),
                    run.project.updated_at.get(),
                    encoded
                ],
            )
            .map_err(failure)?;
        let stored =
            load_in(&transaction, run.request_id)?.ok_or(StagedLorebookRepositoryError::Failure)?;
        if inserted == 0 && stored != run {
            return Err(StagedLorebookRepositoryError::Conflict);
        }
        transaction.commit().map_err(failure)?;
        Ok(stored)
    }

    fn load_staged_lorebook(
        &self,
        request_id: RequestId,
    ) -> Result<StagedLorebookPlanningRun, StagedLorebookRepositoryError> {
        let mut connection = self.connection().map_err(failure)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(failure)?;
        let run =
            load_in(&transaction, request_id)?.ok_or(StagedLorebookRepositoryError::NotFound)?;
        transaction.commit().map_err(failure)?;
        Ok(run)
    }

    fn start_staged_lorebook_planning(
        &self,
        request_id: RequestId,
        expected_revision: Revision,
        now: TimestampMillis,
    ) -> Result<StagedLorebookPlanningRun, StagedLorebookRepositoryError> {
        let mut connection = self.connection().map_err(failure)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(failure)?;
        let current =
            load_in(&transaction, request_id)?.ok_or(StagedLorebookRepositoryError::NotFound)?;
        if current.project.stage == StagedLorebookStage::Planning
            && current.project.revision
                == expected_revision
                    .next()
                    .map_err(|_| StagedLorebookRepositoryError::Conflict)?
            && current.project.updated_at == now
        {
            transaction.commit().map_err(failure)?;
            return Ok(current);
        }
        if current.project.revision != expected_revision {
            return Err(StagedLorebookRepositoryError::Conflict);
        }
        let mut next = current.clone();
        next.project = current
            .project
            .start_planning(now)
            .map_err(|_| StagedLorebookRepositoryError::Conflict)?;
        let encoded = encode_versioned(&next, RUN_FORMAT_VERSION).map_err(failure)?;
        if transaction.execute(
            "UPDATE creation_staged_lorebook_runs SET stage = ?2, revision = ?3, updated_at = ?4, run_json = ?5
             WHERE request_id = ?1 AND revision = ?6 AND stage = 'created'",
            params![request_id.to_string(), stage(next.project.stage), i64::try_from(next.project.revision.get()).map_err(failure)?, now.get(), encoded, i64::try_from(expected_revision.get()).map_err(failure)?],
        ).map_err(failure)? != 1 { return Err(StagedLorebookRepositoryError::Conflict); }
        transaction.commit().map_err(failure)?;
        Ok(next)
    }

    fn submit_staged_lorebook_outline(
        &self,
        request_id: RequestId,
        expected_revision: Revision,
        outline: Vec<lettuce_creation::StagedLorebookEntryPlan>,
        now: TimestampMillis,
    ) -> Result<StagedLorebookPlanningRun, StagedLorebookRepositoryError> {
        let mut connection = self.connection().map_err(failure)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(failure)?;
        let current =
            load_in(&transaction, request_id)?.ok_or(StagedLorebookRepositoryError::NotFound)?;
        let next_revision = expected_revision
            .next()
            .map_err(|_| StagedLorebookRepositoryError::Conflict)?;
        if current.project.stage == StagedLorebookStage::AwaitingOutlineApproval
            && current.project.revision == next_revision
            && current.project.updated_at == now
            && current.project.outline == outline
        {
            transaction.commit().map_err(failure)?;
            return Ok(current);
        }
        if current.project.revision != expected_revision {
            return Err(StagedLorebookRepositoryError::Conflict);
        }
        let mut next = current.clone();
        next.project = current
            .project
            .submit_outline(outline, now)
            .map_err(|_| StagedLorebookRepositoryError::Conflict)?;
        let encoded = encode_versioned(&next, RUN_FORMAT_VERSION).map_err(failure)?;
        if transaction
            .execute(
                "UPDATE creation_staged_lorebook_runs
                 SET stage = ?2, revision = ?3, updated_at = ?4, run_json = ?5
                 WHERE request_id = ?1 AND revision = ?6 AND stage = 'planning'",
                params![
                    request_id.to_string(),
                    stage(next.project.stage),
                    i64::try_from(next.project.revision.get()).map_err(failure)?,
                    now.get(),
                    encoded,
                    i64::try_from(expected_revision.get()).map_err(failure)?,
                ],
            )
            .map_err(failure)?
            != 1
        {
            return Err(StagedLorebookRepositoryError::Conflict);
        }
        transaction.commit().map_err(failure)?;
        Ok(next)
    }
}
