use std::{io::Write, sync::Arc, time::Duration};

use async_trait::async_trait;
use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_database::BackupIndexRecord;
use lettuce_jobs::{CancellationPolicy, CancellationReason, ClaimRef, JobKind, JobMutation, JobSnapshot, JobSpec, JobState, JobStore, JobSubject, OutcomeRef, RecoveryPolicy, ResourceAvailability, ResourceClass, StageSnapshot, SubjectKind, WorkerId, handle::CancellationToken};
use lettuce_types::{JobId, OperationId, RequestId, TimestampMillis};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::api::{ApiContext, backup::{error, file_error}, error::IntoApiError, files::FileExportProtection, jobs::{ClaimedJob, JobHandler, JobLane, JobProgressSink}};

#[derive(Clone, Serialize, Deserialize)]
struct Detail {
    target: Option<String>,
    temporary_intent: String,
    temporary_uri: Option<String>,
    final_name: String,
    header: dto::BackupHeaderView,
    size: Option<u64>,
    publication_header: Option<Vec<u8>>,
    warnings: Vec<dto::BackupWarning>,
}

fn unavailable() -> ApiError { error(ApiErrorCode::Unavailable, dto::BackupFailureReason::Storage) }
fn encoded(detail: &Detail) -> Result<serde_json::Value, ApiError> { serde_json::to_value(detail).map_err(|_| unavailable()) }

pub async fn backup_export(context: &ApiContext, request: dto::BackupExportRequest) -> Result<dto::JobAccepted, ApiError> {
    let key = crate::api::jobs::local::operation_key("backup_export", &request.client_operation_id)?;
    let digest = crate::api::jobs::local::digest(&serde_json::json!({ "target": request.target, "client_operation_id": request.client_operation_id }))?;
    let password = Zeroizing::new(request.password.trim().to_owned());
    let invalid_password = password.is_empty();
    let target = request.target.map(|target| target.uri);
    let job = context.blocking(move |context| {
        let database = context.backend().database();
        if let Some(previous) = database.job_operation(&key).map_err(|_| unavailable())? {
            if previous.request_digest != digest { return Err(error(ApiErrorCode::Conflict, dto::BackupFailureReason::Storage)); }
            return database.get(previous.job_id).map_err(IntoApiError::into_api_error)?.ok_or_else(unavailable);
        }
        if invalid_password { return Err(crate::api::error::invalid_field("password", "backup password is empty")); }
        if target.as_ref().is_some_and(|uri| uri.trim().is_empty()) { return Err(crate::api::error::invalid_field("target", "backup target is empty")); }
        let created_at = context.now();
        let date = chrono::DateTime::from_timestamp_millis(created_at.get()).ok_or_else(unavailable)?;
        let temporary_intent = context.files().backup_temporary_location(target.as_deref(), &format!("lettuce-backup-{}.partial", OperationId::new())).map_err(file_error)?;
        let target = target.map(|target| {
            let name = std::path::Path::new(&target).file_name().ok_or_else(|| file_error(crate::api::FileAccessError::Unsupported))?;
            std::path::Path::new(&temporary_intent).with_file_name(name).into_os_string().into_string().map_err(|_| file_error(crate::api::FileAccessError::Unsupported))
        }).transpose()?;
        let detail = Detail {
            temporary_intent,
            target,
            temporary_uri: None,
            final_name: format!("lettuce_backup_{}.lettuce", date.format("%Y%m%d_%H%M%S")),
            header: dto::BackupHeaderView { version: 2, created_at: created_at.get(), app_version: crate::app_version(env!("CARGO_PKG_VERSION")) },
            size: None,
            publication_header: None,
            warnings: Vec::new(),
        };
        let spec = JobSpec::new(JobKind::BackupExport, JobSubject::new(SubjectKind::Backup, "database-export").map_err(|_| unavailable())?, OutcomeRef::Request(RequestId::new()))
            .with_resources(vec![ResourceClass::DiskWrite])
            .with_policies(RecoveryPolicy::Restart, CancellationPolicy::UntilIrreversibleStage);
        let admitted = database.admit_job_with_detail_result(spec, &key, &digest, &encoded(&detail)?).map_err(IntoApiError::into_api_error)?;
        if admitted.created && admitted.job.state == JobState::Queued { context.jobs().remember_backup_password(admitted.job.id, password); }
        Ok(admitted.job)
    }).await?;
    Ok(dto::JobAccepted { job_id: job.id.to_string() })
}

pub(in crate::api) struct ExportHandler;

#[async_trait]
impl JobHandler for ExportHandler {
    fn kinds(&self) -> &[JobKind] { &[JobKind::BackupExport] }
    fn lane(&self, context: &ApiContext, job: &JobSnapshot) -> Option<JobLane> {
        (job.subject.id.as_str() == "database-export" && context.jobs().has_backup_password(job.id)).then(|| JobLane("backup-export".into()))
    }
    async fn claim(&self, context: &ApiContext, job: &JobSnapshot, worker: WorkerId) -> Result<Option<Box<dyn ClaimedJob>>, ApiError> {
        let id = job.id;
        context.blocking(move |context| {
            let database = context.backend().database();
            let at = context.now();
            let Some(claim) = database.claim(id, worker, at, Duration::from_millis(u64::try_from(i64::MAX.saturating_sub(at.get())).map_err(|_| unavailable())?), &ResourceAvailability::all()).map_err(IntoApiError::into_api_error)? else { return Ok(None); };
            if let Err(failure) = database.append_and_transition(JobMutation::Start { claim: claim.claim.clone(), at }) {
                if database.get(id).map_err(IntoApiError::into_api_error)?.is_some_and(|job| job.state == JobState::CancellationRequested) {
                    crate::models::artifact_install::finish_claimed_cancellation(database, &claim.claim, at).map_err(IntoApiError::into_api_error)?;
                    context.jobs().take_backup_password(id);
                    return Ok(None);
                }
                return Err(failure.into_api_error());
            }
            let detail = serde_json::from_value(database.job_detail(id).map_err(|_| unavailable())?.ok_or_else(unavailable)?.detail).map_err(|_| unavailable())?;
            let password = context.jobs().take_backup_password(id).ok_or_else(unavailable)?;
            Ok(Some(Box::new(Work { claim: claim.claim, cancellation: CancellationToken::new(), password, detail }) as Box<dyn ClaimedJob>))
        }).await
    }
}

struct Work { claim: ClaimRef, cancellation: CancellationToken, password: Zeroizing<String>, detail: Detail }

fn checkpoint(context: &ApiContext, claim: &ClaimRef, detail: &Detail, stage: &str, irreversible: bool) -> Result<(), ApiError> {
    let job = context.backend().database().get(claim.job_id).map_err(IntoApiError::into_api_error)?.ok_or_else(unavailable)?;
    context.backend().database().checkpoint_backup_job(JobMutation::StageChanged { claim: claim.clone(), stage: StageSnapshot::new(stage, irreversible).map_err(|_| unavailable())?, at: context.now().max(job.updated_at) }, &encoded(detail)?).map_err(|failure| {
        if failure == lettuce_jobs::StoreError::IllegalTransition && context.backend().database().get(claim.job_id).ok().flatten().is_some_and(|job| job.state == JobState::CancellationRequested) { error(ApiErrorCode::Cancelled, dto::BackupFailureReason::Cancelled) }
        else { failure.into_api_error() }
    })?;
    Ok(())
}

fn cleanup(context: &ApiContext, detail: &Detail) -> Result<(), ApiError> {
    let uri = detail.temporary_uri.as_deref().unwrap_or(&detail.temporary_intent);
    let description = match context.files().describe(uri) {
        Ok(description) => description,
        Err(crate::api::FileAccessError::NotFound) => return Ok(()),
        Err(failure) => return Err(file_error(failure)),
    };
    let intended = std::path::Path::new(&detail.temporary_intent).file_name().and_then(|name| name.to_str()).ok_or_else(unavailable)?;
    if !intended.starts_with("lettuce-backup-") || !intended.ends_with(".partial") { return Err(unavailable()); }
    if description.name != intended { return Ok(()); }
    match context.files().remove(uri) {
        Ok(()) | Err(crate::api::FileAccessError::NotFound) => Ok(()),
        Err(failure) => Err(file_error(failure)),
    }
}

fn published(context: &ApiContext, detail: &Detail) -> Result<Option<BackupIndexRecord>, ApiError> {
    let Some(size) = detail.size else { return Ok(None); };
    let Some(expected_header) = &detail.publication_header else { return Ok(None); };
    let candidates = if let Some(target) = &detail.target {
        match context.files().describe(detail.temporary_uri.as_deref().unwrap_or(&detail.temporary_intent)) {
            Ok(_) => return Ok(None),
            Err(crate::api::FileAccessError::NotFound) => {},
            Err(failure) => return Err(file_error(failure)),
        }
        vec![target.clone()]
    } else {
        let Some(uri) = &detail.temporary_uri else { return Ok(None); };
        context.files().backup_publication_candidates(uri, &detail.final_name).map_err(file_error)?
    };
    for uri in candidates {
        let description = match context.files().describe(&uri) {
            Ok(description) => description,
            Err(crate::api::FileAccessError::NotFound) => continue,
            Err(failure) => return Err(file_error(failure)),
        };
        if description.name.ends_with(".partial") || description.size != size { continue; }
        let mut input = context.files().open(&uri).map_err(file_error)?;
        let (metadata, bytes) = match lettuce_transfer::read_backup_header_bytes(&mut input) {
            Ok(header) => header,
            Err(lettuce_transfer::BackupEnvelopeError::Io { .. }) => return Err(file_error(crate::api::FileAccessError::Io)),
            Err(_) => continue,
        };
        if &bytes != expected_header { continue; }
        return Ok(Some(BackupIndexRecord { id: JobId::new(), uri, version: metadata.version, created_at: metadata.created_at, app_version: metadata.app_version, size }));
    }
    Ok(None)
}

fn finish(context: &ApiContext, claim: &ClaimRef, mut record: BackupIndexRecord, warnings: Vec<dto::BackupWarning>) -> Result<(), ApiError> {
    record.id = claim.job_id;
    context.backend().database().record_backup(&record).map_err(|_| unavailable())?;
    let backup = dto::BackupIndexView { id: record.id.to_string(), uri: record.uri, header: dto::BackupHeaderView { version: record.version, created_at: record.created_at.get(), app_version: record.app_version }, size: record.size, availability: dto::BackupAvailability::Available };
    super::jobs::settle(context.backend().database(), claim, context, Ok(dto::JobResultDto::BackupExported { backup, warnings }))?;
    Ok(())
}

#[async_trait]
impl ClaimedJob for Work {
    fn cancellation(&self) -> CancellationToken { self.cancellation.clone() }
    async fn run(mut self: Box<Self>, context: ApiContext, _progress: Arc<dyn JobProgressSink>) -> Result<(), ApiError> {
        let protection = match FileExportProtection::new(&context) {
            Ok(protection) => protection,
            Err(failure) => return super::jobs::settle(context.backend().database(), &self.claim, &context, Err(if failure.code == ApiErrorCode::Busy { "backup-file-busy" } else { super::jobs::file_failure_label(&failure) })).map(|_| ()),
        };
        let protected = |uri: &str, file: Option<&std::fs::File>| protection.protects(uri, file);
        let mut created = false;
        let mut renamed = false;
        let result = async {
            if self.cancellation.is_cancelled() { return Err(error(ApiErrorCode::Cancelled, dto::BackupFailureReason::Cancelled)); }
            checkpoint(&context, &self.claim, &self.detail, "write", false)?;
            let (uri, writer) = context.files().create_backup_temporary(&self.detail.temporary_intent, &protected).map_err(file_error)?;
            created = true;
            self.detail.temporary_uri = Some(uri.clone());
            checkpoint(&context, &self.claim, &self.detail, "write", false)?;
            let files = context.database_files().ok_or_else(unavailable)?;
            let root = context.app_folder().ok_or_else(unavailable)?;
            let media = lettuce_media::LocalSyncMediaStore::open(root.join("platform-v2/media-blobs"), lettuce_database::Database::open(&files.active).map_err(|_| unavailable())?, lettuce_database::Database::open(&files.active).map_err(|_| unavailable())?).map_err(|_| unavailable())?;
            let mut export = context.backend().provider_backup(context.secret_store().as_ref(), &media).export_to_controlled(writer, self.detail.header.app_version.clone(), TimestampMillis::new(self.detail.header.created_at), &self.password, &|| if self.cancellation.is_cancelled() { Err(crate::ProviderBackupError::Cancelled) } else { Ok(()) }).await.map_err(|failure| match failure {
                crate::ProviderBackupError::Cancelled => error(ApiErrorCode::Cancelled, dto::BackupFailureReason::Cancelled),
                crate::ProviderBackupError::Envelope(lettuce_transfer::BackupEnvelopeError::Io { .. }) | crate::ProviderBackupError::Media(_) | crate::ProviderBackupError::Artifact(_) => file_error(crate::api::FileAccessError::Io),
                crate::ProviderBackupError::Envelope(lettuce_transfer::BackupEnvelopeError::LimitExceeded { .. }) | crate::ProviderBackupError::Graph(lettuce_transfer::ProviderBackupGraphError::LimitExceeded { .. }) => error(ApiErrorCode::InvalidInput, dto::BackupFailureReason::LimitExceeded),
                _ => unavailable(),
            })?;
            export.output.flush().map_err(|_| file_error(crate::api::FileAccessError::Io))?;
            drop(export.output);
            context.files().sync_backup(&uri).map_err(file_error)?;
            self.detail.size = Some(context.files().describe(&uri).map_err(file_error)?.size);
            let mut input = context.files().open(&uri).map_err(file_error)?;
            self.detail.publication_header = Some(lettuce_transfer::read_backup_header_bytes(&mut input).map_err(|_| unavailable())?.1);
            self.detail.warnings = export.missing_secrets.into_iter().map(|missing| dto::BackupWarning::MissingSecret { owner: format!("{:?}", missing.owner), purpose: format!("{:?}", missing.purpose) }).collect();
            if self.cancellation.is_cancelled() { return Err(error(ApiErrorCode::Cancelled, dto::BackupFailureReason::Cancelled)); }
            checkpoint(&context, &self.claim, &self.detail, "publish", true)?;
            let uri = context.files().publish_backup(&uri, self.detail.target.as_deref(), &self.detail.final_name, &protected).map_err(file_error)?;
            renamed = true;
            checkpoint(&context, &self.claim, &self.detail, "index", true)?;
            let record = BackupIndexRecord { id: self.claim.job_id, uri, version: self.detail.header.version, created_at: TimestampMillis::new(self.detail.header.created_at), app_version: self.detail.header.app_version.clone(), size: self.detail.size.ok_or_else(unavailable)? };
            finish(&context, &self.claim, record, self.detail.warnings.clone())
        }.await;
        if result.is_ok() { return Ok(()); }
        if renamed || published(&context, &self.detail).map_or(true, |record| record.is_some()) {
            return report_recovery(&context, &self.claim);
        }
        let cancelled = self.cancellation.is_cancelled() || result.as_ref().is_err_and(|failure| failure.code == ApiErrorCode::Cancelled);
        if created && cleanup(&context, &self.detail).is_err() {
            return super::jobs::settle(context.backend().database(), &self.claim, &context, Err("backup-unavailable")).map(|_| ());
        }
        if cancelled {
            let job = context.backend().database().get(self.claim.job_id).map_err(IntoApiError::into_api_error)?.ok_or_else(unavailable)?;
            let at = context.now().max(job.updated_at);
            if job.state != JobState::CancellationRequested {
                context.backend().database().append_and_transition(JobMutation::StageChanged { claim: self.claim.clone(), stage: StageSnapshot::new("cancel", false).map_err(|_| unavailable())?, at }).map_err(IntoApiError::into_api_error)?;
                context.backend().database().append_and_transition(JobMutation::RequestCancellation { id: job.id, reason: CancellationReason::Shutdown, at }).map_err(IntoApiError::into_api_error)?;
            }
            crate::models::artifact_install::finish_claimed_cancellation(context.backend().database(), &self.claim, at).map_err(IntoApiError::into_api_error)?;
            return Ok(());
        }
        if let Err(failure) = result {
            super::jobs::settle(context.backend().database(), &self.claim, &context, Err(super::jobs::file_failure_label(&failure)))?;
        }
        Ok(())
    }
}

pub(super) fn pending_export_targets(context: &ApiContext) -> Result<std::collections::BTreeMap<String, bool>, ApiError> {
    use lettuce_jobs::{JobCatalog, JobListFilter};
    use lettuce_types::{PageLimit, PageRequest};
    let database = context.backend().database();
    let mut targets = std::collections::BTreeMap::new();
    let mut filter = JobListFilter { kinds: vec![JobKind::BackupExport], states: vec![JobState::Queued, JobState::Claimed, JobState::Running, JobState::CancellationRequested, JobState::CleaningUp], subject: None, page: PageRequest { cursor: None, limit: PageLimit::new(200) } };
    loop {
        let page = database.list_jobs(&filter).map_err(IntoApiError::into_api_error)?;
        for job in page.items {
            if job.subject.id.as_str() != "database-export" { continue; }
            let detail: Detail = serde_json::from_value(database.job_detail(job.id).map_err(|_| unavailable())?.ok_or_else(unavailable)?.detail).map_err(|_| unavailable())?;
            if let Some(target) = detail.target {
                let published = detail.publication_header.is_some() && detail.size.is_some();
                targets.entry(target).and_modify(|previous| *previous |= published).or_insert(published);
            }
        }
        let Some(cursor) = page.next_cursor else { return Ok(targets); };
        filter.page.cursor = Some(cursor);
    }
}

pub(in crate::api) fn recover_at_startup(context: &ApiContext) -> Result<(), ApiError> {
    use lettuce_jobs::{JobCatalog, JobListFilter};
    use lettuce_types::{PageLimit, PageRequest};
    let database = context.backend().database();
    let mut filter = JobListFilter { kinds: vec![JobKind::BackupExport], states: vec![JobState::Queued, JobState::Interrupted], subject: None, page: PageRequest { cursor: None, limit: PageLimit::new(200) } };
    loop {
        let page = match database.list_jobs(&filter) {
            Ok(page) => page,
            Err(failure) => { tracing::warn!(%failure, "backup export recovery could not list jobs"); return Ok(()); }
        };
        for job in page.items {
            if job.subject.id.as_str() != "database-export" || context.jobs().has_backup_password(job.id) { continue; }
            if let Err(failure) = recover_job(context, &job) {
                tracing::warn!(job_id = %job.id, ?failure, "backup export recovery will retry on next startup");
            }
        }
        let Some(cursor) = page.next_cursor else { return Ok(()); };
        filter.page.cursor = Some(cursor);
    }
}

fn recover_job(context: &ApiContext, job: &JobSnapshot) -> Result<(), ApiError> {
    let database = context.backend().database();
    let detail: Detail = serde_json::from_value(database.job_detail(job.id).map_err(|_| unavailable())?.ok_or_else(unavailable)?.detail).map_err(|_| unavailable())?;
    if job.state == JobState::Interrupted {
        cleanup(context, &detail)?;
        database.append_and_transition(JobMutation::FinishInterruptedBackupCleanup { id: job.id, at: context.now().max(job.updated_at) }).map_err(IntoApiError::into_api_error)?;
        return Ok(());
    }
    let record = published(context, &detail);
    if matches!(record, Ok(None)) { cleanup(context, &detail)?; }
    let claim = super::jobs::claim_started(database, job.id, context.now().max(job.updated_at))?;
    let record = match record {
        Ok(record) => record,
        Err(failure) => { report_recovery(context, &claim)?; return Err(failure); }
    };
    if let Some(mut record) = record {
        if detail.target.is_none() {
            let protection = FileExportProtection::new(context)?;
            let protected = |uri: &str, file: Option<&std::fs::File>| protection.protects(uri, file);
            match context.files().publish_backup(&record.uri, None, &detail.final_name, &protected) {
                Ok(uri) => record.uri = uri,
                Err(failure) => { report_recovery(context, &claim)?; return Err(file_error(failure)); }
            }
        }
        if let Err(failure) = finish(context, &claim, record, detail.warnings) {
            report_recovery(context, &claim)?;
            return Err(failure);
        }
    } else {
        super::jobs::settle(database, &claim, context, Err("backup-retry-required"))?;
    }
    Ok(())
}

pub(in crate::api) fn result_view(context: &ApiContext, job: &JobSnapshot) -> Result<Option<dto::JobResultDto>, ApiError> {
    if job.kind != JobKind::BackupExport || job.state != JobState::Succeeded { return Ok(None); }
    let record = context.backend().database().job_detail(job.id).map_err(|_| unavailable())?.ok_or_else(unavailable)?;
    record.result.map(|value| serde_json::from_value(value).map_err(|_| unavailable())).transpose()
}

pub(super) fn report_recovery(context: &ApiContext, claim: &ClaimRef) -> Result<(), ApiError> {
    let job = context.backend().database().get(claim.job_id).map_err(IntoApiError::into_api_error)?.ok_or_else(unavailable)?;
    let mut view = crate::api::jobs::job_view(context, &job)?;
    view.failure = Some(dto::JobFailureDto { entity_import: None, code: dto::JobFailureCode::StorageFailure, retryable: false, reason: Some(dto::JobFailureReason::BackupRecoveryRequired), model: None, hugging_face: None, ollama: None, image: None, speech: None });
    context.emit(dto::ApiEvent::JobUpdated { job: Box::new(view.clone()) });
    context.jobs().deliver(job.id, dto::JobEvent::Failed { job: view }, true);
    Ok(())
}
