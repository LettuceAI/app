use lettuce_contracts::{self as dto, ApiErrorCode};
use lettuce_jobs::SystemClock;
use lettuce_platform::{DirectorySnapshot, FilesystemAuthority, ManagedRoot};
use lettuce_types::{OperationId, RequestId, TimestampMillis};
use std::sync::Arc;

use super::tests::{NoImages, Reply, harness_over_files};
use super::{ApiDatabaseFiles, ApiMediaStore, NoModels};
use crate::{AppBackend, AppDatabaseLocation};

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
