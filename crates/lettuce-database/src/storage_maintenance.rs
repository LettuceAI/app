use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use crate::Database;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum StorageMaintenanceError {
    #[error("database maintenance was cancelled")]
    Cancelled,
    #[error("the database checkpoint is busy")]
    CheckpointBusy,
    #[error("the database is kept read-only")]
    ReadOnly,
    #[error("database maintenance failed")]
    Storage,
}

impl Database {
    pub fn optimize_storage(
        &self,
        cancellation: Arc<AtomicBool>,
    ) -> Result<(), StorageMaintenanceError> {
        self.optimize_storage_after_checkpoint(cancellation, || {})
    }

    fn optimize_storage_after_checkpoint(
        &self,
        cancellation: Arc<AtomicBool>,
        after_checkpoint: impl FnOnce(),
    ) -> Result<(), StorageMaintenanceError> {
        if cancellation.load(Ordering::Acquire) {
            return Err(StorageMaintenanceError::Cancelled);
        }
        if self.foreign_keys_lost.load(Ordering::SeqCst) {
            return Err(StorageMaintenanceError::Storage);
        }
        let connection = self
            .connection
            .lock()
            .map_err(|_| StorageMaintenanceError::Storage)?;
        let _writes = self
            .write_access
            .as_ref()
            .map(|access| access.acquire_exclusive())
            .transpose()
            .map_err(|_| StorageMaintenanceError::Storage)?;
        if self
            .write_access
            .as_ref()
            .map(|access| access.fenced())
            .transpose()
            .map_err(|_| StorageMaintenanceError::Storage)?
            .unwrap_or(false)
        {
            return Err(StorageMaintenanceError::ReadOnly);
        }
        connection
            .pragma_update(None, "query_only", false)
            .map_err(|_| StorageMaintenanceError::Storage)?;
        if cancellation.load(Ordering::Acquire) {
            return Err(StorageMaintenanceError::Cancelled);
        }
        let cancelled = cancellation.clone();
        connection
            .progress_handler(1000, Some(move || cancelled.load(Ordering::Acquire)))
            .map_err(|_| StorageMaintenanceError::Storage)?;
        let result = (|| {
            let (busy, _, _): (i64, i64, i64) = connection
                .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                })
                .map_err(|_| StorageMaintenanceError::Storage)?;
            if busy != 0 {
                return Err(StorageMaintenanceError::CheckpointBusy);
            }
            after_checkpoint();
            if cancellation.load(Ordering::Acquire) {
                return Err(StorageMaintenanceError::Cancelled);
            }
            connection
                .execute_batch("VACUUM")
                .map_err(|_| StorageMaintenanceError::Storage)
        })();
        connection
            .progress_handler(0, None::<fn() -> bool>)
            .map_err(|_| StorageMaintenanceError::Storage)?;
        if cancellation.load(Ordering::Acquire) && result.is_err() {
            Err(StorageMaintenanceError::Cancelled)
        } else {
            result
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, atomic::AtomicBool};

    use super::*;
    use crate::Database;

    fn persistent() -> (Arc<Database>, std::path::PathBuf) {
        let root =
            std::env::temp_dir().join(format!("lettuce-maintenance-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("root");
        let database = Arc::new(Database::open(root.join("active.sqlite")).expect("database"));
        (database, root)
    }

    #[test]
    fn checkpoint_busy_is_typed_and_preserves_committed_rows() {
        let (database, root) = persistent();
        database
            .connection()
            .expect("connection")
            .busy_timeout(std::time::Duration::ZERO)
            .expect("timeout");
        let reader = rusqlite::Connection::open(root.join("active.sqlite")).expect("reader");
        reader
            .execute_batch("BEGIN; SELECT * FROM app_settings;")
            .expect("read snapshot");
        database
            .connection()
            .expect("connection")
            .execute("UPDATE app_settings SET updated_at=2", [])
            .expect("write");
        assert_eq!(
            database.optimize_storage(Arc::new(AtomicBool::new(false))),
            Err(StorageMaintenanceError::CheckpointBusy)
        );
        reader.execute_batch("ROLLBACK").expect("release reader");
        database
            .optimize_storage(Arc::new(AtomicBool::new(false)))
            .expect("retry");
        assert_eq!(
            database
                .connection()
                .expect("connection")
                .query_row("SELECT updated_at FROM app_settings", [], |row| row
                    .get::<_, i64>(0))
                .expect("preserved write"),
            2
        );
        drop(reader);
        drop(database);
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn optimize_waits_for_a_database_file_writer_and_does_not_fence_the_file() {
        let (database, root) = persistent();
        let write = database
            .write_access
            .as_ref()
            .expect("file access")
            .acquire()
            .expect("sync transaction lease");
        let (started, entered) = std::sync::mpsc::channel();
        let (finished, result) = std::sync::mpsc::channel();
        let worker = {
            let database = database.clone();
            std::thread::spawn(move || {
                started.send(()).expect("started");
                finished
                    .send(database.optimize_storage(Arc::new(AtomicBool::new(false))))
                    .expect("result");
            })
        };
        entered.recv().expect("entered");
        assert!(matches!(
            result.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        ));
        drop(write);
        result.recv().expect("result").expect("optimized");
        worker.join().expect("worker");
        assert!(!database.is_file_write_fenced().expect("write fence"));
        database
            .connection()
            .expect("connection")
            .execute("UPDATE app_settings SET updated_at=3", [])
            .expect("later write");
        drop(database);
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn cancelled_optimize_keeps_the_database_writable_and_intact() {
        let database = Database::open_in_memory().expect("database");
        assert_eq!(
            database.optimize_storage(Arc::new(AtomicBool::new(true))),
            Err(StorageMaintenanceError::Cancelled)
        );
        assert_eq!(
            database
                .connection()
                .expect("connection")
                .query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
                .expect("integrity"),
            "ok"
        );
        database
            .connection()
            .expect("connection")
            .execute("UPDATE app_settings SET updated_at=4", [])
            .expect("write");
    }

    #[test]
    fn optimizing_a_kept_database_is_refused() {
        let (database, root) = persistent();
        let fence = Database::lock_file_writes(&root.join("active.sqlite")).expect("fence");
        fence.set_fenced(true).expect("kept file");
        drop(fence);
        assert_eq!(
            database.optimize_storage(Arc::new(AtomicBool::new(false))),
            Err(StorageMaintenanceError::ReadOnly)
        );
        drop(database);
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn optimize_crash_child() {
        let Some(root) = std::env::var_os("LETTUCE_OPTIMIZE_CRASH_ROOT") else {
            return;
        };
        let database = Database::open(std::path::Path::new(&root).join("active.sqlite"))
            .expect("child database");
        database
            .optimize_storage_after_checkpoint(Arc::new(AtomicBool::new(false)), || {
                std::process::exit(77)
            })
            .expect("crash hook");
        panic!("crash hook did not exit");
    }

    #[test]
    fn crash_after_checkpoint_preserves_the_database_and_releases_the_write_lease() {
        let (database, root) = persistent();
        database
            .connection()
            .expect("connection")
            .execute("UPDATE app_settings SET updated_at=9", [])
            .expect("committed write");
        drop(database);
        let status = std::process::Command::new(std::env::current_exe().expect("test executable"))
            .args([
                "--exact",
                "storage_maintenance::tests::optimize_crash_child",
                "--nocapture",
            ])
            .env("LETTUCE_OPTIMIZE_CRASH_ROOT", &root)
            .status()
            .expect("child");
        assert_eq!(status.code(), Some(77));
        let database = Database::open(root.join("active.sqlite")).expect("reopen");
        assert_eq!(
            database
                .connection()
                .expect("connection")
                .query_row("SELECT updated_at FROM app_settings", [], |row| row
                    .get::<_, i64>(0))
                .expect("preserved write"),
            9
        );
        assert_eq!(
            database
                .connection()
                .expect("connection")
                .query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
                .expect("integrity"),
            "ok"
        );
        database
            .optimize_storage(Arc::new(AtomicBool::new(false)))
            .expect("retry after crash");
        drop(database);
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}
