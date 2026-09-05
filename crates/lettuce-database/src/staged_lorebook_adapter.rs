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

fn lorebook_error(
    error: lettuce_context::LorebookRepositoryError,
) -> StagedLorebookRepositoryError {
    use lettuce_context::LorebookRepositoryError;
    match error {
        LorebookRepositoryError::NotFound | LorebookRepositoryError::EntryNotFound => {
            StagedLorebookRepositoryError::NotFound
        }
        LorebookRepositoryError::Conflict => StagedLorebookRepositoryError::Conflict,
        LorebookRepositoryError::Invalid(_) => StagedLorebookRepositoryError::Invalid,
        LorebookRepositoryError::Failure(_) => StagedLorebookRepositoryError::Failure,
    }
}

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
        StagedLorebookStage::Committed => "committed",
        StagedLorebookStage::Cancelled => "cancelled",
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
    fn retry_staged_lorebook_planner(
        &self,
        request_id: RequestId,
        retry_id: RequestId,
        expected_revision: Revision,
        now: TimestampMillis,
    ) -> Result<StagedLorebookPlanningRun, StagedLorebookRepositoryError> {
        let mut connection = self.connection().map_err(failure)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(failure)?;
        let mut run =
            load_in(&transaction, request_id)?.ok_or(StagedLorebookRepositoryError::NotFound)?;
        if let Some(retry) = run
            .planner_retries
            .iter()
            .find(|retry| retry.retry_id == retry_id)
        {
            if retry.source_revision == expected_revision && retry.admitted_at == now {
                return Ok(run);
            }
            return Err(StagedLorebookRepositoryError::Conflict);
        }
        if run.project.stage != StagedLorebookStage::Planning
            || run.project.revision != expected_revision
            || now < run.project.updated_at
            || matches!(
                run.planner_attempt
                    .as_ref()
                    .map(|attempt| &attempt.decision),
                Some(lettuce_creation::StagedLorebookPlannerDecision::Outline(_))
            )
        {
            return Err(StagedLorebookRepositoryError::Conflict);
        }
        let job_id =
            crate::job_adapter::retry_staged_planner_job(&transaction, run.job_id, retry_id)
                .map_err(|error| match error {
                    lettuce_jobs::StoreError::IllegalTransition => {
                        StagedLorebookRepositoryError::Conflict
                    }
                    _ => StagedLorebookRepositoryError::Failure,
                })?;
        run.planner_retries
            .push(lettuce_creation::StagedLorebookPlannerRetry {
                retry_id,
                source_revision: expected_revision,
                previous_job_id: run.job_id,
                previous_attempt: run.planner_attempt.take(),
                admitted_at: now,
            });
        run.job_id = job_id;
        run.project.revision = expected_revision
            .next()
            .map_err(|_| StagedLorebookRepositoryError::Invalid)?;
        run.project.updated_at = now;
        run.validate()?;
        let encoded = encode_versioned(&run, RUN_FORMAT_VERSION).map_err(failure)?;
        if transaction.execute(
            "UPDATE creation_staged_lorebook_runs SET job_id = ?2, revision = ?3, updated_at = ?4, run_json = ?5 WHERE request_id = ?1 AND revision = ?6",
            params![request_id.to_string(), job_id.to_string(), i64::try_from(run.project.revision.get()).map_err(failure)?, now.get(), encoded, i64::try_from(expected_revision.get()).map_err(failure)?],
        ).map_err(failure)? != 1 { return Err(StagedLorebookRepositoryError::Conflict); }
        transaction.commit().map_err(failure)?;
        Ok(run)
    }

    fn edit_staged_lorebook_outline(
        &self,
        request_id: RequestId,
        expected_revision: Revision,
        mut outline: Vec<lettuce_creation::StagedLorebookEntryPlan>,
        now: TimestampMillis,
    ) -> Result<StagedLorebookPlanningRun, StagedLorebookRepositoryError> {
        for (ordinal, plan) in outline.iter_mut().enumerate() {
            plan.ordinal =
                u32::try_from(ordinal).map_err(|_| StagedLorebookRepositoryError::Invalid)?;
        }
        let mut connection = self.connection().map_err(failure)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(failure)?;
        let mut run =
            load_in(&transaction, request_id)?.ok_or(StagedLorebookRepositoryError::NotFound)?;
        if run.project.stage == StagedLorebookStage::AwaitingOutlineApproval
            && expected_revision.next().ok() == Some(run.project.revision)
            && run.project.updated_at == now
            && run.project.outline == outline
        {
            return Ok(run);
        }
        if run.project.revision != expected_revision {
            return Err(StagedLorebookRepositoryError::Conflict);
        }
        run.project = run
            .project
            .edit_outline(outline, now)
            .map_err(|_| StagedLorebookRepositoryError::Conflict)?;
        let encoded = encode_versioned(&run, RUN_FORMAT_VERSION).map_err(failure)?;
        if transaction.execute(
            "UPDATE creation_staged_lorebook_runs SET revision = ?2, updated_at = ?3, run_json = ?4 WHERE request_id = ?1 AND revision = ?5 AND stage = 'awaiting_outline_approval'",
            params![request_id.to_string(), i64::try_from(run.project.revision.get()).map_err(failure)?, now.get(), encoded, i64::try_from(expected_revision.get()).map_err(failure)?],
        ).map_err(failure)? != 1 { return Err(StagedLorebookRepositoryError::Conflict); }
        transaction.commit().map_err(failure)?;
        Ok(run)
    }

    fn cancel_staged_lorebook(
        &self,
        request_id: RequestId,
        expected_revision: Revision,
        now: TimestampMillis,
    ) -> Result<StagedLorebookPlanningRun, StagedLorebookRepositoryError> {
        let mut connection = self.connection().map_err(failure)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(failure)?;
        let mut run =
            load_in(&transaction, request_id)?.ok_or(StagedLorebookRepositoryError::NotFound)?;
        if run.project.stage == StagedLorebookStage::Cancelled
            && expected_revision.next().ok() == Some(run.project.revision)
            && run.project.updated_at == now
        {
            return Ok(run);
        }
        if run.project.revision != expected_revision {
            return Err(StagedLorebookRepositoryError::Conflict);
        }
        run.project = run
            .project
            .cancel(now)
            .map_err(|_| StagedLorebookRepositoryError::Conflict)?;
        crate::job_adapter::cancel_creation_project_jobs(&transaction, run.project.id, now)
            .map_err(failure)?;
        let encoded = encode_versioned(&run, RUN_FORMAT_VERSION).map_err(failure)?;
        if transaction.execute(
            "UPDATE creation_staged_lorebook_runs SET stage = 'cancelled', revision = ?2, updated_at = ?3, run_json = ?4 WHERE request_id = ?1 AND revision = ?5",
            params![request_id.to_string(), i64::try_from(run.project.revision.get()).map_err(failure)?, now.get(), encoded, i64::try_from(expected_revision.get()).map_err(failure)?],
        ).map_err(failure)? != 1 { return Err(StagedLorebookRepositoryError::Conflict); }
        transaction.commit().map_err(failure)?;
        Ok(run)
    }

    fn commit_staged_lorebook(
        &self,
        request: lettuce_creation::StagedLorebookCommitRequest,
    ) -> Result<lettuce_creation::StagedLorebookCommitReceipt, StagedLorebookRepositoryError> {
        use lettuce_context::{
            DetectionPolicy, KeywordMatchMode, LifecycleStatus, Lorebook, LorebookBehaviorVersion,
            LorebookDetails, LorebookEntry,
        };
        use lettuce_creation::{
            StagedLorebookCommitReceipt, StagedLorebookCommitTarget, StagedLorebookDraftStatus,
        };
        let mut connection = self.connection().map_err(failure)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(failure)?;
        let mut run = load_in(&transaction, request.project_request_id)?
            .ok_or(StagedLorebookRepositoryError::NotFound)?;
        if let Some(receipt) = &run.project.commit_receipt {
            if receipt.request != request {
                return Err(StagedLorebookRepositoryError::Conflict);
            }
            return Ok(receipt.clone());
        }
        if run.project.revision != request.expected_project_revision
            || run.project.stage != StagedLorebookStage::DraftsReady
            || request.now < run.project.updated_at
        {
            return Err(StagedLorebookRepositoryError::Conflict);
        }
        let mut details = match &request.target {
            StagedLorebookCommitTarget::New { id, name } => {
                let name = name
                    .as_deref()
                    .map(str::trim)
                    .filter(|name| !name.is_empty())
                    .or(run
                        .project
                        .initial_lorebook_name
                        .as_deref()
                        .map(str::trim)
                        .filter(|name| !name.is_empty()))
                    .ok_or(StagedLorebookRepositoryError::Invalid)?;
                LorebookDetails {
                    book: Lorebook {
                        id: *id,
                        status: LifecycleStatus::Active,
                        name: name.to_owned(),
                        detection_policy: DetectionPolicy::RecentMessageWindow,
                        icon_asset_id: None,
                        behavior_version: LorebookBehaviorVersion::LegacyV1,
                        revision: Revision::INITIAL,
                        created_at: request.now,
                        updated_at: request.now,
                    },
                    entries: Vec::new(),
                }
            }
            StagedLorebookCommitTarget::Existing {
                id,
                expected_revision,
            } => {
                let mut current = crate::lorebook_adapter::load_required(&transaction, *id)
                    .map_err(lorebook_error)?;
                if current.book.revision != *expected_revision
                    || current.book.status != LifecycleStatus::Active
                    || request.now < current.book.updated_at
                {
                    return Err(StagedLorebookRepositoryError::Conflict);
                }
                current.book.revision = expected_revision
                    .next()
                    .map_err(|_| StagedLorebookRepositoryError::Invalid)?;
                current.book.updated_at = request.now;
                current
            }
        };
        let mut created_entry_ids = Vec::new();
        for draft in run
            .project
            .drafts
            .iter()
            .filter(|draft| draft.status == StagedLorebookDraftStatus::Approved)
        {
            if draft.title.trim().is_empty() && draft.content.trim().is_empty() {
                continue;
            }
            details.entries.push(LorebookEntry {
                id: draft.plan_id,
                lorebook_id: details.book.id,
                title: draft.title.clone(),
                enabled: true,
                always_active: draft.always_active,
                keywords: draft.keywords.clone(),
                case_sensitive: false,
                match_mode: KeywordMatchMode::Literal,
                content: draft.content.clone(),
                priority: 0,
                ordinal: u32::try_from(details.entries.len())
                    .map_err(|_| StagedLorebookRepositoryError::Invalid)?,
                revision: Revision::INITIAL,
                created_at: request.now,
                updated_at: request.now,
            });
            created_entry_ids.push(draft.plan_id);
        }
        match &request.target {
            StagedLorebookCommitTarget::New { .. } => {
                crate::lorebook_adapter::insert_lorebook_details(&transaction, &details)
                    .map_err(lorebook_error)?;
            }
            StagedLorebookCommitTarget::Existing {
                expected_revision, ..
            } => {
                crate::lorebook_adapter::replace_lorebook_details(
                    &transaction,
                    *expected_revision,
                    &details,
                )
                .map_err(lorebook_error)?;
            }
        }
        let receipt = StagedLorebookCommitReceipt {
            request: request.clone(),
            lorebook_id: details.book.id,
            lorebook_revision: details.book.revision,
            created_entry_ids,
        };
        run.project.commit_receipt = Some(receipt.clone());
        run.project.stage = StagedLorebookStage::Committed;
        run.project.revision = request
            .expected_project_revision
            .next()
            .map_err(|_| StagedLorebookRepositoryError::Invalid)?;
        run.project.updated_at = request.now;
        run.validate()?;
        let encoded = encode_versioned(&run, RUN_FORMAT_VERSION).map_err(failure)?;
        if transaction.execute(
            "UPDATE creation_staged_lorebook_runs SET stage = 'committed', revision = ?2, updated_at = ?3, run_json = ?4 WHERE request_id = ?1 AND revision = ?5",
            params![request.project_request_id.to_string(), i64::try_from(run.project.revision.get()).map_err(failure)?, request.now.get(), encoded, i64::try_from(request.expected_project_revision.get()).map_err(failure)?],
        ).map_err(failure)? != 1 { return Err(StagedLorebookRepositoryError::Conflict); }
        transaction.commit().map_err(failure)?;
        Ok(receipt)
    }

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
        if current.project.revision < expected_revision
            || current
                .project
                .draft_batch
                .as_ref()
                .is_some_and(|batch| expected_revision < batch.revision)
        {
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
        inputs: Option<lettuce_creation::StagedLorebookWriterBatchInputs>,
        now: TimestampMillis,
    ) -> Result<StagedLorebookPlanningRun, StagedLorebookRepositoryError> {
        let mut connection = self.connection().map_err(failure)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(failure)?;
        let current =
            load_in(&transaction, request_id)?.ok_or(StagedLorebookRepositoryError::NotFound)?;
        if !matches!(
            current.project.stage,
            StagedLorebookStage::Drafting | StagedLorebookStage::DraftsReady
        ) {
            return Err(StagedLorebookRepositoryError::Conflict);
        }
        if current
            .project
            .drafts
            .iter()
            .any(|draft| draft.status == lettuce_creation::StagedLorebookDraftStatus::Drafting)
        {
            if inputs.is_some() && current.writer_batch_inputs != inputs {
                return Err(StagedLorebookRepositoryError::Conflict);
            }
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
        next.writer_batch_inputs = inputs;
        if next.project.draft_batch.is_none() {
            next.writer_batch_inputs = None;
        }
        next.validate()?;
        let encoded = encode_versioned(&next, RUN_FORMAT_VERSION).map_err(failure)?;
        if transaction.execute("UPDATE creation_staged_lorebook_runs SET stage = ?2, revision = ?3, updated_at = ?4, run_json = ?5 WHERE request_id = ?1 AND revision = ?6", params![request_id.to_string(), stage(next.project.stage), i64::try_from(next.project.revision.get()).map_err(failure)?, now.get(), encoded, i64::try_from(expected_revision.get()).map_err(failure)?]).map_err(failure)? != 1 { return Err(StagedLorebookRepositoryError::Conflict); }
        transaction.commit().map_err(failure)?;
        Ok(next)
    }

    fn fail_staged_lorebook_draft(
        &self,
        request_id: RequestId,
        plan_id: lettuce_types::LorebookEntryId,
        expected_revision: Revision,
        now: TimestampMillis,
    ) -> Result<StagedLorebookPlanningRun, StagedLorebookRepositoryError> {
        let mut connection = self.connection().map_err(failure)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(failure)?;
        let current =
            load_in(&transaction, request_id)?.ok_or(StagedLorebookRepositoryError::NotFound)?;
        if current.project.revision < expected_revision
            || current
                .project
                .draft_batch
                .as_ref()
                .is_some_and(|batch| expected_revision < batch.revision)
        {
            return Err(StagedLorebookRepositoryError::Conflict);
        }
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

    fn admit_staged_lorebook_coherence(
        &self,
        project_request_id: RequestId,
        run: lettuce_creation::StagedLorebookCoherenceRun,
    ) -> Result<StagedLorebookPlanningRun, StagedLorebookRepositoryError> {
        let mut connection = self.connection().map_err(failure)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(failure)?;
        let mut current = load_in(&transaction, project_request_id)?
            .ok_or(StagedLorebookRepositoryError::NotFound)?;
        run.validate(&current.project)?;
        if let Some(stored) = current
            .coherence_runs
            .iter()
            .find(|stored| stored.request_id == run.request_id)
        {
            if stored == &run {
                transaction.commit().map_err(failure)?;
                return Ok(current);
            }
            return Err(StagedLorebookRepositoryError::Conflict);
        }
        if current.project.stage != StagedLorebookStage::DraftsReady
            || current.project.revision != run.project_revision
        {
            return Err(StagedLorebookRepositoryError::Conflict);
        }
        current.coherence_runs.push(run);
        let encoded = encode_versioned(&current, RUN_FORMAT_VERSION).map_err(failure)?;
        if transaction.execute("UPDATE creation_staged_lorebook_runs SET run_json = ?2 WHERE request_id = ?1 AND revision = ?3 AND stage = 'drafts_ready'", params![project_request_id.to_string(), encoded, i64::try_from(current.project.revision.get()).map_err(failure)?]).map_err(failure)? != 1 { return Err(StagedLorebookRepositoryError::Conflict); }
        transaction.commit().map_err(failure)?;
        Ok(current)
    }

    fn commit_staged_lorebook_coherence_attempt(
        &self,
        project_request_id: RequestId,
        coherence_request_id: RequestId,
        attempt: lettuce_creation::StagedLorebookCoherenceAttempt,
    ) -> Result<StagedLorebookPlanningRun, StagedLorebookRepositoryError> {
        let mut connection = self.connection().map_err(failure)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(failure)?;
        let mut current = load_in(&transaction, project_request_id)?
            .ok_or(StagedLorebookRepositoryError::NotFound)?;
        attempt.validate(&current.project)?;
        let run = current
            .coherence_runs
            .iter_mut()
            .find(|run| run.request_id == coherence_request_id)
            .ok_or(StagedLorebookRepositoryError::NotFound)?;
        if let Some(stored) = &run.attempt {
            if stored == &attempt {
                transaction.commit().map_err(failure)?;
                return Ok(current);
            }
            return Err(StagedLorebookRepositoryError::Conflict);
        }
        if current.project.stage != StagedLorebookStage::DraftsReady
            || current.project.revision != run.project_revision
        {
            return Err(StagedLorebookRepositoryError::Conflict);
        }
        run.attempt = Some(attempt);
        let encoded = encode_versioned(&current, RUN_FORMAT_VERSION).map_err(failure)?;
        if transaction.execute("UPDATE creation_staged_lorebook_runs SET run_json = ?2 WHERE request_id = ?1 AND revision = ?3 AND stage = 'drafts_ready'", params![project_request_id.to_string(), encoded, i64::try_from(current.project.revision.get()).map_err(failure)?]).map_err(failure)? != 1 { return Err(StagedLorebookRepositoryError::Conflict); }
        transaction.commit().map_err(failure)?;
        Ok(current)
    }
}
