use rusqlite::session::Session;
use rusqlite::Connection;
use uuid::Uuid;

use super::catalog::{cached_schema_fingerprint, CatalogError};
use super::changeset::inspect_changeset;
use super::model::ChangeRevision;
use super::store::{
    insert_local_revision, load_frontier, next_local_stamp, set_row_version, StoreError,
};

#[derive(Debug)]
pub struct CapturedTransaction<T> {
    pub value: T,
    pub revision: Option<ChangeRevision>,
}

#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    #[error("sync v2 database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("local device ID cannot be empty")]
    EmptyDeviceId,
    #[error("failed to load local device identity: {0}")]
    DeviceIdentity(String),
    #[error("tracked mutation failed: {0}")]
    Mutation(String),
}

pub fn capture_local_transaction<T, F>(
    conn: &Connection,
    now_ms: i64,
    mutate: F,
) -> Result<CapturedTransaction<T>, CaptureError>
where
    F: FnOnce(&Connection) -> Result<T, rusqlite::Error>,
{
    let device_id = crate::sync::db::get_or_create_local_device_id(conn)
        .map_err(CaptureError::DeviceIdentity)?;
    capture_transaction(conn, &device_id, now_ms, mutate)
}

pub fn capture_local_string_transaction<T, F>(
    conn: &Connection,
    now_ms: i64,
    mutate: F,
) -> Result<CapturedTransaction<T>, CaptureError>
where
    F: FnOnce(&Connection) -> Result<T, String>,
{
    let device_id = crate::sync::db::get_or_create_local_device_id(conn)
        .map_err(CaptureError::DeviceIdentity)?;
    capture_transaction_inner(conn, &device_id, now_ms, |tx| {
        mutate(tx).map_err(CaptureError::Mutation)
    })
}

pub fn capture_transaction<T, F>(
    conn: &Connection,
    local_device_id: &str,
    now_ms: i64,
    mutate: F,
) -> Result<CapturedTransaction<T>, CaptureError>
where
    F: FnOnce(&Connection) -> Result<T, rusqlite::Error>,
{
    capture_transaction_inner(conn, local_device_id, now_ms, |tx| {
        mutate(tx).map_err(CaptureError::Database)
    })
}

fn capture_transaction_inner<T, F>(
    conn: &Connection,
    local_device_id: &str,
    now_ms: i64,
    mutate: F,
) -> Result<CapturedTransaction<T>, CaptureError>
where
    F: FnOnce(&Connection) -> Result<T, CaptureError>,
{
    if local_device_id.is_empty() {
        return Err(CaptureError::EmptyDeviceId);
    }

    let fingerprint = cached_schema_fingerprint(conn)?;
    let base_frontier = load_frontier(conn)?;
    let mut session = Session::new(conn)?;
    session.table_filter(Some(super::catalog::is_syncable_table));
    session.attach(None)?;

    let tx = conn.unchecked_transaction()?;
    let value = match mutate(&tx) {
        Ok(value) => value,
        Err(error) => {
            tx.rollback()?;
            return Err(error);
        }
    };

    if session.is_empty() {
        tx.commit()?;
        return Ok(CapturedTransaction {
            value,
            revision: None,
        });
    }

    let mut changeset = Vec::new();
    session.changeset_strm(&mut changeset)?;
    let (origin_sequence, timestamp) = next_local_stamp(&tx, now_ms)?;
    let revision = ChangeRevision {
        change_id: Uuid::new_v4().to_string(),
        origin_device_id: local_device_id.to_string(),
        origin_sequence,
        timestamp,
        base_frontier,
        schema_fingerprint: fingerprint,
        changeset_hash: blake3::hash(&changeset).to_hex().to_string(),
        changeset,
    };
    insert_local_revision(&tx, &revision, now_ms)?;
    for row in inspect_changeset(&revision.changeset)? {
        set_row_version(&tx, &row, &revision)?;
    }
    tx.commit()?;

    Ok(CapturedTransaction {
        value,
        revision: Some(revision),
    })
}

#[cfg(test)]
mod tests {
    use fallible_streaming_iterator::FallibleStreamingIterator;
    use std::io::Read;
    use rusqlite::hooks::Action;
    use rusqlite::session::ChangesetIter;
    use rusqlite::{params, Connection};

    use super::{
        capture_local_string_transaction, capture_local_transaction,
        capture_transaction,
    };
    use crate::sync::v2::{create_schema, load_revision};

    fn connection() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE chats (
               id TEXT PRIMARY KEY,
               title TEXT NOT NULL
             );",
        )
        .unwrap();
        create_schema(&conn).unwrap();
        conn
    }

    #[test]
    fn user_write_and_outbox_revision_commit_atomically() {
        let conn = connection();

        let captured = capture_transaction(&conn, "device-a", 100, |tx| {
            tx.execute(
                "INSERT INTO chats (id, title) VALUES (?1, ?2)",
                params!["chat-1", "Hello"],
            )
        })
        .unwrap();

        assert_eq!(captured.value, 1);
        let revision = captured.revision.unwrap();
        assert_eq!(revision.origin_sequence, 1);
        assert_eq!(revision.changeset_hash, blake3::hash(&revision.changeset).to_hex().to_string());
        assert_eq!(
            load_revision(&conn, &revision.change_id).unwrap(),
            Some(revision)
        );
    }

    #[test]
    fn adding_a_column_requires_no_sync_mapping_change() {
        let conn = connection();
        conn.execute_batch("ALTER TABLE chats ADD COLUMN summary TEXT;")
            .unwrap();

        let revision = capture_transaction(&conn, "device-a", 100, |tx| {
            tx.execute(
                "INSERT INTO chats (id, title, summary) VALUES (?1, ?2, ?3)",
                params!["chat-1", "Hello", "new field"],
            )
        })
        .unwrap()
        .revision
        .unwrap();

        let mut bytes = revision.changeset.as_slice();
        let input: &mut dyn Read = &mut bytes;
        let mut changes = ChangesetIter::start_strm(&input).unwrap();
        let item = changes.next().unwrap().unwrap();
        assert_eq!(item.op().unwrap().number_of_columns(), 3);
        assert_eq!(item.new_value(2).unwrap().as_str().unwrap(), "new field");
    }

    #[test]
    fn adding_a_primary_key_table_is_captured_automatically() {
        let conn = connection();
        conn.execute_batch(
            "CREATE TABLE future_feature (
               id TEXT PRIMARY KEY,
               enabled INTEGER NOT NULL
             );",
        )
        .unwrap();

        let revision = capture_transaction(&conn, "device-a", 100, |tx| {
            tx.execute(
                "INSERT INTO future_feature (id, enabled) VALUES ('feature-1', 1)",
                [],
            )
        })
        .unwrap()
        .revision
        .unwrap();

        let mut bytes = revision.changeset.as_slice();
        let input: &mut dyn Read = &mut bytes;
        let mut changes = ChangesetIter::start_strm(&input).unwrap();
        let item = changes.next().unwrap().unwrap();
        let operation = item.op().unwrap();
        assert_eq!(operation.table_name(), "future_feature");
        assert_eq!(operation.code(), Action::SQLITE_INSERT);
    }

    #[test]
    fn sync_metadata_writes_do_not_create_recursive_changes() {
        let conn = connection();

        let captured = capture_transaction(&conn, "device-a", 100, |tx| {
            tx.execute(
                "INSERT INTO sync_v2_local_state (key, value) VALUES ('test', '1')",
                [],
            )
        })
        .unwrap();

        assert!(captured.revision.is_none());
    }

    #[test]
    fn failed_mutation_rolls_back_data_and_outbox() {
        let conn = connection();

        let result = capture_transaction(&conn, "device-a", 100, |tx| {
            tx.execute(
                "INSERT INTO chats (id, title) VALUES ('chat-1', 'Hello')",
                [],
            )?;
            tx.execute(
                "INSERT INTO chats (id, title) VALUES ('chat-1', 'Duplicate')",
                [],
            )
        });

        assert!(result.is_err());
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM chats", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            0
        );
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM sync_v2_changes", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
            0
        );
    }

    #[test]
    fn app_capture_reuses_the_existing_sync_device_identity() {
        let conn = connection();
        conn.execute_batch(
            "CREATE TABLE sync_local_state (
               key TEXT PRIMARY KEY,
               value TEXT NOT NULL
             );
             INSERT INTO sync_local_state (key, value)
             VALUES ('device_id', 'stable-device');",
        )
        .unwrap();

        let captured = capture_local_transaction(&conn, 100, |tx| {
            tx.execute(
                "INSERT INTO chats (id, title) VALUES ('chat-1', 'Hello')",
                [],
            )
        })
        .unwrap();

        assert_eq!(
            captured.revision.unwrap().origin_device_id,
            "stable-device"
        );
    }

    #[test]
    fn string_mutation_errors_roll_back_the_tracked_transaction() {
        let conn = connection();
        conn.execute_batch(
            "CREATE TABLE sync_local_state (
               key TEXT PRIMARY KEY,
               value TEXT NOT NULL
             );",
        )
        .unwrap();

        let result = capture_local_string_transaction(&conn, 100, |tx| {
            tx.execute(
                "INSERT INTO chats (id, title) VALUES ('chat-1', 'Hello')",
                [],
            )
            .map_err(|error| error.to_string())?;
            Err::<(), _>("validation failed".to_string())
        });

        assert!(result.is_err());
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM chats", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
}
