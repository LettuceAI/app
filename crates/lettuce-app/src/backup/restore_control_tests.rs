use lettuce_media::LocalSyncMediaStore;
use lettuce_platform::{DirectorySnapshot, FilesystemAuthority};
use lettuce_settings::InMemorySecretStore;
use lettuce_types::{OperationId, TimestampMillis};

use crate::{AppBackend, AppDatabaseLocation, BackupRestoreCoordinator, BackupRestoreError};

#[tokio::test]
async fn cancellation_before_switch_recovers_old_fence_and_keeps_old_data() {
    let root = std::env::temp_dir().join(format!("lettuce-restore-control-{}", OperationId::new()));
    let authority = FilesystemAuthority::new(&DirectorySnapshot::new(&root).expect("snapshot")).expect("authority");
    let location = AppDatabaseLocation::new(root.join("private-persistent-v2"), &authority).expect("location");
    let old_path = location.active_path().expect("old path");
    let old;
    {
        let lifecycle = location.file_lifecycle().await.expect("lifecycle");
        lifecycle.prepare_open(TimestampMillis::new(1)).expect("prepare");
        old = AppBackend::open(&old_path, TimestampMillis::new(1)).expect("old backend");
        lifecycle.complete_open().expect("active");
    }
    let secrets = InMemorySecretStore::new();
    let media_root = root.join("platform-v2/media-blobs");
    let media = LocalSyncMediaStore::open(&media_root, lettuce_database::Database::open(&old_path).expect("blobs"), lettuce_database::Database::open(&old_path).expect("assets")).expect("media");
    let export = old.provider_backup(&secrets, &media).export("1.0", TimestampMillis::new(2), "password").await.expect("export");
    let workspace = root.join("workspace");
    let failure = BackupRestoreCoordinator::new(&location, &workspace, &media_root, &secrets)
        .restore_controlled(OperationId::new(), std::io::Cursor::new(export.output), "password", TimestampMillis::new(3), &|stage, database| {
            if stage == "switch" && database.is_some() { Err(BackupRestoreError::Cancelled) } else { Ok(()) }
        }).await.expect_err("cancel before switch");
    assert!(matches!(failure, BackupRestoreError::Cancelled));
    assert_eq!(location.active_path().expect("active"), old_path);
    old.database().record_backup(&lettuce_database::BackupIndexRecord { id: lettuce_types::JobId::new(), uri: "/still-writable.lettuce".into(), version: 2, created_at: TimestampMillis::new(4), app_version: "1.0".into(), size: 100 }).expect("old database unfenced");
    drop(media);
    drop(old);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test]
async fn unfinished_job_in_backup_uses_the_same_recovery_as_a_crash() {
    use lettuce_jobs::{JobStore, JobKind, JobSpec, JobSubject, SubjectKind, OutcomeRef, RecoveryPolicy, CancellationPolicy, JobMutation, ResourceAvailability, WorkerId};
    let root = std::env::temp_dir().join(format!("lettuce-restore-control-{}", OperationId::new()));
    let authority = FilesystemAuthority::new(&DirectorySnapshot::new(&root).expect("snapshot")).expect("authority");
    let location = AppDatabaseLocation::new(root.join("private-persistent-v2"), &authority).expect("location");
    let old_path = location.active_path().expect("old path");
    let old;
    {
        let lifecycle = location.file_lifecycle().await.expect("lifecycle");
        lifecycle.prepare_open(TimestampMillis::new(1)).expect("prepare");
        old = AppBackend::open(&old_path, TimestampMillis::new(1)).expect("old backend");
        lifecycle.complete_open().expect("active");
    }
    let secrets = InMemorySecretStore::new();
    let media_root = root.join("platform-v2/media-blobs");
    let media = LocalSyncMediaStore::open(&media_root, lettuce_database::Database::open(&old_path).expect("blobs"), lettuce_database::Database::open(&old_path).expect("assets")).expect("media");
    let job = old.database().create_or_get(JobSpec::new(JobKind::BackupExport, JobSubject::new(SubjectKind::Backup, "unfinished").expect("subject"), OutcomeRef::Request(lettuce_types::RequestId::new())).with_resources(vec![lettuce_jobs::ResourceClass::DiskWrite]).with_policies(RecoveryPolicy::MarkInterrupted, CancellationPolicy::UntilIrreversibleStage)).expect("job").job;
    let claim = old.database().claim(job.id, WorkerId::new(), job.updated_at, std::time::Duration::from_secs(3600), &ResourceAvailability::all()).expect("claim").expect("owned");
    old.database().append_and_transition(JobMutation::Start { claim: claim.claim, at: job.updated_at }).expect("start");
    let export = old.provider_backup(&secrets, &media).export("1.0", TimestampMillis::new(2), "password").await.expect("export");
    old.recover_after_restart(TimestampMillis::new(3)).expect("ordinary crash recovery");
    let expected = old.database().get(job.id).expect("crash job").expect("exists").state;
    assert_eq!(expected, lettuce_jobs::JobState::Interrupted);
    let workspace = root.join("workspace");
    BackupRestoreCoordinator::new(&location, &workspace, &media_root, &secrets).restore(OperationId::new(), std::io::Cursor::new(export.output), "password", TimestampMillis::new(3)).await.expect("restore");
    let restored = AppBackend::open(location.active_path().expect("active"), TimestampMillis::new(4)).expect("restored");
    restored.recover_after_restart(TimestampMillis::new(4)).expect("restored startup recovery");
    assert_eq!(restored.database().get(job.id).expect("restored job").expect("exists").state, expected);
    drop((media, old, restored));
    std::fs::remove_dir_all(root).expect("cleanup");
}
