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
    for switched in [false, true] {
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
            .env(
                "LETTUCE_CUTOVER_CRASH_SWITCH",
                if switched { "yes" } else { "no" },
            )
            .status()
            .expect("crash process");
        assert_eq!(status.code(), Some(77));
        let recovered = location.file_lifecycle().await.expect("restart recovery");
        assert_eq!(
            location.active_path().expect("pointer"),
            if switched { new_path } else { old_path }
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
    guard
        .prepare_cutover(&name, TimestampMillis::new(30))
        .expect("prepared cutover");
    if std::env::var("LETTUCE_CUTOVER_CRASH_SWITCH").expect("switch") == "yes" {
        location.activate(&name).expect("pointer switch");
    }
    std::process::exit(77);
}
