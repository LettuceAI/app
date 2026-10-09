//! The host start order and the workers it starts.

use std::{
    path::Path,
    sync::{Arc, Mutex},
    thread::JoinHandle,
    time::Duration,
};

use lettuce_contracts::{ApiError, ApiErrorCode};
use lettuce_jobs::WorkerId;

use super::conversation_feed::ConversationFeed;
use super::error::api_error;
use super::jobs::{
    JobFeed, JobHandlers, JobRunner, recover_local_model_jobs, recover_queued_installs,
};
use super::{ApiContext, ConversationGenerationWorker};
use crate::{CompanionFollowUpHost, EmbeddingModelCoordinator, MediaGarbageScope};

/// A background memory or companion job run while resuming outlives its
/// longest provider request.
const RESUME_LEASE: Duration = Duration::from_secs(60 * 60);
const LEGACY_FOLDER: &str = "lettuce";

/// The steps `startup` runs, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupStep {
    RecoverAfterRestart,
    CompletePendingRewinds,
    DetectLegacyDatabase,
    AdoptLegacyEmbedding,
    ResumeMemoryJobs,
    ResumeCompanionFollowUps,
    RecoverQueuedInstalls,
    SweepOrphanMedia,
    StartWorkers,
}

type Steps = Arc<Mutex<Vec<StartupStep>>>;

fn record(steps: &Steps, step: StartupStep) {
    steps
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .push(step);
}

/// The threads `startup` started. `stop` ends them.
pub struct ApiWorkers {
    context: ApiContext,
    stop: tokio::sync::watch::Sender<bool>,
    startup: Option<JoinHandle<()>>,
    threads: Arc<Mutex<Vec<JoinHandle<()>>>>,
    started: tokio::sync::watch::Receiver<bool>,
    steps: Steps,
}

impl std::fmt::Debug for ApiWorkers {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ApiWorkers")
            .field("steps", &self.steps())
            .finish_non_exhaustive()
    }
}

impl ApiWorkers {
    /// The steps run so far, in order.
    #[must_use]
    pub fn steps(&self) -> Vec<StartupStep> {
        self.steps
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// Waits until the workers run, or startup stopped before them.
    pub async fn started(&self) {
        let mut started = self.started.clone();
        if started.wait_for(|started| *started).await.is_err() {
            tracing::debug!("startup ended before the workers started");
        }
    }

    /// Stops taking new work and cancels running work and startup steps
    /// still running, stops the local diffusion server, records the counted
    /// active time, then joins every thread.
    pub async fn stop(mut self) {
        self.stop.send_replace(true);
        self.context.begin_shutdown();
        self.context.backend().shutdown().await;
        self.context.flush_app_usage();
        let startup = self.startup.take();
        let threads = Arc::clone(&self.threads);
        let joined = tokio::task::spawn_blocking(move || {
            if let Some(startup) = startup
                && startup.join().is_err()
            {
                tracing::error!("the startup thread panicked");
            }
            let threads = std::mem::take(
                &mut *threads
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner),
            );
            for thread in threads {
                if thread.join().is_err() {
                    tracing::error!("a worker thread panicked");
                }
            }
        })
        .await;
        if let Err(error) = joined {
            tracing::error!(%error, "the worker threads could not be joined");
        }
    }
}

/// Starts the application. Before returning, so commands are served only
/// afterwards, it takes the job and conversation change feeds' positions,
/// settles what the previous process left running, finishes the memory
/// rewinds delete-after still owes, detects legacy data and records legacy
/// v4 embedding files. Then, on its own thread, it resumes
/// background memory and companion jobs, recovers queued installs, sweeps
/// orphaned media files, and last starts the conversation generation worker,
/// the memory worker, the job runner with the job change feed, and the conversation change
/// feed. It downloads and loads nothing: optional models load when a chat
/// needs them.
pub async fn startup(context: &ApiContext) -> Result<ApiWorkers, ApiError> {
    super::provider_mutations::cleanup_secrets(context, true).await?;
    let steps: Steps = Arc::new(Mutex::new(Vec::new()));
    let feed = JobFeed::start(context).await?;
    let conversation_feed = ConversationFeed::start(context).await?;
    context
        .blocking(|context| context.recover_after_restart())
        .await?;
    record(&steps, StartupStep::RecoverAfterRestart);
    complete_pending_rewinds(context).await?;
    record(&steps, StartupStep::CompletePendingRewinds);
    let detected = context
        .blocking(|context| Ok(legacy_database_present(context)))
        .await?;
    context.set_legacy_database_detected(detected);
    record(&steps, StartupStep::DetectLegacyDatabase);
    adopt_legacy_embedding(context).await;
    record(&steps, StartupStep::AdoptLegacyEmbedding);
    let (stop, stopped) = tokio::sync::watch::channel(false);
    let (ready, started) = tokio::sync::watch::channel(false);
    let threads = Arc::new(Mutex::new(Vec::new()));
    let startup = {
        let context = context.clone();
        let steps = Arc::clone(&steps);
        let threads = Arc::clone(&threads);
        std::thread::Builder::new()
            .name("startup".into())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        tracing::error!(%error, "the startup runtime could not start");
                        return;
                    }
                };
                runtime.block_on(finish_startup(&context, &steps, &stopped));
                if *stopped.borrow() {
                    return;
                }
                match start_workers(&context, feed, conversation_feed, &stopped) {
                    Ok(started) => {
                        threads
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .extend(started);
                        record(&steps, StartupStep::StartWorkers);
                        ready.send_replace(true);
                    }
                    Err(error) => {
                        tracing::error!(%error, "the workers could not start");
                    }
                }
            })
            .map_err(|error| {
                api_error(
                    ApiErrorCode::Internal,
                    format!("the startup thread could not start: {error}"),
                )
            })?
    };
    Ok(ApiWorkers {
        context: context.clone(),
        stop,
        startup: Some(startup),
        threads,
        started,
        steps,
    })
}

async fn finish_startup(
    context: &ApiContext,
    steps: &Steps,
    stopped: &tokio::sync::watch::Receiver<bool>,
) {
    let worker_id = WorkerId::new();
    let backend = context.backend();
    let database = backend.database();
    let inference = context.inference();
    let follow_ups = CompanionFollowUpHost::new(database, inference);
    let embedding = context.embedding();
    match backend
        .companion_memory_host(embedding.as_ref(), inference)
        .resume_after_restart(worker_id, RESUME_LEASE, context.clock(), &follow_ups)
        .await
    {
        Ok(cancelled) if !cancelled.is_empty() => {
            tracing::info!(
                cancelled = cancelled.len(),
                "cancelled memory jobs that are no longer due"
            );
        }
        Ok(_) => {}
        Err(error) => tracing::warn!(%error, "memory jobs could not be resumed"),
    }
    record(steps, StartupStep::ResumeMemoryJobs);
    if *stopped.borrow() {
        return;
    }
    follow_ups
        .resume_after_restart(worker_id, RESUME_LEASE, context.clock())
        .await;
    record(steps, StartupStep::ResumeCompanionFollowUps);
    if *stopped.borrow() {
        return;
    }
    match context.blocking(recover_queued_installs).await {
        Ok(cancelled) if !cancelled.is_empty() => {
            tracing::info!(
                cancelled = cancelled.len(),
                "cancelled installs the previous process queued"
            );
        }
        Ok(_) => {}
        Err(error) => {
            tracing::warn!(code = ?error.code, message = %error.message, "queued installs could not be recovered");
        }
    }
    match context.blocking(recover_local_model_jobs).await {
        Ok(cancelled) if !cancelled.is_empty() => {
            tracing::info!(
                cancelled = cancelled.len(),
                "cancelled models folder moves the previous process queued"
            );
        }
        Ok(_) => {}
        Err(error) => {
            tracing::warn!(code = ?error.code, message = %error.message, "local model jobs could not be recovered");
        }
    }
    record(steps, StartupStep::RecoverQueuedInstalls);
    if *stopped.borrow() {
        return;
    }
    if let Err(error) = context.blocking(sweep_orphan_media).await {
        tracing::warn!(code = ?error.code, message = %error.message, "orphaned media files could not be swept");
    }
    record(steps, StartupStep::SweepOrphanMedia);
}

pub(super) async fn complete_pending_rewinds(context: &ApiContext) -> Result<(), ApiError> {
    let report = context
        .blocking(|context| {
            let database = context.backend().database();
            crate::DynamicMemoryDeleteAfterCoordinator::new(database, database)
                .complete_pending(None, context.now())
                .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))
        })
        .await?;
    if report.completed > 0 {
        context.jobs().wake();
        tracing::info!(
            completed = report.completed,
            "finished memory rewinds a delete-after still owed"
        );
    }
    for (conversation_id, error) in &report.failed {
        tracing::warn!(
            %conversation_id,
            %error,
            "a memory rewind a delete-after owes could not finish; it is retried when the chat's memory is next used"
        );
    }
    Ok(())
}

fn legacy_database_present(context: &ApiContext) -> bool {
    context.app_folder().is_some_and(|folder| {
        let path = folder.join(LEGACY_FOLDER).join("app.db");
        path.is_file() && context.backend().preflight_legacy_database(&path).is_ok()
    })
}

async fn adopt_legacy_embedding(context: &ApiContext) {
    let Some(folder) = context.app_folder().map(Path::to_path_buf) else {
        return;
    };
    let adopted = context
        .blocking(move |context| {
            EmbeddingModelCoordinator::new(
                Path::new(
                    context
                        .retained_model_roots()?
                        .embedding
                        .as_deref()
                        .expect("resolved root"),
                ),
                context.backend().database(),
            )
            .adopt_legacy_install(&folder.join(LEGACY_FOLDER).join("models").join("embedding"))
            .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))
        })
        .await;
    match adopted {
        Ok(Some(_)) => {
            tracing::info!("adopted the legacy v4 embedding model");
            context.models_changed();
        }
        Ok(None) => {}
        Err(error) => {
            tracing::warn!(message = %error.message, "the legacy embedding model could not be adopted");
        }
    }
}

fn sweep_orphan_media(context: &ApiContext) -> Result<(), ApiError> {
    let (Some(media), Some(files)) = (context.media(), context.database_files()) else {
        return Ok(());
    };
    let removal = crate::sweep_orphan_media_files(
        context.backend().database(),
        &MediaGarbageScope {
            store: media,
            location: &files.location,
            open_database: &files.active,
        },
        context.now(),
    )
    .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?;
    if removal.removed > 0 || removal.failed > 0 {
        tracing::info!(
            removed = removal.removed,
            failed = removal.failed,
            "swept orphaned media files"
        );
    }
    Ok(())
}

fn worker_thread(
    name: &str,
    stopped: &tokio::sync::watch::Receiver<bool>,
    work: impl FnOnce(tokio::sync::watch::Receiver<bool>) -> std::pin::Pin<Box<dyn Future<Output = ()>>>
    + Send
    + 'static,
) -> std::io::Result<JoinHandle<()>> {
    let stopped = stopped.clone();
    let label = name.to_owned();
    std::thread::Builder::new().name(label).spawn(move || {
        match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime.block_on(work(stopped)),
            Err(error) => tracing::error!(%error, "a worker runtime could not start"),
        }
    })
}

async fn until_stopped(mut stopped: tokio::sync::watch::Receiver<bool>) {
    if stopped.wait_for(|stopped| *stopped).await.is_err() {
        tracing::debug!("the stop signal was dropped");
    }
}

fn start_workers(
    context: &ApiContext,
    feed: JobFeed,
    conversation_feed: ConversationFeed,
    stopped: &tokio::sync::watch::Receiver<bool>,
) -> std::io::Result<Vec<JoinHandle<()>>> {
    let generation = ConversationGenerationWorker::new(context.clone());
    let conversation = worker_thread("conversation-generation", stopped, move |stopped| {
        Box::pin(async move { generation.run(until_stopped(stopped)).await })
    })?;
    let memory_worker = super::memory_worker::MemoryWorker::new(context.clone());
    let memory = worker_thread("post-turn-memory", stopped, move |stopped| {
        Box::pin(async move { memory_worker.run(until_stopped(stopped)).await })
    })?;
    let runner = JobRunner::new(context.clone(), JobHandlers::standard());
    let feed_context = context.clone();
    let jobs = worker_thread("jobs", stopped, move |stopped| {
        Box::pin(async move {
            let feed = feed.run(feed_context, until_stopped(stopped.clone()));
            tokio::join!(runner.run(until_stopped(stopped.clone())), feed);
        })
    })?;
    let changes_context = context.clone();
    let changes = worker_thread("conversation-changes", stopped, move |stopped| {
        Box::pin(async move {
            tokio::join!(
                conversation_feed.run(changes_context.clone(), until_stopped(stopped.clone())),
                super::content_filter::run_events(changes_context, until_stopped(stopped)),
            );
        })
    })?;
    Ok(vec![conversation, memory, jobs, changes])
}
