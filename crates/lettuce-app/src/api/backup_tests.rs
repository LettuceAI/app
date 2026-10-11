use std::sync::Arc;

use lettuce_contracts::{self as dto, ApiErrorCode};
use lettuce_jobs::SystemClock;
use lettuce_types::{JobId, TimestampMillis};

use super::tests::{Reply, harness_in};

#[tokio::test]
async fn backup_inspection_reads_only_public_metadata_and_fails_typed() {
    let root = std::env::temp_dir().join(format!("lettuce-backup-inspect-{}", JobId::new()));
    std::fs::create_dir_all(&root).expect("folder");
    let path = root.join("backup.lettuce");
    let mut writer = lettuce_transfer::BackupWriter::new(Vec::new(), "1.2.3", TimestampMillis::new(123), "password").expect("writer");
    writer.append_bytes("data.json", "data.v1", b"{}").expect("entry");
    let bytes = writer.finish().expect("backup");
    std::fs::write(&path, bytes).expect("file");
    let harness = harness_in(Reply::Text("Hello"), Arc::new(SystemClock), None, Some(root.clone()), Arc::new(super::NoModels));
    let request = dto::BackupInspectRequest { source: dto::FileSource { uri: path.display().to_string() } };
    assert_eq!(super::backup_inspect(&harness.context, request.clone()).await.expect("inspect"), dto::BackupHeaderView { version: 2, created_at: 123, app_version: "1.2.3".into() });
    std::fs::write(&path, b"malformed").expect("malformed file");
    let malformed = super::backup_inspect(&harness.context, request.clone()).await.expect_err("malformed");
    assert_eq!(malformed.code, ApiErrorCode::Malformed);
    assert!(malformed.details.is_some());
    std::fs::remove_file(&path).expect("remove file");
    let missing = super::backup_inspect(&harness.context, request).await.expect_err("missing");
    assert_eq!(missing.code, ApiErrorCode::NotFound);
    assert!(missing.details.is_some());
    std::fs::remove_dir_all(root).expect("remove folder");
}

#[tokio::test]
async fn backup_index_commands_keep_missing_entries_and_replay_mutations() {
    let root = std::env::temp_dir().join(format!("lettuce-backup-list-{}", JobId::new()));
    std::fs::create_dir_all(&root).expect("folder");
    let harness = harness_in(Reply::Text("Hello"), Arc::new(SystemClock), None, Some(root.clone()), Arc::new(super::NoModels));
    let entry = lettuce_database::BackupIndexRecord { id: JobId::new(), uri: root.join("missing.lettuce").display().to_string(), version: 2, created_at: TimestampMillis::new(123), app_version: "1.2.3".into(), size: 100 };
    harness.context.backend().database().record_backup(&entry).expect("entry");
    let listed = super::backups_list(&harness.context).await.expect("list");
    assert_eq!(listed.len(), 1);
    assert!(listed[0].missing);
    let request = dto::BackupMutationRequest { id: entry.id.to_string(), client_operation_id: "delete-missing".into() };
    super::backup_delete(&harness.context, request.clone()).await.expect("missing is deleted");
    super::backup_delete(&harness.context, request.clone()).await.expect("replay");
    let changed = dto::BackupMutationRequest { id: JobId::new().to_string(), ..request };
    assert_eq!(super::backup_delete(&harness.context, changed).await.expect_err("conflict").code, ApiErrorCode::Conflict);
    assert!(super::backups_list(&harness.context).await.expect("empty").is_empty());
    harness.context.backend().database().record_backup(&entry).expect("entry again");
    let request = dto::BackupMutationRequest { id: entry.id.to_string(), client_operation_id: "forget".into() };
    super::backup_forget(&harness.context, request.clone()).await.expect("forget");
    super::backup_forget(&harness.context, request.clone()).await.expect("replay");
    let changed = dto::BackupMutationRequest { id: JobId::new().to_string(), ..request };
    assert_eq!(super::backup_forget(&harness.context, changed).await.expect_err("conflict").code, ApiErrorCode::Conflict);
    std::fs::remove_dir_all(root).expect("folder");
}

#[tokio::test]
async fn failed_backup_delete_keeps_the_file_index_and_has_no_receipt() {
    let root = std::env::temp_dir().join(format!("lettuce-backup-delete-{}", JobId::new()));
    std::fs::create_dir_all(&root).expect("folder");
    let harness = harness_in(Reply::Text("Hello"), Arc::new(SystemClock), None, Some(root.clone()), Arc::new(super::NoModels));
    let entry = lettuce_database::BackupIndexRecord { id: JobId::new(), uri: root.display().to_string(), version: 2, created_at: TimestampMillis::new(123), app_version: "1.2.3".into(), size: 100 };
    harness.context.backend().database().record_backup(&entry).expect("entry");
    let failure = super::backup_delete(&harness.context, dto::BackupMutationRequest { id: entry.id.to_string(), client_operation_id: "failing-delete".into() }).await.expect_err("cannot delete directory as file");
    assert_eq!(failure.code, ApiErrorCode::Unavailable);
    assert_eq!(harness.context.backend().database().backup_index().expect("retained"), vec![entry]);
    assert!(harness.context.backend().database().lookup_api_operation("backup_delete", "failing-delete").expect("lookup").is_none());
    assert!(root.is_dir());
    std::fs::remove_dir_all(root).expect("folder");
}

struct Progress;

impl super::jobs::JobProgressSink for Progress {
    fn text_delta(&self, _text: Option<String>, _reasoning: Option<String>) {}
    fn image_progress(&self, _progress: dto::ImageProgress) {}
}

#[tokio::test]
async fn restore_admission_replays_conflicts_and_wrong_password_writes_no_target_or_event() {
    use super::jobs::JobHandler;
    use lettuce_jobs::JobStore;
    let (root, location, harness) = super::app_reset_tests::harness();
    let path = root.join("source.lettuce");
    let media = lettuce_media::LocalSyncMediaStore::open(root.join("platform-v2/media-blobs"), lettuce_database::Database::open(location.active_path().expect("path")).expect("blobs"), lettuce_database::Database::open(location.active_path().expect("path")).expect("assets")).expect("media");
    let export = harness.context.backend().provider_backup(harness.context.secret_store().as_ref(), &media).export("1.0", TimestampMillis::new(100), "password").await.expect("export");
    std::fs::write(&path, export.output).expect("backup");
    let request = dto::BackupRestoreRequest { source: dto::FileSource { uri: path.display().to_string() }, password: "wrong".into(), client_operation_id: "restore-wrong-password".into() };
    let accepted = super::backup_restore(&harness.context, request.clone()).await.expect("admit");
    assert_eq!(super::backup_restore(&harness.context, request.clone()).await.expect("replay"), accepted);
    let changed = dto::BackupRestoreRequest { password: "different".into(), ..request };
    assert_eq!(super::backup_restore(&harness.context, changed).await.expect_err("conflict").code, ApiErrorCode::Conflict);
    let old = location.active_path().expect("old");
    let before = std::fs::read_dir(old.parent().expect("folder")).expect("files").count();
    let job = harness.context.backend().database().get(accepted.job_id.parse().expect("id")).expect("job").expect("exists");
    let work = super::backup_jobs::RestoreHandler.claim(&harness.context, &job, lettuce_jobs::WorkerId::new()).await.expect("claim").expect("work");
    work.run(harness.context.clone(), Arc::new(Progress)).await.expect("settled failure");
    assert_eq!(location.active_path().expect("active"), old);
    assert_eq!(std::fs::read_dir(old.parent().expect("folder")).expect("files").count(), before);
    let failed = super::job_get(&harness.context, dto::JobGetRequest { job_id: accepted.job_id }).await.expect("view");
    assert_eq!(failed.state, dto::JobStateDto::Failed);
    assert_eq!(failed.failure.expect("typed failure").reason, Some(dto::JobFailureReason::BackupAuthentication));
    assert!(!harness.events.events().iter().any(|event| matches!(event, dto::ApiEvent::DatabaseReloaded)));
}

#[tokio::test]
async fn reopened_restore_without_password_fails_retry_required_and_removes_staging() {
    let (root, _location, harness) = super::app_reset_tests::harness();
    let accepted = super::backup_restore(&harness.context, dto::BackupRestoreRequest { source: dto::FileSource { uri: "/backup.lettuce".into() }, password: "password".into(), client_operation_id: "reopen".into() }).await.expect("admit");
    let id = accepted.job_id.parse().expect("id");
    harness.context.jobs().take_backup_password(id);
    let temporary = root.join("platform-v2/backup-restore-workspace").join(&accepted.job_id);
    std::fs::create_dir_all(&temporary).expect("staging");
    std::fs::write(temporary.join("partial"), b"partial").expect("partial");
    super::backup_jobs::recover_at_startup(&harness.context).expect("recovery");
    assert!(!temporary.exists());
    let view = super::job_get(&harness.context, dto::JobGetRequest { job_id: accepted.job_id }).await.expect("view");
    assert_eq!(view.state, dto::JobStateDto::Failed);
    assert_eq!(view.failure.expect("typed").reason, Some(dto::JobFailureReason::BackupRetryRequired));
    assert!(!harness.events.events().iter().any(|event| matches!(event, dto::ApiEvent::DatabaseReloaded)));
}

struct ReloadHost {
    root: std::path::PathBuf,
    location: crate::AppDatabaseLocation,
    provider: Arc<super::tests::FakeProvider>,
    events: Arc<super::tests::RecordingEvents>,
    slot: Arc<super::ApiContextSlot>,
    workers: std::sync::Mutex<Option<super::ApiWorkers>>,
    fail: bool,
    fail_settlement: bool,
}

#[async_trait::async_trait]
impl super::AppResetHost for ReloadHost {
    async fn preflight(&self) -> Result<(), dto::ApiError> { Ok(()) }
    async fn stop_workers(&self) -> Result<(), dto::ApiError> { unreachable!() }
    async fn clear_webview_storage(&self) -> Result<(), dto::ApiError> { unreachable!() }
    async fn prepare_restart(&self) -> Result<(), dto::ApiError> { unreachable!() }
    fn exit_for_restart(&self) { unreachable!() }
    async fn reload_database(&self, job: &str) -> Result<Arc<super::ApiContext>, dto::ApiError> {
        if self.fail {
            self.slot.require_recovery();
            return Err(super::backup::error(ApiErrorCode::Unavailable, dto::BackupFailureReason::RecoveryRequired));
        }
        let active = self.location.active_path().expect("active");
        let backend = Arc::new(crate::AppBackend::open(&active, TimestampMillis::new(100)).expect("backend"));
        let fresh = Arc::new(super::ApiContext::new(super::ApiContextParts {
            backend,
            secret_store: Arc::new(lettuce_settings::InMemorySecretStore::new()),
            inference: self.provider.clone(),
            image_provider: Arc::new(super::tests::NoImages),
            models: Arc::new(super::NoModels),
            speech: Arc::new(super::NoSpeech),
            media: None,
            events: self.events.clone(),
            clock: Arc::new(SystemClock),
            files: Arc::new(super::tests::StdFiles),
            app_folder: Some(self.root.clone()),
            resource_dir: None,
            database_files: Some(super::ApiDatabaseFiles { location: self.location.clone(), active }),
            asset_url_base: "test-asset://host".into(),
        }));
        fresh.bind_reloaded_job(job)?;
        let old = self.slot.swap(fresh.clone())?;
        old.retire_database_listeners();
        old.begin_shutdown();
        old.backend().shutdown().await;
        let workers = super::startup(&fresh).await?;
        workers.started_result().await?;
        *self.workers.lock().expect("workers") = Some(workers);
        if self.fail_settlement {
            lettuce_database::Database::lock_file_writes(&fresh.database_files().expect("files").active).expect("write failure injection").set_fenced(true).expect("writes fail");
        }
        Ok(fresh)
    }
}

async fn completed_restore(fail: bool) -> (std::path::PathBuf, crate::AppDatabaseLocation, super::tests::Harness, Arc<ReloadHost>, String) {
    completed_restore_with_faults(fail, false).await
}

async fn completed_restore_with_faults(fail: bool, fail_settlement: bool) -> (std::path::PathBuf, crate::AppDatabaseLocation, super::tests::Harness, Arc<ReloadHost>, String) {
    use super::jobs::JobHandler;
    use lettuce_jobs::JobStore;
    let (root, location, harness) = super::app_reset_tests::harness();
    let host = Arc::new(ReloadHost { root: root.clone(), location: location.clone(), provider: harness.provider.clone(), events: harness.events.clone(), slot: Arc::new(super::ApiContextSlot::new(Arc::new(harness.context.clone()))), workers: std::sync::Mutex::new(None), fail, fail_settlement });
    harness.context.attach_reset_host(host.clone()).expect("host");
    let path = root.join("source.lettuce");
    let media = lettuce_media::LocalSyncMediaStore::open(root.join("platform-v2/media-blobs"), lettuce_database::Database::open(location.active_path().expect("path")).expect("blobs"), lettuce_database::Database::open(location.active_path().expect("path")).expect("assets")).expect("media");
    let export = harness.context.backend().provider_backup(harness.context.secret_store().as_ref(), &media).export("1.0", TimestampMillis::new(100), "password").await.expect("export");
    std::fs::write(&path, export.output).expect("backup");
    harness.context.backend().database().create_or_get(lettuce_jobs::JobSpec::new(lettuce_jobs::JobKind::BackupExport, lettuce_jobs::JobSubject::new(lettuce_jobs::SubjectKind::Backup, "old-database-only").expect("subject"), lettuce_jobs::OutcomeRef::Request(lettuce_types::RequestId::new())).with_resources(vec![lettuce_jobs::ResourceClass::DiskWrite])).expect("old queued job");
    let accepted = super::backup_restore(&harness.context, dto::BackupRestoreRequest { source: dto::FileSource { uri: path.display().to_string() }, password: "password".into(), client_operation_id: "restore".into() }).await.expect("admit");
    let job = harness.context.backend().database().get(accepted.job_id.parse().expect("id")).expect("job").expect("exists");
    let work = super::backup_jobs::RestoreHandler.claim(&harness.context, &job, lettuce_jobs::WorkerId::new()).await.expect("claim").expect("work");
    work.run(harness.context.clone(), Arc::new(Progress)).await.expect("settled");
    (root, location, harness, host, accepted.job_id)
}

#[tokio::test]
async fn successful_restore_rebinds_settings_and_emits_database_reloaded_once() {
    let (_root, location, harness, host, id) = completed_restore(false).await;
    let fresh = host.slot.snapshot().expect("fresh");
    assert_ne!(location.active_path().expect("new"), harness.context.database_files().expect("old").active);
    assert_eq!(super::job_get(&fresh, dto::JobGetRequest { job_id: id }).await.expect("job").state, dto::JobStateDto::Succeeded);
    assert_eq!(harness.events.events().iter().filter(|event| matches!(event, dto::ApiEvent::DatabaseReloaded)).count(), 1);
    let filter = lettuce_jobs::JobListFilter { kinds: vec![lettuce_jobs::JobKind::BackupExport], states: Vec::new(), subject: None, page: lettuce_types::PageRequest { cursor: None, limit: lettuce_types::PageLimit::new(200) } };
    let old_jobs = lettuce_jobs::JobCatalog::list_jobs(harness.context.backend().database(), &filter).expect("old jobs");
    let queued = old_jobs.items.iter().find(|job| job.subject.id.as_str() == "old-database-only").expect("old queued work kept");
    assert_eq!(queued.state, lettuce_jobs::JobState::Queued);
    assert!(lettuce_jobs::JobStore::get(fresh.backend().database(), queued.id).expect("new database job lookup").is_none());
    let before = super::settings_get(&fresh).await.expect("settings");
    super::settings_update(&fresh, dto::SettingsUpdateRequest { expected_revision: before.revision, patch: dto::SettingsPatch::General { lorebook_scan_depth: Some(11), pure_mode: None, analytics_enabled: None, update_checks_enabled: None, developer_mode_enabled: None, auto_download_character_card_avatars: None, manual_mode_context_window: None } }).await.expect("settings update");
    tokio::time::timeout(std::time::Duration::from_secs(5), harness.events.until(|events| events.iter().any(|event| matches!(event, dto::ApiEvent::SettingsChanged { section } if section == "general")))).await.expect("settings event");
    let workers = host.workers.lock().expect("workers").take().expect("started");
    workers.stop().await;
}

#[tokio::test]
async fn failed_rebind_keeps_restored_database_blocks_commands_and_emits_no_reload() {
    let (_root, location, harness, host, id) = completed_restore(true).await;
    assert_ne!(location.active_path().expect("new"), harness.context.database_files().expect("old").active);
    assert!(matches!(host.slot.snapshot().expect_err("blocked").details, Some(dto::ApiErrorDetails::Backup { reason: dto::BackupFailureReason::RecoveryRequired })));
    let view = super::job_get(&harness.context, dto::JobGetRequest { job_id: id }).await.expect("job");
    assert_eq!(view.state, dto::JobStateDto::Failed);
    assert_eq!(view.failure.expect("failure").reason, Some(dto::JobFailureReason::BackupRecoveryRequired));
    assert!(!harness.events.events().iter().any(|event| matches!(event, dto::ApiEvent::DatabaseReloaded)));
    let reopened = crate::AppBackend::open(location.active_path().expect("active"), TimestampMillis::new(100)).expect("reopen restored database");
    reopened.recover_after_restart(TimestampMillis::new(100)).expect("normal recovery");
}

#[tokio::test]
async fn backup_export_replays_conflicts_and_atomically_replaces_one_indexed_path() {
    use super::jobs::JobHandler;
    use lettuce_jobs::JobStore;
    let (root, _location, harness) = super::app_reset_tests::harness();
    let target = root.join("export.lettuce");
    std::fs::write(&target, b"existing user file").expect("existing");
    let request = dto::BackupExportRequest { password: "password".into(), target: Some(dto::FileTarget { uri: target.display().to_string() }), client_operation_id: "export".into() };
    let accepted = super::backup_export(&harness.context, request.clone()).await.expect("admit");
    assert_eq!(super::backup_export(&harness.context, request.clone()).await.expect("replay"), accepted);
    assert_eq!(super::backup_export(&harness.context, dto::BackupExportRequest { password: "changed".into(), ..request }).await.expect_err("conflict").code, ApiErrorCode::Conflict);
    assert_eq!(std::fs::read(&target).expect("old target"), b"existing user file");
    let job = harness.context.backend().database().get(accepted.job_id.parse().expect("id")).expect("job").expect("exists");
    let work = super::backup_export_jobs::ExportHandler.claim(&harness.context, &job, lettuce_jobs::WorkerId::new()).await.expect("claim").expect("work");
    work.run(harness.context.clone(), Arc::new(Progress)).await.expect("run");
    let view = super::job_get(&harness.context, dto::JobGetRequest { job_id: accepted.job_id }).await.expect("job");
    assert_eq!(view.state, dto::JobStateDto::Succeeded);
    let entries = super::backups_list(&harness.context).await.expect("index");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].uri, target.display().to_string());
    assert!(!entries[0].missing);
    assert_eq!(super::backup_inspect(&harness.context, dto::BackupInspectRequest { source: dto::FileSource { uri: target.display().to_string() } }).await.expect("inspect"), entries[0].header);
}

#[tokio::test]
async fn backup_export_failed_rename_removes_temporary_and_keeps_index_empty() {
    use super::jobs::JobHandler;
    use lettuce_jobs::JobStore;
    let (root, _location, harness) = super::app_reset_tests::harness();
    let target = root.join("directory.lettuce");
    std::fs::create_dir(&target).expect("existing directory");
    let accepted = super::backup_export(&harness.context, dto::BackupExportRequest { password: "password".into(), target: Some(dto::FileTarget { uri: target.display().to_string() }), client_operation_id: "export-failing-rename".into() }).await.expect("admit");
    let job = harness.context.backend().database().get(accepted.job_id.parse().expect("id")).expect("job").expect("exists");
    let work = super::backup_export_jobs::ExportHandler.claim(&harness.context, &job, lettuce_jobs::WorkerId::new()).await.expect("claim").expect("work");
    work.run(harness.context.clone(), Arc::new(Progress)).await.expect("settled");
    assert_eq!(super::job_get(&harness.context, dto::JobGetRequest { job_id: accepted.job_id }).await.expect("job").state, dto::JobStateDto::Failed);
    assert!(super::backups_list(&harness.context).await.expect("index").is_empty());
    assert!(target.is_dir());
    assert!(!std::fs::read_dir(&root).expect("folder").any(|entry| entry.expect("entry").file_name().to_string_lossy().ends_with(".partial")));
}

struct FaultFiles { write_fails: bool, close_after_publish: Option<Arc<crate::AppBackend>> }
struct FullDiskWriter(Box<dyn std::io::Write + Send>);

impl std::io::Write for FullDiskWriter {
    fn write(&mut self, _buffer: &[u8]) -> std::io::Result<usize> { Err(std::io::ErrorKind::StorageFull.into()) }
    fn flush(&mut self) -> std::io::Result<()> { self.0.flush() }
}

impl super::FileAccess for FaultFiles {
    fn describe(&self, uri: &str) -> Result<super::FileDescription, super::FileAccessError> { super::tests::StdFiles.describe(uri) }
    fn open(&self, uri: &str) -> Result<Box<dyn super::FileReader>, super::FileAccessError> { super::tests::StdFiles.open(uri) }
    fn create(&self, uri: &str) -> Result<Box<dyn std::io::Write + Send>, super::FileAccessError> { super::tests::StdFiles.create(uri) }
    fn remove(&self, uri: &str) -> Result<(), super::FileAccessError> { super::tests::StdFiles.remove(uri) }
    fn backup_temporary_location(&self, target: Option<&str>, name: &str) -> Result<String, super::FileAccessError> { super::tests::StdFiles.backup_temporary_location(target, name) }
    fn create_backup_temporary(&self, intent: &str, protected: super::ExportProtection<'_>) -> Result<(String, Box<dyn std::io::Write + Send>), super::FileAccessError> {
        let (uri, writer) = super::tests::StdFiles.create_backup_temporary(intent, protected)?;
        Ok((uri, if self.write_fails { Box::new(FullDiskWriter(writer)) } else { writer }))
    }
    fn sync_backup(&self, uri: &str) -> Result<(), super::FileAccessError> { super::tests::StdFiles.sync_backup(uri) }
    fn publish_backup(&self, temporary: &str, target: Option<&str>, name: &str, protected: super::ExportProtection<'_>) -> Result<String, super::FileAccessError> {
        let uri = super::tests::StdFiles.publish_backup(temporary, target, name, protected)?;
        if let Some(backend) = &self.close_after_publish { backend.database().close_for_reset().map_err(|_| super::FileAccessError::Io)?; }
        Ok(uri)
    }
}

#[tokio::test]
async fn full_disk_export_leaves_no_backup_no_temporary_and_no_index_entry() {
    use super::jobs::JobHandler;
    use lettuce_jobs::JobStore;
    let (root, location, harness) = super::app_reset_tests::harness();
    let active = location.active_path().expect("active");
    let context = super::ApiContext::new(super::ApiContextParts {
        backend: Arc::new(crate::AppBackend::open(&active, TimestampMillis::new(100)).expect("backend")),
        secret_store: harness.context.secret_store().clone(),
        inference: harness.provider.clone(), image_provider: Arc::new(super::tests::NoImages), models: Arc::new(super::NoModels), speech: Arc::new(super::NoSpeech), media: None, events: harness.events.clone(), clock: Arc::new(SystemClock), files: Arc::new(FaultFiles { write_fails: true, close_after_publish: None }), app_folder: Some(root.clone()), resource_dir: None, database_files: Some(super::ApiDatabaseFiles { location, active }), asset_url_base: "test-asset://host".into(),
    });
    let target = root.join("full-disk.lettuce");
    let accepted = super::backup_export(&context, dto::BackupExportRequest { password: "password".into(), target: Some(dto::FileTarget { uri: target.display().to_string() }), client_operation_id: "full-disk".into() }).await.expect("admit");
    let job = context.backend().database().get(accepted.job_id.parse().expect("id")).expect("job").expect("exists");
    let work = super::backup_export_jobs::ExportHandler.claim(&context, &job, lettuce_jobs::WorkerId::new()).await.expect("claim").expect("work");
    work.run(context.clone(), Arc::new(Progress)).await.expect("settled");
    assert_eq!(super::job_get(&context, dto::JobGetRequest { job_id: accepted.job_id }).await.expect("job").state, dto::JobStateDto::Failed);
    assert!(!target.exists());
    assert!(super::backups_list(&context).await.expect("index").is_empty());
    assert!(!std::fs::read_dir(root).expect("folder").any(|entry| entry.expect("entry").file_name().to_string_lossy().ends_with(".partial")));
}

#[tokio::test]
async fn export_reopen_recovers_only_a_published_file_matching_the_recorded_header() {
    use lettuce_jobs::{JobStore, JobMutation, StageSnapshot, ResourceAvailability};
    for (renamed, different_salt) in [(false, false), (true, false), (true, true)] {
        let (root, _location, harness) = super::app_reset_tests::harness();
        let target = root.join("publication.lettuce");
        let accepted = super::backup_export(&harness.context, dto::BackupExportRequest { password: "never-store-this-password".into(), target: Some(dto::FileTarget { uri: target.display().to_string() }), client_operation_id: "publication".into() }).await.expect("admit");
        let id = accepted.job_id.parse().expect("id");
        harness.context.jobs().take_backup_password(id);
        let database = harness.context.backend().database();
        let job = database.get(id).expect("job").expect("exists");
        let claim = database.claim(id, lettuce_jobs::WorkerId::new(), job.updated_at, std::time::Duration::from_secs(60), &ResourceAvailability::all()).expect("claim").expect("owned").claim;
        database.append_and_transition(JobMutation::Start { claim: claim.clone(), at: job.updated_at }).expect("start");
        let mut detail = database.job_detail(id).expect("detail").expect("stored").detail;
        assert!(!detail.to_string().contains("never-store-this-password"));
        let header = detail["header"].clone();
        let mut writer = lettuce_transfer::BackupWriter::new(Vec::new(), header["app_version"].as_str().expect("version"), TimestampMillis::new(header["created_at"].as_i64().expect("time")), "never-store-this-password").expect("writer");
        writer.append_bytes("data.json", "data.v1", b"{}").expect("section");
        let bytes = writer.finish().expect("complete backup");
        let temporary = std::path::PathBuf::from(detail["temporary_intent"].as_str().expect("intent"));
        std::fs::write(&temporary, &bytes).expect("temporary");
        std::fs::File::open(&temporary).expect("file").sync_all().expect("durable");
        detail["temporary_uri"] = serde_json::json!(temporary.display().to_string());
        detail["size"] = serde_json::json!(bytes.len());
        let mut header_reader = std::io::Cursor::new(&bytes);
        lettuce_transfer::read_backup_header(&mut header_reader).expect("public header");
        detail["publication_header"] = serde_json::json!(&bytes[..header_reader.position() as usize]);
        database.checkpoint_backup_job(JobMutation::StageChanged { claim, stage: StageSnapshot::new("publish", true).expect("stage"), at: job.updated_at }, &detail).expect("publication intent");
        if renamed {
            std::fs::rename(&temporary, &target).expect("published before crash");
            if different_salt {
                let mut replaced = bytes.clone();
                replaced[header_reader.position() as usize - 1] ^= 1;
                std::fs::write(&target, replaced).expect("different export salt");
            }
        }
        else { std::fs::write(&target, b"original user file").expect("old target"); }
        harness.context.recover_after_restart().expect("reopen recovery");
        let view = super::job_get(&harness.context, dto::JobGetRequest { job_id: accepted.job_id.clone() }).await.expect("job");
        assert!(!temporary.exists());
        if different_salt {
            assert_eq!(view.state, dto::JobStateDto::Failed);
            assert_eq!(view.failure.expect("typed").reason, Some(dto::JobFailureReason::BackupRetryRequired));
            assert!(target.exists());
            assert!(super::backups_list(&harness.context).await.expect("index").is_empty());
        } else if renamed {
            assert_eq!(view.state, dto::JobStateDto::Succeeded);
            assert_eq!(std::fs::read(&target).expect("preserved complete backup"), bytes);
            let entries = super::backups_list(&harness.context).await.expect("index");
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].id, accepted.job_id);
        } else {
            assert_eq!(view.state, dto::JobStateDto::Failed);
            assert_eq!(view.failure.expect("typed").reason, Some(dto::JobFailureReason::BackupRetryRequired));
            assert_eq!(std::fs::read(&target).expect("untouched"), b"original user file");
            assert!(super::backups_list(&harness.context).await.expect("index").is_empty());
        }
    }
}

fn file_test_context(root: &std::path::Path, location: crate::AppDatabaseLocation, harness: &super::tests::Harness, backend: Arc<crate::AppBackend>, files: Arc<dyn super::FileAccess>) -> super::ApiContext {
    super::ApiContext::new(super::ApiContextParts {
        backend, secret_store: harness.context.secret_store().clone(), inference: harness.provider.clone(), image_provider: Arc::new(super::tests::NoImages), models: Arc::new(super::NoModels), speech: Arc::new(super::NoSpeech), media: None, events: harness.events.clone(), clock: Arc::new(SystemClock), files, app_folder: Some(root.to_owned()), resource_dir: None, database_files: Some(super::ApiDatabaseFiles { active: location.active_path().expect("active"), location }), asset_url_base: "test-asset://host".into(),
    })
}

#[tokio::test]
async fn published_export_index_failure_reports_typed_recovery_without_writing_the_job() {
    use super::jobs::JobHandler;
    use lettuce_jobs::JobStore;
    let (root, location, harness) = super::app_reset_tests::harness();
    let active = location.active_path().expect("active");
    let backend = Arc::new(crate::AppBackend::open(&active, TimestampMillis::new(100)).expect("backend"));
    let context = file_test_context(&root, location.clone(), &harness, backend.clone(), Arc::new(FaultFiles { write_fails: false, close_after_publish: Some(backend) }));
    let target = root.join("index-failure.lettuce");
    let accepted = super::backup_export(&context, dto::BackupExportRequest { password: "password".into(), target: Some(dto::FileTarget { uri: target.display().to_string() }), client_operation_id: "index-failure".into() }).await.expect("admit");
    let job = context.backend().database().get(accepted.job_id.parse().expect("id")).expect("job").expect("exists");
    let work = super::backup_export_jobs::ExportHandler.claim(&context, &job, lettuce_jobs::WorkerId::new()).await.expect("claim").expect("work");
    work.run(context.clone(), Arc::new(Progress)).await.expect("typed recovery event");
    assert!(target.is_file());
    let durable = lettuce_database::Database::open(&active).expect("durable database");
    assert_eq!(durable.get(job.id).expect("durable job").expect("exists").state, lettuce_jobs::JobState::Running);
    assert!(harness.events.events().iter().any(|event| matches!(event, dto::ApiEvent::JobUpdated { job } if job.failure.as_ref().is_some_and(|failure| failure.reason == Some(dto::JobFailureReason::BackupRecoveryRequired)))));
    assert!(durable.backup_index().expect("index").is_empty());
    let reopened = file_test_context(&root, location, &harness, Arc::new(crate::AppBackend::open(&active, TimestampMillis::new(100)).expect("reopened backend")), Arc::new(super::tests::StdFiles));
    reopened.recover_after_restart().expect("reopen");
    assert_eq!(super::job_get(&reopened, dto::JobGetRequest { job_id: accepted.job_id }).await.expect("job").state, dto::JobStateDto::Succeeded);
    assert_eq!(super::backups_list(&reopened).await.expect("index").len(), 1);
}

#[tokio::test]
async fn restore_waits_for_a_streaming_generation_and_keeps_its_completed_reply_in_the_old_database() {
    use super::jobs::JobHandler;
    use lettuce_jobs::JobStore;
    let (root, location, harness) = super::app_reset_tests::harness();
    let host = Arc::new(ReloadHost { root: root.clone(), location: location.clone(), provider: harness.provider.clone(), events: harness.events.clone(), slot: Arc::new(super::ApiContextSlot::new(Arc::new(harness.context.clone()))), workers: std::sync::Mutex::new(None), fail: false, fail_settlement: false });
    harness.context.attach_reset_host(host.clone()).expect("host");
    let source = root.join("before-generation.lettuce");
    let old_path = location.active_path().expect("old");
    let media = lettuce_media::LocalSyncMediaStore::open(root.join("platform-v2/media-blobs"), lettuce_database::Database::open(&old_path).expect("blobs"), lettuce_database::Database::open(&old_path).expect("assets")).expect("media");
    let export = harness.context.backend().provider_backup(harness.context.secret_store().as_ref(), &media).export("1.0", TimestampMillis::new(100), "password").await.expect("backup");
    std::fs::write(&source, export.output).expect("source");
    let release = Arc::new(tokio::sync::Notify::new());
    *harness.provider.response_release.lock().expect("release") = Some(release.clone());
    let conversation = super::tests::launch(&harness, "stream-before-restore").await;
    let stream = Arc::new(super::tests::RecordingStream::default());
    super::tests::send(&harness, &conversation, "reply-before-restore", "Reply", stream.clone()).await.expect("send");
    let worker = super::worker::ConversationGenerationWorker::new(harness.context.clone());
    let generation = tokio::spawn(async move { worker.run_once().await });
    harness.provider.entered.notified().await;
    let accepted = super::backup_restore(&harness.context, dto::BackupRestoreRequest { source: dto::FileSource { uri: source.display().to_string() }, password: "password".into(), client_operation_id: "streaming-restore".into() }).await.expect("admit");
    let job = harness.context.backend().database().get(accepted.job_id.parse().expect("id")).expect("job").expect("exists");
    let work = super::backup_jobs::RestoreHandler.claim(&harness.context, &job, lettuce_jobs::WorkerId::new()).await.expect("claim").expect("work");
    let context = harness.context.clone();
    let mut restore = tokio::spawn(async move { work.run(context, Arc::new(Progress)).await });
    assert!(tokio::time::timeout(std::time::Duration::from_secs(3), &mut restore).await.is_err());
    assert_eq!(location.active_path().expect("still old"), old_path);
    release.notify_one();
    assert!(generation.await.expect("generation task").expect("generation finished"));
    restore.await.expect("restore task").expect("restore finished");
    assert!(stream.events().iter().any(|event| matches!(event, dto::GenerationEvent::Completed { .. })));
    let old = super::conversation_open(&harness.context, dto::ConversationOpenRequest { conversation_id: conversation }).await.expect("kept conversation");
    assert!(old.messages.items.iter().any(|message| message.parts.iter().any(|part| matches!(part, dto::MessagePartView::Text { text } if text == "ok"))));
    let workers = host.workers.lock().expect("workers").take().expect("started");
    workers.stop().await;
}

#[tokio::test]
async fn reloaded_restore_job_cannot_be_claimed_by_the_new_runner_during_replay() {
    use super::jobs::JobHandler;
    use lettuce_jobs::JobStore;
    let (_root, _location, harness) = super::app_reset_tests::harness();
    let request = dto::BackupRestoreRequest { source: dto::FileSource { uri: "/source.lettuce".into() }, password: "password".into(), client_operation_id: "reloaded-replay".into() };
    let accepted = super::backup_restore(&harness.context, request.clone()).await.expect("admit");
    harness.context.bind_reloaded_job(&accepted.job_id).expect("rebind owns job");
    assert_eq!(super::backup_restore(&harness.context, request).await.expect("replay"), accepted);
    let job = harness.context.backend().database().get(accepted.job_id.parse().expect("id")).expect("job").expect("exists");
    assert!(super::backup_jobs::RestoreHandler.lane(&harness.context, &job).is_none());
    assert!(super::backup_jobs::RestoreHandler.claim(&harness.context, &job, lettuce_jobs::WorkerId::new()).await.expect("claim attempt").is_none());
    assert_eq!(harness.context.backend().database().get(job.id).expect("job").expect("exists").state, lettuce_jobs::JobState::Queued);
}

#[tokio::test]
async fn changed_backup_requests_conflict_before_revalidating_their_fields() {
    let (root, _location, harness) = super::app_reset_tests::harness();
    let request = dto::BackupExportRequest { password: "password".into(), target: Some(dto::FileTarget { uri: root.join("backup.lettuce").display().to_string() }), client_operation_id: "export-field-conflict".into() };
    super::backup_export(&harness.context, request.clone()).await.expect("admit");
    assert_eq!(super::backup_export(&harness.context, dto::BackupExportRequest { password: String::new(), ..request }).await.expect_err("conflict").code, ApiErrorCode::Conflict);
    for delete in [false, true] {
        let id = JobId::new();
        harness.context.backend().database().record_backup(&lettuce_database::BackupIndexRecord { id, uri: root.join("missing.lettuce").display().to_string(), version: 2, created_at: TimestampMillis::new(100), app_version: "1.0".into(), size: 1 }).expect("entry");
        let request = dto::BackupMutationRequest { id: id.to_string(), client_operation_id: format!("index-field-conflict-{delete}") };
        if delete { super::backup_delete(&harness.context, request.clone()).await.expect("delete"); }
        else { super::backup_forget(&harness.context, request.clone()).await.expect("forget"); }
        let changed = dto::BackupMutationRequest { id: "malformed".into(), ..request };
        let failure = if delete { super::backup_delete(&harness.context, changed).await.expect_err("conflict") } else { super::backup_forget(&harness.context, changed).await.expect_err("conflict") };
        assert_eq!(failure.code, ApiErrorCode::Conflict);
    }
    for legacy in [false, true] {
        let request = dto::BackupRestoreRequest { source: dto::FileSource { uri: "/backup.lettuce".into() }, password: "password".into(), client_operation_id: format!("restore-field-conflict-{legacy}") };
        if legacy { super::legacy_backup_restore(&harness.context, request.clone()).await.expect("admit"); }
        else { super::backup_restore(&harness.context, request.clone()).await.expect("admit"); }
        let changed = dto::BackupRestoreRequest { password: String::new(), ..request };
        let failure = if legacy { super::legacy_backup_restore(&harness.context, changed).await.expect_err("conflict") } else { super::backup_restore(&harness.context, changed).await.expect_err("conflict") };
        assert_eq!(failure.code, ApiErrorCode::Conflict);
    }
}

struct PendingPublicationFiles {
    path: std::path::PathBuf,
    publications: std::sync::atomic::AtomicUsize,
    stale: bool,
}

impl super::FileAccess for PendingPublicationFiles {
    fn describe(&self, uri: &str) -> Result<super::FileDescription, super::FileAccessError> {
        if self.stale && uri == "content://stale-backup" { return Err(super::FileAccessError::NotFound); }
        super::FileAccess::describe(&super::tests::StdFiles, self.path.to_str().expect("path"))
    }
    fn open(&self, uri: &str) -> Result<Box<dyn super::FileReader>, super::FileAccessError> {
        if self.stale && uri == "content://stale-backup" { return Err(super::FileAccessError::NotFound); }
        super::FileAccess::open(&super::tests::StdFiles, self.path.to_str().expect("path"))
    }
    fn create(&self, _uri: &str) -> Result<Box<dyn std::io::Write + Send>, super::FileAccessError> { Err(super::FileAccessError::Unsupported) }
    fn backup_temporary_location(&self, _target: Option<&str>, name: &str) -> Result<String, super::FileAccessError> {
        Ok(self.path.parent().expect("parent").join(name).display().to_string())
    }
    fn remove(&self, _uri: &str) -> Result<(), super::FileAccessError> {
        super::FileAccess::remove(&super::tests::StdFiles, self.path.to_str().expect("path"))
    }
    fn backup_publication_candidates(&self, _temporary: &str, _name: &str) -> Result<Vec<String>, super::FileAccessError> { Ok(vec!["content://complete-backup".into()]) }
    fn publish_backup(&self, temporary: &str, _target: Option<&str>, _name: &str, _protected: super::files::ExportProtection<'_>) -> Result<String, super::FileAccessError> {
        self.publications.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(temporary.to_owned())
    }
}

#[tokio::test]
async fn reopened_android_export_finishes_publication_before_indexing_the_complete_file() {
    recover_android_export(false, false).await;
}

#[tokio::test]
async fn reopened_android_export_recovers_a_rename_before_the_media_store_row_updated() {
    recover_android_export(true, false).await;
}

#[tokio::test]
async fn android_recovery_keeps_a_complete_file_with_a_mismatching_publication_header() {
    recover_android_export(false, true).await;
}

async fn recover_android_export(stale: bool, mismatched: bool) {
    use lettuce_jobs::{JobMutation, StageSnapshot, JobStore, ResourceAvailability};
    let (root, location, harness) = super::app_reset_tests::harness();
    let files = Arc::new(PendingPublicationFiles { path: root.join("lettuce_backup_20261011_120000.lettuce"), publications: std::sync::atomic::AtomicUsize::new(0), stale });
    let backend = Arc::new(crate::AppBackend::open(location.active_path().expect("active"), TimestampMillis::new(100)).expect("backend"));
    let context = file_test_context(&root, location, &harness, backend, files.clone());
    let accepted = super::backup_export(&context, dto::BackupExportRequest { password: "password".into(), target: None, client_operation_id: "android-pending".into() }).await.expect("admit");
    let id = accepted.job_id.parse().expect("id");
    context.jobs().take_backup_password(id);
    let database = context.backend().database();
    let job = database.get(id).expect("job").expect("exists");
    let claim = database.claim(id, lettuce_jobs::WorkerId::new(), job.updated_at, std::time::Duration::from_secs(60), &ResourceAvailability::all()).expect("claim").expect("owned").claim;
    database.append_and_transition(JobMutation::Start { claim: claim.clone(), at: job.updated_at }).expect("start");
    let mut detail = database.job_detail(id).expect("detail").expect("exists").detail;
    let header = &detail["header"];
    let mut writer = lettuce_transfer::BackupWriter::new(Vec::new(), header["app_version"].as_str().expect("version"), TimestampMillis::new(header["created_at"].as_i64().expect("time")), "password").expect("writer");
    writer.append_bytes("data.json", "data.v1", b"{}").expect("section");
    let bytes = writer.finish().expect("complete");
    let mut input = std::io::Cursor::new(&bytes);
    detail["publication_header"] = serde_json::json!(lettuce_transfer::read_backup_header_bytes(&mut input).expect("header").1);
    if mismatched { detail["publication_header"][0] = serde_json::json!(0); }
    detail["size"] = serde_json::json!(bytes.len());
    detail["temporary_uri"] = serde_json::json!(if stale { "content://stale-backup" } else { "content://complete-backup" });
    std::fs::write(&files.path, bytes).expect("renamed file");
    database.checkpoint_backup_job(JobMutation::StageChanged { claim, stage: StageSnapshot::new("publish", true).expect("stage"), at: job.updated_at }, &detail).expect("intent");
    context.recover_after_restart().expect("reopen");
    assert!(files.path.exists());
    if mismatched {
        assert_eq!(files.publications.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert_eq!(database.get(id).expect("job").expect("exists").state, lettuce_jobs::JobState::Failed);
        assert!(database.backup_index().expect("index").is_empty());
    } else {
        assert_eq!(files.publications.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(database.get(id).expect("job").expect("exists").state, lettuce_jobs::JobState::Succeeded);
        assert_eq!(database.backup_index().expect("index")[0].uri, "content://complete-backup");
    }
}

#[tokio::test]
async fn cancellation_while_waiting_for_maintenance_keeps_the_old_database_and_emits_no_reload() {
    use super::jobs::JobHandler;
    use lettuce_jobs::JobStore;
    let (root, location, harness) = super::app_reset_tests::harness();
    let old_path = location.active_path().expect("old path");
    let hold = harness.context.maintenance().work(&lettuce_jobs::handle::CancellationToken::new()).await.expect("running work");
    let request = dto::BackupRestoreRequest { source: dto::FileSource { uri: root.join("never-opened.lettuce").display().to_string() }, password: "password".into(), client_operation_id: "cancel-wait".into() };
    let accepted = super::backup_restore(&harness.context, request).await.expect("admit");
    let job = harness.context.backend().database().get(accepted.job_id.parse().expect("id")).expect("job").expect("exists");
    let work = super::backup_jobs::RestoreHandler.claim(&harness.context, &job, lettuce_jobs::WorkerId::new()).await.expect("claim").expect("work");
    assert!(super::backup_jobs::RestoreHandler.claim(&harness.context, &job, lettuce_jobs::WorkerId::new()).await.expect("another worker").is_none());
    let cancellation = work.cancellation();
    let context = harness.context.clone();
    let task = tokio::spawn(async move { work.run(context, Arc::new(Progress)).await });
    cancellation.cancel();
    tokio::time::timeout(std::time::Duration::from_secs(5), task).await.expect("cancel without releasing old work").expect("join").expect("settled");
    assert_eq!(location.active_path().expect("active"), old_path);
    assert_eq!(harness.context.backend().database().get(job.id).expect("job").expect("exists").state, lettuce_jobs::JobState::Cancelled);
    assert!(!root.join("platform-v2/backup-restore-workspace").exists());
    assert!(!harness.events.events().iter().any(|event| matches!(event, dto::ApiEvent::DatabaseReloaded)));
    drop(hold);
}

#[tokio::test]
async fn live_legacy_import_replays_its_only_request_field_and_cold_restart_never_opens_the_source() {
    use lettuce_jobs::JobStore;
    let (_root, _location, harness) = super::app_reset_tests::harness();
    let request = dto::LegacyDatabaseImportRequest { client_operation_id: "legacy-live".into() };
    let accepted = super::legacy_database_import(&harness.context, request.clone()).await.expect("admit");
    assert_eq!(super::legacy_database_import(&harness.context, request).await.expect("replay"), accepted);
    let id = accepted.job_id.parse().expect("id");
    harness.context.jobs().take_backup_password(id);
    harness.context.recover_after_restart().expect("cold restart");
    assert_eq!(harness.context.backend().database().get(id).expect("job").expect("exists").state, lettuce_jobs::JobState::Failed);
    assert_eq!(super::job_get(&harness.context, dto::JobGetRequest { job_id: accepted.job_id }).await.expect("typed result").failure.expect("failure").reason, Some(dto::JobFailureReason::BackupRetryRequired));
}

struct HeldHandler {
    kind: lettuce_jobs::JobKind,
    entered: Arc<tokio::sync::Notify>,
    release: Arc<tokio::sync::Notify>,
}

#[async_trait::async_trait]
impl super::jobs::JobHandler for HeldHandler {
    fn kinds(&self) -> &[lettuce_jobs::JobKind] { std::slice::from_ref(&self.kind) }
    fn lane(&self, _context: &super::ApiContext, _job: &lettuce_jobs::JobSnapshot) -> Option<super::jobs::JobLane> { Some(super::jobs::JobLane("held-work".into())) }
    async fn claim(&self, context: &super::ApiContext, job: &lettuce_jobs::JobSnapshot, _worker: lettuce_jobs::WorkerId) -> Result<Option<Box<dyn super::jobs::ClaimedJob>>, dto::ApiError> {
        let claim = super::backup_jobs::claim_started(context.backend().database(), job.id, context.now())?;
        Ok(Some(Box::new(HeldWork { claim, entered: self.entered.clone(), release: self.release.clone(), token: lettuce_jobs::handle::CancellationToken::new() })))
    }
}

struct HeldWork {
    claim: lettuce_jobs::ClaimRef,
    entered: Arc<tokio::sync::Notify>,
    release: Arc<tokio::sync::Notify>,
    token: lettuce_jobs::handle::CancellationToken,
}

#[async_trait::async_trait]
impl super::jobs::ClaimedJob for HeldWork {
    fn cancellation(&self) -> lettuce_jobs::handle::CancellationToken { self.token.clone() }
    async fn run(self: Box<Self>, context: super::ApiContext, _progress: Arc<dyn super::jobs::JobProgressSink>) -> Result<(), dto::ApiError> {
        self.entered.notify_one();
        self.release.notified().await;
        super::backup_jobs::settle(context.backend().database(), &self.claim, &context, Err("backup-unavailable"))?;
        Ok(())
    }
}

#[tokio::test]
async fn restore_waits_for_running_install_and_memory_jobs_before_decoding_or_touching_files() {
    use super::jobs::JobHandler;
    use lettuce_jobs::{JobKind, JobSpec, JobSubject, SubjectKind, OutcomeRef, JobStore};
    for kind in [JobKind::ArtifactInstall, JobKind::MemoryExtraction] {
        let (root, location, harness) = super::app_reset_tests::harness();
        let old_path = location.active_path().expect("old");
        harness.context.backend().database().create_or_get(JobSpec::new(kind, JobSubject::new(SubjectKind::Backup, "held-job").expect("subject"), OutcomeRef::Request(lettuce_types::RequestId::new())).with_resources(vec![lettuce_jobs::ResourceClass::DiskWrite])).expect("held job");
        let entered = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let runner = super::jobs::JobRunner::new(harness.context.clone(), super::jobs::JobHandlers::new(vec![Arc::new(HeldHandler { kind, entered: entered.clone(), release: release.clone() })]));
        assert!(runner.run_once().await.expect("start job"));
        tokio::time::timeout(std::time::Duration::from_secs(5), entered.notified()).await.expect("running");
        let accepted = super::backup_restore(&harness.context, dto::BackupRestoreRequest { source: dto::FileSource { uri: root.join("missing.lettuce").display().to_string() }, password: "password".into(), client_operation_id: "restore-wait".into() }).await.expect("admit");
        let job = harness.context.backend().database().get(accepted.job_id.parse().expect("id")).expect("job").expect("exists");
        let work = super::backup_jobs::RestoreHandler.claim(&harness.context, &job, lettuce_jobs::WorkerId::new()).await.expect("claim").expect("work");
        let context = harness.context.clone();
        let mut restoring = tokio::spawn(async move { work.run(context, Arc::new(Progress)).await });
        assert!(tokio::time::timeout(std::time::Duration::from_millis(100), &mut restoring).await.is_err());
        assert_eq!(location.active_path().expect("active"), old_path);
        assert!(!root.join("platform-v2/backup-restore-workspace").exists());
        release.notify_one();
        tokio::time::timeout(std::time::Duration::from_secs(5), &mut restoring).await.expect("restore finishes after work").expect("join").expect("settled");
        runner.wait_idle().await;
        assert_eq!(super::job_get(&harness.context, dto::JobGetRequest { job_id: accepted.job_id }).await.expect("job").state, dto::JobStateDto::Failed);
    }
}

#[tokio::test]
async fn restore_bookkeeping_failure_still_reloads_and_reopen_finishes_without_replaying_restore() {
    use super::jobs::JobHandler;
    use lettuce_jobs::JobStore;
    let (root, location, harness, host, id) = completed_restore_with_faults(false, true).await;
    let fresh = host.slot.snapshot().expect("commands remain available");
    super::settings_get(&fresh).await.expect("database reads remain available");
    assert_eq!(harness.events.events().iter().filter(|event| matches!(event, dto::ApiEvent::DatabaseReloaded)).count(), 1);
    assert!(harness.events.events().iter().any(|event| matches!(event, dto::ApiEvent::JobUpdated { job } if job.failure.as_ref().is_some_and(|failure| failure.reason == Some(dto::JobFailureReason::BackupRecoveryRequired)))));
    let job = fresh.backend().database().get(id.parse().expect("id")).expect("job").expect("exists");
    fresh.jobs().remember_backup_password(job.id, zeroize::Zeroizing::new("password".into()));
    assert!(super::backup_jobs::RestoreHandler.lane(&fresh, &job).is_none());
    assert!(super::backup_jobs::RestoreHandler.claim(&fresh, &job, lettuce_jobs::WorkerId::new()).await.expect("applied restore cannot be claimed").is_none());
    let workers = host.workers.lock().expect("workers").take().expect("workers");
    workers.stop().await;
    {
        let lifecycle = location.file_lifecycle().await.expect("reopen file recovery");
        lifecycle.prepare_open(TimestampMillis::new(100)).expect("prepare active");
        lifecycle.complete_open().expect("active");
    }
    let reopened = file_test_context(&root, location.clone(), &harness, Arc::new(crate::AppBackend::open(location.active_path().expect("active"), TimestampMillis::new(100)).expect("backend")), Arc::new(super::tests::StdFiles));
    reopened.recover_after_restart().expect("reopen job recovery");
    assert_eq!(super::job_get(&reopened, dto::JobGetRequest { job_id: id }).await.expect("job").state, dto::JobStateDto::Succeeded);
}

#[tokio::test]
async fn legacy_archive_authentication_precedes_writes_and_unknown_entries_restore_with_a_warning() {
    use super::jobs::JobHandler;
    use lettuce_jobs::JobStore;
    for correct in [false, true] {
        let (root, location, harness) = super::app_reset_tests::harness();
        let old_path = location.active_path().expect("old");
        let host = Arc::new(ReloadHost { root: root.clone(), location: location.clone(), provider: harness.provider.clone(), events: harness.events.clone(), slot: Arc::new(super::ApiContextSlot::new(Arc::new(harness.context.clone()))), workers: std::sync::Mutex::new(None), fail: false, fail_settlement: false });
        harness.context.attach_reset_host(host.clone()).expect("host");
        let path = root.join("legacy.lettuce");
        std::fs::write(&path, include_bytes!("fixtures/legacy-unknown.lettuce")).expect("fixture");
        let request = dto::BackupRestoreRequest { source: dto::FileSource { uri: path.display().to_string() }, password: if correct { "legacy password" } else { "wrong password" }.into(), client_operation_id: "legacy-archive".into() };
        let accepted = super::legacy_backup_restore(&harness.context, request.clone()).await.expect("admit");
        assert_eq!(super::legacy_backup_restore(&harness.context, request.clone()).await.expect("replay"), accepted);
        assert_eq!(super::legacy_backup_restore(&harness.context, dto::BackupRestoreRequest { password: "changed password".into(), ..request }).await.expect_err("conflict").code, ApiErrorCode::Conflict);
        let job = harness.context.backend().database().get(accepted.job_id.parse().expect("id")).expect("job").expect("exists");
        let work = super::backup_jobs::RestoreHandler.claim(&harness.context, &job, lettuce_jobs::WorkerId::new()).await.expect("claim").expect("work");
        work.run(harness.context.clone(), Arc::new(Progress)).await.expect("settled");
        let current = host.slot.snapshot().expect("current");
        let result = super::job_get(&current, dto::JobGetRequest { job_id: accepted.job_id }).await.expect("job");
        if correct {
            assert_ne!(location.active_path().expect("new"), old_path);
            assert!(matches!(result.result, Some(dto::JobResultDto::BackupRestored { warnings, .. }) if warnings == vec![dto::BackupWarning::UnknownArchiveEntry { name: "data/unknown.json.enc".into() }]));
            let workers = host.workers.lock().expect("workers").take().expect("workers");
            workers.stop().await;
        } else {
            assert_eq!(location.active_path().expect("old active"), old_path);
            assert_eq!(result.failure.expect("typed").reason, Some(dto::JobFailureReason::BackupAuthentication));
            assert!(!root.join("platform-v2/backup-restore-workspace").exists());
            assert!(!harness.events.events().iter().any(|event| matches!(event, dto::ApiEvent::DatabaseReloaded)));
        }
        assert!(path.exists());
    }
}

#[tokio::test]
async fn export_lifecycle_contention_settles_typed_without_creating_a_file_or_index_entry() {
    use super::jobs::JobHandler;
    use lettuce_jobs::JobStore;
    let (root, location, harness) = super::app_reset_tests::harness();
    let target = root.join("busy.lettuce");
    let accepted = super::backup_export(&harness.context, dto::BackupExportRequest { password: "password".into(), target: Some(dto::FileTarget { uri: target.display().to_string() }), client_operation_id: "busy-export".into() }).await.expect("admit");
    let job = harness.context.backend().database().get(accepted.job_id.parse().expect("id")).expect("job").expect("exists");
    let work = super::backup_export_jobs::ExportHandler.claim(&harness.context, &job, lettuce_jobs::WorkerId::new()).await.expect("claim").expect("work");
    let hold = location.file_lifecycle().await.expect("other file operation");
    work.run(harness.context.clone(), Arc::new(Progress)).await.expect("typed job settlement");
    assert_eq!(super::job_get(&harness.context, dto::JobGetRequest { job_id: accepted.job_id }).await.expect("job").failure.expect("typed failure").reason, Some(dto::JobFailureReason::BackupUnavailable));
    assert!(!target.exists());
    assert!(harness.context.backend().database().backup_index().expect("index").is_empty());
    drop(hold);
}
