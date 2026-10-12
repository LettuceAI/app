use std::{path::{Path, PathBuf}, sync::{Arc, Mutex}, time::Duration};

use async_trait::async_trait;
use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_database::Database;
use lettuce_jobs::{CancellationPolicy, CancellationReason, ClaimRef, JobError, JobErrorCode, JobKind, JobMutation, JobOutcome, JobSnapshot, JobSpec, JobState, JobStore, JobSubject, OutcomeRef, RecoveryPolicy, ResourceAvailability, ResourceClass, StageSnapshot, SubjectKind, WorkerId, handle::CancellationToken};
use lettuce_types::{JobId, OperationId, RequestId};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::api::{ApiContext, backup::{error, file_error}, error::IntoApiError, jobs::{ClaimedJob, JobHandler, JobLane, JobProgressSink}};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RestoreKind { Current, LegacyBackup, LegacyDatabase }

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Detail {
    kind: RestoreKind,
    source: String,
    request_id: RequestId,
    operation_key: String,
    request_digest: String,
    target: Option<PathBuf>,
    kept_file: String,
    warnings: Vec<dto::BackupWarning>,
}

fn unavailable() -> ApiError { error(ApiErrorCode::Unavailable, dto::BackupFailureReason::Storage) }

fn encode<T: Serialize>(value: &T) -> Result<serde_json::Value, ApiError> {
    serde_json::to_value(value).map_err(|_| unavailable())
}

fn spec(detail: &Detail) -> Result<JobSpec, ApiError> {
    Ok(JobSpec::new(JobKind::BackupRestore, JobSubject::new(SubjectKind::Backup, "database-restore").map_err(|_| unavailable())?, OutcomeRef::Request(detail.request_id))
        .with_resources(vec![ResourceClass::DiskWrite])
        .with_policies(RecoveryPolicy::Restart, CancellationPolicy::UntilIrreversibleStage))
}

async fn admit(context: &ApiContext, kind: RestoreKind, source: String, password: Zeroizing<String>, key: String, digest: String) -> Result<dto::JobAccepted, ApiError> {
    let files = context.database_files().ok_or_else(unavailable)?;
    let kept_file = files.active.file_name().and_then(|value| value.to_str()).ok_or_else(unavailable)?.to_owned();
    let detail = Detail { kind, source, request_id: RequestId::new(), operation_key: key, request_digest: digest, target: None, kept_file, warnings: Vec::new() };
    let invalid_password = kind != RestoreKind::LegacyDatabase && password.is_empty();
    let job = context.blocking(move |context| {
        let database = context.backend().database();
        if let Some(previous) = database.job_operation(&detail.operation_key).map_err(|_| unavailable())? {
            if previous.request_digest != detail.request_digest { return Err(error(ApiErrorCode::Conflict, dto::BackupFailureReason::Storage)); }
            return database.get(previous.job_id).map_err(IntoApiError::into_api_error)?.ok_or_else(unavailable);
        }
        crate::api::speech::ensure_no_active_dictation(context)?;
        if detail.source.trim().is_empty() { return Err(crate::api::error::invalid_field("source", "file source is empty")); }
        if invalid_password { return Err(crate::api::error::invalid_field("password", "backup password is empty")); }
        let admitted = database.admit_job_with_detail_result(spec(&detail)?, &detail.operation_key, &detail.request_digest, &encode(&detail)?).map_err(IntoApiError::into_api_error)?;
        if admitted.created && admitted.job.state == JobState::Queued { context.jobs().remember_backup_password(admitted.job.id, password); }
        Ok(admitted.job)
    }).await?;
    Ok(dto::JobAccepted { job_id: job.id.to_string() })
}

async fn admit_backup(context: &ApiContext, request: dto::BackupRestoreRequest, kind: RestoreKind, command: &str) -> Result<dto::JobAccepted, ApiError> {
    let digest = crate::api::jobs::local::digest(&serde_json::json!({ "source": request.source, "client_operation_id": request.client_operation_id }))?;
    let key = crate::api::jobs::local::operation_key(command, &request.client_operation_id)?;
    admit(context, kind, request.source.uri, Zeroizing::new(request.password.trim().to_owned()), key, digest).await
}

pub async fn backup_restore(context: &ApiContext, request: dto::BackupRestoreRequest) -> Result<dto::JobAccepted, ApiError> {
    admit_backup(context, request, RestoreKind::Current, "backup_restore").await
}

pub async fn legacy_backup_restore(context: &ApiContext, request: dto::BackupRestoreRequest) -> Result<dto::JobAccepted, ApiError> {
    admit_backup(context, request, RestoreKind::LegacyBackup, "legacy_backup_restore").await
}

pub async fn legacy_database_import(context: &ApiContext, request: dto::LegacyDatabaseImportRequest) -> Result<dto::JobAccepted, ApiError> {
    let source = context.app_folder().ok_or_else(unavailable)?.display().to_string();
    let key = crate::api::jobs::local::operation_key("legacy_database_import", &request.client_operation_id)?;
    let digest = crate::api::jobs::local::digest(&request)?;
    admit(context, RestoreKind::LegacyDatabase, source, Zeroizing::new(String::new()), key, digest).await
}

pub(in crate::api) fn already_applied(context: &ApiContext, id: JobId) -> Result<bool, ApiError> {
    if crate::api::app::reset::database_for_job(context, id)?.is_some() { return Ok(true); }
    let Some(record) = context.backend().database().job_detail(id).map_err(|_| unavailable())? else { return Ok(false); };
    let detail: Detail = serde_json::from_value(record.detail).map_err(|_| unavailable())?;
    Ok(detail.target.as_ref().is_some_and(|target| context.database_files().is_some_and(|files| &files.active == target)))
}

pub(in crate::api) struct RestoreHandler;

#[async_trait]
impl JobHandler for RestoreHandler {
    fn kinds(&self) -> &[JobKind] { &[JobKind::BackupRestore] }
    fn requires_maintenance(&self) -> bool { true }
    fn lane(&self, context: &ApiContext, job: &JobSnapshot) -> Option<JobLane> {
        (job.subject.id.as_str() == "database-restore" && context.jobs().has_backup_password(job.id) && already_applied(context, job.id).is_ok_and(|applied| !applied)).then(|| JobLane("database-maintenance".into()))
    }
    async fn claim(&self, context: &ApiContext, job: &JobSnapshot, worker: WorkerId) -> Result<Option<Box<dyn ClaimedJob>>, ApiError> {
        let id = job.id;
        if already_applied(context, id)? { return Ok(None); }
        context.blocking(move |context| {
            let database = context.backend().database();
            let at = context.now();
            let Some(claim) = database.claim(id, worker, at, Duration::from_millis(u64::try_from(i64::MAX.saturating_sub(at.get())).map_err(|_| unavailable())?), &ResourceAvailability::all()).map_err(IntoApiError::into_api_error)? else { return Ok(None); };
            let token = CancellationToken::new();
            if let Err(failure) = database.append_and_transition(JobMutation::Start { claim: claim.claim.clone(), at }) {
                if database.get(id).map_err(IntoApiError::into_api_error)?.is_some_and(|job| job.state == JobState::CancellationRequested) {
                    crate::models::artifact_install::finish_claimed_cancellation(database, &claim.claim, at).map_err(IntoApiError::into_api_error)?;
                    context.jobs().take_backup_password(id);
                    return Ok(None);
                }
                return Err(failure.into_api_error());
            }
            let password = context.jobs().take_backup_password(id).ok_or_else(unavailable)?;
            let detail: Detail = serde_json::from_value(database.job_detail(id).map_err(|_| unavailable())?.ok_or_else(unavailable)?.detail).map_err(|_| unavailable())?;
            Ok(Some(Box::new(Work { claim: claim.claim, cancellation: token, password, detail }) as Box<dyn ClaimedJob>))
        }).await
    }
}

struct Work { claim: ClaimRef, cancellation: CancellationToken, password: Zeroizing<String>, detail: Detail }

pub(super) fn claim_started(database: &Database, id: JobId, at: lettuce_types::TimestampMillis) -> Result<ClaimRef, ApiError> {
    let claim = database.claim(id, WorkerId::new(), at, Duration::from_secs(3600), &ResourceAvailability::all()).map_err(IntoApiError::into_api_error)?.ok_or_else(unavailable)?;
    database.append_and_transition(JobMutation::Start { claim: claim.claim.clone(), at }).map_err(IntoApiError::into_api_error)?;
    Ok(claim.claim)
}

pub(super) fn file_failure_label(failure: &ApiError) -> &'static str {
    if matches!(failure.details, Some(dto::ApiErrorDetails::Backup { reason: dto::BackupFailureReason::LimitExceeded })) { return "backup-limit-exceeded"; }
    if matches!(failure.details, Some(dto::ApiErrorDetails::Backup { reason: dto::BackupFailureReason::PermissionDenied })) {
        return "backup-permission-denied";
    }
    if matches!(failure.details, Some(dto::ApiErrorDetails::Backup { reason: dto::BackupFailureReason::FileUnavailable })) {
        return match failure.code {
            ApiErrorCode::NotFound => "backup-file-not-found",
            ApiErrorCode::Busy => "backup-file-busy",
            ApiErrorCode::Unsupported => "backup-file-unsupported",
            ApiErrorCode::Conflict => "backup-source-is-target",
            _ => "backup-file-unavailable",
        };
    }
    "backup-unavailable"
}

pub(super) fn settle(database: &Database, claim: &ClaimRef, context: &ApiContext, result: Result<dto::JobResultDto, &'static str>) -> Result<JobSnapshot, ApiError> {
    let job = database.get(claim.job_id).map_err(IntoApiError::into_api_error)?.ok_or_else(unavailable)?;
    let at = context.now().max(job.updated_at);
    match result {
        Ok(result) => database.settle_job_with_detail(JobMutation::Succeed { claim: claim.clone(), outcome: JobOutcome::Success { result_ref: OutcomeRef::Request(RequestId::new()) }, at }, Some(&encode(&result)?), None),
        Err(label) => {
            let code = match label {
                "backup-authentication" => JobErrorCode::Authentication,
                "backup-file-not-found" | "backup-permission-denied" | "backup-file-busy" | "backup-file-unavailable" => JobErrorCode::ResourceUnavailable,
                "backup-file-unsupported" => JobErrorCode::CapabilityUnavailable,
                "backup-source-is-target" | "backup-limit-exceeded" | "backup-invalid-envelope" => JobErrorCode::InvalidInput,
                "backup-verification-failed" => JobErrorCode::IntegrityFailure,
                _ => JobErrorCode::StorageFailure,
            };
            database.settle_job_with_detail(JobMutation::Fail { claim: claim.clone(), error: JobError::new(code, false, label).map_err(|_| unavailable())?, at }, None, None)
        }
    }.map_err(IntoApiError::into_api_error)
}

fn restore_failure_label(failure: crate::BackupRestoreError) -> &'static str {
    use crate::BackupRestoreError as Failure;
    use lettuce_transfer::{BackupEnvelopeError as Envelope, ProviderBackupGraphError as Graph, ProviderBackupRestorePlanError as Plan};
    match failure {
        Failure::Plan(Plan::Envelope(Envelope::Authentication)) => "backup-authentication",
        Failure::Plan(Plan::Io | Plan::Envelope(Envelope::Io)) => "backup-file-unavailable",
        Failure::Plan(Plan::Envelope(Envelope::LimitExceeded) | Plan::InvalidGraph(Graph::LimitExceeded)) | Failure::Graph(Graph::LimitExceeded) => "backup-limit-exceeded",
        Failure::Plan(_) | Failure::Graph(_) => "backup-invalid-envelope",
        Failure::Location(crate::AppDatabaseLocationError::Busy) => "backup-file-busy",
        Failure::TargetDirectory | Failure::Location(_) | Failure::Workspace(_) | Failure::Database(_) | Failure::Restore(_) | Failure::Media(_) | Failure::ReadBack(_) => "backup-file-unavailable",
        Failure::VerificationFailed => "backup-verification-failed",
        Failure::Cancelled => "backup-cancelled",
        Failure::Api(failure) if failure.code == ApiErrorCode::Cancelled => "backup-cancelled",
        Failure::Api(failure) => file_failure_label(&failure),
        _ => "backup-unavailable",
    }
}

fn legacy_restore_failure_label(failure: crate::LegacyRestoreError) -> &'static str {
    use crate::LegacyRestoreError as Failure;
    use crate::LegacyDatabaseImportPlanError as Plan;
    use lettuce_transfer::{LegacyBackupCompatibilityError as Compatibility, LegacyBackupInventoryError as Inventory, LegacyDatabasePreflightError as DatabaseFailure};
    match failure {
        Failure::Inventory(Inventory::Authentication) => "backup-authentication",
        Failure::Inventory(Inventory::Io) => "backup-file-unavailable",
        Failure::Inventory(Inventory::LimitExceeded) | Failure::Compatibility(Compatibility::LimitExceeded) | Failure::Plan(Plan::Compatibility(Compatibility::LimitExceeded)) | Failure::Plan(Plan::Database(DatabaseFailure::LimitExceeded { .. } | DatabaseFailure::MediaReferenceLimitExceeded { .. })) => "backup-limit-exceeded",
        Failure::Inventory(_) => "backup-invalid-envelope",
        Failure::Plan(Plan::Database(DatabaseFailure::Unavailable)) => "backup-file-unavailable",
        Failure::Compatibility(_) | Failure::Plan(Plan::Compatibility(_)) => "backup-invalid-envelope",
        Failure::Location(crate::AppDatabaseLocationError::Busy) => "backup-file-busy",
        Failure::TargetDirectory | Failure::Location(_) | Failure::Workspace(_) | Failure::Database(_) | Failure::Open(_) | Failure::MediaStore(_) | Failure::Stage { .. } => "backup-file-unavailable",
        Failure::Cancelled => "backup-cancelled",
        Failure::Api(failure) if failure.code == ApiErrorCode::Cancelled => "backup-cancelled",
        Failure::Api(failure) => file_failure_label(&failure),
        _ => "backup-unavailable",
    }
}

impl Work {
    async fn finish_switched(&self, context: ApiContext, database: Arc<Database>, outcome: Result<Vec<dto::BackupWarning>, &'static str>) -> Result<(), ApiError> {
        let reloaded = crate::api::app::reset::host(&context)?.reload_database(&self.claim.job_id.to_string()).await;
        let settled = (|| {
            let result = match (&reloaded, outcome) {
                (Ok(_), Ok(warnings)) => Ok(dto::JobResultDto::BackupRestored { kept_file: self.detail.kept_file.clone(), warnings }),
                (Ok(_), Err(_)) => {
                    let detail: Detail = serde_json::from_value(database.job_detail(self.claim.job_id).map_err(|_| unavailable())?.ok_or_else(unavailable)?.detail).map_err(|_| unavailable())?;
                    Ok(dto::JobResultDto::BackupRestored { kept_file: self.detail.kept_file.clone(), warnings: detail.warnings })
                }
                _ => Err("backup-recovery-required"),
            };
            claim_started(&database, self.claim.job_id, context.now()).and_then(|claim| settle(&database, &claim, &context, result))
        })();
        match settled {
            Ok(job) => {
                let view = crate::api::jobs::job_view(&context, &job)?;
                let (event, terminal) = crate::api::jobs::job_event(&job, view);
                context.jobs().deliver(job.id, event, terminal);
            }
            Err(_) => super::export_jobs::report_recovery(&context, &self.claim)?,
        }
        if let Ok(reloaded) = reloaded {
            reloaded.jobs().take_backup_password(self.claim.job_id);
            reloaded.finish_reloaded_job()?;
            reloaded.models_changed();
            reloaded.emit(dto::ApiEvent::DatabaseReloaded);
        }
        Ok(())
    }

    fn cancel(&self, context: &ApiContext) -> Result<(), ApiError> {
        let database = context.backend().database();
        let job = database.get(self.claim.job_id).map_err(IntoApiError::into_api_error)?.ok_or_else(unavailable)?;
        let at = context.now().max(job.updated_at);
        if job.state != JobState::CancellationRequested {
            database.append_and_transition(JobMutation::StageChanged { claim: self.claim.clone(), stage: StageSnapshot::new("cancel", false).map_err(|_| unavailable())?, at }).map_err(IntoApiError::into_api_error)?;
            database.append_and_transition(JobMutation::RequestCancellation { id: job.id, reason: CancellationReason::Shutdown, at }).map_err(IntoApiError::into_api_error)?;
        }
        crate::models::artifact_install::finish_claimed_cancellation(database, &self.claim, at).map_err(IntoApiError::into_api_error)?;
        Ok(())
    }

    fn checkpoint(&self, context: &ApiContext, stage: &str, target: Option<(&Database, &Path, &[lettuce_transfer::LegacyBackupWarning])>, current: &Mutex<Option<Arc<Database>>>) -> Result<(), ApiError> {
        if self.cancellation.is_cancelled() { return Err(error(ApiErrorCode::Cancelled, dto::BackupFailureReason::Cancelled)); }
        if let Some((database, path, warnings)) = target {
            remove_workspace(context, self.claim.job_id)?;
            let mut detail = self.detail.clone();
            detail.target = Some(path.to_owned());
            detail.warnings = warnings.iter().cloned().map(|warning| match warning { lettuce_transfer::LegacyBackupWarning::UnknownEntry { name } => dto::BackupWarning::UnknownArchiveEntry { name } }).collect();
            database.admit_job_with_detail_and_id(spec(&detail)?, &detail.operation_key, &detail.request_digest, &encode(&detail)?, self.claim.job_id).map_err(IntoApiError::into_api_error)?;
            let owned = Arc::new(Database::open(path).map_err(|_| unavailable())?);
            context.track_reloaded_job(self.claim.job_id, owned.clone())?;
            *current.lock().map_err(|_| unavailable())? = Some(owned);
            return Ok(());
        }
        if stage == "switch" {
            crate::api::app::reset::host(context)?;
            context.flush_app_usage_for_reset()?;
        }
        context.backend().database().append_and_transition(JobMutation::StageChanged { claim: self.claim.clone(), stage: StageSnapshot::new(stage, stage == "switch").map_err(|_| unavailable())?, at: context.now() }).map_err(|failure| {
            if failure == lettuce_jobs::StoreError::IllegalTransition && context.backend().database().get(self.claim.job_id).ok().flatten().is_some_and(|job| job.state == JobState::CancellationRequested) { error(ApiErrorCode::Cancelled, dto::BackupFailureReason::Cancelled) }
            else { failure.into_api_error() }
        })?;
        Ok(())
    }
}

#[async_trait]
impl ClaimedJob for Work {
    fn cancellation(&self) -> CancellationToken { self.cancellation.clone() }
    async fn run(self: Box<Self>, context: ApiContext, _progress: Arc<dyn JobProgressSink>) -> Result<(), ApiError> {
        if crate::api::speech::refuse_active_dictation(&context, &self.claim)? { return Ok(()); }
        let Some(_lease) = context.maintenance().maintenance(&self.cancellation).await else { return self.cancel(&context); };
        if crate::api::speech::refuse_active_dictation(&context, &self.claim)? { return Ok(()); }
        let files = context.database_files().ok_or_else(unavailable)?;
        if files.location.active_path().map_err(|_| unavailable())? != files.active { return Err(unavailable()); }
        let root = context.app_folder().ok_or_else(unavailable)?;
        let workspace = root.join("platform-v2/backup-restore-workspace");
        let media = root.join("platform-v2/media-blobs");
        let current = Mutex::new(None);
        let restore_id: OperationId = self.claim.job_id.to_string().parse().map_err(|_| unavailable())?;
        let authority = lettuce_platform::FilesystemAuthority::new(&lettuce_platform::DirectorySnapshot::new(root).map_err(|_| unavailable())?).map_err(|_| unavailable())?;
        let outcome: Result<Vec<dto::BackupWarning>, &'static str> = match self.detail.kind {
            RestoreKind::Current => {
                let source = context.files().open(&self.detail.source).map_err(file_error);
                match source {
                    Ok(source) => crate::BackupRestoreCoordinator::new(&files.location, &workspace, &media, context.secret_store().as_ref())
                        .restore_controlled(restore_id, source, &self.password, context.now(), &|stage, database| self.checkpoint(&context, stage, database.map(|(database, path)| (database, path, &[][..])), &current).map_err(crate::BackupRestoreError::Api)).await
                        .map(|receipt| receipt.app_secret_failures.into_iter().map(|failure| dto::BackupWarning::AppSecretUnavailable { purpose: format!("{:?}", failure.purpose), stage: match failure.stage { crate::BackupAppSecretFailureStage::Status => dto::BackupSecretStage::Status, crate::BackupAppSecretFailureStage::Store => dto::BackupSecretStage::Store } }).collect())
                        .map_err(restore_failure_label),
                    Err(failure) => Err(file_failure_label(&failure)),
                }
            }
            kind => {
                let coordinator = crate::LegacyRestoreCoordinator::new(&files.location, &authority, &workspace, context.secret_store().as_ref());
                let check = |stage: &str, database: Option<(&Database, &Path, &[lettuce_transfer::LegacyBackupWarning])>| self.checkpoint(&context, stage, database, &current).map_err(crate::LegacyRestoreError::Api);
                let receipt = if kind == RestoreKind::LegacyBackup {
                    match context.files().open(&self.detail.source) {
                        Ok(source) => coordinator.restore_backup_controlled(restore_id, source, &self.password, context.now(), &check).await,
                        Err(failure) => return settle(context.backend().database(), &self.claim, &context, Err(file_failure_label(&file_error(failure)))).map(|_| ()),
                    }
                } else {
                    coordinator.restore_database_controlled(Path::new(&self.detail.source), context.now(), &check).await
                };
                receipt.map(|receipt| receipt.warnings.into_iter().map(|warning| match warning { lettuce_transfer::LegacyBackupWarning::UnknownEntry { name } => dto::BackupWarning::UnknownArchiveEntry { name } }).collect())
                    .map_err(legacy_restore_failure_label)
            }
        };
        let switched = files.location.active_path().map_err(|_| unavailable())? != files.active;
        if !switched {
            let cleaned = remove_workspace(&context, self.claim.job_id).is_ok();
            context.finish_reloaded_job()?;
            let outcome = if cleaned { outcome } else { Err("backup-unavailable") };
            if cleaned && (self.cancellation.is_cancelled() || outcome.as_ref().is_err_and(|label| *label == "backup-cancelled")) { return self.cancel(&context); }
            settle(context.backend().database(), &self.claim, &context, outcome.map(|warnings| dto::JobResultDto::BackupRestored { kept_file: self.detail.kept_file.clone(), warnings }))?;
            return Ok(());
        }
        let database = current.into_inner().map_err(|_| unavailable())?.ok_or_else(unavailable)?;
        self.finish_switched(context, database, outcome).await
    }
}

pub(in crate::api) fn result_view(context: &ApiContext, job: &JobSnapshot) -> Result<Option<dto::JobResultDto>, ApiError> {
    if job.kind != JobKind::BackupRestore || job.state != JobState::Succeeded { return Ok(None); }
    let current = crate::api::app::reset::database_for_job(context, job.id)?;
    let database = current.as_deref().unwrap_or_else(|| context.backend().database());
    let record = database.job_detail(job.id).map_err(|_| unavailable())?.ok_or_else(unavailable)?;
    record.result.map(|result| serde_json::from_value(result).map_err(|_| unavailable())).transpose()
}

fn remove_workspace(context: &ApiContext, id: JobId) -> Result<(), ApiError> {
    let root = context.app_folder().ok_or_else(unavailable)?.join("platform-v2/backup-restore-workspace").join(id.to_string());
    match std::fs::remove_dir_all(root) {
        Ok(()) => Ok(()),
        Err(failure) if failure.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(unavailable()),
    }
}

pub(in crate::api) fn recover_at_startup(context: &ApiContext) -> Result<(), ApiError> {
    use lettuce_jobs::{JobCatalog, JobListFilter};
    use lettuce_types::{PageLimit, PageRequest};
    let database = context.backend().database();
    let mut filter = JobListFilter { kinds: vec![JobKind::BackupRestore], states: vec![JobState::Queued, JobState::Interrupted], subject: None, page: PageRequest { cursor: None, limit: PageLimit::new(200) } };
    loop {
        let page = match database.list_jobs(&filter) {
            Ok(page) => page,
            Err(failure) => { tracing::warn!(%failure, "backup restore recovery could not list jobs"); return Ok(()); }
        };
        for job in page.items {
            if job.subject.id.as_str() != "database-restore" || context.jobs().has_backup_password(job.id) { continue; }
            if let Err(failure) = recover_job(context, &job) {
                tracing::warn!(job_id = %job.id, ?failure, "backup restore recovery will retry on next startup");
            }
        }
        let Some(cursor) = page.next_cursor else { return Ok(()); };
        filter.page.cursor = Some(cursor);
    }
}

fn recover_job(context: &ApiContext, job: &JobSnapshot) -> Result<(), ApiError> {
    if crate::api::app::reset::database_for_job(context, job.id)?.is_some() { return Ok(()); }
    let database = context.backend().database();
    let detail: Detail = serde_json::from_value(database.job_detail(job.id).map_err(|_| unavailable())?.ok_or_else(unavailable)?.detail).map_err(|_| unavailable())?;
    remove_workspace(context, job.id)?;
    if job.state == JobState::Interrupted {
        database.append_and_transition(JobMutation::FinishInterruptedBackupCleanup { id: job.id, at: context.now().max(job.updated_at) }).map_err(IntoApiError::into_api_error)?;
        return Ok(());
    }
    let switched = detail.target.as_ref().is_some_and(|target| context.database_files().is_some_and(|files| &files.active == target));
    let claim = claim_started(database, job.id, context.now().max(job.updated_at))?;
    let result = if switched { Ok(dto::JobResultDto::BackupRestored { kept_file: detail.kept_file, warnings: detail.warnings }) } else { Err("backup-retry-required") };
    settle(database, &claim, context, result)?;
    Ok(())
}

#[cfg(test)]
mod fix_tests {
    use super::*;

    #[test]
    fn fix11a_current_source_io_keeps_its_reason() {
        assert_eq!(restore_failure_label(crate::BackupRestoreError::Plan(lettuce_transfer::ProviderBackupRestorePlanError::Io)), "backup-file-unavailable");
        assert_eq!(restore_failure_label(crate::BackupRestoreError::Plan(lettuce_transfer::ProviderBackupRestorePlanError::Envelope(lettuce_transfer::BackupEnvelopeError::Io))), "backup-file-unavailable");
    }

    #[test]
    fn fix11a_format_limits_keep_their_reason() {
        assert_eq!(restore_failure_label(crate::BackupRestoreError::Plan(lettuce_transfer::ProviderBackupRestorePlanError::Envelope(lettuce_transfer::BackupEnvelopeError::LimitExceeded))), "backup-limit-exceeded");
        assert_eq!(legacy_restore_failure_label(crate::LegacyRestoreError::Inventory(lettuce_transfer::LegacyBackupInventoryError::LimitExceeded)), "backup-limit-exceeded");
        assert_eq!(restore_failure_label(crate::BackupRestoreError::Plan(lettuce_transfer::ProviderBackupRestorePlanError::InvalidGraph(lettuce_transfer::ProviderBackupGraphError::LimitExceeded))), "backup-limit-exceeded");
        assert_eq!(restore_failure_label(crate::BackupRestoreError::Graph(lettuce_transfer::ProviderBackupGraphError::LimitExceeded)), "backup-limit-exceeded");
        assert_eq!(legacy_restore_failure_label(crate::LegacyRestoreError::Compatibility(lettuce_transfer::LegacyBackupCompatibilityError::LimitExceeded)), "backup-limit-exceeded");
        assert_eq!(legacy_restore_failure_label(crate::LegacyRestoreError::Plan(crate::LegacyDatabaseImportPlanError::Compatibility(lettuce_transfer::LegacyBackupCompatibilityError::LimitExceeded))), "backup-limit-exceeded");
        assert_eq!(legacy_restore_failure_label(crate::LegacyRestoreError::Plan(crate::LegacyDatabaseImportPlanError::Database(lettuce_transfer::LegacyDatabasePreflightError::LimitExceeded { table: "messages", limit: u32::MAX }))), "backup-limit-exceeded");
        assert_eq!(legacy_restore_failure_label(crate::LegacyRestoreError::Plan(crate::LegacyDatabaseImportPlanError::Database(lettuce_transfer::LegacyDatabasePreflightError::MediaReferenceLimitExceeded { limit: u32::MAX }))), "backup-limit-exceeded");
    }

    #[test]
    fn fix11a_legacy_compatibility_format_errors_keep_their_reason() {
        assert_eq!(legacy_restore_failure_label(crate::LegacyRestoreError::Compatibility(lettuce_transfer::LegacyBackupCompatibilityError::InvalidMeta)), "backup-invalid-envelope");
        assert_eq!(legacy_restore_failure_label(crate::LegacyRestoreError::Plan(crate::LegacyDatabaseImportPlanError::Compatibility(lettuce_transfer::LegacyBackupCompatibilityError::InvalidMeta))), "backup-invalid-envelope");
    }

    struct Fixture {
        root: PathBuf,
        location: crate::AppDatabaseLocation,
        harness: crate::api::tests::Harness,
        host: Arc<super::super::tests::ReloadHost>,
        work: Work,
        source: Vec<u8>,
    }

    async fn fixture() -> Fixture {
        let (root, location, harness) = crate::api::app::reset_tests::harness();
        let host = Arc::new(super::super::tests::ReloadHost { root: root.clone(), location: location.clone(), provider: harness.provider.clone(), events: harness.events.clone(), slot: Arc::new(crate::api::ApiContextSlot::new(Arc::new(harness.context.clone()))), workers: Mutex::new(None), fail: false, fail_settlement: false });
        harness.context.attach_reset_host(host.clone()).expect("host");
        let active = location.active_path().expect("active");
        let media = lettuce_media::LocalSyncMediaStore::open(root.join("platform-v2/media-blobs"), Database::open(&active).expect("blobs"), Database::open(&active).expect("assets")).expect("media");
        let source = harness.context.backend().provider_backup(harness.context.secret_store().as_ref(), &media).export("1.0", harness.context.now(), "password").await.expect("archive").output;
        let input = root.join("input.lettuce");
        std::fs::write(&input, &source).expect("source");
        let accepted = backup_restore(&harness.context, dto::BackupRestoreRequest { password: "password".into(), source: dto::FileSource { uri: input.display().to_string() }, client_operation_id: "cutover-fault".into() }).await.expect("admit");
        let id = accepted.job_id.parse().expect("id");
        harness.context.jobs().take_backup_password(id);
        let database = harness.context.backend().database();
        let claim = claim_started(database, id, harness.context.now()).expect("claim");
        let detail = serde_json::from_value(database.job_detail(id).expect("detail").expect("record").detail).expect("restore detail");
        Fixture { root, location, harness, host, work: Work { claim, detail, cancellation: CancellationToken::new(), password: Zeroizing::new("password".into()) }, source }
    }

    #[tokio::test]
    async fn fix11a_switch_success_with_lifecycle_bookkeeping_error_reloads_once() {
        let fixture = fixture().await;
        let context = &fixture.harness.context;
        let old = fixture.location.active_path().expect("old");
        let metadata = old.with_file_name(format!("{}.meta.json", old.file_name().expect("name").to_str().expect("text")));
        let saved = Mutex::new(None);
        let current = Mutex::new(None);
        let outcome = crate::BackupRestoreCoordinator::new(&fixture.location, &fixture.root.join("platform-v2/backup-restore-workspace"), &fixture.root.join("platform-v2/media-blobs"), context.secret_store().as_ref())
            .restore_controlled(fixture.work.claim.job_id.to_string().parse().expect("operation"), std::io::Cursor::new(fixture.source), "password", context.now(), &|stage, target| {
                fixture.work.checkpoint(context, stage, target.map(|(database, path)| (database, path, &[][..])), &current).map_err(crate::BackupRestoreError::Api)?;
                if target.is_some() {
                    *saved.lock().expect("saved metadata") = Some(std::fs::read(&metadata).expect("keep intent"));
                    std::fs::remove_file(&metadata).expect("remove metadata");
                    std::fs::create_dir(&metadata).expect("metadata write failure");
                }
                Ok(())
            }).await;
        assert!(outcome.is_err(), "finish_cutover fails after the pointer write");
        assert_ne!(fixture.location.active_path().expect("switched"), old);
        std::fs::remove_dir(&metadata).expect("repair fault");
        std::fs::write(&metadata, saved.into_inner().expect("saved").expect("metadata")).expect("repair metadata");
        let database = current.into_inner().expect("current").expect("candidate");
        fixture.work.finish_switched(context.clone(), database, Err("backup-unavailable")).await.expect("rebind succeeds");
        let fresh = fixture.host.slot.snapshot().expect("commands stay available");
        assert_eq!(fixture.harness.events.events().iter().filter(|event| matches!(event, dto::ApiEvent::DatabaseReloaded)).count(), 1);
        assert_eq!(crate::api::job_get(&fresh, dto::JobGetRequest { job_id: fixture.work.claim.job_id.to_string() }).await.expect("completed job").state, dto::JobStateDto::Succeeded);
        let workers = fixture.host.workers.lock().expect("workers").take().expect("started");
        workers.stop_for_restore().await.expect("stop");
    }

    #[tokio::test]
    async fn fix11a_crash_before_pointer_switch_keeps_old_database_and_retry_uses_a_fresh_name() {
        let fixture = fixture().await;
        let old = fixture.location.active_path().expect("old");
        let old_digest = blake3::hash(&std::fs::read(&old).expect("old bytes"));
        let original_context = fixture.harness.context.clone();
        let original_location = fixture.location.clone();
        let host = fixture.host.clone();
        let crash = tokio::spawn(async move {
            let current = Mutex::new(None);
            let context = &fixture.harness.context;
            crate::BackupRestoreCoordinator::new(&fixture.location, &fixture.root.join("platform-v2/backup-restore-workspace"), &fixture.root.join("platform-v2/media-blobs"), context.secret_store().as_ref())
                .restore_controlled(fixture.work.claim.job_id.to_string().parse().expect("operation"), std::io::Cursor::new(fixture.source), "password", context.now(), &|stage, target| {
                    fixture.work.checkpoint(context, stage, target.map(|(database, path)| (database, path, &[][..])), &current).map_err(crate::BackupRestoreError::Api)?;
                    if let Some((database, _)) = target {
                        assert_eq!(database.get(fixture.work.claim.job_id).expect("candidate job").expect("receipt").state, JobState::Queued);
                        panic!("process crash after candidate job receipt and before pointer switch");
                    }
                    Ok(())
                }).await.expect("restore");
        }).await.expect_err("crashed restore");
        assert!(crash.is_panic());
        assert_eq!(original_location.active_path().expect("pointer before reopen"), old);
        let guard = original_location.try_file_lifecycle().expect("reopen recovery");
        assert!(!original_context.backend().database().is_file_write_fenced().expect("old writes recovered"));
        let inventory = guard.inventory(&old).expect("inventory");
        let candidate = inventory.iter().find(|file| file.file != old.file_name().expect("old name").to_str().expect("text")).expect("abandoned candidate").file.clone();
        let candidate_path = original_location.database_path(&candidate).expect("candidate path");
        drop(guard);
        assert_eq!(blake3::hash(&std::fs::read(&old).expect("kept bytes")), old_digest);
        original_context.finish_reloaded_job().expect("restart drops transient tracking");
        original_context.recover_after_restart().expect("job retry required");
        let source = original_context.app_folder().expect("folder").join("input.lettuce");
        let accepted = backup_restore(&original_context, dto::BackupRestoreRequest { password: "password".into(), source: dto::FileSource { uri: source.display().to_string() }, client_operation_id: "fresh-retry".into() }).await.expect("new attempt");
        let id = accepted.job_id.parse().expect("id");
        let job = original_context.backend().database().get(id).expect("job").expect("stored");
        let work = RestoreHandler.claim(&original_context, &job, WorkerId::new()).await.expect("claim").expect("work");
        work.run(original_context.clone(), Arc::new(super::super::tests::Progress)).await.expect("retry completed");
        let active = original_location.active_path().expect("new active");
        assert_ne!(active, old);
        assert_ne!(active, candidate_path);
        let workers = host.workers.lock().expect("workers").take().expect("retry workers");
        workers.stop_for_restore().await.expect("stop");
    }
}
