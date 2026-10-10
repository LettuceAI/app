use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use async_trait::async_trait;
use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_database::Database;
use lettuce_jobs::{
    CancellationPolicy, CancellationReason, ClaimRef, JobError, JobErrorCode, JobKind, JobMutation,
    JobOutcome, JobSnapshot, JobSpec, JobState, JobStore, JobSubject, OutcomeRef, RecoveryPolicy,
    ResourceAvailability, ResourceClass, StageSnapshot, SubjectKind, WorkerId,
    handle::CancellationToken,
};
use lettuce_types::{JobId, RequestId};

use super::{
    ApiContext,
    error::{IntoApiError, api_error, parse_id},
    jobs::{ClaimedJob, JobHandler, JobLane, JobProgressSink},
};
use crate::{AppDatabaseLocationError, DatabaseFileKind, DatabaseFileLifecycle};

#[async_trait]
pub trait AppResetHost: Send + Sync {
    async fn preflight(&self) -> Result<(), ApiError>;
    async fn stop_workers(&self) -> Result<(), ApiError>;
    async fn clear_webview_storage(&self) -> Result<(), ApiError>;
    async fn prepare_restart(&self) -> Result<(), ApiError>;
    fn exit_for_restart(&self);
}

#[derive(Default)]
pub(super) struct ResetState {
    host: Mutex<Option<Arc<dyn AppResetHost>>>,
    current: Mutex<Option<(JobId, Arc<Database>)>>,
}

impl ApiContext {
    pub fn attach_reset_host(&self, host: Arc<dyn AppResetHost>) -> Result<(), ApiError> {
        *self
            .reset_state()
            .host
            .lock()
            .map_err(|_| reset_error(dto::AppDataResetStage::Preflight, None))? = Some(host);
        Ok(())
    }
}

pub(super) fn reset_error(stage: dto::AppDataResetStage, kept: Option<String>) -> ApiError {
    let mut error = api_error(
        ApiErrorCode::Unavailable,
        "application data reset could not complete",
    );
    error.details = Some(dto::ApiErrorDetails::AppDataReset {
        stage,
        kept_file: kept,
    });
    error
}

fn host(context: &ApiContext) -> Result<Arc<dyn AppResetHost>, ApiError> {
    context
        .reset_state()
        .host
        .lock()
        .map_err(|_| reset_error(dto::AppDataResetStage::Preflight, None))?
        .clone()
        .ok_or_else(|| reset_error(dto::AppDataResetStage::Preflight, None))
}

pub(super) fn database_for_job(
    context: &ApiContext,
    id: JobId,
) -> Result<Option<Arc<Database>>, ApiError> {
    Ok(context
        .reset_state()
        .current
        .lock()
        .map_err(|_| reset_error(dto::AppDataResetStage::Database, None))?
        .as_ref()
        .filter(|(job, _)| *job == id)
        .map(|(_, database)| database.clone()))
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Detail {
    request_id: RequestId,
    source: String,
    target: String,
    kept: String,
}

fn spec(id: RequestId) -> Result<JobSpec, ApiError> {
    Ok(JobSpec::new(
        JobKind::Maintenance,
        JobSubject::new(SubjectKind::Maintenance, "app-data-reset")
            .map_err(|_| reset_error(dto::AppDataResetStage::Database, None))?,
        OutcomeRef::Request(id),
    )
    .with_resources(vec![ResourceClass::DiskWrite])
    .with_policies(
        RecoveryPolicy::Restart,
        CancellationPolicy::UntilIrreversibleStage,
    ))
}

fn key(id: RequestId) -> String {
    format!("app-data-reset:{id}")
}

fn digest() -> Result<String, ApiError> {
    super::jobs::local::digest(&serde_json::json!({"command":"app_data_reset"}))
}

pub async fn app_data_reset(
    context: &ApiContext,
    request: dto::AppDataResetRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let id: RequestId = parse_id(&request.client_operation_id, "client_operation_id")?;
    let request_digest = digest()?;
    let operation_key = key(id);
    let replay = context
        .blocking({
            let operation_key = operation_key.clone();
            let request_digest = request_digest.clone();
            move |context| {
                let current = context
                    .reset_state()
                    .current
                    .lock()
                    .map_err(|_| reset_error(dto::AppDataResetStage::Database, None))?
                    .as_ref()
                    .map(|(_, database)| database.clone());
                let database = current
                    .as_deref()
                    .unwrap_or_else(|| context.backend().database());
                let operation = database
                    .job_operation(&operation_key)
                    .map_err(|_| reset_error(dto::AppDataResetStage::Database, None))?;
                if let Some(operation) = operation {
                    if operation.request_digest != request_digest {
                        return Err(api_error(
                            ApiErrorCode::Conflict,
                            "reset operation conflicts",
                        ));
                    }
                    return Ok(Some(operation.job_id));
                }
                if current.is_some() || context.shutdown_token().is_cancelled() {
                    return Err(api_error(
                        ApiErrorCode::Busy,
                        "application restart is pending",
                    ));
                }
                Ok(None)
            }
        })
        .await?;
    if let Some(id) = replay {
        return Ok(dto::JobAccepted {
            job_id: id.to_string(),
        });
    }
    host(context)?.preflight().await?;
    let files = context
        .database_files()
        .ok_or_else(|| reset_error(dto::AppDataResetStage::Preflight, None))?;
    if files
        .location
        .active_path()
        .map_err(|_| reset_error(dto::AppDataResetStage::Database, None))?
        != files.active
    {
        return Err(super::storage::file_error(
            AppDatabaseLocationError::Conflict,
            None,
        ));
    }
    let detail = Detail {
        request_id: id,
        source: files
            .active
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| reset_error(dto::AppDataResetStage::Database, None))?
            .to_owned(),
        target: format!("reset-{id}.sqlite3"),
        kept: format!("kept-{id}.sqlite3"),
    };
    let job = context
        .blocking(move |context| {
            context
                .backend()
                .database()
                .admit_job_with_detail(
                    spec(id)?,
                    &operation_key,
                    &request_digest,
                    &serde_json::to_value(detail)
                        .map_err(|_| reset_error(dto::AppDataResetStage::Database, None))?,
                )
                .map_err(IntoApiError::into_api_error)
        })
        .await?;
    context.jobs().wake();
    Ok(dto::JobAccepted {
        job_id: job.id.to_string(),
    })
}

pub(super) fn result_view(
    context: &ApiContext,
    job: &JobSnapshot,
) -> Result<Option<dto::JobResultDto>, ApiError> {
    if job.kind != JobKind::Maintenance
        || job.subject.id.as_str() != "app-data-reset"
        || job.state != JobState::Succeeded
    {
        return Ok(None);
    }
    let current = database_for_job(context, job.id)?;
    let database = current
        .as_deref()
        .unwrap_or_else(|| context.backend().database());
    let detail: Detail = serde_json::from_value(
        database
            .job_detail(job.id)
            .map_err(|_| reset_error(dto::AppDataResetStage::Database, None))?
            .ok_or_else(|| reset_error(dto::AppDataResetStage::Database, None))?
            .detail,
    )
    .map_err(|_| reset_error(dto::AppDataResetStage::Database, None))?;
    Ok(Some(dto::JobResultDto::AppDataReset {
        kept_file: detail.kept,
    }))
}

pub(super) struct AppResetHandler;

#[async_trait]
impl JobHandler for AppResetHandler {
    fn kinds(&self) -> &[JobKind] {
        &[JobKind::Maintenance]
    }
    fn requires_maintenance(&self) -> bool {
        true
    }
    fn lane(&self, _context: &ApiContext, job: &JobSnapshot) -> Option<JobLane> {
        (job.subject.id.as_str() == "app-data-reset")
            .then(|| JobLane("database-maintenance".into()))
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
                let database = context.backend().database();
                let at = context.now().max(job.updated_at);
                let Some(claim) = database
                    .claim(
                        job.id,
                        worker,
                        at,
                        Duration::from_millis(
                            u64::try_from(i64::MAX.saturating_sub(at.get()))
                                .map_err(|_| reset_error(dto::AppDataResetStage::Database, None))?,
                        ),
                        &ResourceAvailability::all(),
                    )
                    .map_err(IntoApiError::into_api_error)?
                else {
                    return Ok(None);
                };
                if let Err(error) = database.append_and_transition(JobMutation::Start {
                    claim: claim.claim.clone(),
                    at,
                }) {
                    if database
                        .get(job.id)
                        .map_err(IntoApiError::into_api_error)?
                        .is_some_and(|job| job.state == JobState::CancellationRequested)
                    {
                        crate::models::artifact_install::finish_claimed_cancellation(
                            database,
                            &claim.claim,
                            at,
                        )
                        .map_err(IntoApiError::into_api_error)?;
                        return Ok(None);
                    }
                    return Err(error.into_api_error());
                }
                Ok(Some(Box::new(Work {
                    claim: claim.claim,
                    cancellation: CancellationToken::new(),
                    stopped: std::sync::atomic::AtomicBool::new(false),
                }) as Box<dyn ClaimedJob>))
            })
            .await
    }
}

struct Work {
    claim: ClaimRef,
    cancellation: CancellationToken,
    stopped: std::sync::atomic::AtomicBool,
}

fn settle(
    database: &Database,
    claim: &ClaimRef,
    context: &ApiContext,
    result: Result<RequestId, dto::AppDataResetStage>,
) -> Result<JobSnapshot, ApiError> {
    let job = database
        .get(claim.job_id)
        .map_err(IntoApiError::into_api_error)?
        .ok_or_else(|| reset_error(dto::AppDataResetStage::Database, None))?;
    let at = context.now().max(job.updated_at);
    database
        .append_and_transition(match result {
            Ok(id) => JobMutation::Succeed {
                claim: claim.clone(),
                outcome: JobOutcome::Success {
                    result_ref: OutcomeRef::Request(id),
                },
                at,
            },
            Err(stage) => JobMutation::Fail {
                claim: claim.clone(),
                error: JobError::new(
                    JobErrorCode::StorageFailure,
                    false,
                    match stage {
                        dto::AppDataResetStage::Workers => "reset-workers",
                        dto::AppDataResetStage::WebviewStorage => "reset-webview-storage",
                        dto::AppDataResetStage::Restart => "reset-restart",
                        _ => "reset-database",
                    },
                )
                .map_err(|_| reset_error(stage, None))?,
                at,
            },
        })
        .map_err(IntoApiError::into_api_error)
}

impl Work {
    async fn cancel(&self, context: &ApiContext) -> Result<(), ApiError> {
        let claim = self.claim.clone();
        context
            .blocking(move |context| {
                let database = context.backend().database();
                let job = database
                    .get(claim.job_id)
                    .map_err(IntoApiError::into_api_error)?
                    .ok_or_else(|| reset_error(dto::AppDataResetStage::Database, None))?;
                let at = context.now().max(job.updated_at);
                if job.state != JobState::CancellationRequested {
                    database
                        .append_and_transition(JobMutation::RequestCancellation {
                            id: claim.job_id,
                            reason: CancellationReason::Shutdown,
                            at,
                        })
                        .map_err(IntoApiError::into_api_error)?;
                }
                crate::models::artifact_install::finish_claimed_cancellation(database, &claim, at)
                    .map(|_| ())
                    .map_err(IntoApiError::into_api_error)
            })
            .await
    }

    async fn execute(
        &self,
        context: &ApiContext,
        lifecycle: &DatabaseFileLifecycle,
    ) -> Result<(Arc<Database>, ClaimRef, Detail, bool), ApiError> {
        let mut detail: Detail = context
            .blocking({
                let id = self.claim.job_id;
                move |context| {
                    serde_json::from_value(
                        context
                            .backend()
                            .database()
                            .job_detail(id)
                            .map_err(|_| reset_error(dto::AppDataResetStage::Database, None))?
                            .ok_or_else(|| reset_error(dto::AppDataResetStage::Database, None))?
                            .detail,
                    )
                    .map_err(|_| reset_error(dto::AppDataResetStage::Database, None))
                }
            })
            .await?;
        let files = context
            .database_files()
            .ok_or_else(|| reset_error(dto::AppDataResetStage::Database, None))?;
        if lifecycle
            .reset_committed(&detail.kept)
            .map_err(|_| reset_error(dto::AppDataResetStage::Database, None))?
        {
            let restart = files.active.file_name().and_then(|name| name.to_str())
                == Some(detail.target.as_str());
            if restart {
                self.stopped
                    .store(true, std::sync::atomic::Ordering::Release);
                host(context)?
                    .stop_workers()
                    .await
                    .map_err(|_| reset_error(dto::AppDataResetStage::Workers, None))?;
                context.flush_app_usage_for_reset()?;
                context.begin_shutdown();
            }
            return Ok((
                Arc::new(
                    Database::open(&files.active)
                        .map_err(|_| reset_error(dto::AppDataResetStage::Database, None))?,
                ),
                self.claim.clone(),
                detail,
                restart,
            ));
        }
        if files.active.file_name().and_then(|name| name.to_str()) != Some(detail.source.as_str()) {
            return Err(reset_error(dto::AppDataResetStage::Database, None));
        }
        let attempt = lettuce_types::OperationId::new();
        detail.target = format!("reset-{attempt}.sqlite3");
        detail.kept = format!("kept-{attempt}.sqlite3");
        let shell = host(context)?;
        shell.preflight().await?;
        self.stopped
            .store(true, std::sync::atomic::Ordering::Release);
        shell
            .stop_workers()
            .await
            .map_err(|_| reset_error(dto::AppDataResetStage::Workers, None))?;
        context.flush_app_usage_for_reset()?;
        context.begin_shutdown();
        let fence = Database::lock_file_writes(&files.active)
            .map_err(|_| reset_error(dto::AppDataResetStage::Database, None))?;
        fence
            .set_fenced(true)
            .map_err(|_| reset_error(dto::AppDataResetStage::Database, None))?;
        let seed = Database::read_reset_seed(&files.active)
            .map_err(|_| reset_error(dto::AppDataResetStage::Database, None))?;
        drop(fence);
        for account in &seed.accounts {
            for record in lettuce_database::account_secret_records(account) {
                context
                    .secret_store()
                    .load(&record.reference, &record.purpose)
                    .await
                    .map_err(|_| reset_error(dto::AppDataResetStage::Database, None))?;
            }
        }
        let path = lifecycle
            .begin_file(&detail.target, DatabaseFileKind::Reset, context.now())
            .map_err(|_| reset_error(dto::AppDataResetStage::Database, None))?;
        let database = Arc::new(
            Database::create_reset_database(&path, &seed)
                .map_err(|_| reset_error(dto::AppDataResetStage::Database, None))?,
        );
        let job = database
            .admit_job_with_detail_and_id(
                spec(detail.request_id)?,
                &key(detail.request_id),
                &digest()?,
                &serde_json::to_value(&detail)
                    .map_err(|_| reset_error(dto::AppDataResetStage::Database, None))?,
                self.claim.job_id,
            )
            .map_err(IntoApiError::into_api_error)?;
        let at = context.now().max(job.updated_at);
        let claim = database
            .claim(
                job.id,
                self.claim.worker_id,
                at,
                Duration::from_millis(
                    u64::try_from(i64::MAX.saturating_sub(at.get()))
                        .map_err(|_| reset_error(dto::AppDataResetStage::Database, None))?,
                ),
                &ResourceAvailability::all(),
            )
            .map_err(IntoApiError::into_api_error)?
            .ok_or_else(|| reset_error(dto::AppDataResetStage::Database, None))?
            .claim;
        database
            .append_and_transition(JobMutation::Start {
                claim: claim.clone(),
                at,
            })
            .map_err(IntoApiError::into_api_error)?;
        database
            .append_and_transition(JobMutation::StageChanged {
                claim: claim.clone(),
                stage: StageSnapshot::new("cutover", true)
                    .map_err(|_| reset_error(dto::AppDataResetStage::Database, None))?,
                at,
            })
            .map_err(IntoApiError::into_api_error)?;
        lifecycle
            .reset_cutover(&detail.target, &detail.kept, context.now(), || {
                context
                    .backend()
                    .database()
                    .close_for_reset()
                    .map_err(|_| AppDatabaseLocationError::Storage)?;
                if let Some(media) = context.media() {
                    let (blobs, assets) = media.repositories();
                    blobs
                        .close_for_reset()
                        .map_err(|_| AppDatabaseLocationError::Storage)?;
                    assets
                        .close_for_reset()
                        .map_err(|_| AppDatabaseLocationError::Storage)?;
                }
                Ok(())
            })
            .map_err(|_| reset_error(dto::AppDataResetStage::Database, None))?;
        *context
            .reset_state()
            .current
            .lock()
            .map_err(|_| reset_error(dto::AppDataResetStage::Database, None))? =
            Some((job.id, database.clone()));
        Ok((database, claim, detail, true))
    }
}

#[async_trait]
impl ClaimedJob for Work {
    fn cancellation(&self) -> CancellationToken {
        self.cancellation.clone()
    }
    async fn run(
        self: Box<Self>,
        context: ApiContext,
        _progress: Arc<dyn JobProgressSink>,
    ) -> Result<(), ApiError> {
        let Some(_lease) = context.maintenance().maintenance(&self.cancellation).await else {
            return self.cancel(&context).await;
        };
        let files = context
            .database_files()
            .ok_or_else(|| reset_error(dto::AppDataResetStage::Database, None))?;
        let lifecycle = tokio::select! {
            biased;
            () = self.cancellation.cancelled() => return self.cancel(&context).await,
            lifecycle = files.location.file_lifecycle() => lifecycle.map_err(|_| reset_error(dto::AppDataResetStage::Database, None))?,
        };
        if files
            .location
            .active_path()
            .map_err(|_| reset_error(dto::AppDataResetStage::Database, None))?
            != files.active
        {
            settle(
                context.backend().database(),
                &self.claim,
                &context,
                Err(dto::AppDataResetStage::Database),
            )?;
            return Ok(());
        }
        let stage = context
            .backend()
            .database()
            .append_and_transition(JobMutation::StageChanged {
                claim: self.claim.clone(),
                stage: StageSnapshot::new("cutover", true)
                    .map_err(|_| reset_error(dto::AppDataResetStage::Database, None))?,
                at: context.now().max(
                    context
                        .backend()
                        .database()
                        .get(self.claim.job_id)
                        .map_err(IntoApiError::into_api_error)?
                        .ok_or_else(|| reset_error(dto::AppDataResetStage::Database, None))?
                        .updated_at,
                ),
            });
        if let Err(error) = stage {
            if self.cancellation.is_cancelled()
                || context
                    .backend()
                    .database()
                    .get(self.claim.job_id)
                    .map_err(IntoApiError::into_api_error)?
                    .is_some_and(|job| job.state == JobState::CancellationRequested)
            {
                return self.cancel(&context).await;
            }
            return Err(error.into_api_error());
        }
        let (database, claim, detail, needs_restart) =
            match self.execute(&context, &lifecycle).await {
                Ok(result) => result,
                Err(error) => {
                    lifecycle
                        .recover_reset_preparation()
                        .map_err(|_| reset_error(dto::AppDataResetStage::Database, None))?;
                    let stage = match error.details {
                        Some(dto::ApiErrorDetails::AppDataReset { stage, .. }) => stage,
                        _ => dto::AppDataResetStage::Database,
                    };
                    if !self.stopped.load(std::sync::atomic::Ordering::Acquire) {
                        let job = settle(
                            context.backend().database(),
                            &self.claim,
                            &context,
                            Err(stage),
                        )?;
                        let view = super::jobs::job_view(&context, &job)?;
                        let (event, terminal) = super::jobs::job_event(&job, view);
                        context.jobs().deliver(job.id, event, terminal);
                        return Ok(());
                    }
                    let database = Arc::new(
                        Database::open(&files.active)
                            .map_err(|_| reset_error(dto::AppDataResetStage::Database, None))?,
                    );
                    *context
                        .reset_state()
                        .current
                        .lock()
                        .map_err(|_| reset_error(dto::AppDataResetStage::Database, None))? =
                        Some((self.claim.job_id, database.clone()));
                    let job = settle(&database, &self.claim, &context, Err(stage))?;
                    let view = super::jobs::job_view(&context, &job)?;
                    let (event, terminal) = super::jobs::job_event(&job, view);
                    context.jobs().deliver(job.id, event, terminal);
                    drop(lifecycle);
                    let shell = host(&context)?;
                    let _ = shell.prepare_restart().await;
                    shell.exit_for_restart();
                    return Ok(());
                }
            };
        let shell = host(&context)?;
        let result = if !needs_restart {
            Ok(detail.request_id)
        } else {
            let cleared = shell.clear_webview_storage().await;
            let relaunch = shell.prepare_restart().await;
            match (cleared, relaunch) {
                (Err(_), _) => Err(dto::AppDataResetStage::WebviewStorage),
                (Ok(()), Err(_)) => Err(dto::AppDataResetStage::Restart),
                (Ok(()), Ok(())) => Ok(detail.request_id),
            }
        };
        let job = settle(&database, &claim, &context, result)?;
        let view = super::jobs::job_view(&context, &job)?;
        let (event, terminal) = super::jobs::job_event(&job, view);
        context.jobs().deliver(job.id, event, terminal);
        drop(lifecycle);
        if needs_restart {
            shell.exit_for_restart();
        }
        Ok(())
    }
}
