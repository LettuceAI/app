use std::{sync::Arc, time::Duration};

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
        let runtime = context.backend().inference_runtime();
        let sink = RequestId::new();
        let receiver = runtime.register_stream(sink).map_err(internal)?;
        let forwarder = tokio::spawn(forward(receiver, progress));
        let embedding = context.embedding();
        let result = context
            .backend()
            .companion_memory_host(embedding.as_ref(), context.inference())
            .with_inference_runtime(runtime)
            .run_claimed_with_stream(self.0, CancellationReason::User, Some(sink), context.now())
            .await;
        runtime.unregister_stream(sink).map_err(internal)?;
        forwarder.await.map_err(internal)?;
        result.map_err(internal)?;
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
        let result =
            crate::CompanionSoulWriterExecutionCoordinator::new(database, context.inference())
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
