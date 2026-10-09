use crate::{DirectorySnapshot, FilesystemAuthority, PlatformError};

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
