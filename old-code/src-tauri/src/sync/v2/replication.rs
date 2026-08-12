use std::collections::BTreeMap;

use rusqlite::{params, Connection};

use super::batch::{MAX_BATCH_BYTES, MAX_REVISIONS_PER_BATCH};
use super::model::{ChangeRevision, Frontier};
use super::planner::outbound_ranges;
use super::protocol::RevisionPlan;
use super::store::{load_frontier, load_revision, StoreError};

#[derive(Debug, thiserror::Error)]
pub enum ReplicationError {
    #[error("sync v2 database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("sync v2 revision encoding error: {0}")]
    Encoding(#[from] Box<bincode::ErrorKind>),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(
        "change log is missing {origin_device_id}:{origin_sequence}; a checkpoint is required"
    )]
    MissingRevision {
        origin_device_id: String,
        origin_sequence: i64,
    },
    #[error("remaining revisions have unsatisfied causal dependencies")]
    UnsatisfiedDependencies,
    #[error("revision {change_id} is larger than the negotiated batch limit")]
    RevisionTooLarge { change_id: String },
    #[error("peer device ID cannot be empty")]
    EmptyPeerDeviceId,
}

pub fn plan_outbound(
    conn: &Connection,
    remote_frontier: &Frontier,
) -> Result<RevisionPlan, ReplicationError> {
    let local_frontier = load_frontier(conn)?;
    let ranges = outbound_ranges(&local_frontier, remote_frontier);
    let mut estimated_revisions = 0u64;
    let mut estimated_bytes = 0u64;
    for range in &ranges {
        let mut statement = conn.prepare(
            "SELECT origin_sequence, LENGTH(changeset)
             FROM sync_v2_changes
             WHERE origin_device_id = ?1
               AND origin_sequence BETWEEN ?2 AND ?3
             ORDER BY origin_sequence",
        )?;
        let rows = statement.query_map(
            params![
                range.origin_device_id,
                range.first_sequence,
                range.last_sequence
            ],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, u64>(1)?)),
        )?;
        let mut expected_sequence = range.first_sequence;
        for row in rows {
            let (sequence, bytes) = row?;
            if sequence != expected_sequence {
                return Err(ReplicationError::MissingRevision {
                    origin_device_id: range.origin_device_id.clone(),
                    origin_sequence: expected_sequence,
                });
            }
            estimated_revisions += 1;
            estimated_bytes += bytes;
            expected_sequence += 1;
        }
        if expected_sequence <= range.last_sequence {
            return Err(ReplicationError::MissingRevision {
                origin_device_id: range.origin_device_id.clone(),
                origin_sequence: expected_sequence,
            });
        }
    }
    Ok(RevisionPlan {
        ranges,
        estimated_revisions,
        estimated_bytes,
    })
}

pub fn build_outbound_batch(
    conn: &Connection,
    remote_frontier: &Frontier,
    max_revisions: usize,
    max_bytes: usize,
) -> Result<Vec<ChangeRevision>, ReplicationError> {
    let max_revisions = max_revisions.min(MAX_REVISIONS_PER_BATCH);
    let max_bytes = max_bytes.min(MAX_BATCH_BYTES);
    if max_revisions == 0 || max_bytes == 0 {
        return Ok(Vec::new());
    }

    let plan = plan_outbound(conn, remote_frontier)?;
    let mut pending = Vec::new();
    for range in plan.ranges {
        for sequence in range.first_sequence..=range.last_sequence {
            let change_id = conn
                .query_row(
                    "SELECT change_id
                     FROM sync_v2_changes
                     WHERE origin_device_id = ?1 AND origin_sequence = ?2",
                    params![range.origin_device_id, sequence],
                    |row| row.get::<_, String>(0),
                )
                .map_err(|error| match error {
                    rusqlite::Error::QueryReturnedNoRows => {
                        ReplicationError::MissingRevision {
                            origin_device_id: range.origin_device_id.clone(),
                            origin_sequence: sequence,
                        }
                    }
                    error => ReplicationError::Database(error),
                })?;
            let revision = load_revision(conn, &change_id)?.ok_or_else(|| {
                ReplicationError::MissingRevision {
                    origin_device_id: range.origin_device_id.clone(),
                    origin_sequence: sequence,
                }
            })?;
            pending.push(revision);
        }
    }
    pending.sort_by(|left, right| {
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
    });

    let mut simulated_frontier = remote_frontier.clone();
    let mut batch = Vec::new();
    let mut batch_bytes = 0usize;
    while !pending.is_empty() && batch.len() < max_revisions {
        let ready_index = pending
            .iter()
            .position(|revision| is_ready(revision, &simulated_frontier))
            .ok_or(ReplicationError::UnsatisfiedDependencies)?;
        let revision = pending.remove(ready_index);
        let revision_bytes = bincode::serialized_size(&revision)? as usize;
        if revision_bytes > max_bytes {
            if batch.is_empty() {
                return Err(ReplicationError::RevisionTooLarge {
                    change_id: revision.change_id,
                });
            }
            break;
        }
        if batch_bytes.saturating_add(revision_bytes) > max_bytes {
            break;
        }
        batch_bytes += revision_bytes;
        simulated_frontier.insert(
            revision.origin_device_id.clone(),
            revision.origin_sequence,
        );
        batch.push(revision);
    }
    Ok(batch)
}

pub fn record_peer_acknowledgement(
    conn: &Connection,
    peer_device_id: &str,
    frontier: &Frontier,
    now_ms: i64,
) -> Result<(), ReplicationError> {
    if peer_device_id.is_empty() {
        return Err(ReplicationError::EmptyPeerDeviceId);
    }
    let local_frontier = load_frontier(conn)?;
    let tx = conn.unchecked_transaction()?;
    for (origin_device_id, acknowledged_sequence) in frontier {
        let bounded_sequence = (*acknowledged_sequence)
            .min(local_frontier.get(origin_device_id).copied().unwrap_or(0))
            .max(0);
        tx.execute(
            "INSERT INTO sync_v2_peer_frontiers (
               peer_device_id, origin_device_id, acknowledged_sequence, updated_at
             ) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(peer_device_id, origin_device_id) DO UPDATE SET
               acknowledged_sequence = MAX(
                 acknowledged_sequence,
                 excluded.acknowledged_sequence
               ),
               updated_at = excluded.updated_at",
            params![
                peer_device_id,
                origin_device_id,
                bounded_sequence,
                now_ms
            ],
        )?;
    }
    tx.commit()?;
    Ok(())
}

fn is_ready(revision: &ChangeRevision, frontier: &BTreeMap<String, i64>) -> bool {
    frontier
        .get(&revision.origin_device_id)
        .copied()
        .unwrap_or(0)
        + 1
        == revision.origin_sequence
        && revision.base_frontier.iter().all(|(device_id, sequence)| {
            frontier.get(device_id).copied().unwrap_or(0) >= *sequence
        })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use rusqlite::Connection;

    use super::{
        build_outbound_batch, plan_outbound, record_peer_acknowledgement,
    };
    use crate::sync::v2::{
        apply_remote_revision, capture_transaction, create_schema,
    };

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
    fn outbound_batch_respects_cross_device_causal_dependencies() {
        let relay = connection();
        let second_device = connection();
        let first = capture_transaction(&relay, "device-a", 100, |tx| {
            tx.execute(
                "INSERT INTO chats (id, title) VALUES ('chat-1', 'one')",
                [],
            )
        })
        .unwrap()
        .revision
        .unwrap();
        apply_remote_revision(&second_device, &first, 110).unwrap();
        let second = capture_transaction(&second_device, "device-b", 200, |tx| {
            tx.execute("UPDATE chats SET title = 'two' WHERE id = 'chat-1'", [])
        })
        .unwrap()
        .revision
        .unwrap();
        apply_remote_revision(&relay, &second, 210).unwrap();

        let batch =
            build_outbound_batch(&relay, &BTreeMap::new(), 128, 1024 * 1024).unwrap();
        assert_eq!(batch.len(), 2);
        assert_eq!(batch[0].change_id, first.change_id);
        assert_eq!(batch[1].change_id, second.change_id);
    }

    #[test]
    fn plan_and_acknowledgement_use_monotonic_frontiers() {
        let conn = connection();
        capture_transaction(&conn, "device-a", 100, |tx| {
            tx.execute(
                "INSERT INTO chats (id, title) VALUES ('chat-1', 'one')",
                [],
            )
        })
        .unwrap();
        capture_transaction(&conn, "device-a", 200, |tx| {
            tx.execute("UPDATE chats SET title = 'two' WHERE id = 'chat-1'", [])
        })
        .unwrap();

        let plan = plan_outbound(
            &conn,
            &BTreeMap::from([("device-a".to_string(), 1)]),
        )
        .unwrap();
        assert_eq!(plan.estimated_revisions, 1);

        record_peer_acknowledgement(
            &conn,
            "peer",
            &BTreeMap::from([("device-a".to_string(), 99)]),
            300,
        )
        .unwrap();
        record_peer_acknowledgement(
            &conn,
            "peer",
            &BTreeMap::from([("device-a".to_string(), 1)]),
            400,
        )
        .unwrap();
        assert_eq!(
            conn.query_row(
                "SELECT acknowledged_sequence
                 FROM sync_v2_peer_frontiers
                 WHERE peer_device_id = 'peer' AND origin_device_id = 'device-a'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
            2
        );
    }
}
