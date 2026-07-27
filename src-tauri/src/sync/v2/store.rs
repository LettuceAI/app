use std::collections::BTreeMap;

use rusqlite::{params, Connection, OptionalExtension, Transaction};

use super::model::{ChangeRevision, Frontier, HybridTimestamp};
use super::changeset::RowChange;

const LOCAL_SEQUENCE_KEY: &str = "origin_sequence";
const HLC_WALL_KEY: &str = "hlc_wall_time_ms";
const HLC_COUNTER_KEY: &str = "hlc_counter";

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("sync v2 database error: {0}")]
    Database(#[from] rusqlite::Error),
}

pub fn create_schema(conn: &Connection) -> Result<(), StoreError> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS sync_v2_local_state (
          key TEXT PRIMARY KEY,
          value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS sync_v2_changes (
          change_id TEXT PRIMARY KEY,
          origin_device_id TEXT NOT NULL,
          origin_sequence INTEGER NOT NULL CHECK(origin_sequence > 0),
          hlc_wall_time_ms INTEGER NOT NULL,
          hlc_counter INTEGER NOT NULL,
          schema_fingerprint TEXT NOT NULL,
          changeset_hash TEXT NOT NULL,
          changeset BLOB NOT NULL,
          created_at INTEGER NOT NULL,
          UNIQUE(origin_device_id, origin_sequence)
        );

        CREATE TABLE IF NOT EXISTS sync_v2_change_context (
          change_id TEXT NOT NULL,
          origin_device_id TEXT NOT NULL,
          seen_sequence INTEGER NOT NULL CHECK(seen_sequence >= 0),
          PRIMARY KEY (change_id, origin_device_id),
          FOREIGN KEY (change_id) REFERENCES sync_v2_changes(change_id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS sync_v2_frontiers (
          origin_device_id TEXT PRIMARY KEY,
          contiguous_sequence INTEGER NOT NULL CHECK(contiguous_sequence >= 0)
        );

        CREATE TABLE IF NOT EXISTS sync_v2_peer_frontiers (
          peer_device_id TEXT NOT NULL,
          origin_device_id TEXT NOT NULL,
          acknowledged_sequence INTEGER NOT NULL CHECK(acknowledged_sequence >= 0),
          updated_at INTEGER NOT NULL,
          PRIMARY KEY (peer_device_id, origin_device_id)
        );

        CREATE TABLE IF NOT EXISTS sync_v2_row_versions (
          table_name TEXT NOT NULL,
          primary_key_hash TEXT NOT NULL,
          winning_change_id TEXT NOT NULL,
          tombstone INTEGER NOT NULL DEFAULT 0,
          PRIMARY KEY (table_name, primary_key_hash),
          FOREIGN KEY (winning_change_id) REFERENCES sync_v2_changes(change_id)
        );

        CREATE TABLE IF NOT EXISTS sync_v2_conflicts (
          conflict_id TEXT PRIMARY KEY,
          table_name TEXT NOT NULL,
          primary_key BLOB NOT NULL,
          local_change_id TEXT NOT NULL,
          incoming_change_id TEXT NOT NULL,
          local_row BLOB,
          incoming_row BLOB,
          operation TEXT NOT NULL,
          status TEXT NOT NULL CHECK(status IN ('unresolved', 'resolved')),
          detected_at INTEGER NOT NULL,
          resolved_by_change_id TEXT,
          UNIQUE(table_name, primary_key, local_change_id, incoming_change_id)
        );

        CREATE TABLE IF NOT EXISTS sync_v2_incoming_batches (
          batch_id TEXT PRIMARY KEY,
          peer_device_id TEXT NOT NULL,
          batch_hash TEXT NOT NULL,
          expected_revisions INTEGER NOT NULL,
          received_revisions INTEGER NOT NULL DEFAULT 0,
          state TEXT NOT NULL CHECK(state IN ('receiving', 'staged', 'applying', 'committed', 'failed')),
          last_error TEXT,
          created_at INTEGER NOT NULL,
          committed_at INTEGER
        );

        CREATE TABLE IF NOT EXISTS sync_v2_incoming_revisions (
          batch_id TEXT NOT NULL,
          change_id TEXT NOT NULL,
          ordinal INTEGER NOT NULL,
          revision BLOB NOT NULL,
          revision_hash TEXT NOT NULL,
          PRIMARY KEY (batch_id, change_id),
          UNIQUE(batch_id, ordinal),
          FOREIGN KEY (batch_id) REFERENCES sync_v2_incoming_batches(batch_id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS sync_v2_blobs (
          content_hash TEXT PRIMARY KEY,
          size_bytes INTEGER NOT NULL CHECK(size_bytes >= 0),
          relative_path TEXT,
          verified INTEGER NOT NULL DEFAULT 0,
          received_bytes INTEGER NOT NULL DEFAULT 0,
          created_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS sync_v2_change_blobs (
          change_id TEXT NOT NULL,
          content_hash TEXT NOT NULL,
          PRIMARY KEY (change_id, content_hash),
          FOREIGN KEY (change_id) REFERENCES sync_v2_changes(change_id) ON DELETE CASCADE,
          FOREIGN KEY (content_hash) REFERENCES sync_v2_blobs(content_hash)
        );

        CREATE INDEX IF NOT EXISTS idx_sync_v2_changes_origin
          ON sync_v2_changes(origin_device_id, origin_sequence);
        CREATE INDEX IF NOT EXISTS idx_sync_v2_conflicts_status
          ON sync_v2_conflicts(status, detected_at);
        CREATE INDEX IF NOT EXISTS idx_sync_v2_batches_peer_state
          ON sync_v2_incoming_batches(peer_device_id, state);
        "#,
    )?;
    Ok(())
}

pub fn load_frontier(conn: &Connection) -> Result<Frontier, StoreError> {
    let mut statement = conn.prepare(
        "SELECT origin_device_id, contiguous_sequence
         FROM sync_v2_frontiers
         ORDER BY origin_device_id",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;
    let mut frontier = BTreeMap::new();
    for row in rows {
        let (device_id, sequence) = row?;
        frontier.insert(device_id, sequence);
    }
    Ok(frontier)
}

pub fn load_revision(
    conn: &Connection,
    change_id: &str,
) -> Result<Option<ChangeRevision>, StoreError> {
    let revision = conn
        .query_row(
            "SELECT origin_device_id, origin_sequence, hlc_wall_time_ms, hlc_counter,
                    schema_fingerprint, changeset_hash, changeset
             FROM sync_v2_changes
             WHERE change_id = ?1",
            params![change_id],
            |row| {
                Ok(ChangeRevision {
                    change_id: change_id.to_string(),
                    origin_device_id: row.get(0)?,
                    origin_sequence: row.get(1)?,
                    timestamp: HybridTimestamp {
                        wall_time_ms: row.get(2)?,
                        counter: row.get(3)?,
                    },
                    base_frontier: BTreeMap::new(),
                    schema_fingerprint: row.get(4)?,
                    changeset_hash: row.get(5)?,
                    changeset: row.get(6)?,
                })
            },
        )
        .optional()?;
    let Some(mut revision) = revision else {
        return Ok(None);
    };

    let mut statement = conn.prepare(
        "SELECT origin_device_id, seen_sequence
         FROM sync_v2_change_context
         WHERE change_id = ?1
         ORDER BY origin_device_id",
    )?;
    let rows = statement.query_map(params![change_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;
    for row in rows {
        let (device_id, sequence) = row?;
        revision.base_frontier.insert(device_id, sequence);
    }
    Ok(Some(revision))
}

pub(crate) fn next_local_stamp(
    tx: &Transaction<'_>,
    now_ms: i64,
) -> Result<(i64, HybridTimestamp), rusqlite::Error> {
    let sequence = local_integer(tx, LOCAL_SEQUENCE_KEY)? + 1;
    let previous_wall = local_integer(tx, HLC_WALL_KEY)?;
    let previous_counter = local_integer(tx, HLC_COUNTER_KEY)?;
    let wall_time_ms = now_ms.max(previous_wall);
    let counter = if wall_time_ms == previous_wall {
        previous_counter + 1
    } else {
        0
    };

    set_local_integer(tx, LOCAL_SEQUENCE_KEY, sequence)?;
    set_local_integer(tx, HLC_WALL_KEY, wall_time_ms)?;
    set_local_integer(tx, HLC_COUNTER_KEY, counter)?;
    Ok((
        sequence,
        HybridTimestamp {
            wall_time_ms,
            counter,
        },
    ))
}

pub(crate) fn insert_local_revision(
    tx: &Transaction<'_>,
    revision: &ChangeRevision,
    created_at: i64,
) -> Result<(), rusqlite::Error> {
    insert_revision(tx, revision, created_at)?;
    advance_frontier(
        tx,
        &revision.origin_device_id,
        revision.origin_sequence,
    )?;
    Ok(())
}

pub(crate) fn insert_revision(
    tx: &Transaction<'_>,
    revision: &ChangeRevision,
    created_at: i64,
) -> Result<(), rusqlite::Error> {
    tx.execute(
        "INSERT INTO sync_v2_changes (
           change_id, origin_device_id, origin_sequence, hlc_wall_time_ms,
           hlc_counter, schema_fingerprint, changeset_hash, changeset, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            revision.change_id,
            revision.origin_device_id,
            revision.origin_sequence,
            revision.timestamp.wall_time_ms,
            revision.timestamp.counter,
            revision.schema_fingerprint,
            revision.changeset_hash,
            revision.changeset,
            created_at,
        ],
    )?;
    for (device_id, sequence) in &revision.base_frontier {
        tx.execute(
            "INSERT INTO sync_v2_change_context (change_id, origin_device_id, seen_sequence)
             VALUES (?1, ?2, ?3)",
            params![revision.change_id, device_id, sequence],
        )?;
    }
    Ok(())
}

pub(crate) fn advance_frontier(
    tx: &Transaction<'_>,
    origin_device_id: &str,
    origin_sequence: i64,
) -> Result<(), rusqlite::Error> {
    tx.execute(
        "INSERT INTO sync_v2_frontiers (origin_device_id, contiguous_sequence)
         VALUES (?1, ?2)
         ON CONFLICT(origin_device_id) DO UPDATE SET
           contiguous_sequence = MAX(contiguous_sequence, excluded.contiguous_sequence)",
        params![origin_device_id, origin_sequence],
    )?;
    Ok(())
}

pub(crate) fn set_row_version(
    tx: &Transaction<'_>,
    row: &RowChange,
    winning_revision: &ChangeRevision,
) -> Result<(), rusqlite::Error> {
    tx.execute(
        "INSERT INTO sync_v2_row_versions (
           table_name, primary_key_hash, winning_change_id, tombstone
         ) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(table_name, primary_key_hash) DO UPDATE SET
           winning_change_id = excluded.winning_change_id,
           tombstone = excluded.tombstone",
        params![
            row.table_name,
            row.primary_key_hash,
            winning_revision.change_id,
            i64::from(row.operation.is_delete()),
        ],
    )?;
    resolve_dominated_conflicts(tx, row, winning_revision)?;
    Ok(())
}

pub(crate) fn observe_remote_clock(
    tx: &Transaction<'_>,
    remote: HybridTimestamp,
    now_ms: i64,
) -> Result<(), rusqlite::Error> {
    let local_wall = local_integer(tx, HLC_WALL_KEY)?;
    let local_counter = local_integer(tx, HLC_COUNTER_KEY)?;
    let wall = now_ms.max(local_wall).max(remote.wall_time_ms);
    let counter = if wall == local_wall && wall == remote.wall_time_ms {
        local_counter.max(remote.counter) + 1
    } else if wall == local_wall {
        local_counter + 1
    } else if wall == remote.wall_time_ms {
        remote.counter + 1
    } else {
        0
    };
    set_local_integer(tx, HLC_WALL_KEY, wall)?;
    set_local_integer(tx, HLC_COUNTER_KEY, counter)?;
    Ok(())
}

fn local_integer(tx: &Transaction<'_>, key: &str) -> Result<i64, rusqlite::Error> {
    tx.query_row(
        "SELECT value FROM sync_v2_local_state WHERE key = ?1",
        params![key],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map(|value| value.and_then(|value| value.parse().ok()).unwrap_or(0))
}

fn set_local_integer(
    tx: &Transaction<'_>,
    key: &str,
    value: i64,
) -> Result<(), rusqlite::Error> {
    tx.execute(
        "INSERT INTO sync_v2_local_state (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value.to_string()],
    )?;
    Ok(())
}

fn resolve_dominated_conflicts(
    tx: &Transaction<'_>,
    row: &RowChange,
    winner: &ChangeRevision,
) -> Result<(), rusqlite::Error> {
    let mut statement = tx.prepare(
        "SELECT conflict_id, local_change_id, incoming_change_id
         FROM sync_v2_conflicts
         WHERE table_name = ?1 AND primary_key = ?2 AND status = 'unresolved'",
    )?;
    let rows = statement.query_map(
        params![row.table_name, row.primary_key_bytes],
        |result| {
        Ok((
            result.get::<_, String>(0)?,
            result.get::<_, String>(1)?,
            result.get::<_, String>(2)?,
        ))
        },
    )?;
    let mut resolved = Vec::new();
    for result in rows {
        let (conflict_id, left_id, right_id) = result?;
        let left = load_revision_database_error(tx, &left_id)?
            .ok_or_else(|| rusqlite::Error::QueryReturnedNoRows)?;
        let right = load_revision_database_error(tx, &right_id)?
            .ok_or_else(|| rusqlite::Error::QueryReturnedNoRows)?;
        if winner.observes(&left) && winner.observes(&right) {
            resolved.push(conflict_id);
        }
    }
    drop(statement);

    for conflict_id in resolved {
        tx.execute(
            "UPDATE sync_v2_conflicts
             SET status = 'resolved', resolved_by_change_id = ?1
             WHERE conflict_id = ?2",
            params![winner.change_id, conflict_id],
        )?;
    }
    Ok(())
}

fn load_revision_database_error(
    conn: &Connection,
    change_id: &str,
) -> Result<Option<ChangeRevision>, rusqlite::Error> {
    match load_revision(conn, change_id) {
        Ok(revision) => Ok(revision),
        Err(StoreError::Database(error)) => Err(error),
    }
}
