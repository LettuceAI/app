use crate::Database;

#[test]
fn a_fenced_database_name_cannot_be_recreated_after_a_reset_move() {
    let root =
        std::env::temp_dir().join(format!("lettuce-reset-recreate-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("root");
    let path = root.join("active.sqlite3");
    let kept = root.join("kept.sqlite3");
    let database = Database::open(&path).expect("database");
    let fence = Database::lock_file_writes(&path).expect("fence");
    fence.set_fenced(true).expect("freeze");
    drop(fence);
    database.close_for_reset().expect("close");
    std::fs::rename(&path, &kept).expect("move");
    assert!(matches!(
        Database::open(&path),
        Err(crate::DatabaseError::WriteFenced)
    ));
    assert!(
        !path.exists(),
        "a rejected stale open must not create a database file"
    );
    drop(database);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn an_open_database_cannot_be_reserved_for_deletion_until_every_handle_closes() {
    let path =
        std::env::temp_dir().join(format!("lettuce-file-use-{}.sqlite3", uuid::Uuid::new_v4()));
    let first = Database::open(&path).expect("first database");
    let second = Database::open(&path).expect("second database");
    assert!(
        Database::try_reserve_file_deletion(&path)
            .expect("deletion reservation")
            .is_none()
    );
    drop(first);
    assert!(
        Database::try_reserve_file_deletion(&path)
            .expect("deletion reservation")
            .is_none()
    );
    drop(second);
    let permit = Database::try_reserve_file_deletion(&path)
        .expect("deletion reservation")
        .expect("unused file");
    std::fs::remove_file(&path).expect("explicit test deletion");
    drop(permit);
}

#[cfg(unix)]
#[test]
fn database_file_deletion_refuses_a_symlink_and_preserves_its_target() {
    let root = std::env::temp_dir().join(format!("lettuce-db-file-link-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("root");
    let target = root.join("target.sqlite3");
    drop(Database::open(&target).expect("database"));
    let alias = root.join("alias.sqlite3");
    std::os::unix::fs::symlink(&target, &alias).expect("alias");
    assert!(Database::try_reserve_file_deletion(&alias).is_err());
    assert!(target.exists());
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn frozen_database_rejects_writes_and_recovery_restores_them() {
    let root = std::env::temp_dir().join(format!("lettuce-write-fence-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("root");
    let path = root.join("active.sqlite3");
    let database = Database::open(&path).expect("database");
    {
        let connection = database.connection().expect("connection");
        connection
            .execute_batch("CREATE TABLE fence_test(id INTEGER PRIMARY KEY)")
            .expect("fixture");
        let statement = connection
            .prepare("INSERT INTO fence_test VALUES(?1)")
            .expect("prepared write");
        drop(statement);
    }
    let fence = Database::lock_file_writes(&path).expect("write fence");
    fence.set_fenced(true).expect("freeze");
    drop(fence);
    let connection = database
        .connection()
        .expect("read connection remains available");
    let error = connection
        .prepare("INSERT INTO fence_test VALUES(?1)")
        .expect("same SQL statement")
        .execute([1])
        .expect_err("frozen prepared write refused");
    assert_eq!(
        error.sqlite_error_code(),
        Some(rusqlite::ErrorCode::ReadOnly)
    );
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM fence_test", [], |row| row
                .get::<_, i64>(0))
            .expect("read frozen database"),
        0
    );
    drop(connection);
    assert!(matches!(
        Database::open(&path),
        Err(crate::DatabaseError::WriteFenced)
    ));
    let fence = Database::lock_file_writes(&path).expect("recovery fence");
    fence
        .set_fenced(false)
        .expect("pointer unchanged: restore writes");
    drop(fence);
    database
        .connection()
        .expect("recovered connection")
        .prepare("INSERT INTO fence_test VALUES(?1)")
        .expect("prepared retry")
        .execute([1])
        .expect("retry");
    drop(database);
    drop(Database::open(&path).expect("reopen active database"));
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn write_fence_open_process_child() {
    use std::io::Write;
    let Ok(path) = std::env::var("LETTUCE_WRITE_FENCE_CHILD") else {
        return;
    };
    let database = Database::open(&path).expect("already-open worker");
    println!("WRITE_FENCE_READY");
    std::io::stdout().flush().expect("ready signal");
    let mut line = String::new();
    std::io::stdin().read_line(&mut line).expect("resume event");
    assert_eq!(line.trim(), "resume");
    let error = database
        .connection()
        .expect("old connection reader")
        .execute("INSERT INTO fence_test VALUES(1)", [])
        .expect_err("kept writer refused");
    assert_eq!(
        error.sqlite_error_code(),
        Some(rusqlite::ErrorCode::ReadOnly)
    );
}

#[test]
fn write_fence_blocks_a_worker_that_opened_the_database_in_another_process() {
    use std::io::{BufRead, Read, Write};
    let root = std::env::temp_dir().join(format!(
        "lettuce-write-fence-process-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).expect("root");
    let path = root.join("active.sqlite3");
    let database = Database::open(&path).expect("database");
    database
        .connection()
        .expect("connection")
        .execute_batch("CREATE TABLE fence_test(id INTEGER PRIMARY KEY)")
        .expect("fixture");
    drop(database);
    let mut child = std::process::Command::new(std::env::current_exe().expect("test binary"))
        .args([
            "--exact",
            "database_file_tests::write_fence_open_process_child",
            "--nocapture",
        ])
        .env("LETTUCE_WRITE_FENCE_CHILD", &path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("worker process");
    let mut output = std::io::BufReader::new(child.stdout.take().expect("child output"));
    loop {
        let mut line = String::new();
        assert!(
            output.read_line(&mut line).expect("ready event") > 0,
            "worker exited before opening"
        );
        if line.trim() == "WRITE_FENCE_READY" {
            break;
        }
    }
    let fence = Database::lock_file_writes(&path).expect("cutover fence");
    fence.set_fenced(true).expect("freeze");
    drop(fence);
    writeln!(child.stdin.take().expect("resume pipe"), "resume").expect("resume worker");
    let mut remaining = String::new();
    output
        .read_to_string(&mut remaining)
        .expect("worker result");
    print!("{remaining}");
    assert!(child.wait().expect("worker exit").success());
    let fence = Database::lock_file_writes(&path).expect("recovery fence");
    fence.set_fenced(false).expect("unfreeze active file");
    drop(fence);
    let database = Database::open(&path).expect("active");
    assert_eq!(
        database
            .connection()
            .expect("connection")
            .query_row("SELECT count(*) FROM fence_test", [], |row| row
                .get::<_, i64>(0))
            .expect("unchanged rows"),
        0
    );
    drop(database);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn reset_closes_fenced_connections_and_releases_every_file_lease_without_losing_rows() {
    use lettuce_jobs::{
        JobKind, JobSpec, JobStore, JobSubject, OutcomeRef, ResourceClass, SubjectKind,
    };
    let root = std::env::temp_dir().join(format!("lettuce-reset-close-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).expect("root");
    let path = root.join("active.sqlite3");
    let first = Database::open(&path).expect("first");
    let second = Database::open(&path).expect("second");
    let job = first
        .create_or_get(
            JobSpec::new(
                JobKind::Maintenance,
                JobSubject::new(SubjectKind::Maintenance, "reset-close-test").expect("subject"),
                OutcomeRef::Request(lettuce_types::RequestId::new()),
            )
            .with_resources(vec![ResourceClass::DiskWrite]),
        )
        .expect("stored row")
        .job;
    {
        let fence = Database::lock_file_writes(&path).expect("write fence");
        fence.set_fenced(true).expect("freeze");
    }
    first.close_for_reset().expect("close first");
    first.close_for_reset().expect("idempotent close");
    assert!(matches!(
        first.connection(),
        Err(crate::DatabaseError::Closed)
    ));
    assert!(
        Database::try_reserve_file_deletion(&path)
            .expect("reservation")
            .is_none()
    );
    second.close_for_reset().expect("close second");
    let permit = Database::try_reserve_file_deletion(&path)
        .expect("reservation")
        .expect("all database handles closed");
    let kept = root.join("kept.sqlite3");
    std::fs::rename(&path, &kept).expect("rename after native SQLite handles closed");
    drop(permit);
    let readonly =
        rusqlite::Connection::open_with_flags(&kept, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .expect("kept database");
    let count: i64 = readonly
        .query_row(
            "SELECT COUNT(*) FROM jobs WHERE id = ?1",
            [job.id.to_string()],
            |row| row.get(0),
        )
        .expect("preserved job");
    assert_eq!(count, 1);
    drop(readonly);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn reset_cannot_close_an_unfenced_database() {
    let root =
        std::env::temp_dir().join(format!("lettuce-reset-close-live-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).expect("root");
    let database = Database::open(root.join("active.sqlite3")).expect("database");
    assert!(matches!(
        database.close_for_reset(),
        Err(crate::DatabaseError::WriteFenceRequired)
    ));
    assert!(database.connection().is_ok());
    drop(database);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[cfg(target_os = "linux")]
fn descriptors_opened_on(path: &std::path::Path) -> usize {
    let path = path.canonicalize().expect("canonical path");
    std::fs::read_dir("/proc/self/fd")
        .expect("descriptor table")
        .filter_map(|entry| std::fs::read_link(entry.ok()?.path()).ok())
        .filter(|target| target == &path)
        .count()
}

#[cfg(target_os = "linux")]
#[test]
fn file_leases_and_fences_never_open_the_live_database_file() {
    let root = std::env::temp_dir().join(format!("lettuce-lease-paths-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).expect("root");
    let path = root.join("active.sqlite3");
    let database = Database::open(&path).expect("database");
    let sqlite_only = descriptors_opened_on(&path);
    assert_eq!(sqlite_only, 1, "only SQLite holds the database file");
    let fence = Database::lock_file_writes(&path).expect("fence");
    assert_eq!(descriptors_opened_on(&path), sqlite_only);
    drop(fence);
    assert!(
        Database::try_reserve_file_deletion(&path)
            .expect("reservation")
            .is_none(),
        "an open database is never reserved for deletion"
    );
    assert_eq!(descriptors_opened_on(&path), sqlite_only);
    let lease = root.join("active.sqlite3.use.lock");
    assert!(lease.exists(), "the open lease lives in its own lock file");
    drop(database);
    let permit = Database::try_reserve_file_deletion(&path)
        .expect("reservation")
        .expect("closed database");
    assert_eq!(descriptors_opened_on(&path), 0);
    drop(permit);
    std::fs::remove_dir_all(root).expect("cleanup");
}
