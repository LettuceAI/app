use lettuce_contracts::{self as dto, ApiErrorCode};
use lettuce_jobs::SystemClock;
use lettuce_platform::{DirectorySnapshot, FilesystemAuthority, ManagedRoot};
use lettuce_types::{OperationId, RequestId, TimestampMillis};
use std::sync::Arc;

use super::tests::{NoImages, Reply, harness_over_files};
use super::{ApiDatabaseFiles, ApiMediaStore, NoModels};
use crate::{AppBackend, AppDatabaseLocation};

#[tokio::test]
async fn usage_csv_exports_to_a_target_and_refuses_database_files_before_truncation() {
    let root = std::env::temp_dir().join(format!("lettuce-usage-export-{}", OperationId::new()));
    let authority = FilesystemAuthority::new(&DirectorySnapshot::new(&root).expect("snapshot"))
        .expect("authority");
    let location =
        AppDatabaseLocation::new(root.join("private-persistent-v2"), &authority).expect("location");
    let active = location.active_path().expect("active");
    let backend = Arc::new(AppBackend::open(&active, TimestampMillis::new(10)).expect("backend"));
    let h = harness_over_files(
        backend,
        Reply::Text("ok"),
        Arc::new(SystemClock),
        None,
        Some(root.clone()),
        Arc::new(NoModels),
        Arc::new(NoImages),
        Some(ApiDatabaseFiles {
            location,
            active: active.clone(),
        }),
    );
    let output = root.join("usage.csv");
    let request = dto::UsageExportCsvRequest {
        filters: dto::UsageFilters::default(),
        target: dto::FileTarget {
            uri: output.to_string_lossy().into_owned(),
        },
    };
    super::usage_export_csv(&h.context, request.clone())
        .await
        .expect("export");
    assert_eq!(
        std::fs::read_to_string(&output).expect("CSV"),
        format!("{}\n", lettuce_usage::USAGE_CSV_HEADER)
    );
    let before = std::fs::read(&active).expect("database bytes");
    let alias = root.join("database-alias.csv");
    std::fs::hard_link(&active, &alias).expect("hard link");
    for target in [&active, &alias] {
        let error = super::usage_export_csv(
            &h.context,
            dto::UsageExportCsvRequest {
                target: dto::FileTarget {
                    uri: target.to_string_lossy().into_owned(),
                },
                ..request.clone()
            },
        )
        .await
        .expect_err("protected database");
        assert_eq!(error.code, ApiErrorCode::Conflict);
        assert_eq!(std::fs::read(&active).expect("preserved database"), before);
    }
    let error = super::usage_export_csv(
        &h.context,
        dto::UsageExportCsvRequest {
            filters: dto::UsageFilters {
                range: dto::UsageDateRange {
                    start: Some(2),
                    end: Some(1),
                },
                ..dto::UsageFilters::default()
            },
            ..request.clone()
        },
    )
    .await
    .expect_err("invalid range");
    assert_eq!(error.code, ApiErrorCode::InvalidInput);
    assert_eq!(
        std::fs::read_to_string(&output).expect("preserved export"),
        format!("{}\n", lettuce_usage::USAGE_CSV_HEADER)
    );
    drop(h);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test]
async fn media_save_streams_to_a_chosen_target_and_preserves_protected_or_missing_sources() {
    let root = std::env::temp_dir().join(format!("lettuce-media-save-{}", OperationId::new()));
    std::fs::create_dir_all(&root).expect("root");
    let media = super::tests::media_store(&root);
    let png = super::tests::png_bytes();
    let object = media
        .ingest(
            png.as_slice(),
            lettuce_media::IngestRequest::new(
                lettuce_media::AssetKind::OtherImage,
                lettuce_media::AssetOrigin::Upload,
                lettuce_media::RetentionClass::Persistent,
                lettuce_media::AssetProvenanceV1::default(),
            ),
        )
        .expect("media object");
    let h = super::tests::harness_in(
        Reply::Text("ok"),
        Arc::new(SystemClock),
        Some(media),
        Some(root.clone()),
        Arc::new(NoModels),
    );
    let target = root.join("export.png");
    std::fs::write(
        &target,
        b"previous much longer export contents to replace safely",
    )
    .expect("target");
    let request = dto::MediaSaveToRequest {
        asset_id: object.asset.id.to_string(),
        target: dto::FileTarget {
            uri: target.to_string_lossy().into_owned(),
        },
    };
    super::media_save_to(&h.context, request.clone())
        .await
        .expect("save");
    assert_eq!(std::fs::read(&target).expect("export"), png);
    super::media_save_to(&h.context, request.clone())
        .await
        .expect("retry");
    assert_eq!(std::fs::read(&target).expect("export"), png);
    let hash = object.blob.content_hash;
    let source = root
        .join("platform-v2/media-blobs/objects")
        .join(&hash.as_str()[..2])
        .join(&hash.as_str()[2..4])
        .join(hash.as_str());
    let alias = root.join("alias");
    std::fs::hard_link(&source, &alias).expect("hardlink");
    for protected in [&source, &alias] {
        let error = super::media_save_to(
            &h.context,
            dto::MediaSaveToRequest {
                target: dto::FileTarget {
                    uri: protected.to_string_lossy().into_owned(),
                },
                ..request.clone()
            },
        )
        .await
        .expect_err("protected source");
        assert_eq!(error.code, ApiErrorCode::Conflict);
        assert!(error.details.is_some());
        assert_eq!(std::fs::read(&source).expect("preserved source"), png);
    }
    let error = super::media_save_to(
        &h.context,
        dto::MediaSaveToRequest {
            asset_id: lettuce_types::AssetId::new().to_string(),
            ..request.clone()
        },
    )
    .await
    .expect_err("missing asset");
    assert_eq!(error.code, ApiErrorCode::NotFound);
    assert!(error.details.is_some());
    assert_eq!(std::fs::read(&target).expect("preserved target"), png);
    let empty = super::media_save_to(
        &h.context,
        dto::MediaSaveToRequest {
            asset_id: object.asset.id.to_string(),
            target: dto::FileTarget { uri: " ".into() },
        },
    )
    .await
    .expect_err("empty target");
    assert_eq!(empty.code, ApiErrorCode::InvalidInput);
    let unavailable = super::tests::harness(Reply::Text("ok"));
    let error = super::media_save_to(
        &unavailable.context,
        dto::MediaSaveToRequest {
            asset_id: object.asset.id.to_string(),
            target: dto::FileTarget {
                uri: target.to_string_lossy().into_owned(),
            },
        },
    )
    .await
    .expect_err("missing media host");
    assert_eq!(error.code, ApiErrorCode::Unavailable);
    assert!(error.details.is_some());
    std::fs::remove_file(&source).expect("missing object");
    let error = super::media_save_to(&h.context, request)
        .await
        .expect_err("missing bytes");
    assert_eq!(error.code, ApiErrorCode::NotFound);
    assert!(error.details.is_some());
    assert_eq!(std::fs::read(&target).expect("preserved target"), png);
    drop(h);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test]
async fn database_file_commands_protect_the_active_file_and_collect_the_only_kept_holder() {
    let root = std::env::temp_dir().join(format!("lettuce-file-api-{}", OperationId::new()));
    let authority = FilesystemAuthority::new(&DirectorySnapshot::new(&root).expect("snapshot"))
        .expect("authority");
    let location =
        AppDatabaseLocation::new(root.join("private-persistent-v2"), &authority).expect("location");
    let active = location.active_path().expect("active");
    let backend = Arc::new(AppBackend::open(&active, TimestampMillis::new(10)).expect("backend"));
    let media = Arc::new(ApiMediaStore::new(
        authority.managed_files(),
        authority
            .read_capability(ManagedRoot::MediaBlobs)
            .expect("read"),
        authority
            .write_capability(ManagedRoot::MediaBlobs)
            .expect("write"),
        lettuce_database::Database::open(&active).expect("blob database"),
        lettuce_database::Database::open(&active).expect("asset database"),
    ));
    let name = format!("{}.sqlite3", OperationId::new());
    let kept = location.database_path(&name).expect("kept path");
    let kept_media = ApiMediaStore::new(
        authority.managed_files(),
        authority
            .read_capability(ManagedRoot::MediaBlobs)
            .expect("read"),
        authority
            .write_capability(ManagedRoot::MediaBlobs)
            .expect("write"),
        lettuce_database::Database::open(&kept).expect("blob database"),
        lettuce_database::Database::open(&kept).expect("asset database"),
    );
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    png.extend_from_slice(&13_u32.to_be_bytes());
    png.extend_from_slice(b"IHDR");
    png.extend_from_slice(&2_u32.to_be_bytes());
    png.extend_from_slice(&3_u32.to_be_bytes());
    png.extend_from_slice(&[8, 6, 0, 0, 0]);
    let object = kept_media
        .ingest(
            png.as_slice(),
            lettuce_media::IngestRequest::new(
                lettuce_media::AssetKind::OtherImage,
                lettuce_media::AssetOrigin::Upload,
                lettuce_media::RetentionClass::Library,
                lettuce_media::AssetProvenanceV1::default(),
            ),
        )
        .expect("kept asset");
    let hash = object.blob.content_hash;
    let object_path = root
        .join("platform-v2/media-blobs/objects")
        .join(&hash.as_str()[..2])
        .join(&hash.as_str()[2..4])
        .join(hash.as_str());
    drop(kept_media);
    let h = harness_over_files(
        backend,
        Reply::Text("ok"),
        Arc::new(SystemClock),
        Some(media),
        Some(root.clone()),
        Arc::new(NoModels),
        Arc::new(NoImages),
        Some(ApiDatabaseFiles {
            location,
            active: active.clone(),
        }),
    );
    let listed = super::storage_database_files_list(&h.context)
        .await
        .expect("list");
    assert_eq!(listed.len(), 2);
    assert!(
        listed
            .iter()
            .any(|file| file.active && !file.deletable && file.file == "lettuce.sqlite3")
    );
    assert!(
        listed
            .iter()
            .any(|file| file.file == name && file.deletable)
    );
    assert!(
        !serde_json::to_string(&listed)
            .expect("JSON")
            .contains(root.to_str().expect("path"))
    );
    let key = RequestId::new().to_string();
    let error = super::storage_database_file_delete(
        &h.context,
        dto::DatabaseFileDeleteRequest {
            file: "lettuce.sqlite3".into(),
            client_operation_id: key.clone(),
        },
    )
    .await
    .expect_err("active file");
    assert_eq!(error.code, ApiErrorCode::InUse);
    assert!(error.details.is_some());
    assert!(object_path.exists());
    let request = dto::DatabaseFileDeleteRequest {
        file: name.clone(),
        client_operation_id: key.clone(),
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let bucket = object_path.parent().expect("bucket");
        let permissions = std::fs::metadata(bucket)
            .expect("bucket metadata")
            .permissions();
        std::fs::set_permissions(bucket, std::fs::Permissions::from_mode(0o555))
            .expect("deny removal");
        let result = super::storage_database_file_delete(&h.context, request.clone()).await;
        std::fs::set_permissions(bucket, permissions).expect("restore permissions");
        let error = result.expect_err("failed media removal remains visible");
        assert_eq!(error.code, ApiErrorCode::Unavailable);
        assert!(error.details.is_some());
        assert!(!kept.exists());
        assert!(object_path.exists());
    }
    super::storage_database_file_delete(&h.context, request.clone())
        .await
        .expect("delete kept file");
    assert!(!kept.exists());
    assert!(!object_path.exists());
    super::storage_database_file_delete(&h.context, request)
        .await
        .expect("exact replay");
    let error = super::storage_database_file_delete(
        &h.context,
        dto::DatabaseFileDeleteRequest {
            file: "lettuce.sqlite3".into(),
            client_operation_id: key,
        },
    )
    .await
    .expect_err("different digest");
    assert_eq!(error.code, ApiErrorCode::Conflict);
    assert!(active.exists());
    drop(h);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test]
async fn log_export_cannot_truncate_the_active_database_or_its_hardlink() {
    let root = std::env::temp_dir().join(format!("lettuce-export-database-{}", OperationId::new()));
    let authority = FilesystemAuthority::new(&DirectorySnapshot::new(&root).expect("snapshot"))
        .expect("authority");
    let location =
        AppDatabaseLocation::new(root.join("private-persistent-v2"), &authority).expect("location");
    let active = location.active_path().expect("active");
    let backend = Arc::new(AppBackend::open(&active, TimestampMillis::new(10)).expect("backend"));
    let media = Arc::new(ApiMediaStore::new(
        authority.managed_files(),
        authority
            .read_capability(ManagedRoot::MediaBlobs)
            .expect("read"),
        authority
            .write_capability(ManagedRoot::MediaBlobs)
            .expect("write"),
        lettuce_database::Database::open(&active).expect("blob database"),
        lettuce_database::Database::open(&active).expect("asset database"),
    ));
    let png = super::tests::png_bytes();
    let object = media
        .ingest(
            png.as_slice(),
            lettuce_media::IngestRequest::new(
                lettuce_media::AssetKind::OtherImage,
                lettuce_media::AssetOrigin::Upload,
                lettuce_media::RetentionClass::Library,
                lettuce_media::AssetProvenanceV1::default(),
            ),
        )
        .expect("media object");
    let hash = object.blob.content_hash;
    let object_path = root
        .join("platform-v2/media-blobs/objects")
        .join(&hash.as_str()[..2])
        .join(&hash.as_str()[2..4])
        .join(hash.as_str());
    let object_alias = root.join("media-alias");
    std::fs::hard_link(&object_path, &object_alias).expect("media hardlink");
    let h = harness_over_files(
        backend,
        Reply::Text("ok"),
        Arc::new(SystemClock),
        Some(media),
        Some(root.clone()),
        Arc::new(NoModels),
        Arc::new(NoImages),
        Some(ApiDatabaseFiles {
            location,
            active: active.clone(),
        }),
    );
    let log_directory = root.join("logs");
    std::fs::create_dir(&log_directory).expect("logs");
    let output = lettuce_observability::local_output(
        lettuce_observability::LocalOutputConfig::new(&log_directory),
    )
    .expect("output");
    h.context.attach_logs(log_directory, output.sink);
    super::log_append(
        &h.context,
        dto::LogAppendRequest {
            timestamp: "2026-10-09T12:00:00Z".into(),
            level: dto::LogLevel::Info,
            component: "frontend".into(),
            function: None,
            message: "export source".into(),
        },
    )
    .await
    .expect("log");
    let name = super::logs_list(&h.context).await.expect("logs").files[0].clone();
    let alias = root.join("database-alias");
    std::fs::hard_link(&active, &alias).expect("hardlink");
    let before = std::fs::read(&active).expect("database bytes");
    for target in [&active, &alias, &object_path, &object_alias] {
        let error = super::log_export(
            &h.context,
            dto::LogExportRequest {
                name: name.clone(),
                target: dto::FileTarget {
                    uri: target.to_string_lossy().into_owned(),
                },
            },
        )
        .await
        .expect_err("protected database target");
        assert_eq!(error.code, ApiErrorCode::Conflict);
        assert!(error.details.is_some());
        assert_eq!(std::fs::read(&active).expect("database survives"), before);
        assert_eq!(std::fs::read(&object_path).expect("media survives"), png);
    }
    let pointer = root.join("private-persistent-v2/active-database");
    assert!(!pointer.exists());
    let error = super::log_export(
        &h.context,
        dto::LogExportRequest {
            name,
            target: dto::FileTarget {
                uri: pointer.to_string_lossy().into_owned(),
            },
        },
    )
    .await
    .expect_err("reserved pointer target");
    assert_eq!(error.code, ApiErrorCode::Conflict);
    assert!(!pointer.exists());
    super::settings_get(&h.context)
        .await
        .expect("database remains usable");
    drop(h);
    std::fs::remove_dir_all(root).expect("cleanup");
}
