use lettuce_transfer::ProviderBackupSource;
use lettuce_types::TimestampMillis;

fn copy_database(source: &std::path::Path, folder: &std::path::Path) -> std::path::PathBuf {
    let name = source.file_name().expect("database name");
    let target = folder.join(name);
    std::fs::copy(source, &target).expect("copy database");
    for suffix in ["-wal", "-shm"] {
        let mut sidecar = source.as_os_str().to_owned();
        sidecar.push(suffix);
        let sidecar = std::path::PathBuf::from(sidecar);
        if sidecar.exists() {
            let mut copied = target.as_os_str().to_owned();
            copied.push(suffix);
            std::fs::copy(&sidecar, std::path::PathBuf::from(copied)).expect("copy sidecar");
        }
    }
    target
}

#[tokio::test]
#[ignore = "reads a real database named by LETTUCE_BACKUP_PROBE_DB"]
async fn probe_backup_export_against_a_real_database() {
    let source = std::path::PathBuf::from(std::env::var("LETTUCE_BACKUP_PROBE_DB").expect("LETTUCE_BACKUP_PROBE_DB"));
    let folder = std::env::temp_dir().join(format!("lettuce-backup-probe-{}", lettuce_types::JobId::new()));
    std::fs::create_dir_all(&folder).expect("probe folder");
    let path = copy_database(&source, &folder);
    let now = TimestampMillis::new(chrono::Utc::now().timestamp_millis());
    let backend = crate::AppBackend::open(&path, now).expect("open the copied database");
    match backend.database().read_provider_backup_graph() {
        Err(error) => eprintln!("PROBE graph read failed: {error:?}"),
        Ok(graph) => {
            let mut checked = graph.clone();
            match lettuce_transfer::canonicalize_and_validate(&mut checked) {
                Err(error) => eprintln!("PROBE graph validation failed: {error:?}"),
                Ok(()) => eprintln!("PROBE graph validation passed"),
            }
        }
    }
    let media_root = std::env::var("LETTUCE_BACKUP_PROBE_MEDIA").map(std::path::PathBuf::from).unwrap_or_else(|_| folder.join("media-blobs"));
    let media = lettuce_media::LocalSyncMediaStore::open(media_root, lettuce_database::Database::open(&path).expect("blobs"), lettuce_database::Database::open(&path).expect("assets")).expect("media");
    let _ = tracing_subscriber::fmt().with_test_writer().with_ansi(false).with_max_level(tracing::Level::INFO).try_init();
    let native = std::env::var("LETTUCE_BACKUP_PROBE_NATIVE_SECRETS").is_ok();
    let secrets: Box<dyn lettuce_settings::SecretStore> = if native { Box::new(lettuce_settings::NativeSecretStore::try_new().expect("native secret store")) } else { Box::new(lettuce_settings::InMemorySecretStore::new()) };
    eprintln!("PROBE secret store: {}", if native { "native" } else { "in memory" });
    match backend.provider_backup(secrets.as_ref(), &media).export_to(std::io::sink(), "probe", now, "password").await {
        Err(error) => eprintln!("PROBE export failed: {error:?}"),
        Ok(export) => eprintln!("PROBE export succeeded, {} missing secrets", export.missing_secrets.len()),
    }
    drop(media);
    drop(backend);
    let _ = std::fs::remove_dir_all(folder);
}

fn copy_tree(source: &std::path::Path, target: &std::path::Path) {
    std::fs::create_dir_all(target).expect("folder");
    for entry in std::fs::read_dir(source).expect("read folder") {
        let entry = entry.expect("entry");
        let kind = entry.file_type().expect("type");
        let destination = target.join(entry.file_name());
        if kind.is_dir() { copy_tree(&entry.path(), &destination); } else if kind.is_file() { std::fs::copy(entry.path(), destination).expect("copy file"); }
    }
}

async fn run_backup_export(context: &crate::api::ApiContext, root: &std::path::Path, label: &str) {
    use crate::api::jobs::JobHandler;
    use lettuce_jobs::JobStore;
    let target = root.join(format!("{label}.lettuce"));
    let accepted = crate::api::backup_export(context, lettuce_contracts::BackupExportRequest { password: "password".into(), target: Some(lettuce_contracts::FileTarget { uri: target.display().to_string() }), client_operation_id: format!("probe-{label}") }).await.expect("admit export");
    let job = context.backend().database().get(accepted.job_id.parse().expect("id")).expect("job").expect("exists");
    let work = super::export_jobs::ExportHandler.claim(context, &job, lettuce_jobs::WorkerId::new()).await.expect("claim").expect("work");
    work.run(context.clone(), std::sync::Arc::new(super::tests::Progress)).await.expect("run");
    let view = crate::api::job_get(context, lettuce_contracts::JobGetRequest { job_id: accepted.job_id }).await.expect("job view");
    eprintln!("PROBE {label} export state={:?} failure={:?} size={:?}", view.state, view.failure, std::fs::metadata(&target).map(|m| m.len()).ok());
}

#[tokio::test]
#[ignore = "imports the legacy folder named by LETTUCE_BACKUP_PROBE_LEGACY"]
async fn probe_backup_export_after_a_real_legacy_import() {
    use crate::api::jobs::JobHandler;
    use lettuce_jobs::JobStore;
    let legacy = std::path::PathBuf::from(std::env::var("LETTUCE_BACKUP_PROBE_LEGACY").expect("LETTUCE_BACKUP_PROBE_LEGACY"));
    let (root, location, harness) = crate::api::app::reset_tests::harness();
    let copy = root.join("lettuce");
    std::fs::create_dir_all(&copy).expect("legacy copy");
    for entry in std::fs::read_dir(&legacy).expect("legacy folder") {
        let entry = entry.expect("entry");
        let name = entry.file_name().to_string_lossy().into_owned();
        if ["models", "kokoro", "onnxruntime", "runtimes", "downloads"].contains(&name.as_str()) { continue; }
        if entry.file_type().expect("type").is_dir() { copy_tree(&entry.path(), &copy.join(&name)); } else { std::fs::copy(entry.path(), copy.join(&name)).expect("copy"); }
    }
    let host = std::sync::Arc::new(super::tests::ReloadHost { root: root.clone(), location: location.clone(), provider: harness.provider.clone(), events: harness.events.clone(), slot: std::sync::Arc::new(crate::api::ApiContextSlot::new(std::sync::Arc::new(harness.context.clone()))), workers: std::sync::Mutex::new(None), fail: false, fail_settlement: false });
    harness.context.attach_reset_host(host.clone()).expect("host");
    let accepted = crate::api::legacy_database_import(&harness.context, lettuce_contracts::LegacyDatabaseImportRequest { client_operation_id: "probe-legacy".into() }).await.expect("admit legacy import");
    let job = harness.context.backend().database().get(accepted.job_id.parse().expect("id")).expect("job").expect("exists");
    let work = super::jobs::RestoreHandler.claim(&harness.context, &job, lettuce_jobs::WorkerId::new()).await.expect("claim").expect("work");
    work.run(harness.context.clone(), std::sync::Arc::new(super::tests::Progress)).await.expect("settled");
    let fresh = host.slot.snapshot().expect("fresh context");
    let view = crate::api::job_get(&fresh, lettuce_contracts::JobGetRequest { job_id: accepted.job_id }).await.expect("import view");
    eprintln!("PROBE legacy import state={:?} failure={:?}", view.state, view.failure);
    run_backup_export(&fresh, &root, "after-legacy").await;
    if let Some(workers) = host.workers.lock().expect("workers").take() { workers.stop().await; }
}
