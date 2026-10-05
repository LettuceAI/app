use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
    time::Duration,
};

use async_trait::async_trait;
use lettuce_contracts::ApiError;
use lettuce_jobs::{
    JobKind, JobQuery, JobSnapshot, JobState, JobStore, ResourceAvailability, WorkerId,
    handle::CancellationToken,
};
use lettuce_types::{JobId, PageLimit, PageRequest, TimestampMillis};

use super::install::{ArtifactInstallHandler, NetworkInstallSources};
use crate::api::ApiContext;
use crate::api::error::IntoApiError;
use crate::api::worker::{WorkerStep, drive, link_to_shutdown};

const QUEUE_PAGE: u16 = 200;

/// What a running job streams besides its stored progress: text deltas of
/// LLM features and the progress of a local image generation, sent to the
/// job's watch streams.
pub trait JobProgressSink: Send + Sync {
    fn text_delta(&self, text: Option<String>, reasoning: Option<String>);

    fn image_progress(&self, progress: lettuce_contracts::ImageProgress);
}

/// Jobs in one lane run one at a time; jobs in different lanes run
/// concurrently.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct JobLane(pub String);

/// Runs the queued jobs of some kinds through their coordinator.
#[async_trait]
pub trait JobHandler: Send + Sync {
    fn kinds(&self) -> &[JobKind];

    /// The resources a claim may use.
    fn resources(&self) -> ResourceAvailability {
        ResourceAvailability::all()
    }

    /// The lane `job` runs in, or `None` while this process cannot run it;
    /// the job then stays queued.
    fn lane(&self, context: &ApiContext, job: &JobSnapshot) -> Option<JobLane>;

    /// When a queued job may run at the earliest, for a retry scheduled
    /// later; the runner sleeps exactly until the earliest such time.
    fn not_before(&self, _context: &ApiContext, _job: &JobSnapshot) -> Option<TimestampMillis> {
        None
    }

    /// Claims `job` through its coordinator. `None` when it could not be
    /// claimed or had already ended; a job that can never run is settled by
    /// the handler, which then returns `None`. An error is a transient
    /// failure: the job stays queued and the runner retries after a delay.
    async fn claim(
        &self,
        context: &ApiContext,
        job: &JobSnapshot,
        worker_id: WorkerId,
    ) -> Result<Option<Box<dyn ClaimedJob>>, ApiError>;
}

/// A claimed job, run to its settlement.
#[async_trait]
pub trait ClaimedJob: Send {
    /// The token the coordinator checks; cancelling it stops the job.
    fn cancellation(&self) -> CancellationToken;

    async fn run(
        self: Box<Self>,
        context: ApiContext,
        progress: Arc<dyn JobProgressSink>,
    ) -> Result<(), ApiError>;
}

/// The handler of every job kind the runner runs; a kind gets its handler by
/// adding it to `standard`.
#[derive(Clone)]
pub struct JobHandlers {
    handlers: Vec<Arc<dyn JobHandler>>,
}

impl std::fmt::Debug for JobHandlers {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("JobHandlers")
            .field("handlers", &self.handlers.len())
            .finish()
    }
}

impl JobHandlers {
    #[must_use]
    pub fn new(handlers: Vec<Arc<dyn JobHandler>>) -> Self {
        Self { handlers }
    }

    #[must_use]
    pub fn standard() -> Self {
        Self::new(vec![
            Arc::new(ArtifactInstallHandler::new(Arc::new(NetworkInstallSources))),
            Arc::new(super::local::ModelPullHandler),
            Arc::new(super::local::ModelsFolderMoveHandler),
            Arc::new(super::text::TextFeatureHandler),
            Arc::new(super::image::ImageGenerateHandler),
            Arc::new(super::image_tools::ImageToolHandler),
            Arc::new(super::speech::SpeechTranscribeHandler),
            Arc::new(super::speech::SpeechSynthesizeHandler),
            Arc::new(super::voice_creation::VoiceCreationHandler),
        ])
    }

    fn kinds(&self) -> Vec<JobKind> {
        let mut kinds = Vec::new();
        for handler in &self.handlers {
            for kind in handler.kinds() {
                if !kinds.contains(kind) {
                    kinds.push(*kind);
                }
            }
        }
        kinds
    }

    fn handler(&self, kind: JobKind) -> Option<&Arc<dyn JobHandler>> {
        self.handlers
            .iter()
            .find(|handler| handler.kinds().contains(&kind))
    }
}

/// Claims and runs queued jobs of the kinds its handlers take, each lane one
/// job at a time. Like `ConversationGenerationWorker` it claims before a job
/// counts as started, sleeps while idle until woken, and links every job to the
/// context's shutdown token. Repository calls are synchronous, so the host
/// gives the runner its own thread.
#[derive(Clone)]
pub struct JobRunner {
    context: ApiContext,
    worker_id: WorkerId,
    handlers: JobHandlers,
    lanes: Arc<Mutex<HashSet<JobLane>>>,
    tasks: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>,
    next_due: Arc<Mutex<Option<TimestampMillis>>>,
}

impl std::fmt::Debug for JobRunner {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("JobRunner")
            .field("handlers", &self.handlers)
            .finish_non_exhaustive()
    }
}

struct WatchSink {
    context: ApiContext,
    job_id: JobId,
}

impl JobProgressSink for WatchSink {
    fn text_delta(&self, text: Option<String>, reasoning: Option<String>) {
        self.context.jobs().text_delta(self.job_id, text, reasoning);
    }

    fn image_progress(&self, progress: lettuce_contracts::ImageProgress) {
        self.context.jobs().image_progress(self.job_id, progress);
    }
}

impl JobRunner {
    #[must_use]
    pub fn new(context: ApiContext, handlers: JobHandlers) -> Self {
        Self {
            context,
            worker_id: WorkerId::new(),
            handlers,
            lanes: Arc::new(Mutex::new(HashSet::new())),
            tasks: Arc::new(Mutex::new(Vec::new())),
            next_due: Arc::new(Mutex::new(None)),
        }
    }

    /// Runs until `shutdown` completes, then waits for the jobs it started
    /// (`ApiContext::begin_shutdown` cancels them). An idle runner sleeps
    /// until a new job, a cancellation or a finished job wakes it, or until
    /// the earliest time a queued job was scheduled to run; a failed step,
    /// such as a storage error, is retried after a growing delay.
    pub async fn run(&self, shutdown: impl Future<Output = ()>) {
        drive(self, shutdown).await;
        self.wait_idle().await;
    }

    /// Waits until every job this runner started has settled.
    pub async fn wait_idle(&self) {
        loop {
            let tasks = std::mem::take(
                &mut *self
                    .tasks
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner),
            );
            if tasks.is_empty() {
                return;
            }
            for task in tasks {
                if let Err(error) = task.await {
                    tracing::warn!(%error, "a job task stopped");
                }
            }
        }
    }

    /// Starts every queued job whose lane is free and that could be
    /// claimed; returns whether one started, or the error of a claim that
    /// failed transiently so the caller retries.
    pub async fn run_once(&self) -> Result<bool, ApiError> {
        let kinds = self.handlers.kinds();
        let queued = self
            .context
            .blocking(move |context| queued_jobs(context, &kinds))
            .await?;
        let mut started = false;
        let mut failed = None;
        let now = self.context.now();
        let mut next_due: Option<TimestampMillis> = None;
        for job in queued {
            let Some(handler) = self.handlers.handler(job.kind).cloned() else {
                continue;
            };
            if let Some(due) = handler.not_before(&self.context, &job)
                && due > now
            {
                next_due = Some(next_due.map_or(due, |earliest| earliest.min(due)));
                continue;
            }
            let Some(lane) = handler.lane(&self.context, &job) else {
                continue;
            };
            if !self.lock_lanes().insert(lane.clone()) {
                continue;
            }
            match handler.claim(&self.context, &job, self.worker_id).await {
                Ok(Some(claimed)) => {
                    self.spawn(job.id, lane, claimed).await;
                    started = true;
                }
                Ok(None) => {
                    self.lock_lanes().remove(&lane);
                }
                Err(error) => {
                    self.lock_lanes().remove(&lane);
                    tracing::warn!(job_id = %job.id, code = ?error.code, message = %error.message, "a queued job could not be claimed");
                    failed = Some(error);
                }
            }
        }
        *self
            .next_due
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = next_due;
        match failed {
            Some(error) => Err(error),
            None => Ok(started),
        }
    }

    async fn until_due(&self) {
        let due = *self
            .next_due
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match due {
            Some(due) => {
                let wait = due.get().saturating_sub(self.context.now().get());
                tokio::time::sleep(Duration::from_millis(
                    u64::try_from(wait).unwrap_or_default(),
                ))
                .await;
            }
            None => std::future::pending::<()>().await,
        }
    }

    fn lock_lanes(&self) -> std::sync::MutexGuard<'_, HashSet<JobLane>> {
        self.lanes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    async fn spawn(&self, job_id: JobId, lane: JobLane, claimed: Box<dyn ClaimedJob>) {
        let context = self.context.clone();
        let cancellation = claimed.cancellation();
        let link = link_to_shutdown(context.shutdown_token(), cancellation.clone());
        context.jobs().start_running(job_id, cancellation.clone());
        let requested = context
            .blocking(move |context| {
                context
                    .backend()
                    .database()
                    .get(job_id)
                    .map_err(IntoApiError::into_api_error)
            })
            .await
            .ok()
            .flatten()
            .is_some_and(|job| job.cancellation.requested);
        if requested {
            cancellation.cancel();
        }
        let lanes = Arc::clone(&self.lanes);
        let task = tokio::spawn(async move {
            let _link = link;
            let progress: Arc<dyn JobProgressSink> = Arc::new(WatchSink {
                context: context.clone(),
                job_id,
            });
            if let Err(error) = claimed.run(context.clone(), progress).await {
                tracing::warn!(%job_id, code = ?error.code, message = %error.message, "a job failed to run");
            }
            context.jobs().finish_running(job_id);
            lanes
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .remove(&lane);
            context.jobs().wake();
        });
        let mut tasks = self
            .tasks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        tasks.retain(|task| !task.is_finished());
        tasks.push(task);
    }
}

impl WorkerStep for JobRunner {
    const LABEL: &'static str = "jobs";

    async fn step(&self) -> Result<bool, ApiError> {
        self.run_once().await
    }

    async fn woken(&self) {
        self.context.jobs().woken().await;
    }

    async fn due(&self) {
        self.until_due().await;
    }
}

/// Every queued job of `kinds`, oldest first.
fn queued_jobs(context: &ApiContext, kinds: &[JobKind]) -> Result<Vec<JobSnapshot>, ApiError> {
    let database = context.backend().database();
    let mut jobs = Vec::new();
    for kind in kinds {
        let mut cursor = None;
        loop {
            let page = database
                .list(JobQuery {
                    state: Some(JobState::Queued),
                    kind: Some(*kind),
                    subject: None,
                    page: PageRequest {
                        cursor: cursor.take(),
                        limit: PageLimit::new(QUEUE_PAGE),
                    },
                })
                .map_err(IntoApiError::into_api_error)?;
            jobs.extend(page.items);
            match page.next_cursor {
                Some(next) => cursor = Some(next),
                None => break,
            }
        }
    }
    jobs.sort_by_key(|job| (job.created_at, job.id));
    Ok(jobs)
}
