#![allow(clippy::unwrap_used)]

use std::{
    fs,
    io::{self, Write},
    sync::{Arc, Barrier},
    thread,
};

use uuid::Uuid;

use super::*;

fn files() -> (std::path::PathBuf, FilesystemAuthority, ManagedFiles) {
    let root = std::env::temp_dir().join(format!("lettuce-platform-{}", Uuid::new_v4()));
    let snapshot = DirectorySnapshot::new(root.clone()).unwrap();
    let authority = FilesystemAuthority::new(&snapshot).unwrap();
    let files = authority.managed_files();
    (root, authority, files)
}

#[test]
fn object_keys_reject_path_syntax_controls_and_internal_names() {
    for value in [
        "",
        ".",
        "..",
        "a/b",
        "a\\b",
        "/tmp",
        "C:foo",
        "a\0b",
        "a\n",
        ".lettuce-stage-user",
        ".lettuce-trash-user",
        ".lettuce-recovery-user",
        ".lettuce-journal-user",
    ] {
        assert!(ObjectKey::single(value).is_err(), "accepted {value:?}");
    }
    assert!(ObjectKey::single("日本語-🙂").is_ok());
}

#[test]
fn object_keys_enforce_segment_and_total_bounds() {
    assert!(
        ObjectKey::from_segments((0..crate::keys::MAX_KEY_SEGMENTS).map(|_| "segment")).is_ok()
    );
    assert!(
        ObjectKey::from_segments((0..=crate::keys::MAX_KEY_SEGMENTS).map(|_| "segment")).is_err()
    );

    assert!(ObjectKey::single("x".repeat(crate::keys::MAX_SEGMENT_BYTES)).is_ok());
    assert!(ObjectKey::single("x".repeat(crate::keys::MAX_SEGMENT_BYTES + 1)).is_err());

    let unicode_boundary = "é".repeat(crate::keys::MAX_SEGMENT_BYTES / 2);
    assert!(ObjectKey::single(&unicode_boundary).is_ok());
    assert!(ObjectKey::single(format!("{unicode_boundary}é")).is_err());
    assert!(ObjectKey::single("x".repeat(crate::keys::MAX_SEGMENT_SCALARS + 1)).is_err());

    let total_boundary = vec![unicode_boundary; 16];
    assert!(ObjectKey::from_segments(total_boundary.clone()).is_ok());
    assert!(
        ObjectKey::from_segments({
            let mut over = total_boundary;
            over.push("é".repeat(crate::keys::MAX_SEGMENT_BYTES / 2));
            over
        })
        .is_err()
    );
}

#[test]
fn atomic_create_replace_and_create_collision_preserve_stage() {
    let (root, authority, files) = files();
    let write = authority.write_capability(ManagedRoot::JobStaging).unwrap();
    let read = authority.read_capability(ManagedRoot::JobStaging).unwrap();
    let key = ObjectKey::from_segments(["nested", "value"]).unwrap();
    let first = files.write_atomic(&write, key.clone(), b"one").unwrap();
    assert_eq!(first.stage_cleanup, StageCleanupStatus::Cleaned);
    let second = files.write_atomic(&write, key.clone(), b"two").unwrap();
    assert_eq!(second.stage_cleanup, StageCleanupStatus::Cleaned);
    assert_eq!(files.read(&read, &key).unwrap(), b"two");
    let mut staged = files.stage_new(&write, key.clone()).unwrap();
    staged.write_all(b"three").unwrap();
    assert_eq!(staged.commit(), Err(PlatformError::Conflict));
    assert_eq!(files.read(&read, &key).unwrap(), b"two");
    assert_eq!(files.recover_incomplete().unwrap().retained, 1);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn stage_cleanup_failure_is_classified_as_recovery_not_write_failure() {
    assert_eq!(
        crate::atomic::classify_stage_cleanup(Ok(())),
        StageCleanupStatus::Cleaned
    );
    assert_eq!(
        crate::atomic::classify_stage_cleanup(Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "simulated cleanup failure",
        ))),
        StageCleanupStatus::RecoveryNeeded
    );
}

#[test]
fn capabilities_are_leaf_only_and_cannot_reach_siblings() {
    let (root, authority, files) = files();
    let diagnostics = authority
        .write_capability(ManagedRoot::Diagnostics)
        .unwrap();
    files
        .write_atomic(
            &diagnostics,
            ObjectKey::from_segments(["job-staging", "value"]).unwrap(),
            b"diagnostics",
        )
        .unwrap();
    assert!(
        !root
            .join("platform-v2/job-staging/job-staging/value")
            .exists()
    );
    assert!(
        root.join("platform-v2/diagnostics/job-staging/value")
            .exists()
    );
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn symlink_parent_final_and_listing_are_no_follow() {
    use std::os::unix::fs::symlink;

    let (root, authority, files) = files();
    let outside = root.join("outside");
    fs::create_dir_all(&outside).unwrap();
    let managed = root.join("platform-v2/job-staging");
    symlink(&outside, managed.join("escape")).unwrap();
    let write = authority.write_capability(ManagedRoot::JobStaging).unwrap();
    let read = authority.read_capability(ManagedRoot::JobStaging).unwrap();
    assert!(
        files
            .write_atomic(
                &write,
                ObjectKey::from_segments(["escape", "value"]).unwrap(),
                b"no",
            )
            .is_err()
    );
    fs::write(outside.join("target"), b"outside").unwrap();
    symlink(outside.join("target"), managed.join("link")).unwrap();
    assert!(
        files
            .read(&read, &ObjectKey::single("link").unwrap())
            .is_err()
    );
    let entries = files.list(&read, None, 16).unwrap();
    assert_eq!(
        entries
            .iter()
            .find(|entry| entry.name == "link")
            .unwrap()
            .kind,
        ObjectKind::Other
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn listing_has_an_explicit_bound() {
    let (root, authority, files) = files();
    let write = authority
        .write_capability(ManagedRoot::Diagnostics)
        .unwrap();
    let read = authority.read_capability(ManagedRoot::Diagnostics).unwrap();
    for name in ["one", "two"] {
        files
            .write_atomic(&write, ObjectKey::single(name).unwrap(), b"x")
            .unwrap();
    }
    assert_eq!(
        files.list(&read, None, 1),
        Err(PlatformError::LimitExceeded)
    );
    assert_eq!(
        files.list(&read, None, 0),
        Err(PlatformError::LimitExceeded)
    );
    assert_eq!(
        files.list(&read, None, crate::managed::MAX_LIST_ENTRIES + 1),
        Err(PlatformError::LimitExceeded)
    );
    assert_eq!(files.list(&read, None, 2).unwrap().len(), 2);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn private_persistent_root_is_not_listable_but_known_keys_are_readable() {
    let (root, authority, files) = files();
    let write = authority
        .write_capability(ManagedRoot::PrivatePersistent)
        .unwrap();
    let read = authority
        .read_capability(ManagedRoot::PrivatePersistent)
        .unwrap();
    let key = ObjectKey::single("ciphertext").unwrap();
    files.write_atomic(&write, key.clone(), b"opaque").unwrap();
    assert_eq!(files.read(&read, &key).unwrap(), b"opaque");
    assert_eq!(files.list(&read, None, 16), Err(PlatformError::Unsupported));
    assert!(matches!(
        files.remove_to_trash(&write, key),
        Err(PlatformError::Unsupported)
    ));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn trash_restore_collision_keeps_receipt_for_retry_and_foreign_is_rejected() {
    let (root, authority, files) = files();
    let write = authority.write_capability(ManagedRoot::JobStaging).unwrap();
    let read = authority.read_capability(ManagedRoot::JobStaging).unwrap();
    let key = ObjectKey::single("value").unwrap();
    files.write_atomic(&write, key.clone(), b"old").unwrap();
    let receipt = files.remove_to_trash(&write, key.clone()).unwrap();
    files.write_atomic(&write, key.clone(), b"new").unwrap();
    assert_eq!(
        files.restore_from_trash(&write, &receipt),
        Err(PlatformError::Conflict)
    );
    let replacement = files.remove_to_trash(&write, key.clone()).unwrap();
    let _ = files.restore_from_trash(&write, &receipt).unwrap();
    assert_eq!(files.read(&read, &key).unwrap(), b"old");

    let foreign_root = std::env::temp_dir().join(format!("lettuce-platform-{}", Uuid::new_v4()));
    let foreign_snapshot = DirectorySnapshot::new(foreign_root.clone()).unwrap();
    let foreign_authority = FilesystemAuthority::new(&foreign_snapshot).unwrap();
    let foreign_write = foreign_authority
        .write_capability(ManagedRoot::JobStaging)
        .unwrap();
    assert_eq!(
        foreign_authority
            .managed_files()
            .restore_from_trash(&foreign_write, &replacement),
        Err(PlatformError::WrongCapability)
    );
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(foreign_root).unwrap();
}

#[test]
fn concurrent_nested_parent_creation_is_safe() {
    let (root, authority, files) = files();
    let write = authority.write_capability(ManagedRoot::JobStaging).unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let first = {
        let files = files.clone();
        let write = write.clone();
        let barrier = Arc::clone(&barrier);
        thread::spawn(move || {
            barrier.wait();
            files.write_atomic(
                &write,
                ObjectKey::from_segments(["same", "parent", "one"]).unwrap(),
                b"one",
            )
        })
    };
    let second = {
        let files = files.clone();
        let write = write.clone();
        thread::spawn(move || {
            barrier.wait();
            files.write_atomic(
                &write,
                ObjectKey::from_segments(["same", "parent", "two"]).unwrap(),
                b"two",
            )
        })
    };
    assert!(first.join().unwrap().is_ok());
    assert!(second.join().unwrap().is_ok());
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn replace_rejects_existing_final_symlink() {
    use std::os::unix::fs::symlink;

    let (root, authority, files) = files();
    let outside = root.join("outside");
    fs::create_dir_all(&outside).unwrap();
    let managed = root.join("platform-v2/job-staging");
    fs::write(outside.join("target"), b"outside").unwrap();
    symlink(outside.join("target"), managed.join("link")).unwrap();
    let write = authority.write_capability(ManagedRoot::JobStaging).unwrap();
    assert!(
        files
            .write_atomic(&write, ObjectKey::single("link").unwrap(), b"no")
            .is_err()
    );
    assert_eq!(fs::read(outside.join("target")).unwrap(), b"outside");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn recovery_identity_includes_leaf_and_relative_location() {
    let (root, _authority, files) = files();
    let name = ".lettuce-stage-0123456789abcdef0123456789abcdef";
    for leaf in ["diagnostics", "job-staging"] {
        fs::write(root.join("platform-v2").join(leaf).join(name), b"stage").unwrap();
    }
    let report = files.recover_incomplete().unwrap();
    assert_eq!(report.retained, 2);
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn authority_final_root_open_rejects_symlink() {
    use std::os::unix::fs::symlink;

    let base = std::env::temp_dir().join(format!("lettuce-platform-{}", Uuid::new_v4()));
    let outside = base.join("outside");
    fs::create_dir_all(&outside).unwrap();
    let linked = base.join("linked");
    symlink(&outside, &linked).unwrap();
    let snapshot = DirectorySnapshot::new(linked).unwrap();
    assert!(FilesystemAuthority::new(&snapshot).is_err());
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn directory_snapshot_rejects_overlapping_private_roots() {
    let app_data = std::env::temp_dir().join(format!("lettuce-platform-{}", Uuid::new_v4()));
    assert!(DirectorySnapshot::with_private_persistent(&app_data, &app_data).is_err());
    assert!(
        DirectorySnapshot::with_private_persistent(&app_data, app_data.join("platform-v2/secrets"))
            .is_err()
    );
}

#[test]
fn errors_and_receipts_do_not_expose_native_paths() {
    let error = PlatformError::NotFound;
    let rendered = format!("{error:?} {error}");
    assert!(!rendered.contains("/tmp"));
    assert!(!rendered.contains("No such file"));
    let (root, authority, files) = files();
    let write = authority.write_capability(ManagedRoot::JobStaging).unwrap();
    files
        .write_atomic(&write, ObjectKey::single("value").unwrap(), b"x")
        .unwrap();
    let receipt = files
        .remove_to_trash(&write, ObjectKey::single("value").unwrap())
        .unwrap();
    assert!(!format!("{receipt:?}").contains("/tmp"));
    fs::remove_dir_all(root).unwrap();
}
