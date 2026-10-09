use super::*;
use lettuce_jobs::{JobKind, JobSpec, JobStore, JobSubject, OutcomeRef, SubjectKind};
use lettuce_media::{
    AssetKind, AssetOrigin, AssetProvenanceV1, BlobState, MediaAsset, MediaAssetRepository,
    MediaBlob, MediaBlobRepository, MediaKind, RetentionClass,
};
use lettuce_types::{AssetId, MediaBlobId, Revision};
use rusqlite::params;

fn asset(database: &Database, digit: char, retention: RetentionClass) -> (AssetId, ContentHash) {
    let hash = ContentHash::parse(digit.to_string().repeat(64)).expect("hash");
    let blob = MediaBlobRepository::register(
        database,
        MediaBlob {
            id: MediaBlobId::new(),
            content_hash: hash.clone(),
            kind: MediaKind::Image,
            mime_type: "image/png".into(),
            byte_size: 42,
            width: None,
            height: None,
            duration_ms: None,
            validation_version: 1,
            state: BlobState::Staged,
            created_at: TimestampMillis::new(10),
            updated_at: TimestampMillis::new(10),
        },
    )
    .expect("register blob");
    MediaBlobRepository::finalize_staged_to_ready(database, blob.id, blob.updated_at)
        .expect("ready blob");
    let id = AssetId::new();
    MediaAssetRepository::create(
        database,
        MediaAsset::new(
            id,
            blob.id,
            AssetKind::OtherImage,
            AssetOrigin::Upload,
            retention,
            AssetProvenanceV1::default(),
            Revision::INITIAL,
            TimestampMillis::new(10),
            TimestampMillis::new(10),
        )
        .expect("valid asset"),
    )
    .expect("create asset");
    (id, hash)
}

#[test]
fn kept_media_set_uses_reference_and_library_rules_instead_of_the_whole_catalog() {
    let path =
        std::env::temp_dir().join(format!("lettuce-kept-set-{}.sqlite3", uuid::Uuid::new_v4()));
    let database = Database::open(&path).expect("database");
    let (referenced, reference_hash) = asset(&database, 'a', RetentionClass::Persistent);
    let (_, library_hash) = asset(&database, 'b', RetentionClass::Library);
    let (_, garbage_hash) = asset(&database, 'c', RetentionClass::Persistent);
    database
        .create_or_get(
            JobSpec::new(
                JobKind::ArtifactInstall,
                JobSubject::new(SubjectKind::ArtifactInstall, "retained-media").expect("subject"),
                OutcomeRef::ArtifactInstallation(referenced),
            )
            .with_resources(vec![lettuce_jobs::ResourceClass::DiskWrite]),
        )
        .expect("reference job");
    assert_eq!(
        Database::media_objects_in_file(&path).expect("kept hashes before collection"),
        BTreeSet::from([reference_hash.clone(), library_hash.clone()])
    );
    database
        .connection()
        .expect("connection")
        .execute(
            "INSERT INTO media_gc_candidates SELECT id, 11 FROM media_assets",
            [],
        )
        .expect("queue all assets");
    let released = database
        .collect_media_garbage(TimestampMillis::new(11))
        .expect("collect active media");
    assert_eq!(
        released
            .iter()
            .map(|object| object.content_hash.clone())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([garbage_hash.clone()])
    );
    drop(database);
    let retained = Database::media_objects_in_file(&path).expect("kept hashes");
    assert_eq!(retained, BTreeSet::from([reference_hash, library_hash]));
    assert!(!retained.contains(&garbage_hash));
    std::fs::remove_file(path).expect("remove test database");
}

fn imported_asset_proof(
    database: &Database,
) -> lettuce_transfer::LegacyImportMediaCompletionRequest {
    use lettuce_transfer::LegacyImportRepository;
    let (id, hash) = asset(database, 'e', RetentionClass::Persistent);
    let blob = MediaAssetRepository::get(database, id)
        .expect("asset")
        .expect("asset exists")
        .blob_id;
    let run = lettuce_types::LegacyImportRunId::new();
    {
        let connection = database.connection().expect("connection");
        connection.execute("INSERT INTO legacy_import_runs (id,source_schema_version,inventory_fingerprint,plan_fingerprint,status,admitted_at,updated_at) VALUES (?1,96,?2,?2,'admitting',1,1)", params![run.to_string(), "d".repeat(64)]).expect("run");
        connection.execute("INSERT INTO legacy_import_assignments (run_id,source_kind,source_key,destination_id,expected_byte_len,expected_content_hash) VALUES (?1,'media','images/proof.png',?2,42,?3)", params![run.to_string(),id.to_string(),hash.as_str()]).expect("assignment");
        connection
            .execute(
                "UPDATE legacy_import_runs SET status='admitted' WHERE id=?1",
                [run.to_string()],
            )
            .expect("admitted");
    }
    let proof = lettuce_transfer::LegacyImportMediaCompletionRequest {
        run_id: run,
        relative_path: "images/proof.png".into(),
        destination_asset_id: id,
        blob_id: blob,
        byte_len: 42,
        content_hash: hash,
        completed_at: TimestampMillis::new(11),
    };
    database
        .complete_media(proof.clone())
        .expect("complete media");
    database
        .connection()
        .expect("connection")
        .execute(
            "UPDATE legacy_import_runs SET status='failed' WHERE id=?1",
            [run.to_string()],
        )
        .expect("terminal import");
    proof
}

fn pending_import_assignment(
    database: &Database,
    status: &str,
) -> (lettuce_types::LegacyImportRunId, AssetId, ContentHash) {
    let (id, hash) = asset(database, 'f', RetentionClass::Persistent);
    let run = lettuce_types::LegacyImportRunId::new();
    let connection = database.connection().expect("connection");
    connection.execute("INSERT INTO legacy_import_runs (id,source_schema_version,inventory_fingerprint,plan_fingerprint,source_fingerprint,status,admitted_at,updated_at) VALUES (?1,96,?2,?2,?2,'admitting',1,1)", params![run.to_string(), "c".repeat(64)]).expect("run");
    connection.execute("INSERT INTO legacy_import_assignments (run_id,source_kind,source_key,destination_id,expected_byte_len,expected_content_hash) VALUES (?1,'media','images/pending.png',?2,42,?3)", params![run.to_string(),id.to_string(),hash.as_str()]).expect("assignment");
    for next in ["admitted", "importing", "partial"] {
        if status == "admitting" {
            break;
        }
        connection
            .execute(
                "UPDATE legacy_import_runs SET status=?2 WHERE id=?1",
                params![run.to_string(), next],
            )
            .expect("advance import");
        if status == next {
            break;
        }
    }
    (run, id, hash)
}

#[test]
fn pending_media_assignment_survives_gc_before_its_completion_proof() {
    for status in ["admitting", "admitted", "importing"] {
        let database = Database::open_in_memory().expect("database");
        let (_, id, hash) = pending_import_assignment(&database, status);
        database
            .connection()
            .expect("connection")
            .execute(
                "INSERT INTO media_gc_candidates VALUES (?1,12)",
                [id.to_string()],
            )
            .expect("queue");
        assert!(
            database
                .collect_media_garbage(TimestampMillis::new(12))
                .expect("collect")
                .is_empty(),
            "assignment in {status} must keep media"
        );
        assert!(
            MediaAssetRepository::get(&database, id)
                .expect("asset")
                .is_some()
        );
        assert_eq!(
            retained_objects(&database.connection().expect("connection")).expect("kept set"),
            BTreeSet::from([hash])
        );
    }
}

#[test]
fn partial_import_keeps_pending_media_until_all_stage_receipts_and_completion() {
    use lettuce_transfer::{LegacyImportRepository, LegacyImportRunStatus, LegacyImportStage};
    let database = Database::open_in_memory().expect("database");
    let (run, id, hash) = pending_import_assignment(&database, "partial");
    {
        let connection = database.connection().expect("connection");
        for stage in LegacyImportStage::ALL
            .into_iter()
            .filter(|stage| *stage != LegacyImportStage::Characters)
        {
            connection
                .execute(
                    "INSERT INTO legacy_import_stage_results VALUES (?1,?2,0,12)",
                    params![
                        run.to_string(),
                        crate::legacy::legacy_import_adapter::stage_name(stage)
                    ],
                )
                .expect("stage receipt");
        }
        connection
            .execute(
                "INSERT INTO media_gc_candidates VALUES (?1,12)",
                [id.to_string()],
            )
            .expect("queue");
    }
    assert!(
        database
            .collect_media_garbage(TimestampMillis::new(12))
            .expect("collect partial")
            .is_empty()
    );
    assert!(
        MediaAssetRepository::get(&database, id)
            .expect("asset")
            .is_some()
    );
    assert_eq!(
        retained_objects(&database.connection().expect("connection")).expect("kept set"),
        BTreeSet::from([hash.clone()])
    );
    let operation = lettuce_types::RequestId::new();
    let error = database
        .remove_media_library_asset(id, operation, "partial-remove", TimestampMillis::new(12))
        .expect_err("partial import refuses removal");
    let lettuce_media::MediaLibraryError::InUse(references) = error else {
        panic!("expected InUse, got {error:?}");
    };
    assert!(references.iter().any(|reference| {
        reference.kind == lettuce_media::MediaReferenceKind::LegacyImport
            && reference.owner_id == run.to_string()
    }));
    assert!(
        MediaAssetRepository::get(&database, id)
            .expect("preserved asset")
            .is_some()
    );
    assert!(
        database
            .complete_legacy_import_run(run, TimestampMillis::new(13))
            .is_err()
    );
    database
        .connection()
        .expect("connection")
        .execute(
            "INSERT INTO legacy_import_stage_results VALUES (?1,'characters',0,13)",
            [run.to_string()],
        )
        .expect("last stage");
    assert_eq!(
        database
            .complete_legacy_import_run(run, TimestampMillis::new(13))
            .expect("complete run"),
        LegacyImportRunStatus::Completed
    );
    let released = database
        .remove_media_library_asset(id, operation, "partial-remove", TimestampMillis::new(14))
        .expect("completed import permits removal");
    assert_eq!(released.len(), 1);
    assert_eq!(released[0].content_hash, hash);
    assert!(
        MediaAssetRepository::get(&database, id)
            .expect("removed asset")
            .is_none()
    );
    assert_eq!(
        database
            .remove_media_library_asset(id, operation, "partial-remove", TimestampMillis::new(15))
            .expect("same operation replays"),
        released
    );
    assert!(matches!(
        database.remove_media_library_asset(
            id,
            operation,
            "different-digest",
            TimestampMillis::new(15)
        ),
        Err(lettuce_media::MediaLibraryError::Conflict)
    ));
}

#[test]
fn media_removal_receipt_failure_rolls_back_asset_blob_and_retention() {
    let database = Database::open_in_memory().expect("database");
    let (id, hash) = asset(&database, 'a', RetentionClass::Library);
    let before = MediaAssetRepository::get(&database, id)
        .expect("asset")
        .expect("exists");
    database.connection().expect("connection").execute_batch("CREATE TEMP TRIGGER fail_media_remove_receipt BEFORE INSERT ON api_operation_receipts WHEN NEW.command='media_library_remove' BEGIN SELECT RAISE(ABORT,'receipt fault'); END;").expect("fault trigger");
    let key = lettuce_types::RequestId::new();
    assert!(
        database
            .remove_media_library_asset(id, key, "remove", TimestampMillis::new(11))
            .is_err()
    );
    assert_eq!(
        MediaAssetRepository::get(&database, id).expect("rolled back asset"),
        Some(before.clone())
    );
    assert!(
        MediaBlobRepository::get(&database, before.blob_id)
            .expect("rolled back blob")
            .is_some()
    );
    assert!(
        database
            .lookup_api_operation("media_library_remove", &key.to_string())
            .expect("receipt")
            .is_none()
    );
    database
        .connection()
        .expect("connection")
        .execute_batch("DROP TRIGGER fail_media_remove_receipt")
        .expect("remove fault");
    let released = database
        .remove_media_library_asset(id, key, "remove", TimestampMillis::new(12))
        .expect("retry");
    assert_eq!(released.len(), 1);
    assert_eq!(released[0].content_hash, hash);
    assert!(
        MediaAssetRepository::get(&database, id)
            .expect("removed asset")
            .is_none()
    );
    assert!(
        MediaBlobRepository::get(&database, before.blob_id)
            .expect("removed blob")
            .is_none()
    );
}

#[test]
fn competing_media_removal_requests_replay_one_receipt() {
    let database = std::sync::Arc::new(Database::open_in_memory().expect("database"));
    let (id, _) = asset(&database, 'b', RetentionClass::Library);
    let key = lettuce_types::RequestId::new();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
    let workers = (0..2)
        .map(|_| {
            let database = database.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                database.remove_media_library_asset(id, key, "same", TimestampMillis::new(12))
            })
        })
        .collect::<Vec<_>>();
    barrier.wait();
    let results = workers
        .into_iter()
        .map(|worker| worker.join().expect("worker").expect("remove"))
        .collect::<Vec<_>>();
    assert_eq!(results[0], results[1]);
    assert_eq!(results[0].len(), 1);
    assert!(matches!(
        database.remove_media_library_asset(id, key, "changed", TimestampMillis::new(13)),
        Err(lettuce_media::MediaLibraryError::Conflict)
    ));
}

#[test]
fn reference_committing_before_media_removal_is_reported_as_in_use() {
    let root =
        std::env::temp_dir().join(format!("lettuce-remove-reference-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("root");
    let path = root.join("database.sqlite3");
    let database = Database::open(&path).expect("database");
    let (id, hash) = asset(&database, 'b', RetentionClass::Library);
    let contender = Database::open(&path).expect("contender");
    let run = lettuce_types::LegacyImportRunId::new();
    let mut connection = database.connection().expect("connection");
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .expect("writer transaction");
    transaction.execute("INSERT INTO legacy_import_runs (id,source_schema_version,inventory_fingerprint,plan_fingerprint,status,admitted_at,updated_at) VALUES (?1,96,?2,?2,'admitting',1,1)", params![run.to_string(), "e".repeat(64)]).expect("run");
    transaction.execute("INSERT INTO legacy_import_assignments (run_id,source_kind,source_key,destination_id,expected_byte_len,expected_content_hash) VALUES (?1,'media','images/concurrent.png',?2,42,?3)", params![run.to_string(),id.to_string(),hash.as_str()]).expect("new reference");
    let (entered, receiver) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        entered.send(()).expect("entered");
        contender.remove_media_library_asset(
            id,
            lettuce_types::RequestId::new(),
            "remove",
            TimestampMillis::new(12),
        )
    });
    receiver.recv().expect("worker entered");
    transaction.commit().expect("reference commit");
    let error = worker
        .join()
        .expect("worker")
        .expect_err("new reference wins");
    let lettuce_media::MediaLibraryError::InUse(references) = error else {
        panic!("expected InUse, got {error:?}");
    };
    assert!(references.iter().any(|reference| reference.kind
        == lettuce_media::MediaReferenceKind::LegacyImport
        && reference.owner_id == run.to_string()));
    drop(connection);
    assert!(
        MediaAssetRepository::get(&database, id)
            .expect("preserved asset")
            .is_some()
    );
    drop(database);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn media_removal_crash_child() {
    let Ok(root) = std::env::var("LETTUCE_MEDIA_REMOVE_CRASH_ROOT") else {
        return;
    };
    let root = std::path::PathBuf::from(root);
    let id: AssetId = std::env::var("LETTUCE_MEDIA_REMOVE_CRASH_ASSET")
        .expect("asset")
        .parse()
        .expect("asset id");
    let database = Database::open(root.join("database.sqlite3")).expect("database");
    let _: Result<(), lettuce_media::MediaLibraryError> = database.commit_api_operation(
        "media_library_remove",
        "crash",
        "remove",
        TimestampMillis::new(12),
        |operation| {
            let released = operation.remove_media_library_asset(id, TimestampMillis::new(12))?;
            assert_eq!(released.len(), 1);
            std::process::exit(77);
        },
    );
    panic!("child should exit before receipt and commit");
}

#[test]
fn media_removal_crash_before_receipt_commit_preserves_the_asset_and_retries() {
    let root = std::env::temp_dir().join(format!("lettuce-remove-crash-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("root");
    let path = root.join("database.sqlite3");
    let database = Database::open(&path).expect("database");
    let (id, _) = asset(&database, 'd', RetentionClass::Library);
    let before = MediaAssetRepository::get(&database, id)
        .expect("asset")
        .expect("exists");
    drop(database);
    let status = std::process::Command::new(std::env::current_exe().expect("test executable"))
        .args([
            "--exact",
            "purge::media_gc::tests::media_removal_crash_child",
            "--nocapture",
        ])
        .env("LETTUCE_MEDIA_REMOVE_CRASH_ROOT", &root)
        .env("LETTUCE_MEDIA_REMOVE_CRASH_ASSET", id.to_string())
        .status()
        .expect("child");
    assert_eq!(status.code(), Some(77));
    let database = Database::open(&path).expect("reopen");
    assert_eq!(
        MediaAssetRepository::get(&database, id).expect("asset after crash"),
        Some(before.clone())
    );
    assert!(
        MediaBlobRepository::get(&database, before.blob_id)
            .expect("blob after crash")
            .is_some()
    );
    assert!(
        database
            .lookup_api_operation("media_library_remove", "crash")
            .expect("receipt after crash")
            .is_none()
    );
    let key = lettuce_types::RequestId::new();
    let released = database
        .remove_media_library_asset(id, key, "remove", TimestampMillis::new(13))
        .expect("retry");
    assert_eq!(released.len(), 1);
    assert!(
        MediaAssetRepository::get(&database, id)
            .expect("removed asset")
            .is_none()
    );
    drop(database);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn media_library_pages_include_all_retention_classes_and_references() {
    let database = Database::open_in_memory().expect("database");
    let (library, _) = asset(&database, 'a', RetentionClass::Library);
    let (persistent, _) = asset(&database, 'b', RetentionClass::Persistent);
    let (temporary, _) = asset(
        &database,
        'c',
        RetentionClass::Temporary {
            expires_at: TimestampMillis::new(100),
        },
    );
    let (_, pending, _) = pending_import_assignment(&database, "partial");
    let mut ids = BTreeSet::new();
    let mut cursor = None;
    loop {
        let page = database
            .media_library_page(
                Some(MediaKind::Image),
                lettuce_types::PageRequest {
                    cursor,
                    limit: lettuce_types::PageLimit::new(1),
                },
            )
            .expect("page");
        for entry in page.items {
            assert!(
                ids.insert(entry.asset.id),
                "no duplicate asset across pages"
            );
            assert_eq!(entry.blob.kind, MediaKind::Image);
            if entry.asset.id == pending {
                assert!(
                    entry.references.iter().any(|reference| reference.kind
                        == lettuce_media::MediaReferenceKind::LegacyImport)
                );
            }
        }
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(
        ids,
        BTreeSet::from([library, persistent, temporary, pending])
    );
}

#[test]
fn media_library_cursor_rejects_a_changed_kind_and_clamps_only_pages() {
    let database = Database::open_in_memory().expect("database");
    for _ in 0..203 {
        asset(&database, 'a', RetentionClass::Library);
    }
    let first = database
        .media_library_page(
            Some(MediaKind::Image),
            lettuce_types::PageRequest {
                cursor: None,
                limit: lettuce_types::PageLimit::new(u16::MAX),
            },
        )
        .expect("first page");
    assert_eq!(first.items.len(), 200);
    let cursor = first.next_cursor.expect("more assets");
    assert!(matches!(
        database.media_library_page(
            Some(MediaKind::Audio),
            lettuce_types::PageRequest {
                cursor: Some(cursor.clone()),
                limit: lettuce_types::PageLimit::default()
            }
        ),
        Err(lettuce_media::MediaLibraryError::InvalidCursor)
    ));
    let last = database
        .media_library_page(
            Some(MediaKind::Image),
            lettuce_types::PageRequest {
                cursor: Some(cursor),
                limit: lettuce_types::PageLimit::default(),
            },
        )
        .expect("remaining assets");
    assert_eq!(last.items.len(), 3);
    assert!(last.next_cursor.is_none());
    assert!(matches!(
        database.media_library_page(
            None,
            lettuce_types::PageRequest {
                cursor: Some("malformed".into()),
                limit: lettuce_types::PageLimit::default()
            }
        ),
        Err(lettuce_media::MediaLibraryError::InvalidCursor)
    ));
}

#[test]
fn missing_media_is_not_reported_as_having_no_references() {
    let database = Database::open_in_memory().expect("database");
    assert!(matches!(
        database.media_references(AssetId::new()),
        Err(lettuce_media::MediaLibraryError::NotFound)
    ));
}

#[test]
fn removed_imported_media_leaves_an_immutable_snapshot_without_holding_catalog_rows() {
    use lettuce_transfer::LegacyImportRepository;
    let database = Database::open_in_memory().expect("database");
    let proof = imported_asset_proof(&database);
    database
        .connection()
        .expect("connection")
        .execute(
            "INSERT INTO media_gc_candidates VALUES (?1,12)",
            [proof.destination_asset_id.to_string()],
        )
        .expect("queue");
    let released = database
        .collect_media_garbage(TimestampMillis::new(12))
        .expect("collect");
    assert_eq!(released.len(), 1);
    assert_eq!(released[0].content_hash, proof.content_hash);
    assert_eq!(
        MediaAssetRepository::get(&database, proof.destination_asset_id).expect("asset"),
        None
    );
    assert_eq!(
        MediaBlobRepository::get(&database, proof.blob_id).expect("blob"),
        None
    );
    assert!(
        database
            .complete_media(proof.clone())
            .expect("proof replay")
            .replayed
    );
    let connection = database.connection().expect("connection");
    assert!(
        connection
            .execute(
                "UPDATE legacy_import_media_completions SET byte_len=byte_len",
                []
            )
            .is_err()
    );
    assert!(
        connection
            .execute("DELETE FROM legacy_import_media_completions", [])
            .is_err()
    );
    let held: i64 = connection.query_row("SELECT count(*) FROM pragma_foreign_key_list('legacy_import_media_completions') WHERE \"table\" IN ('media_assets','media_blobs')", [], |row| row.get(0)).expect("foreign keys");
    assert_eq!(held, 0);
    let bad_asset = AssetId::new().to_string();
    assert!(connection.execute("INSERT INTO legacy_import_media_completions (run_id,relative_path,destination_asset_id,blob_id,byte_len,content_hash,completed_at) VALUES (?1,'images/proof.png',?2,?3,42,?4,13)", params![proof.run_id.to_string(),bad_asset,proof.blob_id.to_string(),proof.content_hash.as_str()]).is_err());
}

#[test]
fn backup_restores_a_removed_media_proof_and_refuses_a_changed_snapshot() {
    use lettuce_transfer::{
        ProviderBackupRestoreWriter, ProviderBackupSecretSet, ProviderBackupSource,
        plan_provider_backup_export,
    };
    let database = Database::open_in_memory().expect("database");
    let proof = imported_asset_proof(&database);
    let mut graph = database.read_provider_backup_graph().expect("export graph");
    graph.authored.media_assets.clear();
    graph.authored.media_blobs.clear();
    plan_provider_backup_export(graph.clone(), ProviderBackupSecretSet::default())
        .expect("removed media proof export");
    let target = Database::open_in_memory().expect("target");
    target
        .restore_provider_backup_graph(&graph, &[])
        .expect("restore proof without media");
    let restored = target.read_provider_backup_graph().expect("restored graph");
    assert_eq!(restored.legacy_imports, graph.legacy_imports);
    assert!(restored.authored.media_assets.is_empty());
    assert!(restored.authored.media_blobs.is_empty());
    let connection = target.connection().expect("connection");
    assert!(
        connection
            .execute("DELETE FROM legacy_import_media_completions", [])
            .is_err()
    );
    assert!(
        connection
            .execute(
                "UPDATE legacy_import_media_completions SET content_hash=?1",
                ["f".repeat(64)]
            )
            .is_err()
    );
    drop(connection);
    graph.legacy_imports.runs[0].media_completions[0].insert(
        "content_hash".into(),
        lettuce_transfer::BackupSqlValue::Text("f".repeat(64)),
    );
    assert!(
        plan_provider_backup_export(graph.clone(), ProviderBackupSecretSet::default()).is_err()
    );
    let target = Database::open_in_memory().expect("bad target");
    assert!(target.restore_provider_backup_graph(&graph, &[]).is_err());
    let empty = target.read_provider_backup_graph().expect("rollback graph");
    assert!(empty.legacy_imports.runs.is_empty());
    assert!(empty.authored.media_assets.is_empty());
    assert!(empty.authored.media_blobs.is_empty());
    assert_eq!(proof.content_hash.as_str(), "e".repeat(64));
}

#[test]
fn imported_asset_removal_keeps_a_shared_library_blob_until_its_last_asset_is_garbage() {
    let database = Database::open_in_memory().expect("database");
    let proof = imported_asset_proof(&database);
    let (sibling, hash) = asset(&database, 'e', RetentionClass::Library);
    assert_eq!(hash, proof.content_hash);
    assert_eq!(
        MediaAssetRepository::get(&database, sibling)
            .expect("sibling")
            .expect("exists")
            .blob_id,
        proof.blob_id
    );
    database
        .connection()
        .expect("connection")
        .execute(
            "INSERT INTO media_gc_candidates VALUES (?1,12)",
            [proof.destination_asset_id.to_string()],
        )
        .expect("queue imported asset");
    assert!(
        database
            .collect_media_garbage(TimestampMillis::new(12))
            .expect("shared collection")
            .is_empty()
    );
    assert_eq!(
        MediaAssetRepository::get(&database, proof.destination_asset_id).expect("removed asset"),
        None
    );
    assert!(
        MediaAssetRepository::get(&database, sibling)
            .expect("sibling")
            .is_some()
    );
    assert_eq!(
        MediaBlobRepository::get(&database, proof.blob_id)
            .expect("blob")
            .expect("shared blob")
            .state,
        BlobState::Ready
    );
    MediaAssetRepository::update_retention(
        &database,
        sibling,
        Revision::INITIAL,
        RetentionClass::Persistent,
        TimestampMillis::new(13),
    )
    .expect("drop library retention");
    database
        .connection()
        .expect("connection")
        .execute(
            "INSERT INTO media_gc_candidates VALUES (?1,13)",
            [sibling.to_string()],
        )
        .expect("queue sibling");
    let released = database
        .collect_media_garbage(TimestampMillis::new(13))
        .expect("last asset collection");
    assert_eq!(released.len(), 1);
    assert_eq!(released[0].content_hash, proof.content_hash);
    assert_eq!(
        MediaBlobRepository::get(&database, proof.blob_id).expect("blob"),
        None
    );
    let proof_count: i64 = database
        .connection()
        .expect("connection")
        .query_row(
            "SELECT count(*) FROM legacy_import_media_completions",
            [],
            |row| row.get(0),
        )
        .expect("proof count");
    assert_eq!(proof_count, 1);
}

#[test]
fn crash_before_media_proof_restore_commit_leaves_no_partial_evidence_and_retry_restores() {
    use lettuce_transfer::{ProviderBackupRestoreWriter, ProviderBackupSource};
    let root = std::env::temp_dir().join(format!(
        "lettuce-media-proof-crash-{}",
        lettuce_types::LegacyImportRunId::new()
    ));
    std::fs::create_dir_all(&root).expect("root");
    let source = Database::open_in_memory().expect("source");
    imported_asset_proof(&source);
    let mut graph = source.read_provider_backup_graph().expect("source graph");
    graph.authored.media_assets.clear();
    graph.authored.media_blobs.clear();
    std::fs::write(
        root.join("graph.json"),
        serde_json::to_vec(&graph.legacy_imports).expect("serialize proofs"),
    )
    .expect("proof fixture");
    let status = std::process::Command::new(std::env::current_exe().expect("test executable"))
        .args([
            "--exact",
            "purge::media_gc::tests::media_proof_restore_crash_child",
            "--nocapture",
        ])
        .env("LETTUCE_MEDIA_PROOF_CRASH_ROOT", &root)
        .status()
        .expect("crash child");
    assert_eq!(status.code(), Some(77));
    let target = Database::open(root.join("target.sqlite3")).expect("reopen after crash");
    let empty = target
        .read_provider_backup_graph()
        .expect("rolled back graph");
    assert!(empty.legacy_imports.runs.is_empty());
    let allowed: bool = target
        .connection()
        .expect("connection")
        .query_row("SELECT legacy_media_proof_restore_allowed()", [], |row| {
            row.get(0)
        })
        .expect("guard");
    assert!(!allowed);
    target
        .restore_provider_backup_graph(&graph, &[])
        .expect("retry restore");
    assert_eq!(
        target
            .read_provider_backup_graph()
            .expect("restored graph")
            .legacy_imports,
        graph.legacy_imports
    );
    let allowed: bool = target
        .connection()
        .expect("connection")
        .query_row("SELECT legacy_media_proof_restore_allowed()", [], |row| {
            row.get(0)
        })
        .expect("guard after success");
    assert!(!allowed);
    drop(target);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn media_proof_restore_crash_child() {
    let Some(root) = std::env::var_os("LETTUCE_MEDIA_PROOF_CRASH_ROOT") else {
        return;
    };
    let root = std::path::PathBuf::from(root);
    let proofs: lettuce_transfer::LegacyImportBackup =
        serde_json::from_slice(&std::fs::read(root.join("graph.json")).expect("proof bytes"))
            .expect("proofs");
    let database = Database::open(root.join("target.sqlite3")).expect("target");
    let mut connection = database.connection().expect("connection");
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .expect("transaction");
    crate::legacy::legacy_import_backup_adapter::insert_restored_in(
        &transaction,
        &proofs,
        &database.legacy_media_proof_restore_allowed,
    )
    .expect("proof insertion");
    let count: i64 = transaction
        .query_row(
            "SELECT count(*) FROM legacy_import_media_completions",
            [],
            |row| row.get(0),
        )
        .expect("uncommitted proof");
    assert_eq!(count, 1);
    std::process::exit(77);
}

#[test]
fn failed_media_proof_restore_rolls_back_and_revokes_its_private_permission() {
    use lettuce_transfer::{ProviderBackupRestoreWriter, ProviderBackupSource};
    let source = Database::open_in_memory().expect("source");
    imported_asset_proof(&source);
    let mut graph = source.read_provider_backup_graph().expect("source graph");
    graph.authored.media_assets.clear();
    graph.authored.media_blobs.clear();
    let target = Database::open_in_memory().expect("target");
    target.connection().expect("connection").execute_batch("CREATE TEMP TRIGGER refuse_test_media_proof BEFORE INSERT ON legacy_import_media_completions BEGIN SELECT RAISE(ABORT,'injected proof write failure'); END;").expect("inject failure");
    assert!(target.restore_provider_backup_graph(&graph, &[]).is_err());
    let allowed: bool = target
        .connection()
        .expect("connection")
        .query_row("SELECT legacy_media_proof_restore_allowed()", [], |row| {
            row.get(0)
        })
        .expect("guard after failure");
    assert!(!allowed);
    assert!(
        target
            .read_provider_backup_graph()
            .expect("rolled back graph")
            .legacy_imports
            .runs
            .is_empty()
    );
    target
        .connection()
        .expect("connection")
        .execute_batch("DROP TRIGGER refuse_test_media_proof")
        .expect("remove injection");
    target
        .restore_provider_backup_graph(&graph, &[])
        .expect("retry restore");
    assert_eq!(
        target
            .read_provider_backup_graph()
            .expect("restored graph")
            .legacy_imports,
        graph.legacy_imports
    );
}

#[test]
fn storage_media_snapshot_is_read_only_and_has_one_entry_per_blob() {
    let root = std::env::temp_dir().join(format!(
        "media-size-snapshot-{}",
        lettuce_types::OperationId::new()
    ));
    std::fs::create_dir(&root).expect("root");
    let path = root.join("kept.db");
    let database = Database::open(&path).expect("database");
    let (_, hash) = asset(&database, 'a', RetentionClass::Library);
    let rows = Database::media_storage_objects_in_file(&path).expect("sizes");
    assert_eq!(rows, vec![(hash, "image".into(), 42)]);
    let migrations: i64 = database
        .connection()
        .expect("connection")
        .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .expect("migrations");
    assert!(migrations > 0);
    assert!(Database::media_storage_objects_in_file(&root.join("missing.db")).is_err());
    assert!(!root.join("missing.db").exists());
}
