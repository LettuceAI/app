use super::database_files::FileKeepStage;
use super::*;
use lettuce_media::{
    AssetKind, AssetOrigin, AssetProvenanceV1, BlobState, MediaAsset, MediaAssetRepository,
    MediaBlob, MediaBlobRepository, MediaKind, RetentionClass,
};
use lettuce_platform::DirectorySnapshot;
use lettuce_types::{AssetId, ContentHash, MediaBlobId, Revision};

fn location(root: &Path) -> AppDatabaseLocation {
    let authority = FilesystemAuthority::new(&DirectorySnapshot::new(root).expect("snapshot"))
        .expect("authority");
    AppDatabaseLocation::new(root.join("private-persistent-v2"), &authority).expect("location")
}

fn media(database: &Database, digit: char, retention: RetentionClass) -> ContentHash {
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
    MediaAssetRepository::create(
        database,
        MediaAsset::new(
            AssetId::new(),
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
    hash
}

#[tokio::test]
async fn cutover_captures_retained_hashes_and_never_exposes_a_writer_for_deletion() {
    let root = std::env::temp_dir().join(format!("lettuce-cutover-{}", OperationId::new()));
    let location = location(&root);
    let old_path = location.active_path().expect("active path");
    std::fs::create_dir_all(old_path.parent().expect("parent")).expect("database directory");
    let old = Database::open(&old_path).expect("old database");
    let kept = media(&old, 'a', RetentionClass::Library);
    let garbage = media(&old, 'b', RetentionClass::Persistent);
    drop(old);
    let guard = location.file_lifecycle().await.expect("lifecycle guard");
    let name = format!("{}.sqlite3", OperationId::new());
    let path = guard
        .begin_file(&name, DatabaseFileKind::Restore, TimestampMillis::new(20))
        .expect("begin file");
    let target = Database::open(&path).expect("new database");
    let pending = media(&target, 'c', RetentionClass::Library);
    drop(target);
    let files = guard.inventory(&old_path).expect("inventory");
    assert!(
        !files
            .iter()
            .find(|file| file.file == name)
            .expect("writing file")
            .deletable
    );
    assert!(
        !guard
            .kept_media_hashes()
            .expect("kept hashes")
            .contains(&pending)
    );
    assert!(matches!(
        location.try_file_lifecycle(),
        Err(AppDatabaseLocationError::Busy)
    ));
    guard
        .activate_file(&name, TimestampMillis::new(30))
        .expect("cutover");
    assert_eq!(
        guard.kept_media_hashes().expect("kept hashes"),
        std::collections::BTreeSet::from([kept])
    );
    assert!(
        !guard
            .kept_media_hashes()
            .expect("kept hashes")
            .contains(&garbage)
    );
    assert_eq!(location.active_path().expect("active pointer"), path);
    drop(guard);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test]
async fn first_inventory_captures_existing_kept_files_without_changing_the_database() {
    let root = std::env::temp_dir().join(format!("lettuce-existing-kept-{}", OperationId::new()));
    let location = location(&root);
    let active = location.active_path().expect("active path");
    std::fs::create_dir_all(active.parent().expect("parent")).expect("database directory");
    drop(Database::open(&active).expect("active database"));
    let previous = location
        .database_path(&format!("{}.sqlite3", OperationId::new()))
        .expect("previous path");
    let database = Database::open(&previous).expect("previous database");
    let kept = media(&database, 'd', RetentionClass::Library);
    let garbage = media(&database, 'e', RetentionClass::Persistent);
    drop(database);
    let before = std::fs::read(&previous).expect("database bytes");
    let guard = location.file_lifecycle().await.expect("lifecycle guard");
    assert_eq!(
        guard.kept_media_hashes().expect("kept hashes"),
        std::collections::BTreeSet::from([kept])
    );
    assert!(
        !guard
            .kept_media_hashes()
            .expect("kept hashes")
            .contains(&garbage)
    );
    assert_eq!(std::fs::read(previous).expect("database bytes"), before);
    drop(guard);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test]
async fn interrupted_cutover_preserves_the_active_pointer_and_exact_kept_set() {
    for phase in [
        "created",
        "target_intent",
        "keep_intent",
        "frozen",
        "kept_set",
        "switched",
    ] {
        let switched = phase == "switched";
        let root =
            std::env::temp_dir().join(format!("lettuce-cutover-crash-{}", OperationId::new()));
        let location = location(&root);
        let old_path = location.active_path().expect("active path");
        std::fs::create_dir_all(old_path.parent().expect("parent")).expect("database directory");
        let old = Database::open(&old_path).expect("database");
        let kept = media(&old, 'f', RetentionClass::Library);
        drop(old);
        let name = format!("{}.sqlite3", OperationId::new());
        let new_path = location.database_path(&name).expect("new path");
        let status = std::process::Command::new(std::env::current_exe().expect("test executable"))
            .args([
                "--exact",
                "backup::backup_restore::database_files_tests::cutover_crash_child",
                "--nocapture",
            ])
            .env("LETTUCE_CUTOVER_CRASH_ROOT", &root)
            .env("LETTUCE_CUTOVER_CRASH_NAME", &name)
            .env("LETTUCE_CUTOVER_CRASH_PHASE", phase)
            .status()
            .expect("crash process");
        assert_eq!(status.code(), Some(77));
        let recovered = location.file_lifecycle().await.expect("restart recovery");
        assert_eq!(
            location.active_path().expect("pointer"),
            if switched {
                new_path.clone()
            } else {
                old_path.clone()
            }
        );
        assert_eq!(
            recovered.kept_media_hashes().expect("kept set"),
            if switched {
                std::collections::BTreeSet::from([kept])
            } else {
                std::collections::BTreeSet::new()
            }
        );
        drop(recovered);
        if switched {
            assert!(matches!(
                Database::open(&old_path),
                Err(lettuce_database::DatabaseError::WriteFenced)
            ));
            drop(Database::open(&new_path).expect("new active file accepts writes"));
        } else {
            let active = Database::open(&old_path)
                .expect("previous active file accepts writes after recovery");
            media(&active, 'e', RetentionClass::Library);
            drop(active);
        }
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}

#[test]
fn cutover_crash_child() {
    let Some(root) = std::env::var_os("LETTUCE_CUTOVER_CRASH_ROOT") else {
        return;
    };
    let name = std::env::var("LETTUCE_CUTOVER_CRASH_NAME").expect("file name");
    let location = location(Path::new(&root));
    let guard = location.acquire_file_lifecycle(true).expect("guard");
    let new_path = guard
        .begin_file(&name, DatabaseFileKind::Reset, TimestampMillis::new(20))
        .expect("begin");
    drop(Database::open(&new_path).expect("fresh database"));
    let phase = std::env::var("LETTUCE_CUTOVER_CRASH_PHASE").expect("phase");
    if phase == "created" {
        std::process::exit(77);
    }
    let stop = match phase.as_str() {
        "target_intent" => Some(FileKeepStage::TargetIntent),
        "keep_intent" => Some(FileKeepStage::KeepIntent),
        "frozen" => Some(FileKeepStage::Frozen),
        "kept_set" => Some(FileKeepStage::KeptSet),
        "switched" => None,
        _ => panic!("unknown cutover phase"),
    };
    guard
        .prepare_cutover_with_checkpoint(&name, TimestampMillis::new(30), |stage| {
            if Some(stage) == stop {
                std::process::exit(77);
            }
        })
        .expect("prepared cutover");
    if phase == "switched" {
        location.activate(&name).expect("pointer switch");
    }
    std::process::exit(77);
}

#[tokio::test]
async fn explicit_deletion_refuses_active_and_open_files_and_replays_the_exact_request() {
    let root = std::env::temp_dir().join(format!("lettuce-delete-file-{}", OperationId::new()));
    let location = location(&root);
    let active = location.active_path().expect("active");
    std::fs::create_dir_all(active.parent().expect("parent")).expect("directory");
    let database = Database::open(&active).expect("database");
    let guard = location.file_lifecycle().await.expect("guard");
    let name = format!("{}.sqlite3", OperationId::new());
    let path = guard
        .begin_file(&name, DatabaseFileKind::Restore, TimestampMillis::new(20))
        .expect("begin");
    drop(Database::open(&path).expect("new database"));
    let key = lettuce_types::RequestId::new();
    assert!(matches!(
        guard.delete_file(&name, &active, key),
        Err(AppDatabaseLocationError::InUse)
    ));
    guard
        .activate_file(&name, TimestampMillis::new(30))
        .expect("cutover");
    assert!(matches!(
        guard.delete_file(&name, &active, key),
        Err(AppDatabaseLocationError::InUse)
    ));
    let old = active
        .file_name()
        .and_then(|name| name.to_str())
        .expect("old name");
    assert!(matches!(
        guard.delete_file(old, &path, key),
        Err(AppDatabaseLocationError::InUse)
    ));
    drop(database);
    guard
        .delete_file(old, &path, key)
        .expect("explicit deletion");
    assert!(!active.exists());
    guard.delete_file(old, &path, key).expect("exact replay");
    assert!(matches!(
        guard.delete_file(&name, &path, key),
        Err(AppDatabaseLocationError::Conflict)
    ));
    assert!(path.exists());
    drop(guard);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test]
async fn interrupted_explicit_file_deletion_replays_without_touching_the_active_file() {
    for checkpoint in [
        "Intent",
        "DatabaseRemoved",
        "SidecarsRemoved",
        "MetadataRemoved",
    ] {
        let root =
            std::env::temp_dir().join(format!("lettuce-file-delete-crash-{}", OperationId::new()));
        let location = location(&root);
        let old = location.active_path().expect("old");
        drop(Database::open(&old).expect("old database"));
        let guard = location.file_lifecycle().await.expect("guard");
        let name = format!("{}.sqlite3", OperationId::new());
        let active = guard
            .begin_file(&name, DatabaseFileKind::Restore, TimestampMillis::new(10))
            .expect("begin");
        drop(Database::open(&active).expect("new database"));
        guard
            .activate_file(&name, TimestampMillis::new(20))
            .expect("activate");
        drop(guard);
        let key = lettuce_types::RequestId::new();
        let status = std::process::Command::new(std::env::current_exe().expect("test executable"))
            .args([
                "--exact",
                "backup::backup_restore::database_files_tests::file_deletion_crash_child",
                "--nocapture",
            ])
            .env("LETTUCE_FILE_DELETE_CRASH_ROOT", &root)
            .env("LETTUCE_FILE_DELETE_CRASH_KEY", key.to_string())
            .env("LETTUCE_FILE_DELETE_CRASH_STAGE", checkpoint)
            .status()
            .expect("crash child");
        assert_eq!(status.code(), Some(77));
        let guard = location.file_lifecycle().await.expect("recover");
        guard
            .delete_file("lettuce.sqlite3", &active, key)
            .expect("resume deletion");
        guard
            .delete_file("lettuce.sqlite3", &active, key)
            .expect("stable replay");
        assert!(!old.exists());
        assert!(active.exists());
        assert!(guard.kept_media_hashes().expect("sets").is_empty());
        assert!(matches!(
            guard.delete_file(&name, &active, key),
            Err(AppDatabaseLocationError::Conflict)
        ));
        drop(guard);
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}

#[test]
fn file_deletion_crash_child() {
    let Some(root) = std::env::var_os("LETTUCE_FILE_DELETE_CRASH_ROOT") else {
        return;
    };
    let key = std::env::var("LETTUCE_FILE_DELETE_CRASH_KEY")
        .expect("key")
        .parse()
        .expect("request id");
    let stage = std::env::var("LETTUCE_FILE_DELETE_CRASH_STAGE").expect("checkpoint");
    let location = location(Path::new(&root));
    let guard = location.acquire_file_lifecycle(true).expect("guard");
    guard
        .delete_file_with_checkpoint(
            "lettuce.sqlite3",
            &location.active_path().expect("active"),
            key,
            |checkpoint| {
                if format!("{checkpoint:?}") == stage {
                    std::process::exit(77);
                }
            },
        )
        .expect("delete");
    panic!("checkpoint was not reached");
}

#[cfg(unix)]
#[tokio::test]
async fn bad_sidecar_refuses_deletion_before_touching_the_kept_database() {
    let root =
        std::env::temp_dir().join(format!("lettuce-delete-bad-sidecar-{}", OperationId::new()));
    let location = location(&root);
    let old = location.active_path().expect("old path");
    let database = Database::open(&old).expect("database");
    let hash = media(&database, 'a', RetentionClass::Library);
    drop(database);
    let guard = location.file_lifecycle().await.expect("guard");
    let name = format!("{}.sqlite3", OperationId::new());
    let active = guard
        .begin_file(&name, DatabaseFileKind::Restore, TimestampMillis::new(10))
        .expect("begin");
    drop(Database::open(&active).expect("fresh database"));
    guard
        .activate_file(&name, TimestampMillis::new(20))
        .expect("cutover");
    let before = std::fs::read(&old).expect("old bytes");
    let sentinel = root.join("sentinel");
    std::fs::write(&sentinel, b"preserve").expect("sentinel");
    let sidecar = old.with_file_name("lettuce.sqlite3-shm");
    if sidecar.exists() {
        std::fs::remove_file(&sidecar).expect("replace closed test sidecar");
    }
    std::os::unix::fs::symlink(&sentinel, &sidecar).expect("sidecar symlink");
    assert!(matches!(
        guard.delete_file("lettuce.sqlite3", &active, lettuce_types::RequestId::new()),
        Err(AppDatabaseLocationError::Platform(
            PlatformError::SymlinkEscape
        ))
    ));
    assert_eq!(
        std::fs::read(&old).expect("kept file remains intact"),
        before
    );
    assert!(guard.kept_media_hashes().expect("kept set").contains(&hash));
    assert_eq!(std::fs::read(&sentinel).expect("sentinel"), b"preserve");
    drop(guard);
    std::fs::remove_dir_all(root).expect("cleanup");
}
