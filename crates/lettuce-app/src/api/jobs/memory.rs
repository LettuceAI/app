use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use async_trait::async_trait;
use lettuce_contracts::ApiError;
use lettuce_jobs::handle::CancellationToken;
use lettuce_jobs::{
    CancellationReason, JobKind, JobSnapshot, OutcomeRef, ResourceAvailability, WorkerId,
};
use lettuce_types::RequestId;

use super::local::internal;
use super::{ClaimedJob, JobHandler, JobLane, JobProgressSink};
use crate::api::ApiContext;

const MEMORY_LEASE: Duration = Duration::from_secs(60 * 60);

#[derive(Debug, Clone, Copy)]
pub struct MemoryExtractionHandler;

#[async_trait]
impl JobHandler for MemoryExtractionHandler {
    fn kinds(&self) -> &[JobKind] {
        &[JobKind::MemoryExtraction]
    }

    fn lane(&self, _context: &ApiContext, job: &JobSnapshot) -> Option<JobLane> {
        Some(JobLane(format!("memory:{}", job.subject.id.as_str())))
    }

    fn not_before(
        &self,
        _context: &ApiContext,
        job: &JobSnapshot,
    ) -> Option<lettuce_types::TimestampMillis> {
        retry_due(job)
    }

    async fn claim(
        &self,
        context: &ApiContext,
        job: &JobSnapshot,
        worker_id: WorkerId,
    ) -> Result<Option<Box<dyn ClaimedJob>>, ApiError> {
        let job = job.clone();
        let work = context
            .blocking(move |context| {
                let database = context.backend().database();
                let record = database
                    .job_detail(job.id)
                    .map_err(internal)?
                    .ok_or_else(|| internal("the frozen memory admission is missing"))?;
                let batch =
                    crate::companion::companion_memory_job::decode_memory_admission(record.detail)
                        .map_err(internal)?;
                let admission = crate::CompanionPostTurnMemoryAdmission {
                    job,
                    batch,
                    created: false,
                };
                let mut works = crate::CompanionMemoryDispatchCoordinator::new(database, database)
                    .claim_admissions(
                        vec![admission],
                        worker_id,
                        context.now(),
                        MEMORY_LEASE,
                        &ResourceAvailability::all(),
                    )
                    .map_err(internal)?;
                Ok(works.pop())
            })
            .await?;
        Ok(work.map(|work| Box::new(ClaimedMemory(work)) as Box<dyn ClaimedJob>))
    }
}

struct ClaimedMemory(crate::CompanionMemoryClaimedWork);

#[async_trait]
impl ClaimedJob for ClaimedMemory {
    fn cancellation(&self) -> CancellationToken {
        self.0.handle.cancellation_token()
    }

    async fn run(
        self: Box<Self>,
        context: ApiContext,
        progress: Arc<dyn JobProgressSink>,
    ) -> Result<(), ApiError> {
        use crate::CompanionFollowUps;
        let worker_id = self.0.claim.claim.worker_id;
        let output = MemoryJobOutput::new(context.clone()).with_progress(progress);
        let embedding = context.embedding();
        let settled = context
            .backend()
            .companion_memory_host(embedding.as_ref(), context.inference())
            .with_inference_runtime(context.backend().inference_runtime())
            .with_job_output(&output)
            .run_claimed(self.0, CancellationReason::User, context.now())
            .await
            .map_err(internal)?;
        crate::CompanionFollowUpHost::new(context.backend().database(), context.inference())
            .after_memory(&settled, worker_id, MEMORY_LEASE, context.clock())
            .await;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SoulWriterHandler;

#[async_trait]
impl JobHandler for SoulWriterHandler {
    fn kinds(&self) -> &[JobKind] {
        &[JobKind::CompanionSoulWriter]
    }

    fn lane(&self, _context: &ApiContext, job: &JobSnapshot) -> Option<JobLane> {
        Some(JobLane(format!("soul-writer:{}", job.id)))
    }

    fn not_before(
        &self,
        _context: &ApiContext,
        job: &JobSnapshot,
    ) -> Option<lettuce_types::TimestampMillis> {
        retry_due(job)
    }

    async fn claim(
        &self,
        context: &ApiContext,
        job: &JobSnapshot,
        worker_id: WorkerId,
    ) -> Result<Option<Box<dyn ClaimedJob>>, ApiError> {
        let job_id = job.id;
        let work = context
            .blocking(move |context| {
                use lettuce_jobs::{JobStore, events::JobEvent};
                let events = context
                    .backend()
                    .database()
                    .events_since(job_id, None, 1)
                    .map_err(internal)?;
                let Some(lettuce_jobs::events::JobEventEnvelope {
                    event:
                        JobEvent::Created {
                            input_ref: OutcomeRef::Request(request_id),
                            ..
                        },
                    ..
                }) = events.into_iter().next()
                else {
                    return Err(internal("the Soul writer request is invalid"));
                };
                context
                    .backend()
                    .companion_soul_writer_dispatcher()
                    .claim(
                        request_id,
                        worker_id,
                        context.now(),
                        MEMORY_LEASE,
                        &ResourceAvailability::all(),
                    )
                    .map_err(internal)
            })
            .await?;
        Ok(work.map(|work| Box::new(ClaimedSoul(work)) as Box<dyn ClaimedJob>))
    }
}

struct ClaimedSoul(crate::CompanionSoulWriterClaimedWork);

#[async_trait]
impl ClaimedJob for ClaimedSoul {
    fn cancellation(&self) -> CancellationToken {
        self.0.handle.cancellation_token()
    }

    async fn run(
        self: Box<Self>,
        context: ApiContext,
        progress: Arc<dyn JobProgressSink>,
    ) -> Result<(), ApiError> {
        use lettuce_context::PromptRepository;
        let database = context.backend().database();
        let prompt = PromptRepository::get(database, self.0.run.prompt_id);
        let prompt = match prompt {
            Ok(Some(prompt)) => prompt,
            Ok(None) | Err(_) => {
                context
                    .backend()
                    .companion_soul_writer_dispatcher()
                    .settle(
                        self.0,
                        Err(crate::CompanionSoulWriterExecutionError::InvalidPrompt),
                        CancellationReason::User,
                        context.now(),
                    )
                    .map_err(internal)?;
                return Ok(());
            }
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
        let result = crate::CompanionSoulWriterExecutionCoordinator::new(database, &inference)
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
        context
            .backend()
            .companion_soul_writer_dispatcher()
            .settle(self.0, result, CancellationReason::User, context.now())
            .map_err(internal)?;
        Ok(())
    }
}

async fn forward(
    mut receiver: lettuce_inference::InferenceStreamReceiver,
    progress: Arc<dyn JobProgressSink>,
) {
    use lettuce_conversations::GenerationStreamEvent;
    while let Some(envelope) = receiver.recv().await {
        match envelope.event {
            GenerationStreamEvent::TextDelta { text } => progress.text_delta(Some(text), None),
            GenerationStreamEvent::ReasoningDelta { text } => progress.text_delta(None, Some(text)),
        }
    }
}

fn retry_due(job: &JobSnapshot) -> Option<lettuce_types::TimestampMillis> {
    let attempt = job.attempt.get();
    if attempt == 0 {
        return None;
    }
    let delay = 250_i64
        .saturating_mul(
            1_i64
                .checked_shl(attempt.saturating_sub(1).min(7))
                .unwrap_or(128),
        )
        .min(30_000);
    Some(lettuce_types::TimestampMillis::new(
        job.updated_at.get().saturating_add(delay),
    ))
}

pub(crate) struct MemoryJobOutput {
    context: ApiContext,
    progress: Option<Arc<dyn JobProgressSink>>,
    streams: Mutex<HashMap<RequestId, MemoryStream>>,
}

struct MemoryStream {
    forwarder: tokio::task::JoinHandle<()>,
    _shutdown: crate::api::worker::ShutdownLink,
}

impl std::fmt::Debug for MemoryJobOutput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MemoryJobOutput")
            .finish_non_exhaustive()
    }
}

impl MemoryJobOutput {
    pub(crate) fn new(context: ApiContext) -> Self {
        Self {
            context,
            progress: None,
            streams: Mutex::new(HashMap::new()),
        }
    }

    fn with_progress(mut self, progress: Arc<dyn JobProgressSink>) -> Self {
        self.progress = Some(progress);
        self
    }
}

struct MemoryProgress {
    context: ApiContext,
    job_id: lettuce_types::JobId,
}

impl JobProgressSink for MemoryProgress {
    fn text_delta(&self, text: Option<String>, reasoning: Option<String>) {
        self.context.jobs().text_delta(self.job_id, text, reasoning);
    }

    fn image_progress(&self, progress: lettuce_contracts::ImageProgress) {
        self.context.jobs().image_progress(self.job_id, progress);
    }
}

#[async_trait]
impl crate::CompanionMemoryJobOutput for MemoryJobOutput {
    fn open(
        &self,
        job_id: lettuce_types::JobId,
        cancellation: CancellationToken,
    ) -> Result<RequestId, crate::CompanionMemoryOutputError> {
        let sink = RequestId::new();
        let receiver = self
            .context
            .backend()
            .inference_runtime()
            .register_stream(sink)
            .map_err(|_| crate::CompanionMemoryOutputError)?;
        let progress = self.progress.clone().unwrap_or_else(|| {
            Arc::new(MemoryProgress {
                context: self.context.clone(),
                job_id,
            })
        });
        let shutdown = crate::api::worker::link_to_shutdown(
            self.context.shutdown_token(),
            cancellation.clone(),
        );
        self.context.jobs().start_running(job_id, cancellation);
        let stream = MemoryStream {
            forwarder: tokio::spawn(forward(receiver, progress)),
            _shutdown: shutdown,
        };
        self.streams
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(sink, stream);
        Ok(sink)
    }

    async fn close(&self, sink: RequestId) -> Result<(), crate::CompanionMemoryOutputError> {
        let stream = self
            .streams
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&sink)
            .ok_or(crate::CompanionMemoryOutputError)?;
        let unregistered = self
            .context
            .backend()
            .inference_runtime()
            .unregister_stream(sink);
        let forwarded = stream.forwarder.await;
        unregistered.map_err(|_| crate::CompanionMemoryOutputError)?;
        forwarded.map_err(|_| crate::CompanionMemoryOutputError)?;
        Ok(())
    }

    fn finished(&self, job_id: lettuce_types::JobId) {
        self.context.jobs().finish_running(job_id);
    }
}

/// The draft a succeeded Soul writer job produced: its last round's draft,
/// or the starting draft when no round changed anything.
pub(super) fn soul_draft_view(
    context: &ApiContext,
    job: &JobSnapshot,
) -> Result<Option<lettuce_contracts::JobResultDto>, ApiError> {
    use lettuce_companions::CompanionSoulWriterRunRepository;
    if job.kind != JobKind::CompanionSoulWriter {
        return Ok(None);
    }
    let Some(lettuce_jobs::JobOutcome::Success {
        result_ref: OutcomeRef::Request(request_id),
    }) = &job.outcome
    else {
        return Ok(None);
    };
    let run = context
        .backend()
        .database()
        .load_companion_soul_writer_run(*request_id)
        .map_err(internal)?;
    let document = run
        .rounds
        .last()
        .map_or(run.starting_draft, |round| round.resulting_draft.clone());
    Ok(Some(lettuce_contracts::JobResultDto::CompanionSoulDraft {
        draft: Box::new(crate::api::companion::draft_from_document(document)?),
    }))
}
