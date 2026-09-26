use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
    time::Duration,
};

use async_trait::async_trait;
use futures_util::FutureExt;
use lettuce_contracts::ApiError;
use lettuce_jobs::{
    JobKind, JobQuery, JobSnapshot, JobState, JobStore, ResourceAvailability, WorkerId,
    handle::CancellationToken,
};
use lettuce_types::{JobId, PageLimit, PageRequest};

use super::install::{ArtifactInstallHandler, NetworkInstallSources};
use crate::api::ApiContext;
use crate::api::error::IntoApiError;
use crate::api::worker::link_to_shutdown;

const IDLE_BACKOFF_MIN: Duration = Duration::from_millis(250);
const IDLE_BACKOFF_MAX: Duration = Duration::from_secs(5);
const QUEUE_PAGE: u16 = 200;

/// What a running job streams besides its stored progress: text deltas of
/// LLM features, sent to the job's watch streams.
pub trait JobProgressSink: Send + Sync {
    fn text_delta(&self, text: Option<String>, reasoning: Option<String>);
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

    /// Claims `job` through its coordinator. `None` when it could not be
    /// claimed or had already ended.
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
        Self::new(vec![Arc::new(ArtifactInstallHandler::new(Arc::new(
            NetworkInstallSources,
        )))])
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
/// counts as started, backs off while idle, and links every job to the
/// context's shutdown token. Repository calls are synchronous, so the host
/// gives the runner its own thread.
#[derive(Clone)]
pub struct JobRunner {
    context: ApiContext,
    worker_id: WorkerId,
    handlers: JobHandlers,
    lanes: Arc<Mutex<HashSet<JobLane>>>,
    tasks: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>,
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
        }
    }

    /// Polls until `shutdown` completes, then waits for the jobs it started
    /// (`ApiContext::begin_shutdown` cancels them). An idle runner, or one
    /// that could not claim anything, waits with a doubling backoff; a new
    /// job or a finished one wakes it.
    pub async fn run(&self, shutdown: impl Future<Output = ()>) {
        let shutdown = shutdown.fuse();
        futures_util::pin_mut!(shutdown);
        let mut idle = IDLE_BACKOFF_MIN;
        while (&mut shutdown).now_or_never().is_none() {
            match self.run_once().await {
                Ok(true) => {
                    idle = IDLE_BACKOFF_MIN;
                    continue;
                }
                Ok(false) => {}
                Err(error) => {
                    tracing::warn!(code = ?error.code, message = %error.message, "job runner step failed");
                }
            }
            tokio::select! {
                () = &mut shutdown => break,
                () = self.context.jobs().woken() => idle = IDLE_BACKOFF_MIN,
                () = tokio::time::sleep(idle) => idle = (idle * 2).min(IDLE_BACKOFF_MAX),
            }
        }
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
    /// claimed; returns whether one started.
    pub async fn run_once(&self) -> Result<bool, ApiError> {
        let kinds = self.handlers.kinds();
        let queued = self
            .context
            .blocking(move |context| queued_jobs(context, &kinds))
            .await?;
        let mut started = false;
        for job in queued {
            let Some(handler) = self.handlers.handler(job.kind).cloned() else {
                continue;
            };
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
                }
            }
        }
        Ok(started)
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
