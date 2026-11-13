use std::{path::PathBuf, sync::Arc};

use lettuce_contracts as dto;
use lettuce_jobs::{JobMutation, JobStore, StageSnapshot};
use lettuce_types::TimestampMillis;

use crate::api::{ExportProtection, FileAccess, FileAccessError, FileDescription, FileReader};

const TARGET: &str = "content://documents/primary%3ABackups%2Fchosen.lettuce";

struct DocumentFiles { root: PathBuf }

impl DocumentFiles {
    fn document(&self) -> PathBuf { self.root.join("chosen.lettuce") }
    fn resolve(&self, uri: &str) -> String {
        if uri == TARGET { self.document().display().to_string() } else { uri.to_owned() }
    }
}

impl FileAccess for DocumentFiles {
    fn describe(&self, uri: &str) -> Result<FileDescription, FileAccessError> {
        crate::api::tests::StdFiles.describe(&self.resolve(uri))
    }
    fn open(&self, uri: &str) -> Result<Box<dyn FileReader>, FileAccessError> {
        crate::api::tests::StdFiles.open(&self.resolve(uri))
    }
    fn create(&self, uri: &str) -> Result<Box<dyn std::io::Write + Send>, FileAccessError> {
        crate::api::tests::StdFiles.create(&self.resolve(uri))
    }
    fn remove(&self, uri: &str) -> Result<(), FileAccessError> {
        crate::api::tests::StdFiles.remove(&self.resolve(uri))
    }
    fn backup_temporary_location(&self, target: Option<&str>, name: &str) -> Result<String, FileAccessError> {
        assert_eq!(target, Some(TARGET));
        Ok(self.root.join(name).display().to_string())
    }
    fn create_backup_temporary(&self, intent: &str, protected: ExportProtection<'_>) -> Result<(String, Box<dyn std::io::Write + Send>), FileAccessError> {
        crate::api::tests::StdFiles.create_backup_temporary(intent, protected)
    }
    fn sync_backup(&self, uri: &str) -> Result<(), FileAccessError> {
        crate::api::tests::StdFiles.sync_backup(uri)
    }
    fn publish_backup(&self, temporary: &str, target: Option<&str>, _name: &str, protected: ExportProtection<'_>) -> Result<String, FileAccessError> {
        assert_eq!(target, Some(TARGET), "the selected document URI must stay opaque");
        if protected(TARGET, None)? { return Err(FileAccessError::SourceIsTarget); }
        std::fs::copy(temporary, self.document()).map_err(|_| FileAccessError::Io)?;
        std::fs::remove_file(temporary).map_err(|_| FileAccessError::Io)?;
        Ok(TARGET.to_owned())
    }
}

fn fixture() -> (crate::api::ApiContext, Arc<DocumentFiles>) {
    let (root, location, harness) = crate::api::app::reset_tests::harness();
    let backend = Arc::new(crate::AppBackend::open(location.active_path().expect("database"), TimestampMillis::new(100)).expect("backend"));
    let files = Arc::new(DocumentFiles { root: root.clone() });
    (super::tests::file_test_context(&root, location, &harness, backend, files.clone()), files)
}

async fn admit(context: &crate::api::ApiContext) -> lettuce_types::JobId {
    crate::api::backup_export(context, dto::BackupExportRequest {
        target: Some(dto::FileTarget { uri: TARGET.into() }), password: "password".into(), client_operation_id: "document-export".into(),
    }).await.expect("admit").job_id.parse().expect("job id")
}

#[tokio::test]
async fn document_backup_exports_and_indexes_the_selected_uri() {
    use crate::api::jobs::JobHandler;
    let (context, files) = fixture();
    let id = admit(&context).await;
    let database = context.backend().database();
    let detail = database.job_detail(id).expect("detail").expect("exists").detail;
    assert_eq!(detail["target"], TARGET);
    let temporary = detail["temporary_intent"].as_str().expect("temporary");
    let job = database.get(id).expect("job").expect("exists");
    let work = super::export_jobs::ExportHandler.claim(&context, &job, lettuce_jobs::WorkerId::new()).await.expect("claim").expect("work");
    work.run(context.clone(), Arc::new(super::tests::Progress)).await.expect("run");
    assert_eq!(database.get(id).expect("job").expect("exists").state, lettuce_jobs::JobState::Succeeded);
    let backups = crate::api::backups_list(&context).await.expect("backups");
    assert_eq!(backups.len(), 1);
    assert_eq!(backups[0].uri, TARGET);
    assert_eq!(backups[0].availability, dto::BackupAvailability::Available);
    lettuce_transfer::BackupReader::open(std::fs::File::open(files.document()).expect("document"), "password").expect("decrypt complete backup");
    assert!(!std::path::Path::new(temporary).exists());
}

#[tokio::test]
async fn document_backup_resumes_an_interrupted_copy_after_restart() {
    recover_document_copy(false).await;
}

#[tokio::test]
async fn document_backup_recovery_does_not_publish_a_mismatched_staging_file() {
    recover_document_copy(true).await;
}

async fn recover_document_copy(mismatched: bool) {
    let (context, files) = fixture();
    let id = admit(&context).await;
    context.jobs().take_backup_password(id);
    let database = context.backend().database();
    let claim = super::jobs::claim_started(database, id, context.now()).expect("claim");
    let mut detail = database.job_detail(id).expect("detail").expect("exists").detail;
    let header = &detail["header"];
    let mut writer = lettuce_transfer::BackupWriter::new(Vec::new(), header["app_version"].as_str().expect("version"), TimestampMillis::new(header["created_at"].as_i64().expect("time")), "password").expect("writer");
    writer.append_bytes("data.json", "data.v1", b"{}").expect("section");
    let bytes = writer.finish().expect("complete");
    let temporary = detail["temporary_intent"].as_str().expect("temporary").to_owned();
    std::fs::write(&temporary, &bytes).expect("complete staged backup");
    std::fs::write(files.document(), &bytes[..bytes.len() / 2]).expect("interrupted document copy");
    detail["temporary_uri"] = serde_json::json!(&temporary);
    detail["size"] = serde_json::json!(bytes.len());
    detail["publication_header"] = serde_json::json!(lettuce_transfer::read_backup_header_bytes(&mut std::io::Cursor::new(&bytes)).expect("header").1);
    if mismatched { detail["publication_header"][0] = serde_json::json!(0); }
    database.checkpoint_backup_job(JobMutation::StageChanged { claim, stage: StageSnapshot::new("publish", true).expect("stage"), at: context.now() }, &detail).expect("publication intent");
    context.recover_after_restart().expect("restart");
    assert!(!std::path::Path::new(&temporary).exists());
    if mismatched {
        assert_eq!(std::fs::read(files.document()).expect("unchanged document"), bytes[..bytes.len() / 2]);
        assert_eq!(database.get(id).expect("job").expect("exists").state, lettuce_jobs::JobState::Failed);
        assert!(database.backup_index().expect("index").is_empty());
    } else {
        assert_eq!(std::fs::read(files.document()).expect("document"), bytes);
        assert_eq!(database.get(id).expect("job").expect("exists").state, lettuce_jobs::JobState::Succeeded);
        assert_eq!(database.backup_index().expect("index")[0].uri, TARGET);
    }
}
