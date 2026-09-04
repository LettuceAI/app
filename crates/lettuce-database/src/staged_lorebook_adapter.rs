use std::str::FromStr;

use lettuce_creation::{
    StagedLorebookPlannerAttempt, StagedLorebookPlanningRun, StagedLorebookRepository,
    StagedLorebookRepositoryError, StagedLorebookStage,
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
        StagedLorebookStage::Drafting => "drafting",
        StagedLorebookStage::DraftsReady => "drafts_ready",
        StagedLorebookStage::CoherenceReview => "coherence_review",
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

    fn commit_staged_lorebook_planner_attempt(
        &self,
        request_id: RequestId,
        attempt: StagedLorebookPlannerAttempt,
    ) -> Result<StagedLorebookPlanningRun, StagedLorebookRepositoryError> {
        let mut connection = self.connection().map_err(failure)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(failure)?;
        let current =
            load_in(&transaction, request_id)?.ok_or(StagedLorebookRepositoryError::NotFound)?;
        attempt.validate(&current.project)?;
        if let Some(stored) = &current.planner_attempt {
            if stored == &attempt {
                transaction.commit().map_err(failure)?;
                return Ok(current);
            }
            return Err(StagedLorebookRepositoryError::Conflict);
        }
        if current.project.stage != StagedLorebookStage::Planning
            || current.project.revision != attempt.project_revision
        {
            return Err(StagedLorebookRepositoryError::Conflict);
        }
        let mut next = current;
        next.planner_attempt = Some(attempt);
        let encoded = encode_versioned(&next, RUN_FORMAT_VERSION).map_err(failure)?;
        if transaction
            .execute(
                "UPDATE creation_staged_lorebook_runs SET run_json = ?2
                 WHERE request_id = ?1 AND revision = ?3 AND stage = 'planning'",
                params![
                    request_id.to_string(),
                    encoded,
                    i64::try_from(next.project.revision.get()).map_err(failure)?,
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

    fn approve_staged_lorebook_outline(
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
        let next_revision = expected_revision
            .next()
            .map_err(|_| StagedLorebookRepositoryError::Conflict)?;
        if current.project.stage == StagedLorebookStage::Drafting
            && current.project.revision == next_revision
            && current.project.updated_at == now
        {
            transaction.commit().map_err(failure)?;
            return Ok(current);
        }
        if current.project.revision != expected_revision {
            return Err(StagedLorebookRepositoryError::Conflict);
        }
        let mut next = current;
        next.project = next
            .project
            .approve_outline(now)
            .map_err(|_| StagedLorebookRepositoryError::Conflict)?;
        let encoded = encode_versioned(&next, RUN_FORMAT_VERSION).map_err(failure)?;
        if transaction
            .execute(
                "UPDATE creation_staged_lorebook_runs
                 SET stage = ?2, revision = ?3, updated_at = ?4, run_json = ?5
                 WHERE request_id = ?1 AND revision = ?6 AND stage = 'awaiting_outline_approval'",
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

    fn settle_staged_lorebook_draft(
        &self,
        request_id: RequestId,
        expected_revision: Revision,
        draft: lettuce_creation::StagedLorebookEntryDraft,
        now: TimestampMillis,
    ) -> Result<StagedLorebookPlanningRun, StagedLorebookRepositoryError> {
        let mut connection = self.connection().map_err(failure)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(failure)?;
        let current =
            load_in(&transaction, request_id)?.ok_or(StagedLorebookRepositoryError::NotFound)?;
        if current.project.drafts.iter().any(|stored| stored == &draft) {
            transaction.commit().map_err(failure)?;
            return Ok(current);
        }
        if current.project.revision < expected_revision {
            return Err(StagedLorebookRepositoryError::Conflict);
        }
        let current_revision = current.project.revision;
        let settlement_at = now.max(current.project.updated_at);
        let mut next = current;
        next.project = next
            .project
            .settle_draft(draft, settlement_at)
            .map_err(|_| StagedLorebookRepositoryError::Conflict)?;
        let encoded = encode_versioned(&next, RUN_FORMAT_VERSION).map_err(failure)?;
        if transaction
            .execute(
                "UPDATE creation_staged_lorebook_runs
                 SET stage = ?2, revision = ?3, updated_at = ?4, run_json = ?5
                 WHERE request_id = ?1 AND revision = ?6 AND stage = 'drafting'",
                params![
                    request_id.to_string(),
                    stage(next.project.stage),
                    i64::try_from(next.project.revision.get()).map_err(failure)?,
                    settlement_at.get(),
                    encoded,
                    i64::try_from(current_revision.get()).map_err(failure)?,
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

    fn start_staged_lorebook_draft_batch(
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
        if current
            .project
            .drafts
            .iter()
            .any(|draft| draft.status == lettuce_creation::StagedLorebookDraftStatus::Drafting)
        {
            transaction.commit().map_err(failure)?;
            return Ok(current);
        }
        if current.project.revision != expected_revision {
            return Err(StagedLorebookRepositoryError::Conflict);
        }
        let mut next = current;
        next.project = next
            .project
            .start_draft_batch(now)
            .map_err(|_| StagedLorebookRepositoryError::Conflict)?;
        let encoded = encode_versioned(&next, RUN_FORMAT_VERSION).map_err(failure)?;
        if transaction.execute("UPDATE creation_staged_lorebook_runs SET stage = ?2, revision = ?3, updated_at = ?4, run_json = ?5 WHERE request_id = ?1 AND revision = ?6", params![request_id.to_string(), stage(next.project.stage), i64::try_from(next.project.revision.get()).map_err(failure)?, now.get(), encoded, i64::try_from(expected_revision.get()).map_err(failure)?]).map_err(failure)? != 1 { return Err(StagedLorebookRepositoryError::Conflict); }
        transaction.commit().map_err(failure)?;
        Ok(next)
    }

    fn fail_staged_lorebook_draft(
        &self,
        request_id: RequestId,
        plan_id: lettuce_types::LorebookEntryId,
        now: TimestampMillis,
    ) -> Result<StagedLorebookPlanningRun, StagedLorebookRepositoryError> {
        let mut connection = self.connection().map_err(failure)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(failure)?;
        let current =
            load_in(&transaction, request_id)?.ok_or(StagedLorebookRepositoryError::NotFound)?;
        if current.project.drafts.iter().any(|draft| {
            draft.plan_id == plan_id
                && draft.status == lettuce_creation::StagedLorebookDraftStatus::Failed
        }) {
            transaction.commit().map_err(failure)?;
            return Ok(current);
        }
        let current_revision = current.project.revision;
        let settlement_at = now.max(current.project.updated_at);
        let mut next = current;
        next.project = next
            .project
            .fail_draft(plan_id, settlement_at)
            .map_err(|_| StagedLorebookRepositoryError::Conflict)?;
        let encoded = encode_versioned(&next, RUN_FORMAT_VERSION).map_err(failure)?;
        if transaction.execute("UPDATE creation_staged_lorebook_runs SET stage = ?2, revision = ?3, updated_at = ?4, run_json = ?5 WHERE request_id = ?1 AND revision = ?6 AND stage = 'drafting'", params![request_id.to_string(), stage(next.project.stage), i64::try_from(next.project.revision.get()).map_err(failure)?, settlement_at.get(), encoded, i64::try_from(current_revision.get()).map_err(failure)?]).map_err(failure)? != 1 { return Err(StagedLorebookRepositoryError::Conflict); }
        transaction.commit().map_err(failure)?;
        Ok(next)
    }

    fn edit_staged_lorebook_draft(
        &self,
        request_id: RequestId,
        expected_revision: Revision,
        edit: lettuce_creation::StagedLorebookDraftEdit,
        now: TimestampMillis,
    ) -> Result<StagedLorebookPlanningRun, StagedLorebookRepositoryError> {
        let mut connection = self.connection().map_err(failure)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(failure)?;
        let current =
            load_in(&transaction, request_id)?.ok_or(StagedLorebookRepositoryError::NotFound)?;
        let normalized_keywords = edit
            .keywords
            .iter()
            .map(|keyword| keyword.trim().to_owned())
            .filter(|keyword| !keyword.is_empty())
            .collect::<Vec<_>>();
        let next_revision = expected_revision
            .next()
            .map_err(|_| StagedLorebookRepositoryError::Conflict)?;
        if current.project.revision == next_revision
            && current.project.updated_at == now
            && current.project.drafts.iter().any(|draft| {
                draft.plan_id == edit.plan_id
                    && draft.title == edit.title
                    && draft.keywords == normalized_keywords
                    && draft.content == edit.content
                    && draft.always_active == edit.always_active
            })
        {
            transaction.commit().map_err(failure)?;
            return Ok(current);
        }
        if current.project.revision != expected_revision {
            return Err(StagedLorebookRepositoryError::Conflict);
        }
        let mut next = current;
        next.project = next
            .project
            .edit_draft(edit, now)
            .map_err(|_| StagedLorebookRepositoryError::Conflict)?;
        let encoded = encode_versioned(&next, RUN_FORMAT_VERSION).map_err(failure)?;
        if transaction.execute("UPDATE creation_staged_lorebook_runs SET stage = ?2, revision = ?3, updated_at = ?4, run_json = ?5 WHERE request_id = ?1 AND revision = ?6", params![request_id.to_string(), stage(next.project.stage), i64::try_from(next.project.revision.get()).map_err(failure)?, now.get(), encoded, i64::try_from(expected_revision.get()).map_err(failure)?]).map_err(failure)? != 1 { return Err(StagedLorebookRepositoryError::Conflict); }
        transaction.commit().map_err(failure)?;
        Ok(next)
    }

    fn set_staged_lorebook_draft_approved(
        &self,
        request_id: RequestId,
        expected_revision: Revision,
        plan_id: lettuce_types::LorebookEntryId,
        approved: bool,
        now: TimestampMillis,
    ) -> Result<StagedLorebookPlanningRun, StagedLorebookRepositoryError> {
        let mut connection = self.connection().map_err(failure)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(failure)?;
        let current =
            load_in(&transaction, request_id)?.ok_or(StagedLorebookRepositoryError::NotFound)?;
        let status = if approved {
            lettuce_creation::StagedLorebookDraftStatus::Approved
        } else {
            lettuce_creation::StagedLorebookDraftStatus::Drafted
        };
        let next_revision = expected_revision
            .next()
            .map_err(|_| StagedLorebookRepositoryError::Conflict)?;
        if current.project.revision == next_revision
            && current.project.updated_at == now
            && current
                .project
                .drafts
                .iter()
                .any(|draft| draft.plan_id == plan_id && draft.status == status)
        {
            transaction.commit().map_err(failure)?;
            return Ok(current);
        }
        if current.project.revision != expected_revision {
            return Err(StagedLorebookRepositoryError::Conflict);
        }
        let mut next = current;
        next.project = next
            .project
            .set_draft_approved(plan_id, approved, now)
            .map_err(|_| StagedLorebookRepositoryError::Conflict)?;
        let encoded = encode_versioned(&next, RUN_FORMAT_VERSION).map_err(failure)?;
        if transaction.execute("UPDATE creation_staged_lorebook_runs SET stage = ?2, revision = ?3, updated_at = ?4, run_json = ?5 WHERE request_id = ?1 AND revision = ?6", params![request_id.to_string(), stage(next.project.stage), i64::try_from(next.project.revision.get()).map_err(failure)?, now.get(), encoded, i64::try_from(expected_revision.get()).map_err(failure)?]).map_err(failure)? != 1 { return Err(StagedLorebookRepositoryError::Conflict); }
        transaction.commit().map_err(failure)?;
        Ok(next)
    }

    fn settle_staged_lorebook_refinement(
        &self,
        request_id: RequestId,
        expected_revision: Revision,
        draft: lettuce_creation::StagedLorebookEntryDraft,
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
        if current.project.revision == next_revision
            && current.project.updated_at == now
            && current.project.drafts.iter().any(|stored| stored == &draft)
        {
            transaction.commit().map_err(failure)?;
            return Ok(current);
        }
        if current.project.revision != expected_revision {
            return Err(StagedLorebookRepositoryError::Conflict);
        }
        let mut next = current;
        next.project = next
            .project
            .settle_refinement(draft, now)
            .map_err(|_| StagedLorebookRepositoryError::Conflict)?;
        let encoded = encode_versioned(&next, RUN_FORMAT_VERSION).map_err(failure)?;
        if transaction.execute("UPDATE creation_staged_lorebook_runs SET stage = ?2, revision = ?3, updated_at = ?4, run_json = ?5 WHERE request_id = ?1 AND revision = ?6", params![request_id.to_string(), stage(next.project.stage), i64::try_from(next.project.revision.get()).map_err(failure)?, now.get(), encoded, i64::try_from(expected_revision.get()).map_err(failure)?]).map_err(failure)? != 1 { return Err(StagedLorebookRepositoryError::Conflict); }
        transaction.commit().map_err(failure)?;
        Ok(next)
    }

    fn submit_staged_lorebook_coherence(
        &self,
        request_id: RequestId,
        expected_revision: Revision,
        proposals: Vec<lettuce_creation::StagedLorebookCoherenceChange>,
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
        if current.project.revision == next_revision
            && current.project.updated_at == now
            && current.project.stage == StagedLorebookStage::CoherenceReview
            && current.project.coherence_proposals == proposals
        {
            transaction.commit().map_err(failure)?;
            return Ok(current);
        }
        if current.project.revision != expected_revision {
            return Err(StagedLorebookRepositoryError::Conflict);
        }
        let mut next = current;
        next.project = next
            .project
            .submit_coherence_proposals(proposals, now)
            .map_err(|_| StagedLorebookRepositoryError::Conflict)?;
        let encoded = encode_versioned(&next, RUN_FORMAT_VERSION).map_err(failure)?;
        if transaction.execute("UPDATE creation_staged_lorebook_runs SET stage = ?2, revision = ?3, updated_at = ?4, run_json = ?5 WHERE request_id = ?1 AND revision = ?6", params![request_id.to_string(), stage(next.project.stage), i64::try_from(next.project.revision.get()).map_err(failure)?, now.get(), encoded, i64::try_from(expected_revision.get()).map_err(failure)?]).map_err(failure)? != 1 { return Err(StagedLorebookRepositoryError::Conflict); }
        transaction.commit().map_err(failure)?;
        Ok(next)
    }

    fn apply_staged_lorebook_coherence(
        &self,
        request_id: RequestId,
        expected_revision: Revision,
        accepted_change_ids: Vec<String>,
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
        if current.project.revision == next_revision
            && current.project.updated_at == now
            && current.project.stage == StagedLorebookStage::DraftsReady
            && current
                .project
                .last_coherence_application
                .as_ref()
                .is_some_and(|receipt| {
                    receipt.source_revision == expected_revision
                        && receipt.accepted_change_ids == accepted_change_ids
                })
        {
            transaction.commit().map_err(failure)?;
            return Ok(current);
        }
        if current.project.revision != expected_revision {
            return Err(StagedLorebookRepositoryError::Conflict);
        }
        let mut next = current;
        next.project = next
            .project
            .apply_coherence(&accepted_change_ids, now)
            .map_err(|_| StagedLorebookRepositoryError::Conflict)?;
        let encoded = encode_versioned(&next, RUN_FORMAT_VERSION).map_err(failure)?;
        if transaction.execute("UPDATE creation_staged_lorebook_runs SET stage = ?2, revision = ?3, updated_at = ?4, run_json = ?5 WHERE request_id = ?1 AND revision = ?6", params![request_id.to_string(), stage(next.project.stage), i64::try_from(next.project.revision.get()).map_err(failure)?, now.get(), encoded, i64::try_from(expected_revision.get()).map_err(failure)?]).map_err(failure)? != 1 { return Err(StagedLorebookRepositoryError::Conflict); }
        transaction.commit().map_err(failure)?;
        Ok(next)
    }
}
