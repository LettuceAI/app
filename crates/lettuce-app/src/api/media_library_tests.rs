use std::sync::Arc;

use lettuce_contracts::{self as dto, ApiErrorCode};
use lettuce_media::{MediaAssetRepository, MediaBlobRepository};
use lettuce_platform::{DirectorySnapshot, FilesystemAuthority, ManagedRoot};
use lettuce_transfer::{
    BackupSqlRow, BackupSqlValue, LegacyImportRepository, ProviderBackupRestoreWriter,
    ProviderBackupSource,
};
use lettuce_types::{ContentHash, LegacyImportRunId, RequestId, TimestampMillis};

fn row(values: Vec<(&str, BackupSqlValue)>) -> BackupSqlRow {
    values
        .into_iter()
        .map(|(key, value)| (key.into(), value))
        .collect()
}

fn harness(root: &std::path::Path) -> super::tests::Harness {
    let authority = FilesystemAuthority::new(&DirectorySnapshot::new(root).expect("snapshot"))
        .expect("authority");
    let location = crate::AppDatabaseLocation::new(root.join("private-persistent-v2"), &authority)
        .expect("location");
    let active = location.active_path().expect("active");
    let backend =
        Arc::new(crate::AppBackend::open(&active, TimestampMillis::new(10)).expect("backend"));
    let media = Arc::new(super::ApiMediaStore::new(
        authority.managed_files(),
        authority
            .read_capability(ManagedRoot::MediaBlobs)
            .expect("read"),
        authority
            .write_capability(ManagedRoot::MediaBlobs)
            .expect("write"),
        lettuce_database::Database::open(&active).expect("blobs"),
        lettuce_database::Database::open(&active).expect("assets"),
    ));
    super::tests::harness_over_files(
        backend,
        super::tests::Reply::Text("ok"),
        Arc::new(lettuce_jobs::SystemClock),
        Some(media),
        Some(root.to_path_buf()),
        Arc::new(super::NoModels),
        Arc::new(super::tests::NoImages),
        Some(super::ApiDatabaseFiles { location, active }),
    )
}

#[tokio::test]
async fn media_api_removal_after_commit_crash_child() {
    let Ok(root) = std::env::var("LETTUCE_MEDIA_API_REMOVE_CRASH_ROOT") else {
        return;
    };
    let request: dto::MediaLibraryRemoveRequest = serde_json::from_str(
        &std::env::var("LETTUCE_MEDIA_API_REMOVE_CRASH_REQUEST").expect("request"),
    )
    .expect("typed request");
    let h = harness(std::path::Path::new(&root));
    super::media_library::media_library_remove_with_checkpoint(&h.context, request, || {
        std::process::exit(77)
    })
    .await
    .expect("remove");
    panic!("child should exit after catalog commit and before object deletion");
}

#[tokio::test]
async fn crash_after_media_removal_commit_replays_without_deleting_a_new_reference() {
    let root = std::env::temp_dir().join(format!(
        "lettuce-media-api-remove-crash-{}",
        RequestId::new()
    ));
    let h = harness(&root);
    let bytes = super::tests::png_bytes();
    let object = h
        .context
        .media()
        .expect("media")
        .ingest(
            bytes.as_slice(),
            lettuce_media::IngestRequest::new(
                lettuce_media::AssetKind::OtherImage,
                lettuce_media::AssetOrigin::Upload,
                lettuce_media::RetentionClass::Library,
                lettuce_media::AssetProvenanceV1::default(),
            ),
        )
        .expect("asset");
    let request = dto::MediaLibraryRemoveRequest {
        asset_id: object.asset.id.to_string(),
        client_operation_id: RequestId::new().to_string(),
    };
    drop(h);
    let status = std::process::Command::new(std::env::current_exe().expect("test executable"))
        .args([
            "--exact",
            "api::media_library_tests::media_api_removal_after_commit_crash_child",
            "--nocapture",
        ])
        .env("LETTUCE_MEDIA_API_REMOVE_CRASH_ROOT", &root)
        .env(
            "LETTUCE_MEDIA_API_REMOVE_CRASH_REQUEST",
            serde_json::to_string(&request).expect("request JSON"),
        )
        .status()
        .expect("child");
    assert_eq!(status.code(), Some(77));
    let h = harness(&root);
    assert!(
        MediaAssetRepository::get(h.context.backend().database(), object.asset.id)
            .expect("removed catalog asset")
            .is_none()
    );
    assert!(
        h.context
            .backend()
            .database()
            .lookup_api_operation("media_library_remove", &request.client_operation_id)
            .expect("receipt")
            .is_some()
    );
    let hash = object.blob.content_hash;
    let path = root
        .join("platform-v2/media-blobs/objects")
        .join(&hash.as_str()[..2])
        .join(&hash.as_str()[2..4])
        .join(hash.as_str());
    assert_eq!(std::fs::read(&path).expect("object survives crash"), bytes);
    let replacement = h
        .context
        .media()
        .expect("media")
        .ingest(
            bytes.as_slice(),
            lettuce_media::IngestRequest::new(
                lettuce_media::AssetKind::OtherImage,
                lettuce_media::AssetOrigin::Upload,
                lettuce_media::RetentionClass::Library,
                lettuce_media::AssetProvenanceV1::default(),
            ),
        )
        .expect("new reference");
    super::media_library_remove(&h.context, request.clone())
        .await
        .expect("replay after crash");
    h.context
        .media()
        .expect("media")
        .open_ready(replacement.asset.id)
        .expect("new reference still reads");
    assert_eq!(std::fs::read(&path).expect("preserved object"), bytes);
    let error = super::media_library_remove(
        &h.context,
        dto::MediaLibraryRemoveRequest {
            asset_id: replacement.asset.id.to_string(),
            ..request
        },
    )
    .await
    .expect_err("changed digest conflicts");
    assert_eq!(error.code, ApiErrorCode::Conflict);
    h.context
        .media()
        .expect("media")
        .open_ready(replacement.asset.id)
        .expect("conflict preserves replacement");
    drop(h);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn gc_racing_a_new_reference_keeps_the_object_in_a_kept_set() {
    use lettuce_characters::{
        CharacterMediaLink, CharacterMediaSlot, CharacterRepository, RepositoryError,
        ValidationError,
    };
    let root = std::env::temp_dir().join(format!(
        "lettuce-kept-media-reference-race-{}",
        RequestId::new()
    ));
    let original = harness(&root);
    let bytes = super::tests::png_bytes();
    let kept_asset = original
        .context
        .media()
        .expect("media")
        .ingest(
            bytes.as_slice(),
            lettuce_media::IngestRequest::new(
                lettuce_media::AssetKind::OtherImage,
                lettuce_media::AssetOrigin::Upload,
                lettuce_media::RetentionClass::Library,
                lettuce_media::AssetProvenanceV1::default(),
            ),
        )
        .expect("kept asset");
    let source = original.context.database_files().expect("files");
    let files = super::ApiDatabaseFiles {
        location: source.location.clone(),
        active: source.active.clone(),
    };
    drop(original);
    let guard = files.location.try_file_lifecycle().expect("lifecycle");
    let name = format!("{}.sqlite3", lettuce_types::OperationId::new());
    let active = guard
        .begin_file(
            &name,
            crate::DatabaseFileKind::Restore,
            TimestampMillis::new(20),
        )
        .expect("new file");
    drop(lettuce_database::Database::open(&active).expect("new database"));
    guard
        .activate_file(&name, TimestampMillis::new(20))
        .expect("keep old file");
    assert!(
        guard
            .kept_media_hashes()
            .expect("kept set")
            .contains(&kept_asset.blob.content_hash)
    );
    drop(guard);
    let h = harness(&root);
    let active_asset = h
        .context
        .media()
        .expect("media")
        .ingest_with_id(
            kept_asset.asset.id,
            bytes.as_slice(),
            lettuce_media::IngestRequest::new(
                lettuce_media::AssetKind::OtherImage,
                lettuce_media::AssetOrigin::Upload,
                lettuce_media::RetentionClass::Temporary {
                    expires_at: TimestampMillis::new(11),
                },
                lettuce_media::AssetProvenanceV1::default(),
            ),
        )
        .expect("same asset in active database");
    let id = active_asset.asset.id;
    let character = h.character_id;
    let source = h.context.database_files().expect("files");
    let files = super::ApiDatabaseFiles {
        location: source.location.clone(),
        active: source.active.clone(),
    };
    let writer = lettuce_database::Database::open(&files.active).expect("reference writer");
    let barrier = Arc::new(std::sync::Barrier::new(3));
    let reference_barrier = barrier.clone();
    let reference = std::thread::spawn(move || {
        reference_barrier.wait();
        writer.attach_media(
            character,
            lettuce_types::Revision::INITIAL,
            CharacterMediaLink {
                asset_id: id,
                slot: CharacterMediaSlot::DesignReference,
                ordinal: 0,
            },
            TimestampMillis::new(12),
        )
    });
    let context = h.context.clone();
    let gc_barrier = barrier.clone();
    let collector = std::thread::spawn(move || {
        gc_barrier.wait();
        let scope = crate::MediaGarbageScope {
            store: context.media().expect("media"),
            location: &files.location,
            open_database: &files.active,
        };
        crate::collect_media_garbage(
            context.backend().database(),
            &scope,
            TimestampMillis::new(12),
        )
    });
    barrier.wait();
    let added = reference.join().expect("reference worker");
    let collected = collector.join().expect("GC worker").expect("collection");
    assert_eq!(collected.removed, 0);
    assert_eq!(collected.failed, 0);
    match added {
        Ok(_) => assert!(
            MediaAssetRepository::get(h.context.backend().database(), id)
                .expect("referenced asset")
                .is_some()
        ),
        Err(RepositoryError::Invalid(ValidationError::InvalidReference { .. })) => assert!(
            MediaAssetRepository::get(h.context.backend().database(), id)
                .expect("collected active asset")
                .is_none()
        ),
        error => panic!("unexpected reference race result: {error:?}"),
    }
    let hash = kept_asset.blob.content_hash;
    let path = root
        .join("platform-v2/media-blobs/objects")
        .join(&hash.as_str()[..2])
        .join(&hash.as_str()[2..4])
        .join(hash.as_str());
    assert_eq!(
        std::fs::read(path).expect("kept object survives the race"),
        bytes
    );
    assert!(
        h.context
            .database_files()
            .expect("files")
            .location
            .try_file_lifecycle()
            .expect("lifecycle")
            .kept_media_hashes()
            .expect("kept set after race")
            .contains(&hash)
    );
    drop(h);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test]
async fn partial_import_missing_a_stage_keeps_media_through_gc_and_api_remove_until_completion() {
    use BackupSqlValue::{Integer, Null, Text};
    let root = std::env::temp_dir().join(format!("lettuce-partial-media-api-{}", RequestId::new()));
    let authority = FilesystemAuthority::new(&DirectorySnapshot::new(&root).expect("snapshot"))
        .expect("authority");
    let location = crate::AppDatabaseLocation::new(root.join("private-persistent-v2"), &authority)
        .expect("location");
    let active = location.active_path().expect("active");
    let source_path = root.join("source.sqlite3");
    let source = lettuce_database::Database::open(&source_path).expect("source");
    let source_media = super::ApiMediaStore::new(
        authority.managed_files(),
        authority
            .read_capability(ManagedRoot::MediaBlobs)
            .expect("read"),
        authority
            .write_capability(ManagedRoot::MediaBlobs)
            .expect("write"),
        lettuce_database::Database::open(&source_path).expect("blobs"),
        lettuce_database::Database::open(&source_path).expect("assets"),
    );
    let object = source_media
        .ingest(
            super::tests::png_bytes().as_slice(),
            lettuce_media::IngestRequest::new(
                lettuce_media::AssetKind::OtherImage,
                lettuce_media::AssetOrigin::Legacy,
                lettuce_media::RetentionClass::Persistent,
                lettuce_media::AssetProvenanceV1::default(),
            ),
        )
        .expect("pending media");
    let run = LegacyImportRunId::new();
    let fingerprint = "c".repeat(64);
    let mut graph = source.read_provider_backup_graph().expect("graph");
    let retained_asset = graph
        .authored
        .media_assets
        .iter_mut()
        .find(|asset| asset.id == object.asset.id)
        .expect("asset in fixture");
    retained_asset.retention = lettuce_media::RetentionClass::Temporary {
        expires_at: TimestampMillis::new(11),
    };
    let binding = row(vec![
        ("run_id", Text(run.to_string())),
        ("plan_fingerprint", Text(fingerprint.clone())),
        ("completed_at", Integer(10)),
    ]);
    let mut results = binding.clone();
    for key in ["persona_count", "lorebook_count", "lorebook_entry_count"] {
        results.insert(key.into(), Integer(0));
    }
    let mut providers = binding;
    for key in [
        "provider_account_count",
        "model_profile_count",
        "prompt_count",
    ] {
        providers.insert(key.into(), Integer(0));
    }
    let stage_results = [
        "groups",
        "audio",
        "settings",
        "direct_conversations",
        "group_conversations",
        "usage_records",
        "creation_helper",
        "images",
        "llm_metrics",
    ]
    .into_iter()
    .map(|stage| {
        row(vec![
            ("run_id", Text(run.to_string())),
            ("stage", Text(stage.into())),
            ("record_count", Integer(0)),
            ("completed_at", Integer(10)),
        ])
    })
    .collect();
    graph
        .legacy_imports
        .runs
        .push(lettuce_transfer::BackupLegacyImportRun {
            run: row(vec![
                ("id", Text(run.to_string())),
                ("source_schema_version", Integer(96)),
                ("inventory_fingerprint", Text(fingerprint.clone())),
                ("plan_fingerprint", Text(fingerprint.clone())),
                ("source_fingerprint", Text(fingerprint.clone())),
                ("status", Text("partial".into())),
                ("admitted_at", Integer(10)),
                ("updated_at", Integer(10)),
            ]),
            assignments: vec![row(vec![
                ("run_id", Text(run.to_string())),
                ("source_kind", Text("media".into())),
                ("source_key", Text("images/pending.png".into())),
                ("source_detail", Text(String::new())),
                ("destination_id", Text(object.asset.id.to_string())),
                ("auxiliary_id", Null),
                (
                    "expected_byte_len",
                    Integer(i64::try_from(object.blob.byte_size).expect("size")),
                ),
                (
                    "expected_content_hash",
                    Text(object.blob.content_hash.to_string()),
                ),
            ])],
            skips: vec![],
            secret_completions: vec![],
            media_completions: vec![row(vec![
                ("run_id", Text(run.to_string())),
                ("relative_path", Text("images/pending.png".into())),
                ("destination_asset_id", Text(object.asset.id.to_string())),
                ("blob_id", Text(object.blob.id.to_string())),
                (
                    "byte_len",
                    Integer(i64::try_from(object.blob.byte_size).expect("size")),
                ),
                ("content_hash", Text(object.blob.content_hash.to_string())),
                ("completed_at", Integer(10)),
            ])],
            stage_results,
            results: Some(results),
            provider_model_results: Some(providers),
            asr_results: None,
            preserved_rows: vec![],
        });
    drop(source_media);
    drop(source);
    let target = lettuce_database::Database::open(&active).expect("target");
    target
        .restore_provider_backup_graph(&graph, &[])
        .expect("restore fixture");
    drop(target);
    let backend =
        Arc::new(crate::AppBackend::open(&active, TimestampMillis::new(10)).expect("backend"));
    let media = Arc::new(super::ApiMediaStore::new(
        authority.managed_files(),
        authority
            .read_capability(ManagedRoot::MediaBlobs)
            .expect("read"),
        authority
            .write_capability(ManagedRoot::MediaBlobs)
            .expect("write"),
        lettuce_database::Database::open(&active).expect("blobs"),
        lettuce_database::Database::open(&active).expect("assets"),
    ));
    let h = super::tests::harness_over_files(
        backend,
        super::tests::Reply::Text("ok"),
        Arc::new(lettuce_jobs::SystemClock),
        Some(media),
        Some(root.clone()),
        Arc::new(super::NoModels),
        Arc::new(super::tests::NoImages),
        Some(super::ApiDatabaseFiles {
            location,
            active: active.clone(),
        }),
    );
    assert!(
        MediaAssetRepository::get(h.context.backend().database(), object.asset.id)
            .expect("pending asset")
            .is_some()
    );
    assert!(
        h.context
            .backend()
            .database()
            .collect_media_garbage(TimestampMillis::new(12))
            .expect("GC pass")
            .is_empty()
    );
    let request = dto::MediaLibraryRemoveRequest {
        asset_id: object.asset.id.to_string(),
        client_operation_id: RequestId::new().to_string(),
    };
    let error = super::media_library_remove(&h.context, request.clone())
        .await
        .expect_err("pending import is in use");
    assert_eq!(error.code, ApiErrorCode::InUse);
    let Some(dto::ApiErrorDetails::MediaInUse { references, .. }) = error.details else {
        panic!("missing references");
    };
    assert!(
        references
            .iter()
            .any(|reference| reference.owner_id == run.to_string())
    );
    assert!(
        MediaAssetRepository::get(h.context.backend().database(), object.asset.id)
            .expect("preserved asset")
            .is_some()
    );
    let page = super::media_library_list(
        &h.context,
        dto::MediaLibraryListRequest {
            kind: Some(dto::MediaLibraryRole::Image),
            cursor: None,
            limit: Some(1),
        },
    )
    .await
    .expect("library includes temporary media");
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].asset, h.context.asset_ref(object.asset.id));
    assert_eq!(
        page.items[0].retention,
        dto::MediaLibraryRetention::Temporary
    );
    assert!(
        page.items[0]
            .references
            .iter()
            .any(|reference| reference.owner_id == run.to_string())
    );
    let fingerprint = ContentHash::parse(fingerprint).expect("fingerprint");
    h.context
        .backend()
        .database()
        .materialize_characters(lettuce_transfer::LegacyCharacterMaterializationRequest {
            run_id: run,
            plan_fingerprint: fingerprint.clone(),
            source_fingerprint: fingerprint,
            media: lettuce_transfer::LegacyMediaPlan {
                media: vec![lettuce_transfer::LegacyMediaCandidate {
                    relative_path: "images/pending.png".into(),
                    source_locator: "images/pending.png".into(),
                    byte_len: object.blob.byte_size,
                    content_hash: object.blob.content_hash.clone(),
                    uses: vec![],
                }],
                total_bytes: object.blob.byte_size,
                skipped: vec![],
            },
            characters: vec![],
            character_lorebooks: vec![],
            completed_at: TimestampMillis::new(13),
        })
        .expect("last stage");
    h.context
        .backend()
        .database()
        .complete_legacy_import_run(run, TimestampMillis::new(13))
        .expect("complete run");
    super::media_library_remove(&h.context, request.clone())
        .await
        .expect("released after completion");
    super::media_library_remove(&h.context, request.clone())
        .await
        .expect("replay");
    assert!(
        MediaAssetRepository::get(h.context.backend().database(), object.asset.id)
            .expect("removed asset")
            .is_none()
    );
    assert!(
        MediaBlobRepository::get(h.context.backend().database(), object.blob.id)
            .expect("removed blob")
            .is_none()
    );
    let proof = h
        .context
        .backend()
        .database()
        .get_media_completion(run, "images/pending.png")
        .expect("proof")
        .expect("immutable proof survives removal");
    assert_eq!(proof.destination_asset_id, object.asset.id);
    assert_eq!(proof.blob_id, object.blob.id);
    assert_eq!(proof.content_hash, object.blob.content_hash);
    let hash = object.blob.content_hash;
    let path = root
        .join("platform-v2/media-blobs/objects")
        .join(&hash.as_str()[..2])
        .join(&hash.as_str()[2..4])
        .join(hash.as_str());
    assert!(!path.exists());
    let replacement = h
        .context
        .media()
        .expect("media")
        .ingest(
            super::tests::png_bytes().as_slice(),
            lettuce_media::IngestRequest::new(
                lettuce_media::AssetKind::OtherImage,
                lettuce_media::AssetOrigin::Upload,
                lettuce_media::RetentionClass::Library,
                lettuce_media::AssetProvenanceV1::default(),
            ),
        )
        .expect("new reference to the same content");
    assert_eq!(replacement.blob.content_hash, hash);
    super::media_library_remove(&h.context, request)
        .await
        .expect("old receipt replays after new ingest");
    h.context
        .media()
        .expect("media")
        .open_ready(replacement.asset.id)
        .expect("new asset still has its object after old removal replay");
    assert!(path.exists());
    drop(h);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test]
async fn missing_library_object_fails_typed_and_failed_unlink_replays() {
    let root = std::env::temp_dir().join(format!(
        "lettuce-media-library-failures-{}",
        RequestId::new()
    ));
    let h = harness(&root);
    let bytes = super::tests::png_bytes();
    let object = h
        .context
        .media()
        .expect("media")
        .ingest(
            bytes.as_slice(),
            lettuce_media::IngestRequest::new(
                lettuce_media::AssetKind::OtherImage,
                lettuce_media::AssetOrigin::Upload,
                lettuce_media::RetentionClass::Library,
                lettuce_media::AssetProvenanceV1::default(),
            ),
        )
        .expect("asset");
    let hash = object.blob.content_hash;
    let path = root
        .join("platform-v2/media-blobs/objects")
        .join(&hash.as_str()[..2])
        .join(&hash.as_str()[2..4])
        .join(hash.as_str());
    std::fs::remove_file(&path).expect("missing object fixture");
    let error = super::media_library_list(
        &h.context,
        dto::MediaLibraryListRequest {
            kind: None,
            cursor: None,
            limit: None,
        },
    )
    .await
    .expect_err("missing required object");
    assert_eq!(error.code, ApiErrorCode::NotFound);
    assert_eq!(
        error.details,
        Some(dto::ApiErrorDetails::Media {
            asset_id: Some(object.asset.id.to_string()),
            reason: dto::MediaFailureReason::ObjectMissing
        })
    );
    std::fs::create_dir(&path).expect("unlink failure fixture");
    let request = dto::MediaLibraryRemoveRequest {
        asset_id: object.asset.id.to_string(),
        client_operation_id: RequestId::new().to_string(),
    };
    let error = super::media_library_remove(&h.context, request.clone())
        .await
        .expect_err("physical deletion failure");
    assert_eq!(error.code, ApiErrorCode::Internal);
    assert_eq!(
        error.details,
        Some(dto::ApiErrorDetails::Media {
            asset_id: Some(object.asset.id.to_string()),
            reason: dto::MediaFailureReason::Storage
        })
    );
    assert!(
        h.context
            .backend()
            .database()
            .lookup_api_operation("media_library_remove", &request.client_operation_id)
            .expect("receipt")
            .is_some()
    );
    assert!(
        MediaAssetRepository::get(h.context.backend().database(), object.asset.id)
            .expect("catalog")
            .is_none()
    );
    std::fs::remove_dir(&path).expect("fix unlink fixture");
    std::fs::write(&path, &bytes).expect("restore orphan bytes");
    super::media_library_remove(&h.context, request)
        .await
        .expect("retry finishes physical deletion");
    assert!(!path.exists());
    drop(h);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn a_writer_remaining_open_after_restore_cannot_lose_its_new_media_to_active_sweep() {
    let root = std::env::temp_dir().join(format!("lettuce-kept-open-writer-{}", RequestId::new()));
    let old = harness(&root);
    let original_bytes = super::tests::png_bytes();
    let original_asset = old
        .context
        .media()
        .expect("old media")
        .ingest(
            original_bytes.as_slice(),
            lettuce_media::IngestRequest::new(
                lettuce_media::AssetKind::OtherImage,
                lettuce_media::AssetOrigin::Upload,
                lettuce_media::RetentionClass::Library,
                lettuce_media::AssetProvenanceV1::default(),
            ),
        )
        .expect("accepted before cutover");
    let files = old.context.database_files().expect("files");
    let guard = files.location.try_file_lifecycle().expect("lifecycle");
    let name = format!("{}.sqlite3", lettuce_types::OperationId::new());
    let next = guard
        .begin_file(
            &name,
            crate::DatabaseFileKind::Restore,
            TimestampMillis::new(20),
        )
        .expect("new file");
    drop(lettuce_database::Database::open(&next).expect("fresh database"));
    guard
        .activate_file(&name, TimestampMillis::new(20))
        .expect("cutover while prior writer remains open");
    drop(guard);
    let mut bytes = super::tests::png_bytes();
    bytes.extend_from_slice(b"late-media-after-cutover");
    let late = old.context.media().expect("old media writer").ingest(
        bytes.as_slice(),
        lettuce_media::IngestRequest::new(
            lettuce_media::AssetKind::OtherImage,
            lettuce_media::AssetOrigin::Upload,
            lettuce_media::RetentionClass::Library,
            lettuce_media::AssetProvenanceV1::default(),
        ),
    );
    assert!(
        matches!(late, Err(lettuce_media::MediaStoreError::CatalogFailure)),
        "kept database refuses new writes: {late:?}"
    );
    let active = harness(&root);
    let current = active.context.database_files().expect("current files");
    let scope = crate::MediaGarbageScope {
        store: active.context.media().expect("active media"),
        location: &current.location,
        open_database: &current.active,
    };
    crate::sweep_orphan_media_files(
        active.context.backend().database(),
        &scope,
        TimestampMillis::new(30),
    )
    .expect("active orphan sweep");
    let readable = old
        .context
        .media()
        .expect("old media")
        .open_ready(original_asset.asset.id);
    assert!(
        readable.is_ok(),
        "a kept database still references these bytes: {:?}",
        readable.as_ref().err()
    );
    drop(readable);
    drop(active);
    drop(old);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn a_stale_database_scope_cannot_sweep_new_active_catalog_media() {
    let root = std::env::temp_dir().join(format!("lettuce-stale-media-sweep-{}", RequestId::new()));
    let old = harness(&root);
    let files = old.context.database_files().expect("files");
    let guard = files.location.try_file_lifecycle().expect("lifecycle");
    let name = format!("{}.sqlite3", lettuce_types::OperationId::new());
    let next = guard
        .begin_file(
            &name,
            crate::DatabaseFileKind::Restore,
            TimestampMillis::new(20),
        )
        .expect("new file");
    drop(lettuce_database::Database::open(&next).expect("fresh database"));
    guard
        .activate_file(&name, TimestampMillis::new(20))
        .expect("cutover");
    drop(guard);
    let current = harness(&root);
    let bytes = super::tests::png_bytes();
    let pending = current
        .context
        .media()
        .expect("current media")
        .ingest(
            bytes.as_slice(),
            lettuce_media::IngestRequest::new(
                lettuce_media::AssetKind::OtherImage,
                lettuce_media::AssetOrigin::Upload,
                lettuce_media::RetentionClass::Persistent,
                lettuce_media::AssetProvenanceV1::default(),
            ),
        )
        .expect("new catalog asset before its reference");
    let scope = crate::MediaGarbageScope {
        store: old.context.media().expect("old media"),
        location: &files.location,
        open_database: &files.active,
    };
    let result = crate::sweep_orphan_media_files(
        old.context.backend().database(),
        &scope,
        TimestampMillis::new(30),
    );
    assert!(
        matches!(
            result,
            Err(crate::HardDeleteError::DatabaseFiles(
                crate::AppDatabaseLocationError::Conflict
            ))
        ),
        "stale sweep must be refused: {result:?}"
    );
    current
        .context
        .media()
        .expect("current media")
        .open_ready(pending.asset.id)
        .expect("pending active asset still reads");
    drop(current);
    drop(old);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test]
async fn workers_on_a_kept_database_idle_without_claiming_or_a_due_timer() {
    use super::worker::WorkerStep;
    use futures_util::FutureExt;
    use lettuce_conversations::ConversationReader;
    use lettuce_jobs::{JobKind, JobSpec, JobStore, JobSubject, OutcomeRef, SubjectKind};
    use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};

    struct DeferredHandler {
        due: Arc<AtomicI64>,
        claims: Arc<AtomicUsize>,
    }
    #[async_trait::async_trait]
    impl super::JobHandler for DeferredHandler {
        fn kinds(&self) -> &[JobKind] {
            &[JobKind::Maintenance]
        }
        fn lane(
            &self,
            _: &super::ApiContext,
            _: &lettuce_jobs::JobSnapshot,
        ) -> Option<super::JobLane> {
            Some(super::JobLane("fence-test".into()))
        }
        fn not_before(
            &self,
            _: &super::ApiContext,
            _: &lettuce_jobs::JobSnapshot,
        ) -> Option<TimestampMillis> {
            Some(TimestampMillis::new(self.due.load(Ordering::SeqCst)))
        }
        async fn claim(
            &self,
            _: &super::ApiContext,
            _: &lettuce_jobs::JobSnapshot,
            _: lettuce_jobs::WorkerId,
        ) -> Result<Option<Box<dyn super::ClaimedJob>>, dto::ApiError> {
            self.claims.fetch_add(1, Ordering::SeqCst);
            Err(super::error::api_error(
                ApiErrorCode::Unavailable,
                "unexpected claim on a kept database",
            ))
        }
    }
    let root = std::env::temp_dir().join(format!("lettuce-kept-idle-workers-{}", RequestId::new()));
    let h = harness(&root);
    let conversation = super::tests::launch(&h, &RequestId::new().to_string()).await;
    super::tests::send(
        &h,
        &conversation,
        &RequestId::new().to_string(),
        "Hello",
        Arc::new(super::tests::RecordingStream::default()),
    )
    .await
    .expect("queued generation");
    let conversation_id = conversation.parse().expect("conversation id");
    let branch = ConversationReader::get(h.context.backend().database(), conversation_id)
        .expect("conversation")
        .conversation
        .active_branch_id;
    h.context.memory_work().enqueue(conversation_id, branch);
    h.context
        .backend()
        .database()
        .create_or_get(
            JobSpec::new(
                JobKind::Maintenance,
                JobSubject::new(SubjectKind::Maintenance, "fenced-work").expect("subject"),
                OutcomeRef::ArtifactInstallation(lettuce_types::AssetId::new()),
            )
            .with_resources(vec![lettuce_jobs::ResourceClass::Cpu]),
        )
        .expect("maintenance job");
    let due = Arc::new(AtomicI64::new(h.context.clock().now().get() + 60_000));
    let claims = Arc::new(AtomicUsize::new(0));
    let jobs = super::JobRunner::new(
        h.context.clone(),
        super::JobHandlers::new(vec![Arc::new(DeferredHandler {
            due: due.clone(),
            claims: claims.clone(),
        })]),
    );
    assert!(!jobs.run_once().await.expect("not due yet"));
    let files = h.context.database_files().expect("files");
    let guard = files.location.try_file_lifecycle().expect("lifecycle");
    let name = format!("{}.sqlite3", lettuce_types::OperationId::new());
    let next = guard
        .begin_file(
            &name,
            crate::DatabaseFileKind::Restore,
            TimestampMillis::new(20),
        )
        .expect("new file");
    drop(lettuce_database::Database::open(&next).expect("fresh database"));
    guard
        .activate_file(&name, TimestampMillis::new(20))
        .expect("cutover");
    drop(guard);
    due.store(1, Ordering::SeqCst);
    assert!(
        !super::ConversationGenerationWorker::new(h.context.clone())
            .run_once()
            .await
            .expect("kept generation worker idles")
    );
    assert!(!jobs.run_once().await.expect("kept job worker idles"));
    assert_eq!(claims.load(Ordering::SeqCst), 0);
    assert!(
        WorkerStep::due(&jobs).now_or_never().is_none(),
        "kept job worker clears its scheduled wake-up"
    );
    let memory = super::memory_worker::MemoryWorker::new(h.context.clone());
    assert!(
        !WorkerStep::step(&memory)
            .await
            .expect("kept memory worker idles")
    );
    drop((memory, jobs, h));
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test]
async fn an_unreadable_stray_database_file_blocks_only_media_collection() {
    let root = std::env::temp_dir().join(format!("lettuce-stray-database-{}", RequestId::new()));
    let h = harness(&root);
    let databases = root.join("private-persistent-v2/databases");
    std::fs::write(databases.join("stray.sqlite3"), b"").expect("stray file");
    let files = h.context.database_files().expect("files");
    drop(
        files
            .location
            .try_file_lifecycle()
            .expect("a stray file never blocks startup"),
    );
    let listed = super::storage_database_files_list(&h.context)
        .await
        .expect("listing");
    let stray = listed
        .iter()
        .find(|file| file.file == "stray.sqlite3")
        .expect("stray listed");
    assert_eq!(stray.error, Some(dto::DatabaseFileError::Unreadable));
    assert!(stray.deletable && !stray.active);
    let bytes = super::tests::png_bytes();
    let object = h
        .context
        .media()
        .expect("media")
        .ingest(
            bytes.as_slice(),
            lettuce_media::IngestRequest::new(
                lettuce_media::AssetKind::OtherImage,
                lettuce_media::AssetOrigin::Upload,
                lettuce_media::RetentionClass::Library,
                lettuce_media::AssetProvenanceV1::default(),
            ),
        )
        .expect("library asset");
    let hash = object.blob.content_hash.clone();
    let path = root
        .join("platform-v2/media-blobs/objects")
        .join(&hash.as_str()[..2])
        .join(&hash.as_str()[2..4])
        .join(hash.as_str());
    super::media_library_remove(
        &h.context,
        dto::MediaLibraryRemoveRequest {
            asset_id: object.asset.id.to_string(),
            client_operation_id: RequestId::new().to_string(),
        },
    )
    .await
    .expect("catalog removal still works");
    assert!(
        path.exists(),
        "an unreadable file may reference it, so the bytes stay"
    );
    let notices = h
        .context
        .backend()
        .database()
        .purge_notices()
        .expect("notices");
    assert!(notices.iter().any(|notice| {
        notice.entity == lettuce_database::PurgeNoticeEntity::DatabaseFile
            && notice.entity_id == "stray.sqlite3"
            && notice.reason == lettuce_database::PurgeNoticeReason::MediaCollectionSkipped
    }));
    super::storage_database_file_delete(
        &h.context,
        dto::DatabaseFileDeleteRequest {
            file: "stray.sqlite3".into(),
            client_operation_id: RequestId::new().to_string(),
        },
    )
    .await
    .expect("delete the stray file");
    assert!(!path.exists(), "collection resumes once the file is gone");
    drop(h);
    std::fs::remove_dir_all(root).expect("cleanup");
}
