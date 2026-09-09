use std::collections::BTreeMap;

use lettuce_sync::{
    CANONICAL_CHANGE_VERSION, CanonicalChange, CanonicalPayload, CausalFrontier, ChangeOperation,
    HybridTimestamp, LocalChangeAdmission, LocalChangeJournal, LocalChangeJournalError,
    MAX_FRONTIER_DEVICES, MAX_OUTBOUND_CHANGES, MAX_OUTBOUND_PAYLOAD_BYTES, NewCanonicalChange,
    OutboundChangeBatch, SyncChangeId, SyncDeviceId, SyncEntity,
};
use lettuce_types::{ContentHash, OperationId, TimestampMillis};
use rusqlite::{Connection, OptionalExtension, Row, TransactionBehavior, params};
use uuid::Uuid;

use crate::Database;

fn storage(_: impl std::fmt::Debug) -> LocalChangeJournalError {
    LocalChangeJournalError::Storage
}

fn corrupt(_: impl std::fmt::Debug) -> LocalChangeJournalError {
    LocalChangeJournalError::Corrupt
}

fn operation_name(operation: ChangeOperation) -> &'static str {
    match operation {
        ChangeOperation::Insert => "insert",
        ChangeOperation::Update => "update",
        ChangeOperation::Delete => "delete",
    }
}

fn operation_from_name(value: &str) -> Result<ChangeOperation, LocalChangeJournalError> {
    match value {
        "insert" => Ok(ChangeOperation::Insert),
        "update" => Ok(ChangeOperation::Update),
        "delete" => Ok(ChangeOperation::Delete),
        _ => Err(LocalChangeJournalError::Corrupt),
    }
}

struct StoredChangeRow {
    id: String,
    format_version: i64,
    fingerprint: String,
    origin_device: String,
    origin_sequence: i64,
    wall_time: i64,
    counter: i64,
    entity_kind: String,
    entity_id: String,
    operation: String,
    base_revision: Option<String>,
    payload_schema: Option<String>,
    payload_version: Option<i64>,
    payload_bytes: Option<Vec<u8>>,
    payload_hash: Option<String>,
}

impl StoredChangeRow {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            format_version: row.get(1)?,
            fingerprint: row.get(2)?,
            origin_device: row.get(3)?,
            origin_sequence: row.get(4)?,
            wall_time: row.get(5)?,
            counter: row.get(6)?,
            entity_kind: row.get(7)?,
            entity_id: row.get(8)?,
            operation: row.get(9)?,
            base_revision: row.get(10)?,
            payload_schema: row.get(11)?,
            payload_version: row.get(12)?,
            payload_bytes: row.get(13)?,
            payload_hash: row.get(14)?,
        })
    }
}

pub(crate) fn load_local_change_in(
    connection: &Connection,
    operation_id: OperationId,
) -> Result<Option<CanonicalChange>, LocalChangeJournalError> {
    let row = connection
        .query_row(
            "SELECT change_id, format_version, fingerprint, origin_device_id,
                    origin_sequence, hlc_wall_time, hlc_counter, entity_kind,
                    entity_id, operation, base_revision, payload_schema,
                    payload_version, payload_bytes, payload_hash
             FROM sync_changes WHERE operation_id = ?1",
            [operation_id.to_string()],
            StoredChangeRow::from_row,
        )
        .optional()
        .map_err(storage)?;
    row.map(|row| hydrate_change(connection, row)).transpose()
}

pub(crate) fn record_local_change_in(
    connection: &Connection,
    operation_id: OperationId,
    request: &NewCanonicalChange,
    now: TimestampMillis,
) -> Result<LocalChangeAdmission, LocalChangeJournalError> {
    if let Some(change) = load_local_change_in(connection, operation_id)? {
        if !request_matches(&change, request) {
            return Err(LocalChangeJournalError::Conflict);
        }
        return Ok(LocalChangeAdmission {
            change,
            created: false,
        });
    }
    let frontier = load_frontier(connection)?;
    let (device, sequence, timestamp) = next_identity_and_stamp(connection, now, &frontier)?;
    let change = CanonicalChange::new(
        SyncChangeId::new(),
        device,
        sequence,
        timestamp,
        frontier,
        request.entity().clone(),
        request.operation(),
        request.base_revision().cloned(),
        request.payload().cloned(),
    )
    .map_err(|_| LocalChangeJournalError::Invalid)?;
    insert_change(connection, operation_id, &change, now)?;
    Ok(LocalChangeAdmission {
        change,
        created: true,
    })
}

fn hydrate_change(
    connection: &Connection,
    row: StoredChangeRow,
) -> Result<CanonicalChange, LocalChangeJournalError> {
    if row.format_version != i64::from(CANONICAL_CHANGE_VERSION) {
        return Err(LocalChangeJournalError::Corrupt);
    }
    let id = SyncChangeId::from_uuid(Uuid::parse_str(&row.id).map_err(corrupt)?);
    let origin_device =
        SyncDeviceId::from_uuid(Uuid::parse_str(&row.origin_device).map_err(corrupt)?);
    let origin_sequence = u64::try_from(row.origin_sequence).map_err(corrupt)?;
    let counter = u32::try_from(row.counter).map_err(corrupt)?;
    let entity = SyncEntity::new(row.entity_kind, row.entity_id).map_err(corrupt)?;
    let operation = operation_from_name(&row.operation)?;
    let base_revision = row
        .base_revision
        .map(ContentHash::parse)
        .transpose()
        .map_err(corrupt)?;
    let payload = hydrate_payload(
        row.payload_schema,
        row.payload_version,
        row.payload_bytes,
        row.payload_hash,
    )?;
    let mut frontier = BTreeMap::new();
    let mut statement = connection
        .prepare(
            "SELECT origin_device_id, seen_sequence
             FROM sync_change_frontiers WHERE change_id = ?1
             ORDER BY origin_device_id",
        )
        .map_err(storage)?;
    let rows = statement
        .query_map([id.as_uuid().to_string()], |frontier_row| {
            Ok((
                frontier_row.get::<_, String>(0)?,
                frontier_row.get::<_, i64>(1)?,
            ))
        })
        .map_err(storage)?;
    for item in rows {
        let (device, sequence) = item.map_err(storage)?;
        let device = SyncDeviceId::from_uuid(Uuid::parse_str(&device).map_err(corrupt)?);
        let sequence = u64::try_from(sequence).map_err(corrupt)?;
        if frontier.insert(device, sequence).is_some() {
            return Err(LocalChangeJournalError::Corrupt);
        }
    }
    let change = CanonicalChange::new(
        id,
        origin_device,
        origin_sequence,
        HybridTimestamp::new(TimestampMillis::new(row.wall_time), counter),
        frontier,
        entity,
        operation,
        base_revision,
        payload,
    )
    .map_err(corrupt)?;
    let fingerprint = ContentHash::parse(row.fingerprint).map_err(corrupt)?;
    if change.fingerprint() != &fingerprint {
        return Err(LocalChangeJournalError::Corrupt);
    }
    Ok(change)
}

fn hydrate_payload(
    schema: Option<String>,
    version: Option<i64>,
    bytes: Option<Vec<u8>>,
    hash: Option<String>,
) -> Result<Option<CanonicalPayload>, LocalChangeJournalError> {
    match (schema, version, bytes, hash) {
        (None, None, None, None) => Ok(None),
        (Some(schema), Some(version), Some(bytes), Some(hash)) => {
            let version = u32::try_from(version).map_err(corrupt)?;
            let payload = CanonicalPayload::new(schema, version, bytes).map_err(corrupt)?;
            if payload.content_hash() != &ContentHash::parse(hash).map_err(corrupt)? {
                return Err(LocalChangeJournalError::Corrupt);
            }
            Ok(Some(payload))
        }
        _ => Err(LocalChangeJournalError::Corrupt),
    }
}

fn request_matches(change: &CanonicalChange, request: &NewCanonicalChange) -> bool {
    change.entity() == request.entity()
        && change.operation() == request.operation()
        && change.base_revision() == request.base_revision()
        && change.payload() == request.payload()
}

fn load_frontier(connection: &Connection) -> Result<CausalFrontier, LocalChangeJournalError> {
    let mut statement = connection
        .prepare(
            "SELECT origin_device_id, contiguous_sequence
             FROM sync_frontiers ORDER BY origin_device_id",
        )
        .map_err(storage)?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(storage)?;
    let mut frontier = BTreeMap::new();
    for row in rows {
        let (device, sequence) = row.map_err(storage)?;
        let device = SyncDeviceId::from_uuid(Uuid::parse_str(&device).map_err(corrupt)?);
        let sequence = u64::try_from(sequence).map_err(corrupt)?;
        if sequence == 0 || frontier.insert(device, sequence).is_some() {
            return Err(LocalChangeJournalError::Corrupt);
        }
    }
    Ok(frontier)
}

fn validate_frontier(frontier: &CausalFrontier) -> Result<(), LocalChangeJournalError> {
    if frontier.len() > MAX_FRONTIER_DEVICES || frontier.values().any(|sequence| *sequence == 0) {
        return Err(LocalChangeJournalError::InvalidFrontier);
    }
    Ok(())
}

fn local_device(connection: &Connection) -> Result<Option<SyncDeviceId>, LocalChangeJournalError> {
    connection
        .query_row(
            "SELECT device_id FROM sync_local_state WHERE id = 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(storage)?
        .map(|value| {
            Uuid::parse_str(&value)
                .map(SyncDeviceId::from_uuid)
                .map_err(corrupt)
        })
        .transpose()
}

fn change_for_sequence(
    connection: &Connection,
    device: SyncDeviceId,
    sequence: u64,
) -> Result<CanonicalChange, LocalChangeJournalError> {
    let change_id = connection
        .query_row(
            "SELECT change_id FROM sync_changes
             WHERE origin_device_id = ?1 AND origin_sequence = ?2",
            params![
                device.as_uuid().to_string(),
                i64::try_from(sequence).map_err(|_| LocalChangeJournalError::Exhausted)?
            ],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(storage)?
        .ok_or(LocalChangeJournalError::MissingSequence)?;
    let row = connection
        .query_row(
            "SELECT change_id, format_version, fingerprint, origin_device_id,
                    origin_sequence, hlc_wall_time, hlc_counter, entity_kind,
                    entity_id, operation, base_revision, payload_schema,
                    payload_version, payload_bytes, payload_hash
             FROM sync_changes WHERE change_id = ?1",
            [change_id],
            StoredChangeRow::from_row,
        )
        .map_err(storage)?;
    hydrate_change(connection, row)
}

fn load_peer_frontier(
    connection: &Connection,
    peer: SyncDeviceId,
) -> Result<CausalFrontier, LocalChangeJournalError> {
    let mut statement = connection
        .prepare(
            "SELECT origin_device_id, acknowledged_sequence
             FROM sync_peer_frontiers WHERE peer_device_id = ?1
             ORDER BY origin_device_id",
        )
        .map_err(storage)?;
    let rows = statement
        .query_map([peer.as_uuid().to_string()], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(storage)?;
    let mut frontier = CausalFrontier::new();
    for row in rows {
        let (device, sequence) = row.map_err(storage)?;
        let device = SyncDeviceId::from_uuid(Uuid::parse_str(&device).map_err(corrupt)?);
        let sequence = u64::try_from(sequence).map_err(corrupt)?;
        if sequence == 0 || frontier.insert(device, sequence).is_some() {
            return Err(LocalChangeJournalError::Corrupt);
        }
    }
    validate_frontier(&frontier)?;
    Ok(frontier)
}

fn next_identity_and_stamp(
    connection: &Connection,
    now: TimestampMillis,
    frontier: &CausalFrontier,
) -> Result<(SyncDeviceId, u64, HybridTimestamp), LocalChangeJournalError> {
    let state = connection
        .query_row(
            "SELECT device_id, origin_sequence, hlc_wall_time, hlc_counter
             FROM sync_local_state WHERE id = 1",
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            },
        )
        .optional()
        .map_err(storage)?;
    let Some((device, sequence, wall_time, counter)) = state else {
        if !frontier.is_empty() {
            return Err(LocalChangeJournalError::Corrupt);
        }
        let device = SyncDeviceId::new();
        connection
            .execute(
                "INSERT INTO sync_local_state
                 (id, device_id, origin_sequence, hlc_wall_time, hlc_counter)
                 VALUES (1, ?1, 1, ?2, 0)",
                params![device.as_uuid().to_string(), now.get()],
            )
            .map_err(storage)?;
        return Ok((device, 1, HybridTimestamp::new(now, 0)));
    };
    let device = SyncDeviceId::from_uuid(Uuid::parse_str(&device).map_err(corrupt)?);
    let current_sequence = u64::try_from(sequence).map_err(corrupt)?;
    if current_sequence == 0 || frontier.get(&device) != Some(&current_sequence) {
        return Err(LocalChangeJournalError::Corrupt);
    }
    let sequence = sequence
        .checked_add(1)
        .ok_or(LocalChangeJournalError::Exhausted)?;
    let (next_wall, next_counter) = if now.get() > wall_time {
        (now.get(), 0)
    } else {
        (
            wall_time,
            counter
                .checked_add(1)
                .ok_or(LocalChangeJournalError::Exhausted)?,
        )
    };
    let sequence_u64 = u64::try_from(sequence).map_err(corrupt)?;
    let counter_u32 =
        u32::try_from(next_counter).map_err(|_| LocalChangeJournalError::Exhausted)?;
    connection
        .execute(
            "UPDATE sync_local_state
             SET origin_sequence = ?1, hlc_wall_time = ?2, hlc_counter = ?3
             WHERE id = 1",
            params![sequence, next_wall, next_counter],
        )
        .map_err(storage)?;
    Ok((
        device,
        sequence_u64,
        HybridTimestamp::new(TimestampMillis::new(next_wall), counter_u32),
    ))
}

fn insert_change(
    connection: &Connection,
    operation_id: OperationId,
    change: &CanonicalChange,
    created_at: TimestampMillis,
) -> Result<(), LocalChangeJournalError> {
    let (payload_schema, payload_version, payload_bytes, payload_hash) =
        if let Some(payload) = change.payload() {
            (
                Some(payload.schema()),
                Some(i64::from(payload.version())),
                Some(payload.bytes()),
                Some(payload.content_hash().as_str()),
            )
        } else {
            (None, None, None, None)
        };
    let origin_sequence =
        i64::try_from(change.origin_sequence()).map_err(|_| LocalChangeJournalError::Exhausted)?;
    connection
        .execute(
            "INSERT INTO sync_changes (
                change_id, operation_id, format_version, fingerprint,
                origin_device_id, origin_sequence, hlc_wall_time, hlc_counter,
                entity_kind, entity_id, operation, base_revision,
                payload_schema, payload_version, payload_bytes, payload_hash,
                created_at
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
                ?13, ?14, ?15, ?16, ?17
             )",
            params![
                change.id().as_uuid().to_string(),
                operation_id.to_string(),
                i64::from(change.version()),
                change.fingerprint().as_str(),
                change.origin_device().as_uuid().to_string(),
                origin_sequence,
                change.timestamp().wall_time().get(),
                i64::from(change.timestamp().counter()),
                change.entity().kind(),
                change.entity().id(),
                operation_name(change.operation()),
                change.base_revision().map(ContentHash::as_str),
                payload_schema,
                payload_version,
                payload_bytes,
                payload_hash,
                created_at.get(),
            ],
        )
        .map_err(storage)?;
    for (device, sequence) in change.base_frontier() {
        connection
            .execute(
                "INSERT INTO sync_change_frontiers
                 (change_id, origin_device_id, seen_sequence) VALUES (?1, ?2, ?3)",
                params![
                    change.id().as_uuid().to_string(),
                    device.as_uuid().to_string(),
                    i64::try_from(*sequence).map_err(|_| LocalChangeJournalError::Exhausted)?,
                ],
            )
            .map_err(storage)?;
    }
    connection
        .execute(
            "INSERT INTO sync_frontiers (origin_device_id, contiguous_sequence)
             VALUES (?1, ?2)
             ON CONFLICT(origin_device_id) DO UPDATE SET
                contiguous_sequence = excluded.contiguous_sequence",
            params![
                change.origin_device().as_uuid().to_string(),
                origin_sequence
            ],
        )
        .map_err(storage)?;
    Ok(())
}

impl LocalChangeJournal for Database {
    fn record_local_change(
        &self,
        operation_id: OperationId,
        request: NewCanonicalChange,
        now: TimestampMillis,
    ) -> Result<LocalChangeAdmission, LocalChangeJournalError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let admission = record_local_change_in(&transaction, operation_id, &request, now)?;
        transaction.commit().map_err(storage)?;
        Ok(admission)
    }

    fn local_change_for_operation(
        &self,
        operation_id: OperationId,
    ) -> Result<Option<CanonicalChange>, LocalChangeJournalError> {
        let connection = self.connection().map_err(storage)?;
        load_local_change_in(&connection, operation_id)
    }

    fn local_frontier(&self) -> Result<CausalFrontier, LocalChangeJournalError> {
        let connection = self.connection().map_err(storage)?;
        load_frontier(&connection)
    }

    fn outbound_changes(
        &self,
        remote_frontier: &CausalFrontier,
        max_changes: usize,
        max_payload_bytes: usize,
    ) -> Result<OutboundChangeBatch, LocalChangeJournalError> {
        validate_frontier(remote_frontier)?;
        let connection = self.connection().map_err(storage)?;
        let local_frontier = load_frontier(&connection)?;
        let Some(device) = local_device(&connection)? else {
            if !local_frontier.is_empty() {
                return Err(LocalChangeJournalError::Corrupt);
            }
            return Ok(OutboundChangeBatch {
                changes: Vec::new(),
                payload_bytes: 0,
                has_more: false,
            });
        };
        let local_sequence = local_frontier.get(&device).copied().unwrap_or(0);
        let seen = remote_frontier
            .get(&device)
            .copied()
            .unwrap_or(0)
            .min(local_sequence);
        let max_changes = max_changes.min(MAX_OUTBOUND_CHANGES);
        let max_payload_bytes = max_payload_bytes.min(MAX_OUTBOUND_PAYLOAD_BYTES);
        if max_changes == 0 || max_payload_bytes == 0 {
            return Ok(OutboundChangeBatch {
                changes: Vec::new(),
                payload_bytes: 0,
                has_more: seen < local_sequence,
            });
        }
        let mut simulated = remote_frontier.clone();
        if seen == 0 {
            simulated.remove(&device);
        } else {
            simulated.insert(device, seen);
        }
        let mut changes = Vec::new();
        let mut payload_bytes = 0usize;
        let mut sequence = seen.saturating_add(1);
        while sequence <= local_sequence && changes.len() < max_changes {
            let change = change_for_sequence(&connection, device, sequence)?;
            let ready = change
                .base_frontier()
                .iter()
                .all(|(origin, required)| simulated.get(origin).copied().unwrap_or(0) >= *required);
            if !ready {
                return Err(LocalChangeJournalError::UnsatisfiedDependencies);
            }
            let bytes = change.payload().map_or(0, |payload| payload.bytes().len());
            if payload_bytes.saturating_add(bytes) > max_payload_bytes {
                if changes.is_empty() {
                    return Err(LocalChangeJournalError::ChangeTooLarge);
                }
                break;
            }
            payload_bytes += bytes;
            simulated.insert(device, sequence);
            changes.push(change);
            sequence = sequence.saturating_add(1);
        }
        Ok(OutboundChangeBatch {
            changes,
            payload_bytes,
            has_more: sequence <= local_sequence,
        })
    }

    fn record_peer_acknowledgement(
        &self,
        peer: SyncDeviceId,
        frontier: &CausalFrontier,
        now: TimestampMillis,
    ) -> Result<CausalFrontier, LocalChangeJournalError> {
        validate_frontier(frontier)?;
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let local = load_frontier(&transaction)?;
        for (origin, acknowledged) in frontier {
            let bounded = (*acknowledged).min(local.get(origin).copied().unwrap_or(0));
            if bounded == 0 {
                continue;
            }
            transaction
                .execute(
                    "INSERT INTO sync_peer_frontiers
                     (peer_device_id, origin_device_id, acknowledged_sequence, updated_at)
                     VALUES (?1, ?2, ?3, ?4)
                     ON CONFLICT(peer_device_id, origin_device_id) DO UPDATE SET
                       acknowledged_sequence = MAX(
                         sync_peer_frontiers.acknowledged_sequence,
                         excluded.acknowledged_sequence
                       ),
                       updated_at = CASE
                         WHEN excluded.acknowledged_sequence >
                              sync_peer_frontiers.acknowledged_sequence
                         THEN excluded.updated_at
                         ELSE sync_peer_frontiers.updated_at
                       END",
                    params![
                        peer.as_uuid().to_string(),
                        origin.as_uuid().to_string(),
                        i64::try_from(bounded).map_err(|_| LocalChangeJournalError::Exhausted)?,
                        now.get()
                    ],
                )
                .map_err(storage)?;
        }
        let stored = load_peer_frontier(&transaction, peer)?;
        transaction.commit().map_err(storage)?;
        Ok(stored)
    }

    fn peer_acknowledgement(
        &self,
        peer: SyncDeviceId,
    ) -> Result<CausalFrontier, LocalChangeJournalError> {
        let connection = self.connection().map_err(storage)?;
        load_peer_frontier(&connection, peer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lettuce_characters::{
        Persona, PersonaArchiveRequest, PersonaDraftUpdate, PersonaRepository,
    };
    use lettuce_types::PersonaId;

    fn request(bytes: &[u8]) -> NewCanonicalChange {
        NewCanonicalChange::new(
            SyncEntity::new("persona", "persona-1").expect("entity"),
            ChangeOperation::Insert,
            None,
            Some(CanonicalPayload::new("persona", 1, bytes.to_vec()).expect("payload")),
        )
        .expect("request")
    }

    #[test]
    fn local_journal_replays_and_allocates_monotonic_causal_stamps() {
        let database = Database::open_in_memory().expect("database");
        let first_operation = OperationId::new();
        let first = database
            .record_local_change(
                first_operation,
                request(b"first"),
                TimestampMillis::new(100),
            )
            .expect("first");
        let replay = database
            .record_local_change(first_operation, request(b"first"), TimestampMillis::new(50))
            .expect("replay");
        let second = database
            .record_local_change(
                OperationId::new(),
                request(b"second"),
                TimestampMillis::new(50),
            )
            .expect("second");

        assert!(first.created);
        assert!(!replay.created);
        assert_eq!(replay.change, first.change);
        assert_eq!(second.change.origin_device(), first.change.origin_device());
        assert_eq!(second.change.origin_sequence(), 2);
        assert_eq!(
            second.change.timestamp().wall_time(),
            TimestampMillis::new(100)
        );
        assert_eq!(second.change.timestamp().counter(), 1);
        assert_eq!(
            second
                .change
                .base_frontier()
                .get(&first.change.origin_device()),
            Some(&1)
        );
        assert!(second.change.observes(&first.change));
    }

    #[test]
    fn changed_operation_input_conflicts_without_advancing_state() {
        let database = Database::open_in_memory().expect("database");
        let operation = OperationId::new();
        let first = database
            .record_local_change(operation, request(b"first"), TimestampMillis::new(10))
            .expect("first");

        assert_eq!(
            database.record_local_change(operation, request(b"changed"), TimestampMillis::new(20)),
            Err(LocalChangeJournalError::Conflict)
        );
        let second = database
            .record_local_change(
                OperationId::new(),
                request(b"second"),
                TimestampMillis::new(20),
            )
            .expect("second");
        assert_eq!(first.change.origin_sequence(), 1);
        assert_eq!(second.change.origin_sequence(), 2);
    }

    #[test]
    fn failed_change_insert_rolls_back_identity_and_clock() {
        let database = Database::open_in_memory().expect("database");
        database
            .connection()
            .expect("connection")
            .execute_batch(
                "CREATE TRIGGER reject_sync_change
                 BEFORE INSERT ON sync_changes
                 BEGIN
                   SELECT RAISE(ABORT, 'injected failure');
                 END;",
            )
            .expect("trigger");

        assert_eq!(
            database.record_local_change(
                OperationId::new(),
                request(b"first"),
                TimestampMillis::new(10)
            ),
            Err(LocalChangeJournalError::Storage)
        );
        let connection = database.connection().expect("connection");
        assert_eq!(
            connection
                .query_row("SELECT count(*) FROM sync_local_state", [], |row| row
                    .get::<_, i64>(0))
                .expect("state count"),
            0
        );
        assert_eq!(
            connection
                .query_row("SELECT count(*) FROM sync_frontiers", [], |row| row
                    .get::<_, i64>(0))
                .expect("frontier count"),
            0
        );
    }

    #[test]
    fn missing_current_frontier_fails_before_advancing_the_clock() {
        let database = Database::open_in_memory().expect("database");
        let first = database
            .record_local_change(
                OperationId::new(),
                request(b"first"),
                TimestampMillis::new(10),
            )
            .expect("first");
        database
            .connection()
            .expect("connection")
            .execute("DELETE FROM sync_frontiers", [])
            .expect("corrupt frontier");

        assert_eq!(
            database.record_local_change(
                OperationId::new(),
                request(b"second"),
                TimestampMillis::new(20)
            ),
            Err(LocalChangeJournalError::Corrupt)
        );
        let connection = database.connection().expect("connection");
        let sequence = connection
            .query_row(
                "SELECT origin_sequence FROM sync_local_state WHERE id = 1",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("sequence");
        assert_eq!(
            sequence,
            i64::try_from(first.change.origin_sequence()).expect("sequence")
        );
    }

    #[test]
    fn journal_reopens_and_rejects_evidence_mutation_or_deletion() {
        let path =
            std::env::temp_dir().join(format!("sync-journal-{}.sqlite3", OperationId::new()));
        let operation = OperationId::new();
        let admitted = {
            let database = Database::open(&path).expect("database");
            database
                .record_local_change(operation, request(b"persisted"), TimestampMillis::new(10))
                .expect("admission")
                .change
        };
        let database = Database::open(&path).expect("reopen");
        assert_eq!(
            database
                .local_change_for_operation(operation)
                .expect("load"),
            Some(admitted.clone())
        );
        let connection = database.connection().expect("connection");
        assert!(
            connection
                .execute(
                    "UPDATE sync_changes SET entity_id = 'changed' WHERE operation_id = ?1",
                    [operation.to_string()]
                )
                .is_err()
        );
        assert!(
            connection
                .execute(
                    "DELETE FROM sync_changes WHERE operation_id = ?1",
                    [operation.to_string()]
                )
                .is_err()
        );
        drop(connection);
        assert_eq!(
            database
                .local_change_for_operation(operation)
                .expect("reload"),
            Some(admitted)
        );
        drop(database);
        std::fs::remove_file(path).expect("remove test database");
    }

    #[test]
    fn outbound_batches_and_peer_acknowledgements_survive_reopen() {
        let path =
            std::env::temp_dir().join(format!("sync-outbound-{}.sqlite3", OperationId::new()));
        let peer = SyncDeviceId::new();
        let (device, local_frontier) = {
            let database = Database::open(&path).expect("database");
            let created = PersonaRepository::create(
                &database,
                Persona::new(
                    PersonaId::new(),
                    "Writer".into(),
                    "A careful writer".into(),
                    TimestampMillis::new(10),
                )
                .expect("persona"),
            )
            .expect("create");
            let revised = PersonaRepository::revise(
                &database,
                created.id,
                created.revision,
                PersonaDraftUpdate {
                    title: "Editor".into(),
                    description: "A careful editor".into(),
                    nickname: None,
                    design_description: None,
                    avatar_crop: None,
                    image_recommendation: None,
                },
                TimestampMillis::new(20),
            )
            .expect("revise");
            PersonaRepository::archive(
                &database,
                PersonaArchiveRequest {
                    persona_id: revised.id,
                    expected_persona_revision: revised.revision,
                    expected_default_revision: None,
                    now: TimestampMillis::new(30),
                },
            )
            .expect("archive");

            let frontier = database.local_frontier().expect("local frontier");
            let (&device, &sequence) = frontier.iter().next().expect("local device");
            assert_eq!(sequence, 3);
            let first = database
                .outbound_changes(&CausalFrontier::new(), 2, MAX_OUTBOUND_PAYLOAD_BYTES)
                .expect("first batch");
            assert_eq!(
                first
                    .changes
                    .iter()
                    .map(CanonicalChange::origin_sequence)
                    .collect::<Vec<_>>(),
                vec![1, 2]
            );
            assert!(first.payload_bytes > 0);
            assert!(first.has_more);

            let remote = CausalFrontier::from([(device, 2)]);
            let second = database
                .outbound_changes(&remote, 20, MAX_OUTBOUND_PAYLOAD_BYTES)
                .expect("second batch");
            assert_eq!(second.changes.len(), 1);
            assert_eq!(second.changes[0].origin_sequence(), 3);
            assert!(!second.has_more);

            let acknowledged = database
                .record_peer_acknowledgement(
                    peer,
                    &CausalFrontier::from([(device, 99)]),
                    TimestampMillis::new(40),
                )
                .expect("acknowledge");
            assert_eq!(acknowledged.get(&device), Some(&3));
            let unchanged = database
                .record_peer_acknowledgement(
                    peer,
                    &CausalFrontier::from([(device, 1)]),
                    TimestampMillis::new(50),
                )
                .expect("older acknowledgement");
            assert_eq!(unchanged, acknowledged);
            (device, frontier)
        };

        let database = Database::open(&path).expect("reopen");
        assert_eq!(database.local_frontier().expect("frontier"), local_frontier);
        assert_eq!(
            database
                .peer_acknowledgement(peer)
                .expect("peer acknowledgement")
                .get(&device),
            Some(&3)
        );
        database
            .connection()
            .expect("connection")
            .execute_batch(
                "DROP TRIGGER sync_change_frontiers_no_delete;
                 DROP TRIGGER sync_changes_no_delete;
                 DELETE FROM sync_change_frontiers
                 WHERE change_id = (
                   SELECT change_id FROM sync_changes WHERE origin_sequence = 2
                 );
                 DELETE FROM sync_changes WHERE origin_sequence = 2;",
            )
            .expect("create missing sequence");
        assert_eq!(
            database.outbound_changes(
                &CausalFrontier::new(),
                MAX_OUTBOUND_CHANGES,
                MAX_OUTBOUND_PAYLOAD_BYTES,
            ),
            Err(LocalChangeJournalError::MissingSequence)
        );
        drop(database);
        std::fs::remove_file(path).expect("remove test database");
    }
}
