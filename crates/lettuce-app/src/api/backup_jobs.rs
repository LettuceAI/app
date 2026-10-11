use std::{path::{Path, PathBuf}, sync::{Arc, Mutex}, time::Duration};

use async_trait::async_trait;
use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_database::Database;
use lettuce_jobs::{CancellationPolicy, CancellationReason, ClaimRef, JobError, JobErrorCode, JobKind, JobMutation, JobOutcome, JobSnapshot, JobSpec, JobState, JobStore, JobSubject, OutcomeRef, RecoveryPolicy, ResourceAvailability, ResourceClass, StageSnapshot, SubjectKind, WorkerId, handle::CancellationToken};
use lettuce_types::{JobId, OperationId, RequestId};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use super::{ApiContext, backup::{error, file_error}, error::IntoApiError, jobs::{ClaimedJob, JobHandler, JobLane, JobProgressSink}};

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
        super::speech::ensure_no_active_dictation(context)?;
        if detail.source.trim().is_empty() { return Err(super::error::invalid_field("source", "file source is empty")); }
        if invalid_password { return Err(super::error::invalid_field("password", "backup password is empty")); }
        database.admit_job_with_detail(spec(&detail)?, &detail.operation_key, &detail.request_digest, &encode(&detail)?).map_err(IntoApiError::into_api_error)
    }).await?;
    if job.state == JobState::Queued {
        context.jobs().remember_backup_password(job.id, password);
    }
    Ok(dto::JobAccepted { job_id: job.id.to_string() })
}

async fn admit_backup(context: &ApiContext, request: dto::BackupRestoreRequest, kind: RestoreKind, command: &str) -> Result<dto::JobAccepted, ApiError> {
    let encoded = Zeroizing::new(serde_json::to_vec(&request).map_err(|_| unavailable())?);
    let digest = blake3::hash(&encoded).to_hex().to_string();
    let key = super::jobs::local::operation_key(command, &request.client_operation_id)?;
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
    let key = super::jobs::local::operation_key("legacy_database_import", &request.client_operation_id)?;
    let digest = super::jobs::local::digest(&request)?;
    admit(context, RestoreKind::LegacyDatabase, source, Zeroizing::new(String::new()), key, digest).await
}

fn already_applied(context: &ApiContext, id: JobId) -> Result<bool, ApiError> {
    if super::app_reset::database_for_job(context, id)?.is_some() { return Ok(true); }
    let Some(record) = context.backend().database().job_detail(id).map_err(|_| unavailable())? else { return Ok(false); };
    let detail: Detail = serde_json::from_value(record.detail).map_err(|_| unavailable())?;
    Ok(detail.target.as_ref().is_some_and(|target| context.database_files().is_some_and(|files| &files.active == target)))
}

pub(super) struct RestoreHandler;

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

pub(super) fn settle(database: &Database, claim: &ClaimRef, context: &ApiContext, result: Result<dto::JobResultDto, &'static str>) -> Result<JobSnapshot, ApiError> {
    let job = database.get(claim.job_id).map_err(IntoApiError::into_api_error)?.ok_or_else(unavailable)?;
    let at = context.now().max(job.updated_at);
    match result {
        Ok(result) => database.settle_job_with_detail(JobMutation::Succeed { claim: claim.clone(), outcome: JobOutcome::Success { result_ref: OutcomeRef::Request(RequestId::new()) }, at }, Some(&encode(&result)?), None),
        Err(label) => database.settle_job_with_detail(JobMutation::Fail { claim: claim.clone(), error: JobError::new(if label == "backup-authentication" { JobErrorCode::Authentication } else { JobErrorCode::StorageFailure }, false, label).map_err(|_| unavailable())?, at }, None, None),
    }.map_err(IntoApiError::into_api_error)
}

impl Work {
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
            super::app_reset::host(context)?;
            context.flush_app_usage_for_reset()?;
        }
        context.backend().database().checkpoint_backup_job(JobMutation::StageChanged { claim: self.claim.clone(), stage: StageSnapshot::new(stage, stage == "switch").map_err(|_| unavailable())?, at: context.now() }, &encode(&self.detail)?).map_err(|failure| {
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
        if super::speech::refuse_active_dictation(&context, &self.claim)? { return Ok(()); }
        let Some(_lease) = context.maintenance().maintenance(&self.cancellation).await else { return self.cancel(&context); };
        if super::speech::refuse_active_dictation(&context, &self.claim)? { return Ok(()); }
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
                        .map_err(|failure| match failure {
                            crate::BackupRestoreError::Plan(lettuce_transfer::ProviderBackupRestorePlanError::Envelope(lettuce_transfer::BackupEnvelopeError::Authentication)) => "backup-authentication",
                            crate::BackupRestoreError::Plan(_) => "backup-invalid-envelope",
                            crate::BackupRestoreError::VerificationFailed => "backup-verification-failed",
                            crate::BackupRestoreError::Cancelled => "backup-cancelled",
                            crate::BackupRestoreError::Api(failure) if failure.code == ApiErrorCode::Cancelled => "backup-cancelled",
                            _ => "backup-unavailable",
                        }),
                    Err(_) => Err("backup-unavailable"),
                }
            }
            kind => {
                let coordinator = crate::LegacyRestoreCoordinator::new(&files.location, &authority, &workspace, context.secret_store().as_ref());
                let check = |stage: &str, database: Option<(&Database, &Path, &[lettuce_transfer::LegacyBackupWarning])>| self.checkpoint(&context, stage, database, &current).map_err(crate::LegacyRestoreError::Api);
                let receipt = if kind == RestoreKind::LegacyBackup {
                    match context.files().open(&self.detail.source) {
                        Ok(source) => coordinator.restore_backup_controlled(restore_id, source, &self.password, context.now(), &check).await,
                        Err(_) => return settle(context.backend().database(), &self.claim, &context, Err("backup-unavailable")).map(|_| ()),
                    }
                } else {
                    coordinator.restore_database_controlled(restore_id, Path::new(&self.detail.source), context.now(), &check).await
                };
                receipt.map(|receipt| receipt.warnings.into_iter().map(|warning| match warning { lettuce_transfer::LegacyBackupWarning::UnknownEntry { name } => dto::BackupWarning::UnknownArchiveEntry { name } }).collect())
                    .map_err(|failure| match failure {
                        crate::LegacyRestoreError::Inventory(lettuce_transfer::LegacyBackupInventoryError::Authentication) => "backup-authentication",
                        crate::LegacyRestoreError::Inventory(_) => "backup-invalid-envelope",
                        crate::LegacyRestoreError::Cancelled => "backup-cancelled",
                        crate::LegacyRestoreError::Api(failure) if failure.code == ApiErrorCode::Cancelled => "backup-cancelled",
                        _ => "backup-unavailable",
                    })
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
        let reloaded = super::app_reset::host(&context)?.reload_database(&self.claim.job_id.to_string()).await;
        let result = match (&reloaded, outcome) {
            (Ok(_), Ok(warnings)) => Ok(dto::JobResultDto::BackupRestored { kept_file: self.detail.kept_file.clone(), warnings }),
            _ => Err("backup-recovery-required"),
        };
        let succeeded = result.is_ok();
        match claim_started(&database, self.claim.job_id, context.now()).and_then(|claim| settle(&database, &claim, &context, result)) {
            Ok(job) => {
                let view = super::jobs::job_view(&context, &job)?;
                let (event, terminal) = super::jobs::job_event(&job, view);
                context.jobs().deliver(job.id, event, terminal);
            }
            Err(_) => super::backup_export_jobs::report_recovery(&context, &self.claim)?,
        }
        if let Ok(reloaded) = reloaded {
            reloaded.jobs().take_backup_password(self.claim.job_id);
            reloaded.finish_reloaded_job()?;
            if succeeded { reloaded.models_changed(); reloaded.emit(dto::ApiEvent::DatabaseReloaded); }
        }
        Ok(())
    }
}

pub(super) fn result_view(context: &ApiContext, job: &JobSnapshot) -> Result<Option<dto::JobResultDto>, ApiError> {
    if job.kind != JobKind::BackupRestore || job.state != JobState::Succeeded { return Ok(None); }
    let current = super::app_reset::database_for_job(context, job.id)?;
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

pub(super) fn recover_at_startup(context: &ApiContext) -> Result<(), ApiError> {
    use lettuce_jobs::{JobCatalog, JobListFilter};
    use lettuce_types::{PageLimit, PageRequest};
    let database = context.backend().database();
    let mut filter = JobListFilter { kinds: vec![JobKind::BackupRestore], states: vec![JobState::Queued], subject: None, page: PageRequest { cursor: None, limit: PageLimit::new(200) } };
    loop {
        let page = database.list_jobs(&filter).map_err(IntoApiError::into_api_error)?;
        for job in page.items {
            if job.subject.id.as_str() != "database-restore" || context.jobs().has_backup_password(job.id) || super::app_reset::database_for_job(context, job.id)?.is_some() { continue; }
            let detail: Detail = serde_json::from_value(database.job_detail(job.id).map_err(|_| unavailable())?.ok_or_else(unavailable)?.detail).map_err(|_| unavailable())?;
            remove_workspace(context, job.id)?;
            let switched = detail.target.as_ref().is_some_and(|target| context.database_files().is_some_and(|files| &files.active == target));
            let claim = claim_started(database, job.id, context.now().max(job.updated_at))?;
            let result = if switched { Ok(dto::JobResultDto::BackupRestored { kept_file: detail.kept_file, warnings: detail.warnings }) } else { Err("backup-retry-required") };
            settle(database, &claim, context, result)?;
        }
        let Some(cursor) = page.next_cursor else { return Ok(()); };
        filter.page.cursor = Some(cursor);
    }
}
