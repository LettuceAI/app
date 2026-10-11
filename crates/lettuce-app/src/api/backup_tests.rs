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
