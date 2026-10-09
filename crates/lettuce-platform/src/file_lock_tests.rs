use super::*;
use std::path::PathBuf;

fn host(root: &PathBuf) -> (ManagedFiles, WriteCapability) {
    let authority = FilesystemAuthority::new(&DirectorySnapshot::new(root).expect("snapshot"))
        .expect("authority");
    (
        authority.managed_files(),
        authority
            .write_capability(ManagedRoot::PrivatePersistent)
            .expect("write capability"),
    )
}

#[test]
fn file_lifecycle_lock_is_shared_across_authorities_and_released_on_drop() {
    let root = std::env::temp_dir().join(format!("lettuce-file-lock-{}", uuid::Uuid::new_v4()));
    let key = ObjectKey::single("database-file-lifecycle.lock").expect("key");
    let (files, write) = host(&root);
    let guard = files
        .lock_file(&write, &key, false)
        .expect("lock")
        .expect("acquired");
    let (other, other_write) = host(&root);
    assert!(
        other
            .lock_file(&other_write, &key, false)
            .expect("try lock")
            .is_none()
    );
    drop(guard);
    assert!(
        other
            .lock_file(&other_write, &key, false)
            .expect("try lock")
            .is_some()
    );
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn process_exit_releases_file_lifecycle_lock() {
    let root = std::env::temp_dir().join(format!("lettuce-lock-crash-{}", uuid::Uuid::new_v4()));
    let status = std::process::Command::new(std::env::current_exe().expect("test executable"))
        .args([
            "--exact",
            "file_lock_tests::lock_crash_child",
            "--nocapture",
        ])
        .env("LETTUCE_FILE_LOCK_CRASH_ROOT", &root)
        .status()
        .expect("child");
    assert_eq!(status.code(), Some(77));
    let (files, write) = host(&root);
    assert!(
        files
            .lock_file(
                &write,
                &ObjectKey::single("database-file-lifecycle.lock").expect("key"),
                false
            )
            .expect("try lock")
            .is_some()
    );
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn lock_crash_child() {
    let Some(root) = std::env::var_os("LETTUCE_FILE_LOCK_CRASH_ROOT") else {
        return;
    };
    let (files, write) = host(&PathBuf::from(root));
    let _guard = files
        .lock_file(
            &write,
            &ObjectKey::single("database-file-lifecycle.lock").expect("key"),
            false,
        )
        .expect("try lock")
        .expect("acquired");
    std::process::exit(77);
}

#[cfg(unix)]
#[test]
fn lock_file_refuses_symlinks_without_changing_the_target() {
    let root = std::env::temp_dir().join(format!("lettuce-lock-link-{}", uuid::Uuid::new_v4()));
    let (files, write) = host(&root);
    let target = root.join("preserved");
    std::fs::write(&target, b"preserved").expect("source");
    std::os::unix::fs::symlink(
        &target,
        root.join("private-persistent-v2/database-file-lifecycle.lock"),
    )
    .expect("symlink");
    assert!(
        files
            .lock_file(
                &write,
                &ObjectKey::single("database-file-lifecycle.lock").expect("key"),
                false
            )
            .is_err()
    );
    assert_eq!(std::fs::read(target).expect("source remains"), b"preserved");
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn blocking_lock_waits_without_holding_the_managed_mutation_lock() {
    let root = std::env::temp_dir().join(format!("lettuce-lock-wait-{}", uuid::Uuid::new_v4()));
    let authority = FilesystemAuthority::new(&DirectorySnapshot::new(&root).expect("snapshot"))
        .expect("authority");
    let files = authority.managed_files();
    let write = authority
        .write_capability(ManagedRoot::PrivatePersistent)
        .expect("write capability");
    let key = ObjectKey::single("database-file-lifecycle.lock").expect("key");
    let held = files
        .lock_file(&write, &key, false)
        .expect("lock")
        .expect("acquired");
    let (started, ready) = std::sync::mpsc::channel();
    let waiter_files = files.clone();
    let waiter_write = authority
        .write_capability(ManagedRoot::PrivatePersistent)
        .expect("waiter capability");
    let worker = std::thread::spawn(move || {
        started.send(()).expect("start signal");
        waiter_files
            .lock_file(&waiter_write, &key, true)
            .expect("blocking lock")
            .expect("acquired")
    });
    ready
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("waiter starts");
    files
        .write_atomic(
            &write,
            ObjectKey::single("independent-write").expect("key"),
            b"preserved",
        )
        .expect("write while another thread waits");
    drop(held);
    drop(worker.join().expect("waiter completes"));
    std::fs::remove_dir_all(root).expect("cleanup");
}
