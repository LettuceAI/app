use super::local::internal;
use super::{ClaimedJob, JobHandler, JobLane, JobProgressSink};
use crate::api::ApiContext;
use async_trait::async_trait;
use lettuce_contracts::ApiError;
use lettuce_jobs::{
    CancellationReason, JobKind, JobSnapshot, JobStore, OutcomeRef, ResourceAvailability, WorkerId,
    handle::CancellationToken,
};
use lettuce_types::RequestId;
use std::{sync::Arc, time::Duration};

#[derive(Debug, Clone, Copy)]
pub struct LorebookHandler;

const PLANNER_RETRY_PREFIX: &str = "staged-planner-retry-";

fn is_planner_key(key: &str) -> bool {
    key.starts_with(PLANNER_RETRY_PREFIX)
        || (key.starts_with("staged-lorebook-")
            && !key.starts_with("staged-lorebook-writer-")
            && !key.starts_with("staged-lorebook-refine-")
            && !key.starts_with("staged-lorebook-coherence-"))
}

fn is_staged_key(key: &str) -> bool {
    key.starts_with("staged-lorebook-") || key.starts_with(PLANNER_RETRY_PREFIX)
}

fn request_id(context: &ApiContext, job: &JobSnapshot) -> Result<RequestId, ApiError> {
    let events = context
        .backend()
        .database()
        .events_since(job.id, None, 1)
        .map_err(internal)?;
    match events.first().map(|event| &event.event) {
        Some(lettuce_jobs::events::JobEvent::Created {
            input_ref: OutcomeRef::Request(id),
            ..
        }) => Ok(*id),
        _ => Err(internal("the lorebook job request is missing")),
    }
}

#[async_trait]
impl JobHandler for LorebookHandler {
    fn kinds(&self) -> &[JobKind] {
        &[JobKind::CreationRun]
    }
    fn lane(&self, context: &ApiContext, job: &JobSnapshot) -> Option<JobLane> {
        let key = job.idempotency_key.as_ref().map_or("", |key| key.as_str());
        if is_staged_key(key)
            || key.starts_with("lorebook-entry-generator-")
            || key.starts_with("lorebook-keyword-generator-")
        {
            if is_planner_key(key) {
                use lettuce_creation::StagedLorebookRepository;
                let id = request_id(context, job).ok()?;
                let run = context.backend().database().load_staged_lorebook(id).ok()?;
                if run.project.stage == lettuce_creation::StagedLorebookStage::Created {
                    return None;
                }
            }
            Some(JobLane(format!("lorebook:{}", job.id)))
        } else {
            None
        }
    }
    fn not_before(
        &self,
        _context: &ApiContext,
        job: &JobSnapshot,
    ) -> Option<lettuce_types::TimestampMillis> {
        super::memory::retry_due(job)
    }
    async fn claim(
        &self,
        context: &ApiContext,
        job: &JobSnapshot,
        worker: WorkerId,
    ) -> Result<Option<Box<dyn ClaimedJob>>, ApiError> {
        let job = job.clone();
        context
            .blocking(move |context| {
                let id = request_id(context, &job)?;
                let db = context.backend().database();
                let now = context.now();
                let lease = Duration::from_secs(3600);
                let allowed = ResourceAvailability::all();
                let key = job.idempotency_key.as_ref().map_or("", |key| key.as_str());
                if key.starts_with("lorebook-entry-generator-") {
                    return crate::LorebookEntryDispatchCoordinator::new(db, db)
                        .claim(id, worker, now, lease, &allowed)
                        .map(|work| {
                            work.map(|work| Box::new(ClaimedEntry(work)) as Box<dyn ClaimedJob>)
                        })
                        .map_err(internal);
                }
                if key.starts_with("lorebook-keyword-generator-") {
                    return crate::LorebookKeywordDispatchCoordinator::new(db, db)
                        .claim(id, worker, now, lease, &allowed)
                        .map(|work| {
                            work.map(|work| Box::new(ClaimedKeyword(work)) as Box<dyn ClaimedJob>)
                        })
                        .map_err(internal);
                }
                if key.starts_with("staged-lorebook-writer-")
                    || key.starts_with("staged-lorebook-refine-")
                {
                    return crate::StagedLorebookWriterDispatchCoordinator::new(db, db, db)
                        .claim(id, worker, now, lease, &allowed)
                        .map(|work| {
                            work.map(|work| Box::new(ClaimedWriter(work)) as Box<dyn ClaimedJob>)
                        })
                        .map_err(internal);
                }
                if key.starts_with("staged-lorebook-coherence-") {
                    let project = db
                        .staged_lorebook_request_for_project(job.subject.id.as_str())
                        .map_err(internal)?;
                    return crate::StagedLorebookCoherenceDispatchCoordinator::new(db, db)
                        .claim(project, id, worker, now, lease, &allowed)
                        .map(|work| {
                            work.map(|work| Box::new(ClaimedCoherence(work)) as Box<dyn ClaimedJob>)
                        })
                        .map_err(internal);
                }
                if is_planner_key(key) {
                    return crate::StagedLorebookPlannerDispatchCoordinator::new(db, db)
                        .claim(id, worker, now, lease, &allowed)
                        .map(|work| {
                            work.map(|work| Box::new(ClaimedPlanner(work)) as Box<dyn ClaimedJob>)
                        })
                        .map_err(internal);
                }
                Err(internal("the creation job is not a lorebook job"))
            })
            .await
    }
}

async fn forward(
    mut receiver: lettuce_inference::InferenceStreamReceiver,
    progress: Arc<dyn JobProgressSink>,
) {
    while let Some(envelope) = receiver.recv().await {
        match envelope.event {
            lettuce_conversations::GenerationStreamEvent::TextDelta { text } => {
                progress.text_delta(Some(text), None)
            }
            lettuce_conversations::GenerationStreamEvent::ReasoningDelta { text } => {
                progress.text_delta(None, Some(text))
            }
        }
    }
}

struct ClaimedEntry(crate::LorebookEntryClaimedWork);
#[async_trait]
impl ClaimedJob for ClaimedEntry {
    fn cancellation(&self) -> CancellationToken {
        self.0.handle.cancellation_token()
    }
    async fn run(
        self: Box<Self>,
        context: ApiContext,
        progress: Arc<dyn JobProgressSink>,
    ) -> Result<(), ApiError> {
        let db = context.backend().database();
        let runtime = context.backend().inference_runtime();
        let sink = RequestId::new();
        let receiver = runtime.register_stream(sink).map_err(internal)?;
        let forwarder = tokio::spawn(forward(receiver, progress));
        let inference = crate::companion::companion_memory_host::JobOutputInference::new(
            context.inference(),
            Some(runtime),
            Some(sink),
        );
        let prompt = self.0.run.prompt_snapshot.clone();
        let result = crate::LorebookEntryExecutionCoordinator::new(db, &inference)
            .with_job_attempt(self.0.claim.claim.attempt.get())
            .run(
                self.0.run.request_id,
                &prompt,
                &self.0.handle,
                Some(sink),
                context.now(),
            )
            .await;
        runtime.unregister_stream(sink).map_err(internal)?;
        forwarder.await.map_err(internal)?;
        if context.shutdown_token().is_cancelled()
            && db
                .get(self.0.job.id)
                .map_err(internal)?
                .is_some_and(|job| job.state == lettuce_jobs::JobState::Running)
        {
            db.append_and_transition(lettuce_jobs::JobMutation::RetryScheduled {
                claim: self.0.claim.claim,
                at: context.now().max(self.0.job.updated_at),
            })
            .map_err(internal)?;
            return Ok(());
        }
        crate::LorebookEntryDispatchCoordinator::new(db, db)
            .settle(self.0, result, CancellationReason::User, context.now())
            .map_err(internal)?;
        Ok(())
    }
}

struct ClaimedKeyword(crate::LorebookKeywordClaimedWork);
#[async_trait]
impl ClaimedJob for ClaimedKeyword {
    fn cancellation(&self) -> CancellationToken {
        self.0.handle.cancellation_token()
    }
    async fn run(
        self: Box<Self>,
        context: ApiContext,
        progress: Arc<dyn JobProgressSink>,
    ) -> Result<(), ApiError> {
        let db = context.backend().database();
        let runtime = context.backend().inference_runtime();
        let sink = RequestId::new();
        let receiver = runtime.register_stream(sink).map_err(internal)?;
        let forwarder = tokio::spawn(forward(receiver, progress));
        let inference = crate::companion::companion_memory_host::JobOutputInference::new(
            context.inference(),
            Some(runtime),
            Some(sink),
        );
        let prompt = self.0.run.prompt_snapshot.clone();
        let result = crate::LorebookKeywordExecutionCoordinator::new(db, &inference)
            .with_job_attempt(self.0.claim.claim.attempt.get())
            .run(
                self.0.run.request_id,
                &prompt,
                &self.0.handle,
                Some(sink),
                context.now(),
            )
            .await;
        runtime.unregister_stream(sink).map_err(internal)?;
        forwarder.await.map_err(internal)?;
        if context.shutdown_token().is_cancelled()
            && db
                .get(self.0.job.id)
                .map_err(internal)?
                .is_some_and(|job| job.state == lettuce_jobs::JobState::Running)
        {
            db.append_and_transition(lettuce_jobs::JobMutation::RetryScheduled {
                claim: self.0.claim.claim,
                at: context.now().max(self.0.job.updated_at),
            })
            .map_err(internal)?;
            return Ok(());
        }
        crate::LorebookKeywordDispatchCoordinator::new(db, db)
            .settle(self.0, result, CancellationReason::User, context.now())
            .map_err(internal)?;
        Ok(())
    }
}

struct ClaimedWriter(crate::StagedLorebookWriterClaimedWork);
#[async_trait]
impl ClaimedJob for ClaimedWriter {
    fn cancellation(&self) -> CancellationToken {
        self.0.handle.cancellation_token()
    }
    async fn run(
        self: Box<Self>,
        context: ApiContext,
        progress: Arc<dyn JobProgressSink>,
    ) -> Result<(), ApiError> {
        let db = context.backend().database();
        let Some(prompt) = self.0.run.prompt_snapshot.clone() else {
            crate::StagedLorebookWriterDispatchCoordinator::new(db, db, db)
                .settle(
                    self.0,
                    Err(crate::StagedLorebookWriterExecutionError::InvalidPrompt),
                    CancellationReason::User,
                    context.now(),
                )
                .map_err(internal)?;
            return Ok(());
        };
        let runtime = context.backend().inference_runtime();
        let sink = RequestId::new();
        let receiver = runtime.register_stream(sink).map_err(internal)?;
        let forwarder = tokio::spawn(forward(receiver, progress));
        let inference = crate::companion::companion_memory_host::JobOutputInference::new(
            context.inference(),
            Some(runtime),
            Some(sink),
        );
        let result = crate::StagedLorebookWriterExecutionCoordinator::new(db, db, &inference)
            .with_job_attempt(self.0.claim.claim.attempt.get())
            .run(
                self.0.run.request_id,
                &prompt,
                &self.0.handle,
                Some(sink),
                context.now(),
            )
            .await;
        runtime.unregister_stream(sink).map_err(internal)?;
        forwarder.await.map_err(internal)?;
        if context.shutdown_token().is_cancelled()
            && db
                .get(self.0.job.id)
                .map_err(internal)?
                .is_some_and(|job| job.state == lettuce_jobs::JobState::Running)
        {
            db.append_and_transition(lettuce_jobs::JobMutation::RetryScheduled {
                claim: self.0.claim.claim,
                at: context.now().max(self.0.job.updated_at),
            })
            .map_err(internal)?;
            return Ok(());
        }
        crate::StagedLorebookWriterDispatchCoordinator::new(db, db, db)
            .settle(self.0, result, CancellationReason::User, context.now())
            .map_err(internal)?;
        Ok(())
    }
}

struct ClaimedCoherence(crate::StagedLorebookCoherenceClaimedWork);
#[async_trait]
impl ClaimedJob for ClaimedCoherence {
    fn cancellation(&self) -> CancellationToken {
        self.0.handle.cancellation_token()
    }
    async fn run(
        self: Box<Self>,
        context: ApiContext,
        progress: Arc<dyn JobProgressSink>,
    ) -> Result<(), ApiError> {
        let db = context.backend().database();
        let Some(prompt) = self.0.run.prompt_snapshot.clone() else {
            crate::StagedLorebookCoherenceDispatchCoordinator::new(db, db)
                .settle(
                    self.0,
                    Err(crate::StagedLorebookCoherenceExecutionError::InvalidPrompt),
                    CancellationReason::User,
                    context.now(),
                )
                .map_err(internal)?;
            return Ok(());
        };
        let runtime = context.backend().inference_runtime();
        let sink = RequestId::new();
        let receiver = runtime.register_stream(sink).map_err(internal)?;
        let forwarder = tokio::spawn(forward(receiver, progress));
        let inference = crate::companion::companion_memory_host::JobOutputInference::new(
            context.inference(),
            Some(runtime),
            Some(sink),
        );
        let result = crate::StagedLorebookCoherenceExecutionCoordinator::new(db, &inference)
            .with_job_attempt(self.0.claim.claim.attempt.get())
            .run(
                self.0.project_request_id,
                self.0.run.request_id,
                &prompt,
                &self.0.handle,
                Some(sink),
                context.now(),
            )
            .await;
        runtime.unregister_stream(sink).map_err(internal)?;
        forwarder.await.map_err(internal)?;
        if context.shutdown_token().is_cancelled()
            && db
                .get(self.0.job.id)
                .map_err(internal)?
                .is_some_and(|job| job.state == lettuce_jobs::JobState::Running)
        {
            db.append_and_transition(lettuce_jobs::JobMutation::RetryScheduled {
                claim: self.0.claim.claim,
                at: context.now().max(self.0.job.updated_at),
            })
            .map_err(internal)?;
            return Ok(());
        }
        crate::StagedLorebookCoherenceDispatchCoordinator::new(db, db)
            .settle(self.0, result, CancellationReason::User, context.now())
            .map_err(internal)?;
        Ok(())
    }
}

struct ClaimedPlanner(crate::StagedLorebookPlannerClaimedWork);
#[async_trait]
impl ClaimedJob for ClaimedPlanner {
    fn cancellation(&self) -> CancellationToken {
        self.0.handle.cancellation_token()
    }
    async fn run(
        self: Box<Self>,
        context: ApiContext,
        progress: Arc<dyn JobProgressSink>,
    ) -> Result<(), ApiError> {
        let db = context.backend().database();
        let Some(prompt) = self.0.run.planner_prompt_snapshot.clone() else {
            crate::StagedLorebookPlannerDispatchCoordinator::new(db, db)
                .settle(
                    self.0,
                    Err(crate::StagedLorebookPlannerExecutionError::InvalidPrompt),
                    CancellationReason::User,
                    context.now(),
                )
                .map_err(internal)?;
            return Ok(());
        };
        let runtime = context.backend().inference_runtime();
        let sink = RequestId::new();
        let receiver = runtime.register_stream(sink).map_err(internal)?;
        let forwarder = tokio::spawn(forward(receiver, progress));
        let inference = crate::companion::companion_memory_host::JobOutputInference::new(
            context.inference(),
            Some(runtime),
            Some(sink),
        );
        let result = crate::StagedLorebookPlannerExecutionCoordinator::new(db, &inference)
            .with_job_attempt(self.0.claim.claim.attempt.get())
            .run(
                self.0.run.request_id,
                &prompt,
                &self.0.handle,
                Some(sink),
                context.now(),
            )
            .await;
        runtime.unregister_stream(sink).map_err(internal)?;
        forwarder.await.map_err(internal)?;
        if context.shutdown_token().is_cancelled()
            && db
                .get(self.0.job.id)
                .map_err(internal)?
                .is_some_and(|job| job.state == lettuce_jobs::JobState::Running)
        {
            db.append_and_transition(lettuce_jobs::JobMutation::RetryScheduled {
                claim: self.0.claim.claim,
                at: context.now().max(self.0.job.updated_at),
            })
            .map_err(internal)?;
            return Ok(());
        }
        crate::StagedLorebookPlannerDispatchCoordinator::new(db, db)
            .settle(self.0, result, CancellationReason::User, context.now())
            .map_err(internal)?;
        Ok(())
    }
}

pub(super) fn result_view(
    context: &ApiContext,
    job: &JobSnapshot,
) -> Result<Option<lettuce_contracts::JobResultDto>, ApiError> {
    use lettuce_creation::{LorebookEntryRunRepository, LorebookKeywordRunRepository};
    if job.kind != JobKind::CreationRun || job.state != lettuce_jobs::JobState::Succeeded {
        return Ok(None);
    }
    let key = job.idempotency_key.as_ref().map_or("", |key| key.as_str());
    if !is_staged_key(key)
        && !key.starts_with("lorebook-entry-generator-")
        && !key.starts_with("lorebook-keyword-generator-")
    {
        return Ok(None);
    }
    let db = context.backend().database();
    let id = request_id(context, job)?;
    if key.starts_with("lorebook-entry-generator-") {
        let attempts = db.load_lorebook_entry_attempts(id).map_err(internal)?;
        for attempt in attempts.iter().rev() {
            if let lettuce_creation::LorebookEntryAttemptDecision::Result(result) =
                &attempt.decision
            {
                return Ok(Some(match result {
                    lettuce_creation::LorebookEntryGenerationResult::Entry { draft } => {
                        lettuce_contracts::JobResultDto::LorebookEntryDraft {
                            draft: lettuce_contracts::LorebookEntryDraftResult {
                                title: draft.title.clone(),
                                content: draft.content.clone(),
                                keywords: draft.keywords.clone(),
                                always_active: draft.always_active,
                            },
                        }
                    }
                    lettuce_creation::LorebookEntryGenerationResult::None { reason } => {
                        lettuce_contracts::JobResultDto::LorebookNoEntry {
                            reason: Some(reason.clone()),
                        }
                    }
                }));
            }
        }
        return Err(internal("the succeeded entry job has no result"));
    }
    if key.starts_with("lorebook-keyword-generator-") {
        let attempts = db.load_lorebook_keyword_attempts(id).map_err(internal)?;
        for attempt in attempts.iter().rev() {
            if let lettuce_creation::LorebookKeywordAttemptDecision::Result(draft) =
                &attempt.decision
            {
                return Ok(Some(lettuce_contracts::JobResultDto::LorebookKeywords {
                    keywords: draft.keywords.clone(),
                }));
            }
        }
        return Err(internal("the succeeded keyword job has no result"));
    }
    if is_staged_key(key) {
        return Ok(Some(lettuce_contracts::JobResultDto::LorebookProject {
            project_id: job.subject.id.to_string(),
        }));
    }
    Ok(None)
}

pub(super) fn subject_view(
    context: &ApiContext,
    job: &JobSnapshot,
) -> Result<Option<lettuce_contracts::JobSubjectDetail>, ApiError> {
    use lettuce_companions::CompanionSoulWriterRunRepository;
    use lettuce_contracts::{HistoricalSourceView, JobSubjectDetail};
    use lettuce_creation::{
        LorebookEntryRunRepository, LorebookKeywordRunRepository, StagedLorebookRepository,
        StagedLorebookWriterRunRepository,
    };
    if !matches!(
        job.kind,
        JobKind::CreationRun | JobKind::CompanionSoulWriter
    ) {
        return Ok(None);
    }
    let key = job.idempotency_key.as_ref().map_or("", |key| key.as_str());
    let db = context.backend().database();
    let prompt_view = |id, name| -> Result<HistoricalSourceView, ApiError> {
        Ok(HistoricalSourceView {
            id: format!("{id}"),
            name,
            deleted: lettuce_context::PromptRepository::get(db, id)
                .map_err(internal)?
                .is_none(),
        })
    };
    let book_view = |id, name| -> Result<HistoricalSourceView, ApiError> {
        Ok(HistoricalSourceView {
            id: format!("{id}"),
            name,
            deleted: lettuce_context::LorebookRepository::get(db, id)
                .map_err(internal)?
                .is_none(),
        })
    };
    if job.kind == JobKind::CreationRun
        && !is_staged_key(key)
        && !key.starts_with("lorebook-entry-generator-")
        && !key.starts_with("lorebook-keyword-generator-")
    {
        return Ok(None);
    }
    let id = request_id(context, job)?;
    let detail = if job.kind == JobKind::CompanionSoulWriter {
        let run = db.load_companion_soul_writer_run(id).map_err(internal)?;
        JobSubjectDetail::CompanionSoulWriter {
            prompt: prompt_view(run.prompt_id, run.prompt_name)?,
        }
    } else if key.starts_with("lorebook-entry-generator-") {
        let run = db.load_lorebook_entry_run(id).map_err(internal)?;
        JobSubjectDetail::LorebookDraft {
            lorebook: Some(book_view(run.lorebook_id, run.prompt_values.lorebook_name)?),
            prompt: prompt_view(run.prompt_id, run.prompt_name)?,
        }
    } else if key.starts_with("lorebook-keyword-generator-") {
        let run = db.load_lorebook_keyword_run(id).map_err(internal)?;
        JobSubjectDetail::LorebookDraft {
            lorebook: None,
            prompt: prompt_view(run.prompt_id, run.prompt_name)?,
        }
    } else if key.starts_with("staged-lorebook-writer-")
        || key.starts_with("staged-lorebook-refine-")
    {
        let run = db.load_staged_lorebook_writer_run(id).map_err(internal)?;
        JobSubjectDetail::LorebookProject {
            project_id: run.project_id.to_string(),
            prompt: prompt_view(run.prompt_id, run.prompt_name)?,
        }
    } else if key.starts_with("staged-lorebook-coherence-") {
        let project = db
            .staged_lorebook_request_for_project(job.subject.id.as_str())
            .map_err(internal)?;
        let project = db.load_staged_lorebook(project).map_err(internal)?;
        let run = project
            .coherence_runs
            .iter()
            .find(|run| run.request_id == id)
            .ok_or_else(|| internal("the coherence input is missing"))?;
        JobSubjectDetail::LorebookProject {
            project_id: project.project.id.to_string(),
            prompt: prompt_view(run.prompt_id, run.prompt_name.clone())?,
        }
    } else if is_planner_key(key) {
        let run = db.load_staged_lorebook(id).map_err(internal)?;
        JobSubjectDetail::LorebookProject {
            project_id: run.project.id.to_string(),
            prompt: prompt_view(run.planner_prompt_id, run.planner_prompt_name)?,
        }
    } else {
        return Ok(None);
    };
    Ok(Some(detail))
}
