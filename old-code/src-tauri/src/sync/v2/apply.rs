use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::Arc;

use rusqlite::session::{invert_strm, ConflictAction, ConflictType};
use rusqlite::types::Value;
use rusqlite::{
    params, params_from_iter, Connection, OptionalExtension, Transaction,
    TransactionBehavior,
};

use super::catalog::{cached_schema_fingerprint, CatalogError};
use super::changeset::{inspect_changeset, inspect_item, RowChange};
use super::model::{ApplyOutcome, ApplyResult, ChangeRevision};
use super::store::{
    advance_frontier, insert_revision, load_frontier, load_revision, observe_remote_clock,
    set_row_version, StoreError,
};

#[derive(Debug, thiserror::Error)]
pub enum ApplyError {
    #[error("sync v2 database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("revision identity is incomplete")]
    InvalidIdentity,
    #[error("revision {change_id} has an invalid changeset hash")]
    InvalidHash { change_id: String },
    #[error(
        "revision {change_id} uses schema {received}, but this device uses {expected}"
    )]
    SchemaMismatch {
        change_id: String,
        expected: String,
        received: String,
    },
    #[error(
        "revision {change_id} has origin sequence {received}; expected {expected} for {origin_device_id}"
    )]
    NonContiguousSequence {
        change_id: String,
        origin_device_id: String,
        expected: i64,
        received: i64,
    },
    #[error(
        "revision {change_id} depends on {origin_device_id}:{required}, but only {available} is applied"
    )]
    MissingDependency {
        change_id: String,
        origin_device_id: String,
        required: i64,
        available: i64,
    },
    #[error("revision {change_id} already exists with different content")]
    DuplicateMismatch { change_id: String },
    #[error("revision {change_id} could not be applied without violating database constraints")]
    ApplyConflict { change_id: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowPolicy {
    Incoming,
    Current,
}

struct PendingConflict {
    row: RowChange,
    current_change_id: String,
    current_row: RowSnapshot,
    incoming_row: Option<RowSnapshot>,
}

#[derive(Clone)]
struct RowSnapshot {
    row: RowChange,
    columns: Vec<String>,
    values: Option<Vec<Value>>,
}

pub fn apply_remote_revision(
    conn: &Connection,
    revision: &ChangeRevision,
    now_ms: i64,
) -> Result<ApplyResult, ApplyError> {
    validate_revision(conn, revision)?;
    let changes = inspect_changeset(&revision.changeset)?;
    let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;

    if let Some(existing) = load_revision(&tx, &revision.change_id)? {
        if existing != *revision {
            return Err(ApplyError::DuplicateMismatch {
                change_id: revision.change_id.clone(),
            });
        }
        return Ok(ApplyResult {
            outcome: ApplyOutcome::Duplicate,
            conflicts_created: 0,
            branches_created: 0,
        });
    }

    let frontier = load_frontier(&tx)?;
    let available = frontier
        .get(&revision.origin_device_id)
        .copied()
        .unwrap_or(0);
    let expected = available + 1;
    if revision.origin_sequence != expected {
        return Err(ApplyError::NonContiguousSequence {
            change_id: revision.change_id.clone(),
            origin_device_id: revision.origin_device_id.clone(),
            expected,
            received: revision.origin_sequence,
        });
    }
    for (device_id, required) in &revision.base_frontier {
        let available = frontier.get(device_id).copied().unwrap_or(0);
        if available < *required {
            return Err(ApplyError::MissingDependency {
                change_id: revision.change_id.clone(),
                origin_device_id: device_id.clone(),
                required: *required,
                available,
            });
        }
    }

    let mut policies = HashMap::new();
    let mut losing_snapshots = Vec::new();
    let mut pending_conflicts = Vec::new();
    let mut incoming_winners = Vec::new();

    for row in changes {
        let current = load_row_revision(&tx, &row)?;
        let (policy, is_concurrent) = match current.as_ref() {
            None => (RowPolicy::Incoming, false),
            Some((current_revision, current_tombstone)) => {
                choose_policy(revision, &row, current_revision, *current_tombstone)
            }
        };

        policies.insert(row_identity(&row), policy);
        let current_snapshot = if policy == RowPolicy::Current || is_concurrent {
            Some(load_row_snapshot(&tx, row.clone())?)
        } else {
            None
        };
        if policy == RowPolicy::Current {
            losing_snapshots.push(
                current_snapshot
                    .as_ref()
                    .expect("current policy always captures the current row")
                    .clone(),
            );
        } else {
            incoming_winners.push(row.clone());
        }
        if is_concurrent {
            let (current_revision, _) =
                current.expect("concurrent rows always have current provenance");
            let current_row =
                current_snapshot.expect("concurrent rows always capture the current row");
            let incoming_row = materialize_incoming_snapshot(
                &tx,
                &row,
                &current_row,
                &current_revision,
                revision,
            )
            .ok();
            pending_conflicts.push(PendingConflict {
                row,
                current_change_id: current_revision.change_id,
                current_row,
                incoming_row,
            });
        }
    }

    insert_revision(&tx, revision, now_ms)?;
    tx.execute(
        "INSERT INTO sync_v2_local_state (key, value)
         VALUES ('applying_remote', '1')
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [],
    )?;
    apply_native_changeset(&tx, revision, policies)?;
    for snapshot in &losing_snapshots {
        restore_snapshot(&tx, snapshot)?;
    }

    for row in &incoming_winners {
        set_row_version(&tx, row, revision)?;
    }
    let branches_created = super::branches::materialize_message_forks(&tx)?;
    tx.execute(
        "DELETE FROM sync_v2_local_state WHERE key = 'applying_remote'",
        [],
    )?;
    let mut conflicts_created = 0;
    for conflict in pending_conflicts {
        conflicts_created += insert_conflict(&tx, revision, &conflict, now_ms)?;
    }
    advance_frontier(
        &tx,
        &revision.origin_device_id,
        revision.origin_sequence,
    )?;
    observe_remote_clock(&tx, revision.timestamp, now_ms)?;
    tx.commit()?;

    Ok(ApplyResult {
        outcome: ApplyOutcome::Applied,
        conflicts_created,
        branches_created,
    })
}

fn validate_revision(conn: &Connection, revision: &ChangeRevision) -> Result<(), ApplyError> {
    if revision.change_id.is_empty()
        || revision.origin_device_id.is_empty()
        || revision.origin_sequence <= 0
        || revision
            .base_frontier
            .values()
            .any(|sequence| *sequence < 0)
    {
        return Err(ApplyError::InvalidIdentity);
    }
    let expected_origin_base = revision.origin_sequence - 1;
    let origin_base = revision
        .base_frontier
        .get(&revision.origin_device_id)
        .copied()
        .unwrap_or(0);
    if origin_base != expected_origin_base {
        return Err(ApplyError::NonContiguousSequence {
            change_id: revision.change_id.clone(),
            origin_device_id: revision.origin_device_id.clone(),
            expected: expected_origin_base,
            received: origin_base,
        });
    }
    if revision.changeset_hash != blake3::hash(&revision.changeset).to_hex().to_string() {
        return Err(ApplyError::InvalidHash {
            change_id: revision.change_id.clone(),
        });
    }
    let expected = cached_schema_fingerprint(conn)?;
    if revision.schema_fingerprint != expected {
        return Err(ApplyError::SchemaMismatch {
            change_id: revision.change_id.clone(),
            expected,
            received: revision.schema_fingerprint.clone(),
        });
    }
    if let Some(existing_change_id) = conn
        .query_row(
            "SELECT change_id
             FROM sync_v2_changes
             WHERE origin_device_id = ?1 AND origin_sequence = ?2",
            params![revision.origin_device_id, revision.origin_sequence],
            |row| row.get::<_, String>(0),
        )
        .optional()?
    {
        if existing_change_id != revision.change_id {
            return Err(ApplyError::DuplicateMismatch {
                change_id: revision.change_id.clone(),
            });
        }
    }
    Ok(())
}

fn choose_policy(
    incoming: &ChangeRevision,
    incoming_row: &RowChange,
    current: &ChangeRevision,
    current_tombstone: bool,
) -> (RowPolicy, bool) {
    if incoming.observes(current) {
        return (RowPolicy::Incoming, false);
    }
    if current.observes(incoming) {
        return (RowPolicy::Current, false);
    }

    let incoming_tombstone = incoming_row.operation.is_delete();
    let policy = match (incoming_tombstone, current_tombstone) {
        (true, false) => RowPolicy::Incoming,
        (false, true) => RowPolicy::Current,
        _ if compare_revisions(incoming, current) == Ordering::Greater => RowPolicy::Incoming,
        _ => RowPolicy::Current,
    };
    (policy, true)
}

fn compare_revisions(left: &ChangeRevision, right: &ChangeRevision) -> Ordering {
    (
        left.timestamp,
        &left.origin_device_id,
        left.origin_sequence,
        &left.change_id,
    )
        .cmp(&(
            right.timestamp,
            &right.origin_device_id,
            right.origin_sequence,
            &right.change_id,
        ))
}

fn load_row_revision(
    tx: &Transaction<'_>,
    row: &RowChange,
) -> Result<Option<(ChangeRevision, bool)>, ApplyError> {
    let current = tx
        .query_row(
            "SELECT winning_change_id, tombstone
             FROM sync_v2_row_versions
             WHERE table_name = ?1 AND primary_key_hash = ?2",
            params![row.table_name, row.primary_key_hash],
            |result| Ok((result.get::<_, String>(0)?, result.get::<_, bool>(1)?)),
        )
        .optional()?;
    let Some((change_id, tombstone)) = current else {
        return Ok(None);
    };
    let revision = load_revision(tx, &change_id)?.ok_or_else(|| {
        ApplyError::DuplicateMismatch {
            change_id: change_id.clone(),
        }
    })?;
    Ok(Some((revision, tombstone)))
}

fn apply_native_changeset(
    tx: &Transaction<'_>,
    revision: &ChangeRevision,
    policies: HashMap<String, RowPolicy>,
) -> Result<(), ApplyError> {
    let policies = Arc::new(policies);
    let callback_failed = Arc::new(AtomicBool::new(false));
    let callback_failed_inner = Arc::clone(&callback_failed);
    let policies_inner = Arc::clone(&policies);
    let mut input = revision.changeset.as_slice();

    let result = tx.apply_strm(
        &mut input,
        None::<fn(&str) -> bool>,
        move |conflict_type, item| {
            if matches!(
                conflict_type,
                ConflictType::SQLITE_CHANGESET_CONSTRAINT
                    | ConflictType::SQLITE_CHANGESET_FOREIGN_KEY
                    | ConflictType::UNKNOWN
            ) {
                callback_failed_inner.store(true, AtomicOrdering::Relaxed);
                return ConflictAction::SQLITE_CHANGESET_ABORT;
            }

            let row = match inspect_item(&item) {
                Ok(row) => row,
                Err(_) => {
                    callback_failed_inner.store(true, AtomicOrdering::Relaxed);
                    return ConflictAction::SQLITE_CHANGESET_ABORT;
                }
            };
            match policies_inner.get(&row_identity(&row)) {
                Some(RowPolicy::Current) => ConflictAction::SQLITE_CHANGESET_OMIT,
                Some(RowPolicy::Incoming)
                    if conflict_type == ConflictType::SQLITE_CHANGESET_NOTFOUND
                        && row.operation.is_delete() =>
                {
                    ConflictAction::SQLITE_CHANGESET_OMIT
                }
                Some(RowPolicy::Incoming)
                    if matches!(
                        conflict_type,
                        ConflictType::SQLITE_CHANGESET_DATA
                            | ConflictType::SQLITE_CHANGESET_CONFLICT
                    ) =>
                {
                    ConflictAction::SQLITE_CHANGESET_REPLACE
                }
                _ => {
                    callback_failed_inner.store(true, AtomicOrdering::Relaxed);
                    ConflictAction::SQLITE_CHANGESET_ABORT
                }
            }
        },
    );

    if result.is_err() || callback_failed.load(AtomicOrdering::Relaxed) {
        return Err(ApplyError::ApplyConflict {
            change_id: revision.change_id.clone(),
        });
    }
    Ok(())
}

fn load_row_snapshot(
    tx: &Transaction<'_>,
    row: RowChange,
) -> Result<RowSnapshot, rusqlite::Error> {
    let (columns, primary_key_columns) = table_columns(tx, &row.table_name)?;
    if primary_key_columns.len() != row.primary_key.len() {
        return Err(rusqlite::Error::InvalidParameterName(format!(
            "primary key shape changed for {}",
            row.table_name
        )));
    }
    let selected_columns = columns
        .iter()
        .map(|column| quote_identifier(column))
        .collect::<Vec<_>>()
        .join(", ");
    let predicate = primary_key_columns
        .iter()
        .map(|column| format!("{} IS ?", quote_identifier(column)))
        .collect::<Vec<_>>()
        .join(" AND ");
    let sql = format!(
        "SELECT {selected_columns} FROM {} WHERE {predicate}",
        quote_identifier(&row.table_name)
    );
    let values = tx
        .query_row(&sql, params_from_iter(row.primary_key.iter()), |result| {
            let mut values = Vec::with_capacity(columns.len());
            for column in 0..columns.len() {
                values.push(result.get_ref(column)?.into());
            }
            Ok(values)
        })
        .optional()?;
    Ok(RowSnapshot {
        row,
        columns,
        values,
    })
}

fn restore_snapshot(tx: &Transaction<'_>, snapshot: &RowSnapshot) -> Result<(), rusqlite::Error> {
    let (_, primary_key_columns) = table_columns(tx, &snapshot.row.table_name)?;
    if let Some(values) = &snapshot.values {
        let columns = snapshot
            .columns
            .iter()
            .map(|column| quote_identifier(column))
            .collect::<Vec<_>>()
            .join(", ");
        let placeholders = (1..=values.len())
            .map(|index| format!("?{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let updates = snapshot
            .columns
            .iter()
            .map(|column| {
                let column = quote_identifier(column);
                format!("{column} = excluded.{column}")
            })
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "INSERT INTO {} ({columns}) VALUES ({placeholders})
             ON CONFLICT DO UPDATE SET {updates}",
            quote_identifier(&snapshot.row.table_name)
        );
        tx.execute(&sql, params_from_iter(values.iter()))?;
    } else {
        let predicate = primary_key_columns
            .iter()
            .map(|column| format!("{} IS ?", quote_identifier(column)))
            .collect::<Vec<_>>()
            .join(" AND ");
        let sql = format!(
            "DELETE FROM {} WHERE {predicate}",
            quote_identifier(&snapshot.row.table_name)
        );
        tx.execute(
            &sql,
            params_from_iter(snapshot.row.primary_key.iter()),
        )?;
    }
    Ok(())
}

fn materialize_incoming_snapshot(
    tx: &Transaction<'_>,
    row: &RowChange,
    current_row: &RowSnapshot,
    current_revision: &ChangeRevision,
    incoming_revision: &ChangeRevision,
) -> Result<RowSnapshot, rusqlite::Error> {
    let create_sql = tx.query_row(
        "SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = ?1",
        params![row.table_name],
        |result| result.get::<_, String>(0),
    )?;
    let scratch = Connection::open_in_memory()?;
    scratch.execute_batch("PRAGMA foreign_keys = OFF;")?;
    scratch.execute_batch(&create_sql)?;

    if current_row.values.is_some() {
        restore_snapshot_on_connection(&scratch, current_row)?;
    } else {
        let mut input = current_revision.changeset.as_slice();
        let mut inverted = Vec::new();
        invert_strm(&mut input, &mut inverted)?;
        apply_snapshot_changeset(&scratch, &row.table_name, &inverted)?;
    }
    apply_snapshot_changeset(
        &scratch,
        &row.table_name,
        &incoming_revision.changeset,
    )?;
    load_row_snapshot_from_connection(&scratch, row.clone())
}

fn apply_snapshot_changeset(
    conn: &Connection,
    table_name: &str,
    changeset: &[u8],
) -> Result<(), rusqlite::Error> {
    let target = table_name.to_string();
    let mut input = changeset;
    conn.apply_strm(
        &mut input,
        Some(move |table: &str| table == target),
        |conflict_type, item| match conflict_type {
            ConflictType::SQLITE_CHANGESET_DATA | ConflictType::SQLITE_CHANGESET_CONFLICT => {
                ConflictAction::SQLITE_CHANGESET_REPLACE
            }
            ConflictType::SQLITE_CHANGESET_NOTFOUND => match item.op() {
                Ok(operation) if operation.code() == rusqlite::hooks::Action::SQLITE_DELETE => {
                    ConflictAction::SQLITE_CHANGESET_OMIT
                }
                _ => ConflictAction::SQLITE_CHANGESET_ABORT,
            },
            _ => ConflictAction::SQLITE_CHANGESET_ABORT,
        },
    )
}

fn load_row_snapshot_from_connection(
    conn: &Connection,
    row: RowChange,
) -> Result<RowSnapshot, rusqlite::Error> {
    let (columns, primary_key_columns) = table_columns(conn, &row.table_name)?;
    let selected_columns = columns
        .iter()
        .map(|column| quote_identifier(column))
        .collect::<Vec<_>>()
        .join(", ");
    let predicate = primary_key_columns
        .iter()
        .map(|column| format!("{} IS ?", quote_identifier(column)))
        .collect::<Vec<_>>()
        .join(" AND ");
    let sql = format!(
        "SELECT {selected_columns} FROM {} WHERE {predicate}",
        quote_identifier(&row.table_name)
    );
    let values = conn
        .query_row(&sql, params_from_iter(row.primary_key.iter()), |result| {
            let mut values = Vec::with_capacity(columns.len());
            for column in 0..columns.len() {
                values.push(result.get_ref(column)?.into());
            }
            Ok(values)
        })
        .optional()?;
    Ok(RowSnapshot {
        row,
        columns,
        values,
    })
}

fn restore_snapshot_on_connection(
    conn: &Connection,
    snapshot: &RowSnapshot,
) -> Result<(), rusqlite::Error> {
    let tx = conn.unchecked_transaction()?;
    restore_snapshot(&tx, snapshot)?;
    tx.commit()
}

fn table_columns(
    conn: &Connection,
    table_name: &str,
) -> Result<(Vec<String>, Vec<String>), rusqlite::Error> {
    let escaped_table = table_name.replace('\'', "''");
    let mut statement = conn.prepare(&format!(
        "SELECT name, pk FROM pragma_table_info('{escaped_table}') ORDER BY cid"
    ))?;
    let rows = statement.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;
    let mut columns = Vec::new();
    let mut primary_key_columns = Vec::new();
    for row in rows {
        let (column, primary_key_position) = row?;
        if primary_key_position > 0 {
            primary_key_columns.push(column.clone());
        }
        columns.push(column);
    }
    Ok((columns, primary_key_columns))
}

fn insert_conflict(
    tx: &Transaction<'_>,
    incoming: &ChangeRevision,
    conflict: &PendingConflict,
    now_ms: i64,
) -> Result<usize, rusqlite::Error> {
    let mut revision_ids = [
        conflict.current_change_id.as_str(),
        incoming.change_id.as_str(),
    ];
    revision_ids.sort_unstable();
    let identity = format!(
        "{}\0{}\0{}\0{}",
        conflict.row.table_name,
        conflict.row.primary_key_hash,
        revision_ids[0],
        revision_ids[1]
    );
    let conflict_id = blake3::hash(identity.as_bytes()).to_hex().to_string();
    let local_row = super::conflicts::encode_row_snapshot(
        &conflict.current_row.columns,
        &conflict.current_row.row.primary_key,
        conflict.current_row.values.as_deref(),
    )?;
    let incoming_row = conflict
        .incoming_row
        .as_ref()
        .map(|snapshot| {
            super::conflicts::encode_row_snapshot(
                &snapshot.columns,
                &snapshot.row.primary_key,
                snapshot.values.as_deref(),
            )
        })
        .transpose()?;
    tx.execute(
        "INSERT OR IGNORE INTO sync_v2_conflicts (
           conflict_id, table_name, primary_key, local_change_id,
           incoming_change_id, local_row, incoming_row, operation, status, detected_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'unresolved', ?9)",
        params![
            conflict_id,
            conflict.row.table_name,
            conflict.row.primary_key_bytes,
            conflict.current_change_id,
            incoming.change_id,
            local_row,
            incoming_row,
            conflict.row.operation.as_str(),
            now_ms,
        ],
    )
}

fn row_identity(row: &RowChange) -> String {
    format!("{}\0{}", row.table_name, row.primary_key_hash)
}

fn quote_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use rusqlite::{params, Connection};

    use super::{apply_remote_revision, ApplyError};
    use crate::sync::v2::{
        capture_transaction, create_schema, load_frontier, ApplyOutcome, ChangeRevision,
    };

    fn connection() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialize_connection(&conn);
        conn
    }

    fn initialize_connection(conn: &Connection) {
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE chats (
               id TEXT PRIMARY KEY,
               title TEXT NOT NULL
             );
             CREATE TABLE local_usage (
               id INTEGER PRIMARY KEY,
               elapsed_ms INTEGER NOT NULL
             );",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO local_usage (id, elapsed_ms) VALUES (1, 0)",
            [],
        )
        .unwrap();
        create_schema(conn).unwrap();
    }

    fn title(conn: &Connection) -> Option<String> {
        conn.query_row("SELECT title FROM chats WHERE id = 'chat-1'", [], |row| {
            row.get(0)
        })
        .optional()
        .unwrap()
    }

    use rusqlite::OptionalExtension;

    #[test]
    fn applies_clean_revision_and_treats_replay_as_duplicate() {
        let source = connection();
        let target = connection();
        let revision = capture_transaction(&source, "device-a", 100, |tx| {
            tx.execute(
                "INSERT INTO chats (id, title) VALUES (?1, ?2)",
                params!["chat-1", "Hello"],
            )
        })
        .unwrap()
        .revision
        .unwrap();

        let applied = apply_remote_revision(&target, &revision, 110).unwrap();
        assert_eq!(applied.outcome, ApplyOutcome::Applied);
        assert_eq!(title(&target).as_deref(), Some("Hello"));
        assert_eq!(load_frontier(&target).unwrap().get("device-a"), Some(&1));

        let duplicate = apply_remote_revision(&target, &revision, 120).unwrap();
        assert_eq!(duplicate.outcome, ApplyOutcome::Duplicate);
        assert_eq!(title(&target).as_deref(), Some("Hello"));
    }

    #[test]
    fn waits_for_a_concurrent_local_writer_before_reading_apply_state() {
        let source = connection();
        let revision = capture_transaction(&source, "device-a", 100, |tx| {
            tx.execute(
                "INSERT INTO chats (id, title) VALUES (?1, ?2)",
                params!["chat-1", "Hello"],
            )
        })
        .unwrap()
        .revision
        .unwrap();

        let path = std::env::temp_dir().join(format!(
            "lettuce-sync-v2-writer-collision-{}.db",
            uuid::Uuid::new_v4()
        ));
        {
            let setup = Connection::open(&path).unwrap();
            setup.pragma_update(None, "journal_mode", "WAL").unwrap();
            initialize_connection(&setup);
        }

        let blocker = Connection::open(&path).unwrap();
        blocker.busy_timeout(Duration::from_secs(2)).unwrap();
        let blocker_tx =
            rusqlite::Transaction::new_unchecked(
                &blocker,
                rusqlite::TransactionBehavior::Immediate,
            )
            .unwrap();
        blocker_tx
            .execute(
                "UPDATE local_usage SET elapsed_ms = elapsed_ms + 30000 WHERE id = 1",
                [],
            )
            .unwrap();

        let apply_path = path.clone();
        let apply = std::thread::spawn(move || {
            let target = Connection::open(apply_path).unwrap();
            target.busy_timeout(Duration::from_secs(2)).unwrap();
            apply_remote_revision(&target, &revision, 110)
        });
        std::thread::sleep(Duration::from_millis(100));
        blocker_tx.commit().unwrap();

        let applied = apply.join().unwrap().unwrap();
        assert_eq!(applied.outcome, ApplyOutcome::Applied);
        let target = Connection::open(&path).unwrap();
        assert_eq!(title(&target).as_deref(), Some("Hello"));
        assert_eq!(
            target
                .query_row(
                    "SELECT elapsed_ms FROM local_usage WHERE id = 1",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            30_000
        );
        drop(target);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn rejects_missing_predecessors_without_advancing_frontier() {
        let source = connection();
        let target = connection();
        let first = capture_transaction(&source, "device-a", 100, |tx| {
            tx.execute(
                "INSERT INTO chats (id, title) VALUES ('chat-1', 'one')",
                [],
            )
        })
        .unwrap()
        .revision
        .unwrap();
        let second = capture_transaction(&source, "device-a", 200, |tx| {
            tx.execute("UPDATE chats SET title = 'two' WHERE id = 'chat-1'", [])
        })
        .unwrap()
        .revision
        .unwrap();

        assert!(matches!(
            apply_remote_revision(&target, &second, 210),
            Err(ApplyError::NonContiguousSequence { .. })
        ));
        assert!(load_frontier(&target).unwrap().is_empty());
        assert_eq!(title(&target), None);
        apply_remote_revision(&target, &first, 220).unwrap();
        apply_remote_revision(&target, &second, 230).unwrap();
        assert_eq!(title(&target).as_deref(), Some("two"));
    }

    #[test]
    fn concurrent_updates_converge_and_preserve_a_conflict() {
        let left = connection();
        let right = connection();
        let base = capture_transaction(&left, "device-a", 100, |tx| {
            tx.execute(
                "INSERT INTO chats (id, title) VALUES ('chat-1', 'base')",
                [],
            )
        })
        .unwrap()
        .revision
        .unwrap();
        apply_remote_revision(&right, &base, 110).unwrap();

        let left_edit = capture_transaction(&left, "device-a", 200, |tx| {
            tx.execute("UPDATE chats SET title = 'left' WHERE id = 'chat-1'", [])
        })
        .unwrap()
        .revision
        .unwrap();
        let right_edit = capture_transaction(&right, "device-b", 300, |tx| {
            tx.execute("UPDATE chats SET title = 'right' WHERE id = 'chat-1'", [])
        })
        .unwrap()
        .revision
        .unwrap();

        let left_result = apply_remote_revision(&left, &right_edit, 310).unwrap();
        let right_result = apply_remote_revision(&right, &left_edit, 320).unwrap();

        assert_eq!(title(&left).as_deref(), Some("right"));
        assert_eq!(title(&right).as_deref(), Some("right"));
        assert_eq!(left_result.conflicts_created, 1);
        assert_eq!(right_result.conflicts_created, 1);
        for conn in [&left, &right] {
            assert_eq!(
                conn.query_row(
                    "SELECT COUNT(*) FROM sync_v2_conflicts WHERE status = 'unresolved'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
                1
            );
        }
    }

    #[test]
    fn concurrent_delete_beats_update_and_prevents_resurrection() {
        let left = connection();
        let right = connection();
        let base = capture_transaction(&left, "device-a", 100, |tx| {
            tx.execute(
                "INSERT INTO chats (id, title) VALUES ('chat-1', 'base')",
                [],
            )
        })
        .unwrap()
        .revision
        .unwrap();
        apply_remote_revision(&right, &base, 110).unwrap();

        let update = capture_transaction(&left, "device-a", 400, |tx| {
            tx.execute("UPDATE chats SET title = 'updated' WHERE id = 'chat-1'", [])
        })
        .unwrap()
        .revision
        .unwrap();
        let delete = capture_transaction(&right, "device-b", 200, |tx| {
            tx.execute("DELETE FROM chats WHERE id = 'chat-1'", [])
        })
        .unwrap()
        .revision
        .unwrap();

        apply_remote_revision(&left, &delete, 410).unwrap();
        apply_remote_revision(&right, &update, 420).unwrap();

        assert_eq!(title(&left), None);
        assert_eq!(title(&right), None);
    }

    #[test]
    fn conflict_resolution_is_a_normal_change_that_resolves_every_replica() {
        let left = connection();
        let right = connection();
        let base = capture_transaction(&left, "device-a", 100, |tx| {
            tx.execute(
                "INSERT INTO chats (id, title) VALUES ('chat-1', 'base')",
                [],
            )
        })
        .unwrap()
        .revision
        .unwrap();
        apply_remote_revision(&right, &base, 110).unwrap();
        let left_edit = capture_transaction(&left, "device-a", 200, |tx| {
            tx.execute("UPDATE chats SET title = 'left' WHERE id = 'chat-1'", [])
        })
        .unwrap()
        .revision
        .unwrap();
        let right_edit = capture_transaction(&right, "device-b", 300, |tx| {
            tx.execute("UPDATE chats SET title = 'right' WHERE id = 'chat-1'", [])
        })
        .unwrap()
        .revision
        .unwrap();
        apply_remote_revision(&left, &right_edit, 310).unwrap();
        apply_remote_revision(&right, &left_edit, 320).unwrap();

        let resolution = capture_transaction(&left, "device-a", 400, |tx| {
            tx.execute(
                "UPDATE chats SET title = 'resolved' WHERE id = 'chat-1'",
                [],
            )
        })
        .unwrap()
        .revision
        .unwrap();
        apply_remote_revision(&right, &resolution, 410).unwrap();

        for conn in [&left, &right] {
            assert_eq!(title(conn).as_deref(), Some("resolved"));
            assert_eq!(
                conn.query_row(
                    "SELECT COUNT(*) FROM sync_v2_conflicts WHERE status = 'unresolved'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
                0
            );
        }
    }

    #[test]
    fn three_replicas_converge_after_concurrent_edits_in_different_orders() {
        let first = connection();
        let second = connection();
        let third = connection();
        let base = capture_transaction(&first, "device-a", 100, |tx| {
            tx.execute(
                "INSERT INTO chats (id, title) VALUES ('chat-1', 'base')",
                [],
            )
        })
        .unwrap()
        .revision
        .unwrap();
        apply_remote_revision(&second, &base, 110).unwrap();
        apply_remote_revision(&third, &base, 120).unwrap();

        let first_edit = capture_transaction(&first, "device-a", 200, |tx| {
            tx.execute("UPDATE chats SET title = 'first' WHERE id = 'chat-1'", [])
        })
        .unwrap()
        .revision
        .unwrap();
        let second_edit = capture_transaction(&second, "device-b", 300, |tx| {
            tx.execute("UPDATE chats SET title = 'second' WHERE id = 'chat-1'", [])
        })
        .unwrap()
        .revision
        .unwrap();
        let third_edit = capture_transaction(&third, "device-c", 250, |tx| {
            tx.execute("UPDATE chats SET title = 'third' WHERE id = 'chat-1'", [])
        })
        .unwrap()
        .revision
        .unwrap();

        apply_remote_revision(&first, &second_edit, 310).unwrap();
        apply_remote_revision(&first, &third_edit, 320).unwrap();
        apply_remote_revision(&second, &third_edit, 330).unwrap();
        apply_remote_revision(&second, &first_edit, 340).unwrap();
        apply_remote_revision(&third, &first_edit, 350).unwrap();
        apply_remote_revision(&third, &second_edit, 360).unwrap();

        for conn in [&first, &second, &third] {
            assert_eq!(title(conn).as_deref(), Some("second"));
            assert_eq!(
                load_frontier(conn).unwrap(),
                std::collections::BTreeMap::from([
                    ("device-a".to_string(), 2),
                    ("device-b".to_string(), 1),
                    ("device-c".to_string(), 1),
                ])
            );
        }
    }

    #[test]
    fn rejects_schema_and_hash_mismatches() {
        let source = connection();
        let target = connection();
        let revision = capture_transaction(&source, "device-a", 100, |tx| {
            tx.execute(
                "INSERT INTO chats (id, title) VALUES ('chat-1', 'base')",
                [],
            )
        })
        .unwrap()
        .revision
        .unwrap();

        let mut bad_hash: ChangeRevision = revision.clone();
        bad_hash.changeset_hash = "broken".to_string();
        assert!(matches!(
            apply_remote_revision(&target, &bad_hash, 110),
            Err(ApplyError::InvalidHash { .. })
        ));

        let mut bad_schema = revision;
        bad_schema.schema_fingerprint = "other-schema".to_string();
        assert!(matches!(
            apply_remote_revision(&target, &bad_schema, 110),
            Err(ApplyError::SchemaMismatch { .. })
        ));
        assert_eq!(title(&target), None);
    }
}
