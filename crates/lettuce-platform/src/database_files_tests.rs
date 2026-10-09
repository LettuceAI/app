use crate::{DirectorySnapshot, FilesystemAuthority, PlatformError};

#[test]
fn database_reset_move_is_confined_idempotent_and_never_overwrites_a_file() {
    let root = std::env::temp_dir().join(format!("lettuce-db-reset-move-{}", uuid::Uuid::new_v4()));
    let authority = FilesystemAuthority::new(&DirectorySnapshot::new(&root).expect("snapshot"))
        .expect("authority");
    let directory = authority.database_files().expect("directory");
    let parent = root.join("private-persistent-v2/databases");
    std::fs::write(parent.join("active.sqlite3"), b"user data").expect("source");
    std::fs::write(parent.join("other.sqlite3"), b"other user data").expect("other");
    let expected = directory
        .open_database_component("active.sqlite3", "")
        .expect("source handle");
    assert_eq!(
        directory.move_database_component("active.sqlite3", "other.sqlite3", "", &expected),
        Err(PlatformError::Conflict)
    );
    assert_eq!(
        std::fs::read(parent.join("other.sqlite3")).expect("other"),
        b"other user data"
    );
    assert_eq!(
        directory.move_database_component("active.sqlite3", "../kept.sqlite3", "", &expected),
        Err(PlatformError::InvalidKey)
    );
    assert_eq!(
        directory.move_database_component(
            "active.sqlite3",
            "kept.sqlite3",
            ".meta.json",
            &expected
        ),
        Err(PlatformError::InvalidKey)
    );
    directory
        .move_database_component("active.sqlite3", "kept.sqlite3", "", &expected)
        .expect("move");
    assert!(!parent.join("active.sqlite3").exists());
    assert_eq!(
        std::fs::read(parent.join("kept.sqlite3")).expect("kept"),
        b"user data"
    );
    directory
        .move_database_component("active.sqlite3", "kept.sqlite3", "", &expected)
        .expect("replay");
    drop((directory, authority, expected));
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn database_deletion_requires_the_exact_open_file_and_refuses_other_names() {
    let root = std::env::temp_dir().join(format!("lettuce-db-directory-{}", uuid::Uuid::new_v4()));
    let authority = FilesystemAuthority::new(&DirectorySnapshot::new(&root).expect("snapshot"))
        .expect("authority");
    let directory = authority.database_files().expect("database directory");
    let path = root.join("private-persistent-v2/databases/old.sqlite3");
    std::fs::write(&path, b"old database").expect("database");
    let expected = std::fs::File::open(&path).expect("descriptor");
    std::fs::write(
        root.join("private-persistent-v2/databases/other.sqlite3"),
        b"other",
    )
    .expect("other");
    assert_eq!(
        directory.remove_database_file("other.sqlite3", &expected),
        Err(PlatformError::Conflict)
    );
    assert_eq!(
        directory.remove_database_file("../active-database", &expected),
        Err(PlatformError::InvalidKey)
    );
    directory
        .remove_database_file("old.sqlite3", &expected)
        .expect("delete exact file");
    assert!(!path.exists());
    assert!(
        root.join("private-persistent-v2/databases/other.sqlite3")
            .exists()
    );
    drop((directory, authority, expected));
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[cfg(unix)]
#[test]
fn database_sidecar_deletion_refuses_symlinks_before_removing_any_sidecar() {
    let root = std::env::temp_dir().join(format!("lettuce-db-sidecars-{}", uuid::Uuid::new_v4()));
    let authority = FilesystemAuthority::new(&DirectorySnapshot::new(&root).expect("snapshot"))
        .expect("authority");
    let directory = authority.database_files().expect("database directory");
    let parent = root.join("private-persistent-v2/databases");
    let target = root.join("sentinel");
    std::fs::write(&target, b"keep").expect("target");
    std::fs::write(parent.join("old.sqlite3-wal"), b"wal").expect("wal");
    std::os::unix::fs::symlink(&target, parent.join("old.sqlite3-shm")).expect("symlink");
    assert_eq!(
        directory.remove_database_sidecars("old.sqlite3"),
        Err(PlatformError::SymlinkEscape)
    );
    assert_eq!(std::fs::read(&target).expect("sentinel"), b"keep");
    assert!(parent.join("old.sqlite3-wal").exists());
    drop((directory, authority));
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn explicit_database_deletion_removes_write_fence_sidecars() {
    let root = std::env::temp_dir().join(format!(
        "lettuce-db-fence-sidecars-{}",
        uuid::Uuid::new_v4()
    ));
    let authority = FilesystemAuthority::new(&DirectorySnapshot::new(&root).expect("snapshot"))
        .expect("authority");
    let directory = authority.database_files().expect("database directory");
    let parent = root.join("private-persistent-v2/databases");
    for suffix in ["-wal", "-shm", ".writes.lock", ".write-fenced", ".use.lock"] {
        std::fs::write(parent.join(format!("old.sqlite3{suffix}")), b"").expect("sidecar");
    }
    directory
        .remove_database_sidecars("old.sqlite3")
        .expect("explicit cleanup");
    for suffix in ["-wal", "-shm", ".writes.lock", ".write-fenced", ".use.lock"] {
        assert!(
            !parent.join(format!("old.sqlite3{suffix}")).exists(),
            "{suffix} survives explicit deletion"
        );
    }
    drop((directory, authority));
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[cfg(unix)]
#[test]
fn export_target_check_identifies_database_files_without_opening_them() {
    use std::os::unix::fs::PermissionsExt;
    let root = std::env::temp_dir().join(format!(
        "lettuce-db-contains-no-open-{}",
        uuid::Uuid::new_v4()
    ));
    let authority = FilesystemAuthority::new(&DirectorySnapshot::new(&root).expect("snapshot"))
        .expect("authority");
    let directory = authority.database_files().expect("directory");
    let parent = root.join("private-persistent-v2/databases");
    let live = parent.join("active.sqlite3");
    std::fs::write(&live, b"live database").expect("database");
    let outside = root.join("export.txt");
    std::fs::write(&outside, b"export").expect("outside");
    let target = std::fs::File::open(&outside).expect("target");
    std::fs::set_permissions(&live, std::fs::Permissions::from_mode(0o000)).expect("chmod");
    let unreadable = std::fs::File::open(&live).is_err();
    let absent = directory.contains_file(&target);
    let alias = parent.join("alias.sqlite3");
    std::fs::hard_link(&outside, &alias).expect("alias");
    let present = directory.contains_file(&target);
    std::fs::set_permissions(&live, std::fs::Permissions::from_mode(0o600)).expect("chmod");
    if unreadable {
        assert_eq!(absent, Ok(false));
        assert_eq!(present, Ok(true));
    }
    drop((target, directory, authority));
    std::fs::remove_dir_all(root).expect("cleanup");
}
