use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_jobs::{
    CancellationPolicy, CancellationReason, ClaimRef, JobError, JobErrorCode, JobKind, JobMutation,
    JobOutcome, JobSnapshot, JobSpec, JobState, JobStore, JobSubject, OutcomeRef, RecoveryPolicy,
    ResourceAvailability, ResourceClass, SubjectKind, WorkerId, handle::CancellationToken,
};
use lettuce_types::{JobId, RequestId};

use super::{
    ApiContext,
    error::{IntoApiError, api_error, parse_id},
    jobs::{ClaimedJob, JobHandler, JobLane, JobProgressSink},
};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum MaintenanceDetail {
    StorageOptimize,
}

pub async fn storage_optimize(
    context: &ApiContext,
    request: dto::StorageOptimizeRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let id: RequestId = parse_id(&request.client_operation_id, "client_operation_id")?;
    let key = format!("storage-optimize:{id}");
    let detail = MaintenanceDetail::StorageOptimize;
    let digest = super::jobs::local::digest(&detail)?;
    let job = context
        .blocking(move |context| {
            let spec = JobSpec::new(
                JobKind::Maintenance,
                JobSubject::new(SubjectKind::Maintenance, "storage-optimize")
                    .map_err(|error| api_error(ApiErrorCode::Internal, error.to_string()))?,
                OutcomeRef::Request(id),
            )
            .with_resources(vec![ResourceClass::DiskWrite])
            .with_policies(RecoveryPolicy::Restart, CancellationPolicy::Cooperative);
            context
                .backend()
                .database()
                .admit_job_with_detail(
                    spec,
                    &key,
                    &digest,
                    &serde_json::to_value(detail).map_err(internal)?,
                )
                .map_err(IntoApiError::into_api_error)
        })
        .await?;
    context.jobs().wake();
    Ok(dto::JobAccepted {
        job_id: job.id.to_string(),
    })
}

fn internal(error: impl std::fmt::Display) -> ApiError {
    api_error(ApiErrorCode::Internal, error.to_string())
}

pub(super) fn result_view(
    context: &ApiContext,
    job: &JobSnapshot,
) -> Result<Option<dto::JobResultDto>, ApiError> {
    if job.kind != JobKind::Maintenance || job.subject.id.as_str() != "storage-optimize" {
        return Ok(None);
    }
    let detail = context
        .backend()
        .database()
        .job_detail(job.id)
        .map_err(internal)?
        .ok_or_else(|| internal("maintenance detail is missing"))?;
    serde_json::from_value::<MaintenanceDetail>(detail.detail).map_err(internal)?;
    Ok((job.state == JobState::Succeeded).then_some(dto::JobResultDto::StorageOptimized))
}

pub(super) struct MaintenanceHandler;

#[async_trait]
impl JobHandler for MaintenanceHandler {
    fn kinds(&self) -> &[JobKind] {
        &[JobKind::Maintenance]
    }

    fn requires_maintenance(&self) -> bool {
        true
    }

    fn lane(&self, _context: &ApiContext, job: &JobSnapshot) -> Option<JobLane> {
        (job.subject.id.as_str() == "storage-optimize")
            .then(|| JobLane("database-maintenance".into()))
    }

    async fn claim(
        &self,
        context: &ApiContext,
        job: &JobSnapshot,
        worker_id: WorkerId,
    ) -> Result<Option<Box<dyn ClaimedJob>>, ApiError> {
        let job = job.clone();
        context
            .blocking(move |context| {
                let database = context.backend().database();
                let at = context.now().max(job.updated_at);
                let detail = database
                    .job_detail(job.id)
                    .map_err(internal)?
                    .and_then(|record| {
                        serde_json::from_value::<MaintenanceDetail>(record.detail).ok()
                    });
                let Some(claim) = database
                    .claim(
                        job.id,
                        worker_id,
                        at,
                        Duration::from_millis(
                            u64::try_from(i64::MAX.saturating_sub(at.get())).map_err(internal)?,
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
                if detail.is_none() {
                    database
                        .append_and_transition(JobMutation::Fail {
                            claim: claim.claim,
                            error: JobError::new(
                                JobErrorCode::IntegrityFailure,
                                false,
                                "storage-unavailable",
                            )
                            .map_err(internal)?,
                            at,
                        })
                        .map_err(IntoApiError::into_api_error)?;
                    return Ok(None);
                }
                Ok(Some(Box::new(ClaimedMaintenance {
                    job_id: job.id,
                    claim: claim.claim,
                    cancellation: CancellationToken::new(),
                }) as Box<dyn ClaimedJob>))
            })
            .await
    }
}

struct ClaimedMaintenance {
    job_id: JobId,
    claim: ClaimRef,
    cancellation: CancellationToken,
}

#[async_trait]
impl ClaimedJob for ClaimedMaintenance {
    fn cancellation(&self) -> CancellationToken {
        self.cancellation.clone()
    }

    async fn run(
        self: Box<Self>,
        context: ApiContext,
        _progress: Arc<dyn JobProgressSink>,
    ) -> Result<(), ApiError> {
        let result = match context.maintenance().maintenance(&self.cancellation).await {
            None => Err(lettuce_database::StorageMaintenanceError::Cancelled),
            Some(lease) => {
                let cancelled = Arc::new(AtomicBool::new(false));
                let signal = cancelled.clone();
                let token = self.cancellation.clone();
                let cancellation = tokio::spawn(async move {
                    token.cancelled().await;
                    signal.store(true, Ordering::Release);
                });
                let result = optimize(&context, lease, cancelled).await;
                cancellation.abort();
                result
            }
        };
        context
            .blocking(move |context| self.settle(context, result))
            .await?;
        context.jobs().wake();
        context.wake_workers();
        Ok(())
    }
}

async fn optimize(
    context: &ApiContext,
    lease: tokio::sync::OwnedRwLockWriteGuard<()>,
    cancellation: Arc<AtomicBool>,
) -> Result<(), lettuce_database::StorageMaintenanceError> {
    let lifecycle = if let Some(files) = context.database_files() {
        let lifecycle = files
            .location
            .file_lifecycle()
            .await
            .map_err(|_| lettuce_database::StorageMaintenanceError::Storage)?;
        if files
            .location
            .active_path()
            .map_err(|_| lettuce_database::StorageMaintenanceError::Storage)?
            != files.active
        {
            return Err(lettuce_database::StorageMaintenanceError::ReadOnly);
        }
        Some(lifecycle)
    } else {
        None
    };
    let context = context.clone();
    tokio::task::spawn_blocking(move || {
        let _lease = lease;
        let _lifecycle = lifecycle;
        context.backend().database().optimize_storage(cancellation)
    })
    .await
    .map_err(|_| lettuce_database::StorageMaintenanceError::Storage)?
}

impl ClaimedMaintenance {
    fn settle(
        &self,
        context: &ApiContext,
        result: Result<(), lettuce_database::StorageMaintenanceError>,
    ) -> Result<(), ApiError> {
        use lettuce_database::StorageMaintenanceError;
        let database = context.backend().database();
        let job = database
            .get(self.job_id)
            .map_err(IntoApiError::into_api_error)?
            .ok_or_else(|| api_error(ApiErrorCode::NotFound, "maintenance job was not found"))?;
        if job.state.is_terminal() {
            return Ok(());
        }
        let at = context.now().max(job.updated_at);
        let mutation = match result {
            Ok(()) => JobMutation::Succeed {
                claim: self.claim.clone(),
                outcome: JobOutcome::Success {
                    result_ref: OutcomeRef::Request(RequestId::from_uuid(
                        super::jobs::local::stable_uuid(&[
                            "storage-optimize",
                            &self.job_id.to_string(),
                        ]),
                    )),
                },
                at,
            },
            Err(StorageMaintenanceError::Cancelled) => {
                if job.state != JobState::CancellationRequested {
                    database
                        .append_and_transition(JobMutation::RequestCancellation {
                            id: self.job_id,
                            reason: CancellationReason::Shutdown,
                            at,
                        })
                        .map_err(IntoApiError::into_api_error)?;
                }
                crate::models::artifact_install::finish_claimed_cancellation(
                    database,
                    &self.claim,
                    at,
                )
                .map_err(IntoApiError::into_api_error)?;
                return Ok(());
            }
            Err(error) => JobMutation::Fail {
                claim: self.claim.clone(),
                error: JobError::new(
                    JobErrorCode::StorageFailure,
                    error == StorageMaintenanceError::CheckpointBusy,
                    match error {
                        StorageMaintenanceError::CheckpointBusy => "storage-checkpoint-busy",
                        StorageMaintenanceError::ReadOnly => "database-kept-read-only",
                        _ => "storage-unavailable",
                    },
                )
                .map_err(internal)?,
                at,
            },
        };
        database
            .append_and_transition(mutation)
            .map_err(IntoApiError::into_api_error)?;
        Ok(())
    }
}
