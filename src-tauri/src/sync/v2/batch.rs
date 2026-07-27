use rusqlite::{params, Connection, OptionalExtension};

use super::apply::{apply_remote_revision, ApplyError};
use super::model::{ApplyOutcome, ChangeRevision};
use super::store::{load_frontier, load_revision};

pub const MAX_REVISIONS_PER_BATCH: usize = 128;
pub const MAX_REVISION_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_BATCH_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageOutcome {
    Staged,
    Duplicate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchApplyResult {
    pub revisions_applied: usize,
    pub revisions_already_applied: usize,
    pub conflicts_created: usize,
    pub already_committed: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum BatchError {
    #[error("sync v2 database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("sync v2 batch encoding error: {0}")]
    Encoding(#[from] Box<bincode::ErrorKind>),
    #[error("batch identity is incomplete")]
    InvalidIdentity,
    #[error("batch has {received} revisions; the limit is {limit}")]
    TooManyRevisions { received: usize, limit: usize },
    #[error("revision {change_id} has {received} bytes; the limit is {limit}")]
    RevisionTooLarge {
        change_id: String,
        received: usize,
        limit: usize,
    },
    #[error("batch has {received} bytes; the limit is {limit}")]
    BatchTooLarge { received: usize, limit: usize },
    #[error("batch {batch_id} has an invalid hash")]
    InvalidBatchHash { batch_id: String },
    #[error("batch {batch_id} already exists with different content")]
    DuplicateMismatch { batch_id: String },
    #[error("batch {batch_id} was not staged")]
    NotStaged { batch_id: String },
    #[error("staged revision {change_id} is corrupt")]
    CorruptRevision { change_id: String },
    #[error("failed to apply staged batch {batch_id}: {source}")]
    Apply {
        batch_id: String,
        #[source]
        source: ApplyError,
    },
}

pub fn revision_batch_hash(revisions: &[ChangeRevision]) -> Result<String, BatchError> {
    let encoded = encode_revisions(revisions)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(&(encoded.len() as u64).to_le_bytes());
    for revision in encoded {
        hasher.update(&(revision.len() as u64).to_le_bytes());
        hasher.update(&revision);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

pub fn stage_revision_batch(
    conn: &Connection,
    peer_device_id: &str,
    batch_id: &str,
    declared_batch_hash: &str,
    revisions: &[ChangeRevision],
    now_ms: i64,
) -> Result<StageOutcome, BatchError> {
    if peer_device_id.is_empty() || batch_id.is_empty() {
        return Err(BatchError::InvalidIdentity);
    }
    let encoded = encode_revisions(revisions)?;
    validate_batch_sizes(revisions, &encoded)?;
    let computed_hash = hash_encoded_revisions(&encoded);
    if computed_hash != declared_batch_hash {
        return Err(BatchError::InvalidBatchHash {
            batch_id: batch_id.to_string(),
        });
    }

    if let Some((existing_hash, expected_revisions)) = conn
        .query_row(
            "SELECT batch_hash, expected_revisions
             FROM sync_v2_incoming_batches
             WHERE batch_id = ?1",
            params![batch_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()?
    {
        if existing_hash != declared_batch_hash
            || expected_revisions != revisions.len() as i64
        {
            return Err(BatchError::DuplicateMismatch {
                batch_id: batch_id.to_string(),
            });
        }
        return Ok(StageOutcome::Duplicate);
    }

    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO sync_v2_incoming_batches (
           batch_id, peer_device_id, batch_hash, expected_revisions,
           received_revisions, state, created_at
         ) VALUES (?1, ?2, ?3, ?4, 0, 'receiving', ?5)",
        params![
            batch_id,
            peer_device_id,
            declared_batch_hash,
            revisions.len() as i64,
            now_ms,
        ],
    )?;
    for (ordinal, (revision, bytes)) in revisions.iter().zip(encoded).enumerate() {
        tx.execute(
            "INSERT INTO sync_v2_incoming_revisions (
               batch_id, change_id, ordinal, revision, revision_hash
             ) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                batch_id,
                revision.change_id,
                ordinal as i64,
                bytes,
                blake3::hash(&bytes).to_hex().to_string(),
            ],
        )?;
    }
    tx.execute(
        "UPDATE sync_v2_incoming_batches
         SET received_revisions = expected_revisions, state = 'staged'
         WHERE batch_id = ?1",
        params![batch_id],
    )?;
    tx.commit()?;
    Ok(StageOutcome::Staged)
}

pub fn apply_staged_batch(
    conn: &Connection,
    batch_id: &str,
    now_ms: i64,
) -> Result<BatchApplyResult, BatchError> {
    let state = conn
        .query_row(
            "SELECT state FROM sync_v2_incoming_batches WHERE batch_id = ?1",
            params![batch_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or_else(|| BatchError::NotStaged {
            batch_id: batch_id.to_string(),
        })?;
    if state == "committed" {
        return Ok(BatchApplyResult {
            revisions_applied: 0,
            revisions_already_applied: 0,
            conflicts_created: 0,
            already_committed: true,
        });
    }
    if !matches!(state.as_str(), "staged" | "applying" | "failed") {
        return Err(BatchError::NotStaged {
            batch_id: batch_id.to_string(),
        });
    }

    let revisions = load_staged_revisions(conn, batch_id)?;
    conn.execute(
        "UPDATE sync_v2_incoming_batches
         SET state = 'applying', last_error = NULL
         WHERE batch_id = ?1",
        params![batch_id],
    )?;

    let mut result = BatchApplyResult {
        revisions_applied: 0,
        revisions_already_applied: 0,
        conflicts_created: 0,
        already_committed: false,
    };
    let mut pending = revisions;
    while !pending.is_empty() {
        let mut ready_index = None;
        for (index, revision) in pending.iter().enumerate() {
            if revision_is_ready(conn, revision)? {
                ready_index = Some(index);
                break;
            }
        }
        let index = ready_index.unwrap_or(0);
        let revision = pending.remove(index);
        match apply_remote_revision(conn, &revision, now_ms) {
            Ok(applied) => {
                match applied.outcome {
                    ApplyOutcome::Applied => result.revisions_applied += 1,
                    ApplyOutcome::Duplicate => result.revisions_already_applied += 1,
                }
                result.conflicts_created += applied.conflicts_created;
            }
            Err(source) => {
                let error = source.to_string();
                conn.execute(
                    "UPDATE sync_v2_incoming_batches
                     SET state = 'failed', last_error = ?2
                     WHERE batch_id = ?1",
                    params![batch_id, error],
                )?;
                return Err(BatchError::Apply {
                    batch_id: batch_id.to_string(),
                    source,
                });
            }
        }
    }

    conn.execute(
        "UPDATE sync_v2_incoming_batches
         SET state = 'committed', committed_at = ?2, last_error = NULL
         WHERE batch_id = ?1",
        params![batch_id, now_ms],
    )?;
    Ok(result)
}

fn revision_is_ready(
    conn: &Connection,
    revision: &ChangeRevision,
) -> Result<bool, BatchError> {
    if load_revision(conn, &revision.change_id)
        .map_err(|error| match error {
            super::store::StoreError::Database(error) => BatchError::Database(error),
        })?
        .is_some()
    {
        return Ok(true);
    }
    let frontier = load_frontier(conn).map_err(|error| match error {
        super::store::StoreError::Database(error) => BatchError::Database(error),
    })?;
    if frontier
        .get(&revision.origin_device_id)
        .copied()
        .unwrap_or(0)
        + 1
        != revision.origin_sequence
    {
        return Ok(false);
    }
    Ok(revision.base_frontier.iter().all(|(device_id, required)| {
        frontier.get(device_id).copied().unwrap_or(0) >= *required
    }))
}

fn encode_revisions(revisions: &[ChangeRevision]) -> Result<Vec<Vec<u8>>, BatchError> {
    revisions
        .iter()
        .map(|revision| bincode::serialize(revision).map_err(BatchError::from))
        .collect()
}

fn hash_encoded_revisions(revisions: &[Vec<u8>]) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&(revisions.len() as u64).to_le_bytes());
    for revision in revisions {
        hasher.update(&(revision.len() as u64).to_le_bytes());
        hasher.update(revision);
    }
    hasher.finalize().to_hex().to_string()
}

fn validate_batch_sizes(
    revisions: &[ChangeRevision],
    encoded: &[Vec<u8>],
) -> Result<(), BatchError> {
    if revisions.len() > MAX_REVISIONS_PER_BATCH {
        return Err(BatchError::TooManyRevisions {
            received: revisions.len(),
            limit: MAX_REVISIONS_PER_BATCH,
        });
    }
    let mut total = 0usize;
    for (revision, bytes) in revisions.iter().zip(encoded) {
        if bytes.len() > MAX_REVISION_BYTES {
            return Err(BatchError::RevisionTooLarge {
                change_id: revision.change_id.clone(),
                received: bytes.len(),
                limit: MAX_REVISION_BYTES,
            });
        }
        total = total.saturating_add(bytes.len());
    }
    if total > MAX_BATCH_BYTES {
        return Err(BatchError::BatchTooLarge {
            received: total,
            limit: MAX_BATCH_BYTES,
        });
    }
    Ok(())
}

fn load_staged_revisions(
    conn: &Connection,
    batch_id: &str,
) -> Result<Vec<ChangeRevision>, BatchError> {
    let expected = conn.query_row(
        "SELECT expected_revisions
         FROM sync_v2_incoming_batches
         WHERE batch_id = ?1",
        params![batch_id],
        |row| row.get::<_, i64>(0),
    )?;
    let mut statement = conn.prepare(
        "SELECT change_id, revision, revision_hash
         FROM sync_v2_incoming_revisions
         WHERE batch_id = ?1
         ORDER BY ordinal",
    )?;
    let rows = statement.query_map(params![batch_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Vec<u8>>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    let mut revisions = Vec::new();
    for row in rows {
        let (change_id, bytes, expected_hash) = row?;
        if blake3::hash(&bytes).to_hex().to_string() != expected_hash {
            return Err(BatchError::CorruptRevision { change_id });
        }
        let revision: ChangeRevision = bincode::deserialize(&bytes)?;
        if revision.change_id != change_id {
            return Err(BatchError::CorruptRevision { change_id });
        }
        revisions.push(revision);
    }
    if revisions.len() as i64 != expected {
        return Err(BatchError::NotStaged {
            batch_id: batch_id.to_string(),
        });
    }
    Ok(revisions)
}

#[cfg(test)]
mod tests {
    use rusqlite::{Connection, OptionalExtension};

    use super::{
        apply_staged_batch, revision_batch_hash, stage_revision_batch, BatchError, StageOutcome,
    };
    use crate::sync::v2::{capture_transaction, create_schema};

    fn connection() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE chats (
               id TEXT PRIMARY KEY,
               title TEXT NOT NULL
             );",
        )
        .unwrap();
        create_schema(&conn).unwrap();
        conn
    }

    #[test]
    fn durable_batch_applies_in_order_and_replay_is_safe() {
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
        let revisions = vec![first, second];
        let hash = revision_batch_hash(&revisions).unwrap();

        assert_eq!(
            stage_revision_batch(&target, "device-a", "batch-1", &hash, &revisions, 210)
                .unwrap(),
            StageOutcome::Staged
        );
        let applied = apply_staged_batch(&target, "batch-1", 220).unwrap();
        assert_eq!(applied.revisions_applied, 2);
        assert_eq!(
            target
                .query_row("SELECT title FROM chats WHERE id = 'chat-1'", [], |row| {
                    row.get::<_, String>(0)
                })
                .unwrap(),
            "two"
        );

        assert_eq!(
            stage_revision_batch(&target, "device-a", "batch-1", &hash, &revisions, 230)
                .unwrap(),
            StageOutcome::Duplicate
        );
        assert!(apply_staged_batch(&target, "batch-1", 240)
            .unwrap()
            .already_committed);
    }

    #[test]
    fn retry_after_partial_application_finishes_idempotently() {
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
        let revisions = vec![first.clone(), second];
        let hash = revision_batch_hash(&revisions).unwrap();
        stage_revision_batch(&target, "device-a", "batch-1", &hash, &revisions, 210)
            .unwrap();

        crate::sync::v2::apply_remote_revision(&target, &first, 215).unwrap();
        let result = apply_staged_batch(&target, "batch-1", 220).unwrap();
        assert_eq!(result.revisions_already_applied, 1);
        assert_eq!(result.revisions_applied, 1);
    }

    #[test]
    fn batch_applies_revisions_in_dependency_order() {
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
        let reversed = vec![second, first];
        let hash = revision_batch_hash(&reversed).unwrap();
        stage_revision_batch(&target, "device-a", "batch-1", &hash, &reversed, 210)
            .unwrap();

        let result = apply_staged_batch(&target, "batch-1", 220).unwrap();
        assert_eq!(result.revisions_applied, 2);
        assert_eq!(
            target
                .query_row("SELECT title FROM chats WHERE id = 'chat-1'", [], |row| {
                    row.get::<_, String>(0)
                })
                .unwrap(),
            "two"
        );
    }

    #[test]
    fn invalid_batch_hash_is_rejected_before_staging() {
        let source = connection();
        let target = connection();
        let revision = capture_transaction(&source, "device-a", 100, |tx| {
            tx.execute(
                "INSERT INTO chats (id, title) VALUES ('chat-1', 'one')",
                [],
            )
        })
        .unwrap()
        .revision
        .unwrap();

        assert!(matches!(
            stage_revision_batch(
                &target,
                "device-a",
                "batch-1",
                "invalid",
                &[revision],
                110
            ),
            Err(BatchError::InvalidBatchHash { .. })
        ));
        assert_eq!(
            target
                .query_row(
                    "SELECT state FROM sync_v2_incoming_batches WHERE batch_id = 'batch-1'",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .unwrap(),
            None
        );
    }
}
