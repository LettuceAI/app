use rusqlite::session::Session;
use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

use super::batch::MAX_REVISION_BYTES;
use super::catalog::{cached_schema_fingerprint, syncable_tables, CatalogError};
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
    #[error("a single row in table {table} exceeds the {limit}-byte sync revision limit")]
    SeedRowTooLarge { table: String, limit: usize },
}

const SEED_ROWS_PER_REVISION: usize = 256;

pub fn capture_local_transaction<T, F>(
    conn: &Connection,
    now_ms: i64,
    mutate: F,
) -> Result<CapturedTransaction<T>, CaptureError>
where
    F: FnOnce(&Connection) -> Result<T, rusqlite::Error>,
{
    let device_id = super::identity::get_or_create_device_id(conn)
        .map_err(|error| CaptureError::DeviceIdentity(error.to_string()))?;
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
    let device_id = super::identity::get_or_create_device_id(conn)
        .map_err(|error| CaptureError::DeviceIdentity(error.to_string()))?;
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

pub fn ensure_current_database_seeded(
    conn: &Connection,
    now_ms: i64,
) -> Result<usize, CaptureError> {
    let device_id = super::identity::get_or_create_device_id(conn)
        .map_err(|error| CaptureError::DeviceIdentity(error.to_string()))?;
    let fingerprint = cached_schema_fingerprint(conn)?;
    let tables = seed_ordered_tables(conn, syncable_tables(conn)?)?;
    let mut revisions_created = 0usize;
    for table in tables {
        let state_key = format!("seeded_table:{}", table.name);
        let table_fingerprint =
            blake3::hash(table.create_sql.as_bytes()).to_hex().to_string();
        let seeded_fingerprint = conn
            .query_row(
                "SELECT value FROM sync_v2_local_state WHERE key = ?1",
                params![state_key],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        if seeded_fingerprint.as_deref() == Some(table_fingerprint.as_str()) {
            continue;
        }

        let changesets = table_seed_changesets(conn, &table.name, &table.create_sql)?;
        let tx = conn.unchecked_transaction()?;
        for changeset in changesets {
            if !changeset.is_empty() {
                let base_frontier = load_frontier(&tx)?;
                let (origin_sequence, timestamp) = next_local_stamp(&tx, now_ms)?;
                let revision = ChangeRevision {
                    change_id: Uuid::new_v4().to_string(),
                    origin_device_id: device_id.clone(),
                    origin_sequence,
                    timestamp,
                    base_frontier,
                    schema_fingerprint: fingerprint.clone(),
                    changeset_hash: blake3::hash(&changeset).to_hex().to_string(),
                    changeset,
                };
                insert_local_revision(&tx, &revision, now_ms)?;
                for row in inspect_changeset(&revision.changeset)? {
                    set_row_version(&tx, &row, &revision)?;
                }
                revisions_created += 1;
            }
        }
        tx.execute(
            "INSERT INTO sync_v2_local_state (key, value)
             VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![state_key, table_fingerprint],
        )?;
        tx.commit()?;
    }
    Ok(revisions_created)
}

fn seed_ordered_tables(
    conn: &Connection,
    tables: Vec<super::catalog::TableInfo>,
) -> Result<Vec<super::catalog::TableInfo>, CaptureError> {
    let table_names = tables
        .iter()
        .map(|table| table.name.clone())
        .collect::<BTreeSet<_>>();
    let mut dependencies = BTreeMap::new();
    for table in &tables {
        let escaped = table.name.replace('\'', "''");
        let mut statement = conn.prepare(&format!(
            "SELECT DISTINCT \"table\" FROM pragma_foreign_key_list('{escaped}')"
        ))?;
        let parents = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<BTreeSet<_>, _>>()?
            .into_iter()
            .filter(|parent| parent != &table.name && table_names.contains(parent))
            .collect::<BTreeSet<_>>();
        dependencies.insert(table.name.clone(), parents);
    }

    let mut remaining = tables
        .into_iter()
        .map(|table| (table.name.clone(), table))
        .collect::<BTreeMap<_, _>>();
    let mut emitted = BTreeSet::new();
    let mut ordered = Vec::with_capacity(remaining.len());
    loop {
        let ready = remaining
            .keys()
            .filter(|name| {
                dependencies
                    .get(*name)
                    .map(|parents| parents.is_subset(&emitted))
                    .unwrap_or(true)
            })
            .cloned()
            .collect::<Vec<_>>();
        if ready.is_empty() {
            break;
        }
        for name in ready {
            if let Some(table) = remaining.remove(&name) {
                emitted.insert(name);
                ordered.push(table);
            }
        }
    }
    ordered.extend(remaining.into_values());
    Ok(ordered)
}

fn table_seed_changesets(
    conn: &Connection,
    table_name: &str,
    create_sql: &str,
) -> Result<Vec<Vec<u8>>, CaptureError> {
    let quoted_table = quote_identifier(table_name);
    let columns = {
        let escaped = table_name.replace('\'', "''");
        let mut statement = conn.prepare(&format!(
            "SELECT name FROM pragma_table_info('{escaped}') ORDER BY cid"
        ))?;
        let columns = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        columns
    };
    let quoted_columns = columns
        .iter()
        .map(|column| quote_identifier(column))
        .collect::<Vec<_>>();
    let mut statement = conn.prepare(&format!(
        "SELECT {} FROM {quoted_table}",
        quoted_columns.join(", ")
    ))?;
    let column_count = columns.len();
    let rows = statement
        .query_map([], |row| {
            (0..column_count)
                .map(|index| row.get::<_, Value>(index))
                .collect::<Result<Vec<_>, _>>()
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut changesets = Vec::new();
    for rows in rows.chunks(SEED_ROWS_PER_REVISION) {
        append_bounded_seed_changesets(
            table_name,
            create_sql,
            &quoted_table,
            &quoted_columns,
            rows,
            &mut changesets,
        )?;
    }
    Ok(changesets)
}

fn append_bounded_seed_changesets(
    table_name: &str,
    create_sql: &str,
    quoted_table: &str,
    quoted_columns: &[String],
    rows: &[Vec<Value>],
    output: &mut Vec<Vec<u8>>,
) -> Result<(), CaptureError> {
    if rows.is_empty() {
        return Ok(());
    }
    let scratch = Connection::open_in_memory()?;
    scratch.execute_batch("PRAGMA foreign_keys = OFF;")?;
    scratch.execute_batch(create_sql)?;
    let mut session = Session::new(&scratch)?;
    session.attach(Some(table_name))?;
    let placeholders = (1..=quoted_columns.len())
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let insert_sql = format!(
        "INSERT INTO {quoted_table} ({}) VALUES ({placeholders})",
        quoted_columns.join(", ")
    );
    let tx = scratch.unchecked_transaction()?;
    for row in rows {
        tx.execute(&insert_sql, params_from_iter(row.iter()))?;
    }
    let mut changeset = Vec::new();
    session.changeset_strm(&mut changeset)?;
    tx.commit()?;
    drop(session);

    if changeset.len() <= MAX_REVISION_BYTES {
        output.push(changeset);
        return Ok(());
    }
    if rows.len() == 1 {
        return Err(CaptureError::SeedRowTooLarge {
            table: table_name.to_string(),
            limit: MAX_REVISION_BYTES,
        });
    }
    let midpoint = rows.len() / 2;
    append_bounded_seed_changesets(
        table_name,
        create_sql,
        quoted_table,
        quoted_columns,
        &rows[..midpoint],
        output,
    )?;
    append_bounded_seed_changesets(
        table_name,
        create_sql,
        quoted_table,
        quoted_columns,
        &rows[midpoint..],
        output,
    )
}

fn quote_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
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
        capture_transaction, ensure_current_database_seeded,
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
    fn app_capture_reuses_the_v2_device_identity() {
        let conn = connection();
        conn.execute_batch(
            "INSERT INTO sync_v2_local_state (key, value)
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
    fn existing_rows_are_seeded_once_for_v2_bootstrap() {
        let source = connection();
        source
            .execute(
                "INSERT INTO chats (id, title) VALUES ('chat-1', 'Existing')",
                [],
            )
            .unwrap();

        assert_eq!(ensure_current_database_seeded(&source, 100).unwrap(), 1);
        assert_eq!(ensure_current_database_seeded(&source, 200).unwrap(), 0);
        let revision = source
            .query_row(
                "SELECT change_id FROM sync_v2_changes LIMIT 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap();
        let revision = crate::sync::v2::load_revision(&source, &revision)
            .unwrap()
            .unwrap();

        let target = connection();
        crate::sync::v2::apply_remote_revision(&target, &revision, 300).unwrap();
        assert_eq!(
            target
                .query_row(
                    "SELECT title FROM chats WHERE id = 'chat-1'",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .unwrap(),
            "Existing"
        );
    }

    #[test]
    fn bootstrap_splits_large_tables_into_bounded_revisions() {
        let source = connection();
        for index in 0..300 {
            source
                .execute(
                    "INSERT INTO chats (id, title) VALUES (?1, ?2)",
                    params![format!("chat-{index:03}"), format!("Chat {index}")],
                )
                .unwrap();
        }

        assert_eq!(ensure_current_database_seeded(&source, 100).unwrap(), 2);
        let mut statement = source
            .prepare(
                "SELECT change_id FROM sync_v2_changes
                 ORDER BY origin_sequence",
            )
            .unwrap();
        let change_ids = statement
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        let target = connection();
        for change_id in change_ids {
            let revision = crate::sync::v2::load_revision(&source, &change_id)
                .unwrap()
                .unwrap();
            crate::sync::v2::apply_remote_revision(&target, &revision, 300).unwrap();
        }
        assert_eq!(
            target
                .query_row("SELECT COUNT(*) FROM chats", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            300
        );
    }

    #[test]
    fn bootstrap_chunks_foreign_key_tables_without_requiring_the_parent_in_scratch() {
        let source = connection();
        source
            .execute_batch(
                "CREATE TABLE character_notes (
                   id TEXT PRIMARY KEY,
                   character_id TEXT NOT NULL,
                   note TEXT NOT NULL,
                   FOREIGN KEY(character_id) REFERENCES chats(id) ON DELETE CASCADE
                 );
                 INSERT INTO chats (id, title) VALUES ('chat-1', 'Existing');
                 INSERT INTO character_notes (id, character_id, note)
                 VALUES ('note-1', 'chat-1', 'Remember this');",
            )
            .unwrap();

        assert_eq!(ensure_current_database_seeded(&source, 100).unwrap(), 2);
        let mut statement = source
            .prepare(
                "SELECT change_id FROM sync_v2_changes
                 ORDER BY origin_sequence",
            )
            .unwrap();
        let change_ids = statement
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        let target = connection();
        target
            .execute_batch(
                "CREATE TABLE character_notes (
                   id TEXT PRIMARY KEY,
                   character_id TEXT NOT NULL,
                   note TEXT NOT NULL,
                   FOREIGN KEY(character_id) REFERENCES chats(id) ON DELETE CASCADE
                 );",
            )
            .unwrap();
        for change_id in change_ids {
            let revision = crate::sync::v2::load_revision(&source, &change_id)
                .unwrap()
                .unwrap();
            crate::sync::v2::apply_remote_revision(&target, &revision, 200).unwrap();
        }
        assert_eq!(
            target
                .query_row("SELECT note FROM character_notes WHERE id = 'note-1'", [], |row| {
                    row.get::<_, String>(0)
                })
                .unwrap(),
            "Remember this"
        );
    }

    #[test]
    fn string_mutation_errors_roll_back_the_tracked_transaction() {
        let conn = connection();

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
