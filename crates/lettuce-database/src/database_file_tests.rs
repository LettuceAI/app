use crate::Database;

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
