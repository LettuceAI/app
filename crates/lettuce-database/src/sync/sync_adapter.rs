use std::collections::BTreeMap;

use lettuce_characters::{Persona, PersonaDefaultState, RepositoryError};
use lettuce_sync::{
    CANONICAL_CHANGE_VERSION, CanonicalChange, CanonicalPayload, CausalFrontier, ChangeOperation,
    ConflictChoice, ConflictRepositoryError, HybridTimestamp, IncomingBatchAdmission,
    IncomingBatchResult, IncomingBatchState, IncomingChangeError, IncomingChangeRepository,
    LocalChangeAdmission, LocalChangeJournal, LocalChangeJournalError, MAX_FRONTIER_DEVICES,
    MAX_INCOMING_CHANGES, MAX_INCOMING_PAYLOAD_BYTES, MAX_OUTBOUND_CHANGES,
    MAX_OUTBOUND_PAYLOAD_BYTES, MAX_UNRESOLVED_CONFLICTS, MEDIA_ASSET_SYNC_SCHEMA,
    MEDIA_ASSET_SYNC_VERSION, NewCanonicalChange, OutboundChangeBatch, PERSONA_DEFAULT_SYNC_SCHEMA,
    PERSONA_DEFAULT_SYNC_VERSION, PERSONA_SYNC_SCHEMA, PERSONA_SYNC_VERSION, PersonaConflict,
    PersonaConflictCandidate, PersonaConflictRepository, PersonaConflictValue, SyncChangeId,
    SyncDeviceId, SyncEntity, canonical_batch_hash, canonical_persona_default_payload,
    canonical_persona_payload, persona_default_sync_entity, persona_sync_entity,
};
use lettuce_types::{ContentHash, OperationId, PersonaId, TimestampMillis};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::Database;
use crate::catalog::persona_adapter::{
    apply_synced_persona, apply_synced_persona_default, load_persona, read_default,
};

fn storage(_: impl std::fmt::Debug) -> LocalChangeJournalError {
    LocalChangeJournalError::Storage
}

fn corrupt(_: impl std::fmt::Debug) -> LocalChangeJournalError {
    LocalChangeJournalError::Corrupt
}

fn incoming_storage(_: impl std::fmt::Debug) -> IncomingChangeError {
    IncomingChangeError::Storage
}

fn incoming_corrupt(_: impl std::fmt::Debug) -> IncomingChangeError {
    IncomingChangeError::Corrupt
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredIncomingChange {
    version: u32,
    id: String,
    origin_device: String,
    origin_sequence: u64,
    wall_time: i64,
    counter: u32,
    base_frontier: Vec<(String, u64)>,
    entity_kind: String,
    entity_id: String,
    operation: String,
    base_revision: Option<String>,
    payload_schema: Option<String>,
    payload_version: Option<u32>,
    fingerprint: String,
}

impl StoredIncomingChange {
    fn from_change(change: &CanonicalChange) -> Self {
        Self {
            version: change.version(),
            id: change.id().as_uuid().to_string(),
            origin_device: change.origin_device().as_uuid().to_string(),
            origin_sequence: change.origin_sequence(),
            wall_time: change.timestamp().wall_time().get(),
            counter: change.timestamp().counter(),
            base_frontier: change
                .base_frontier()
                .iter()
                .map(|(device, sequence)| (device.as_uuid().to_string(), *sequence))
                .collect(),
            entity_kind: change.entity().kind().into(),
            entity_id: change.entity().id().into(),
            operation: operation_name(change.operation()).into(),
            base_revision: change.base_revision().map(ToString::to_string),
            payload_schema: change.payload().map(|payload| payload.schema().into()),
            payload_version: change.payload().map(CanonicalPayload::version),
            fingerprint: change.fingerprint().to_string(),
        }
    }

    fn into_change(
        self,
        payload_bytes: Option<Vec<u8>>,
    ) -> Result<CanonicalChange, IncomingChangeError> {
        if self.version != CANONICAL_CHANGE_VERSION {
            return Err(IncomingChangeError::Corrupt);
        }
        let id = SyncChangeId::from_uuid(Uuid::parse_str(&self.id).map_err(incoming_corrupt)?);
        let origin_device = SyncDeviceId::from_uuid(
            Uuid::parse_str(&self.origin_device).map_err(incoming_corrupt)?,
        );
        let mut frontier = CausalFrontier::new();
        for (device, sequence) in self.base_frontier {
            let device =
                SyncDeviceId::from_uuid(Uuid::parse_str(&device).map_err(incoming_corrupt)?);
            if frontier.insert(device, sequence).is_some() {
                return Err(IncomingChangeError::Corrupt);
            }
        }
        let payload = match (self.payload_schema, self.payload_version, payload_bytes) {
            (None, None, None) => None,
            (Some(schema), Some(version), Some(bytes)) => {
                Some(CanonicalPayload::new(schema, version, bytes).map_err(incoming_corrupt)?)
            }
            _ => return Err(IncomingChangeError::Corrupt),
        };
        let change = CanonicalChange::new(
            id,
            origin_device,
            self.origin_sequence,
            HybridTimestamp::new(TimestampMillis::new(self.wall_time), self.counter),
            frontier,
            SyncEntity::new(self.entity_kind, self.entity_id).map_err(incoming_corrupt)?,
            operation_from_name(&self.operation).map_err(incoming_corrupt)?,
            self.base_revision
                .map(ContentHash::parse)
                .transpose()
                .map_err(incoming_corrupt)?,
            payload,
        )
        .map_err(incoming_corrupt)?;
        if change.fingerprint()
            != &ContentHash::parse(self.fingerprint).map_err(incoming_corrupt)?
        {
            return Err(IncomingChangeError::Corrupt);
        }
        Ok(change)
    }
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

/// A local operation identity scoped to the latest remote change that won
/// locally for the entity. A remote winner can move an aggregate revision
/// backwards, so revision-derived identities would otherwise repeat on this
/// device; losing remote changes leave the scope, and so retries, unchanged.
pub(crate) fn entity_scoped_operation(
    connection: &Connection,
    entity: &SyncEntity,
    operation: OperationId,
) -> Result<OperationId, LocalChangeJournalError> {
    let last_remote = connection
        .query_row(
            "SELECT change.change_id FROM sync_changes change
             WHERE change.entity_kind = ?1 AND change.entity_id = ?2
               AND change.operation_id IS NULL
               AND NOT EXISTS (
                 SELECT 1 FROM sync_conflicts conflict
                 WHERE conflict.incoming_change_id = change.change_id
                   AND conflict.winning_side = 'current'
               )
               AND NOT EXISTS (
                 SELECT 1 FROM sync_deferred_changes deferred
                 WHERE deferred.change_id = change.change_id
               )
             ORDER BY change.rowid DESC LIMIT 1",
            params![entity.kind(), entity.id()],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(storage)?;
    Ok(match last_remote {
        Some(change_id) => {
            OperationId::from_uuid(Uuid::new_v5(&operation.as_uuid(), change_id.as_bytes()))
        }
        None => operation,
    })
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
    record_local_change_in_skipping(connection, operation_id, request, now, None, None)
}

fn record_local_change_in_skipping(
    connection: &Connection,
    operation_id: OperationId,
    request: &NewCanonicalChange,
    now: TimestampMillis,
    skipped_conflict: Option<OperationId>,
    source_time: Option<TimestampMillis>,
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
    let (device, sequence, mut timestamp) = next_identity_and_stamp(connection, now, &frontier)?;
    if let Some(source_time) = source_time.filter(|time| *time < timestamp.wall_time()) {
        let source = HybridTimestamp::new(source_time, 0);
        timestamp = match latest_entity_stamp(connection, request.entity())? {
            Some(latest) if latest >= source => successor_stamp(latest)
                .filter(|next| *next < timestamp)
                .unwrap_or(timestamp),
            _ => source,
        };
    }
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
    insert_change(connection, Some(operation_id), &change, now)?;
    resolve_dominated_conflicts(connection, &change, now, skipped_conflict).map_err(|error| {
        match error {
            ApplyOneError::Storage => LocalChangeJournalError::Storage,
            _ => LocalChangeJournalError::Corrupt,
        }
    })?;
    Ok(LocalChangeAdmission {
        change,
        created: true,
    })
}

/// The latest hybrid timestamp journaled for an entity, from any device.
fn latest_entity_stamp(
    connection: &Connection,
    entity: &SyncEntity,
) -> Result<Option<HybridTimestamp>, LocalChangeJournalError> {
    let latest = connection
        .query_row(
            "SELECT hlc_wall_time, hlc_counter FROM sync_changes
             WHERE entity_kind = ?1 AND entity_id = ?2
             ORDER BY hlc_wall_time DESC, hlc_counter DESC LIMIT 1",
            params![entity.kind(), entity.id()],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()
        .map_err(storage)?;
    latest
        .map(|(wall_time, counter)| {
            Ok(HybridTimestamp::new(
                TimestampMillis::new(wall_time),
                u32::try_from(counter).map_err(corrupt)?,
            ))
        })
        .transpose()
}

fn successor_stamp(stamp: HybridTimestamp) -> Option<HybridTimestamp> {
    stamp
        .counter()
        .checked_add(1)
        .map(|counter| HybridTimestamp::new(stamp.wall_time(), counter))
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

fn ensure_local_device(
    connection: &Connection,
    now: TimestampMillis,
) -> Result<SyncDeviceId, LocalChangeJournalError> {
    if let Some(device) = local_device(connection)? {
        return Ok(device);
    }
    let device = SyncDeviceId::new();
    connection
        .execute(
            "INSERT INTO sync_local_state
             (id, device_id, origin_sequence, hlc_wall_time, hlc_counter)
             VALUES (1, ?1, 0, ?2, 0)",
            params![device.as_uuid().to_string(), now.get()],
        )
        .map_err(storage)?;
    Ok(device)
}

/// Tables that hold journal evidence, emptied together when the journal
/// starts over. Children come before the tables they reference.
const JOURNAL_TABLES: [&str; 11] = [
    "sync_change_frontiers",
    "sync_conflicts",
    "sync_deferred_changes",
    "sync_incoming_changes",
    "sync_incoming_batches",
    "sync_changes",
    "sync_frontiers",
    "sync_peer_frontiers",
    "sync_conversation_marks",
    "purge_queue",
    "purge_rejournals",
];

/// Starts the journal over when it was written under other payload schemas
/// than this build exchanges. Journaled payloads are immutable and bound into
/// change fingerprints peers have acknowledged, so they are never rewritten:
/// every journal table is emptied, the device takes a new sync identity (its
/// hybrid clock carries on), and the next scan journals the current state as
/// inserts stamped with each entity's own change time. Peers updated to the
/// same build start over the same way and settle the concurrent inserts by
/// last writer wins; no domain row is touched, so nothing held here is lost.
pub(crate) fn rebaseline_journal_if_format_changed(
    connection: &Connection,
) -> rusqlite::Result<bool> {
    let current = lettuce_sync::current_sync_schema_fingerprint();
    let stored = connection
        .query_row(
            "SELECT schema_fingerprint FROM sync_journal_format WHERE id = 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    if stored.as_deref() == Some(current.as_str()) {
        return Ok(false);
    }
    let clock = connection
        .query_row(
            "SELECT hlc_wall_time, hlc_counter FROM sync_local_state WHERE id = 1",
            [],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()?;
    let rebaselined = clock.is_some();
    if let Some((wall_time, counter)) = clock {
        let mut guarded = JOURNAL_TABLES.to_vec();
        guarded.push("sync_local_state");
        let placeholders = vec!["?"; guarded.len()].join(", ");
        let triggers = connection
            .prepare(&format!(
                "SELECT name, sql FROM sqlite_master
                 WHERE type = 'trigger' AND tbl_name IN ({placeholders})
                 ORDER BY name"
            ))?
            .query_map(rusqlite::params_from_iter(&guarded), |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for (name, _) in &triggers {
            connection.execute_batch(&format!("DROP TRIGGER \"{name}\""))?;
        }
        for table in JOURNAL_TABLES {
            connection.execute(&format!("DELETE FROM {table}"), [])?;
        }
        connection.execute("DELETE FROM sync_local_state", [])?;
        connection.execute(
            "INSERT INTO sync_local_state
             (id, device_id, origin_sequence, hlc_wall_time, hlc_counter)
             VALUES (1, ?1, 0, ?2, ?3)",
            params![
                SyncDeviceId::new().as_uuid().to_string(),
                wall_time,
                counter
            ],
        )?;
        for (_, sql) in &triggers {
            connection.execute_batch(sql)?;
        }
        tracing::info!("sync journal started over for changed payload schemas");
    }
    connection.execute(
        "INSERT INTO sync_journal_format (id, schema_fingerprint) VALUES (1, ?1)
         ON CONFLICT(id) DO UPDATE SET schema_fingerprint = excluded.schema_fingerprint",
        [current.as_str()],
    )?;
    Ok(rebaselined)
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
    if (current_sequence == 0 && frontier.contains_key(&device))
        || (current_sequence > 0 && frontier.get(&device) != Some(&current_sequence))
    {
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

fn observe_remote_clock(
    connection: &Connection,
    remote: HybridTimestamp,
    now: TimestampMillis,
) -> Result<(), LocalChangeJournalError> {
    let state = connection
        .query_row(
            "SELECT hlc_wall_time, hlc_counter FROM sync_local_state WHERE id = 1",
            [],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()
        .map_err(storage)?;
    let remote_wall = remote.wall_time().get();
    let remote_counter = i64::from(remote.counter());
    let Some((local_wall, local_counter)) = state else {
        let wall = now.get().max(remote_wall);
        let counter = if wall == remote_wall {
            remote_counter
                .checked_add(1)
                .ok_or(LocalChangeJournalError::Exhausted)?
        } else {
            0
        };
        connection
            .execute(
                "INSERT INTO sync_local_state
                 (id, device_id, origin_sequence, hlc_wall_time, hlc_counter)
                 VALUES (1, ?1, 0, ?2, ?3)",
                params![SyncDeviceId::new().as_uuid().to_string(), wall, counter],
            )
            .map_err(storage)?;
        return Ok(());
    };
    let wall = now.get().max(local_wall).max(remote_wall);
    let counter = if wall == local_wall && wall == remote_wall {
        local_counter
            .max(remote_counter)
            .checked_add(1)
            .ok_or(LocalChangeJournalError::Exhausted)?
    } else if wall == local_wall {
        local_counter
            .checked_add(1)
            .ok_or(LocalChangeJournalError::Exhausted)?
    } else if wall == remote_wall {
        remote_counter
            .checked_add(1)
            .ok_or(LocalChangeJournalError::Exhausted)?
    } else {
        0
    };
    connection
        .execute(
            "UPDATE sync_local_state SET hlc_wall_time = ?1, hlc_counter = ?2 WHERE id = 1",
            params![wall, counter],
        )
        .map_err(storage)?;
    Ok(())
}

fn insert_change(
    connection: &Connection,
    operation_id: Option<OperationId>,
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
                operation_id.map(|id| id.to_string()),
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

fn load_change_by_id(
    connection: &Connection,
    id: SyncChangeId,
) -> Result<Option<CanonicalChange>, IncomingChangeError> {
    let row = connection
        .query_row(
            "SELECT change_id, format_version, fingerprint, origin_device_id,
                    origin_sequence, hlc_wall_time, hlc_counter, entity_kind,
                    entity_id, operation, base_revision, payload_schema,
                    payload_version, payload_bytes, payload_hash
             FROM sync_changes WHERE change_id = ?1",
            [id.as_uuid().to_string()],
            StoredChangeRow::from_row,
        )
        .optional()
        .map_err(incoming_storage)?;
    row.map(|row| hydrate_change(connection, row).map_err(incoming_corrupt))
        .transpose()
}

fn load_materialized_change(
    connection: &Connection,
    entity: &SyncEntity,
    payload_hash: &ContentHash,
) -> Result<Option<CanonicalChange>, IncomingChangeError> {
    let id = connection
        .query_row(
            "SELECT change_id FROM sync_changes
             WHERE entity_kind = ?1 AND entity_id = ?2 AND payload_hash = ?3
             ORDER BY hlc_wall_time DESC, hlc_counter DESC,
                      origin_device_id DESC, origin_sequence DESC, change_id DESC
             LIMIT 1",
            params![entity.kind(), entity.id(), payload_hash.as_str()],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(incoming_storage)?;
    id.map(|id| {
        let id = SyncChangeId::from_uuid(Uuid::parse_str(&id).map_err(incoming_corrupt)?);
        load_change_by_id(connection, id)?.ok_or(IncomingChangeError::Corrupt)
    })
    .transpose()
}

fn load_staged_changes(
    connection: &Connection,
    batch_id: OperationId,
) -> Result<Vec<CanonicalChange>, IncomingChangeError> {
    let mut statement = connection
        .prepare(
            "SELECT change_id, fingerprint, document, payload_bytes
             FROM sync_incoming_changes WHERE batch_id = ?1 ORDER BY ordinal",
        )
        .map_err(incoming_storage)?;
    let rows = statement
        .query_map([batch_id.to_string()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Vec<u8>>(2)?,
                row.get::<_, Option<Vec<u8>>>(3)?,
            ))
        })
        .map_err(incoming_storage)?;
    let mut changes = Vec::new();
    for row in rows {
        let (id, fingerprint, document, payload_bytes) = row.map_err(incoming_storage)?;
        let stored: StoredIncomingChange =
            serde_json::from_slice(&document).map_err(incoming_corrupt)?;
        let change = stored.into_change(payload_bytes)?;
        if change.id().as_uuid().to_string() != id || change.fingerprint().as_str() != fingerprint {
            return Err(IncomingChangeError::Corrupt);
        }
        changes.push(change);
    }
    Ok(changes)
}

fn supported_change(change: &CanonicalChange) -> Result<bool, IncomingChangeError> {
    let Some(payload) = change.payload() else {
        return Ok(change.operation() == ChangeOperation::Delete
            && snapshot_codec(change.entity().kind()).is_some_and(|codec| codec.delete.is_some()));
    };
    match (change.entity().kind(), payload.schema(), payload.version()) {
        ("persona", PERSONA_SYNC_SCHEMA, PERSONA_SYNC_VERSION) => {
            let persona: Persona =
                serde_json::from_slice(payload.bytes()).map_err(incoming_corrupt)?;
            persona.validate().map_err(incoming_corrupt)?;
            if persona.id.to_string() != change.entity().id()
                || !matches!(
                    change.operation(),
                    ChangeOperation::Insert | ChangeOperation::Update
                )
            {
                return Err(IncomingChangeError::Corrupt);
            }
            Ok(true)
        }
        ("persona_default", PERSONA_DEFAULT_SYNC_SCHEMA, PERSONA_DEFAULT_SYNC_VERSION) => {
            let state: PersonaDefaultState =
                serde_json::from_slice(payload.bytes()).map_err(incoming_corrupt)?;
            state.validate().map_err(incoming_corrupt)?;
            if change.entity().id() != "application"
                || !matches!(
                    change.operation(),
                    ChangeOperation::Insert | ChangeOperation::Update
                )
            {
                return Err(IncomingChangeError::Corrupt);
            }
            Ok(true)
        }
        (
            lettuce_sync::PROVIDER_ACCOUNT_SYNC_KIND,
            lettuce_sync::PROVIDER_ACCOUNT_SYNC_SCHEMA,
            lettuce_sync::PROVIDER_ACCOUNT_SYNC_VERSION,
        )
        | (
            lettuce_sync::MODEL_PROFILE_SYNC_KIND,
            lettuce_sync::MODEL_PROFILE_SYNC_SCHEMA,
            lettuce_sync::MODEL_PROFILE_SYNC_VERSION,
        )
        | (
            lettuce_sync::CHARACTER_SYNC_KIND,
            lettuce_sync::CHARACTER_SYNC_SCHEMA,
            lettuce_sync::CHARACTER_SYNC_VERSION,
        )
        | (
            lettuce_sync::LOREBOOK_SYNC_KIND,
            lettuce_sync::LOREBOOK_SYNC_SCHEMA,
            lettuce_sync::LOREBOOK_SYNC_VERSION,
        )
        | (
            lettuce_sync::APP_SETTINGS_SYNC_KIND,
            lettuce_sync::APP_SETTINGS_SYNC_SCHEMA,
            lettuce_sync::APP_SETTINGS_SYNC_VERSION,
        )
        | (
            lettuce_sync::CONVERSATION_SYNC_KIND,
            lettuce_sync::CONVERSATION_SYNC_SCHEMA,
            lettuce_sync::CONVERSATION_SYNC_VERSION,
        )
        | (
            lettuce_sync::AUDIO_PROVIDER_SYNC_KIND,
            lettuce_sync::AUDIO_PROVIDER_SYNC_SCHEMA,
            lettuce_sync::AUDIO_PROVIDER_SYNC_VERSION,
        )
        | (
            lettuce_sync::USER_VOICE_SYNC_KIND,
            lettuce_sync::USER_VOICE_SYNC_SCHEMA,
            lettuce_sync::USER_VOICE_SYNC_VERSION,
        )
        | (
            lettuce_sync::ASR_VOCABULARY_TERM_SYNC_KIND,
            lettuce_sync::ASR_VOCABULARY_TERM_SYNC_SCHEMA,
            lettuce_sync::ASR_VOCABULARY_TERM_SYNC_VERSION,
        )
        | (
            lettuce_sync::ASR_CORRECTION_SYNC_KIND,
            lettuce_sync::ASR_CORRECTION_SYNC_SCHEMA,
            lettuce_sync::ASR_CORRECTION_SYNC_VERSION,
        )
        | (
            lettuce_sync::ASR_IGNORED_SUGGESTION_SYNC_KIND,
            lettuce_sync::ASR_IGNORED_SUGGESTION_SYNC_SCHEMA,
            lettuce_sync::ASR_IGNORED_SUGGESTION_SYNC_VERSION,
        )
        | (
            lettuce_sync::ASR_VOICE_EXAMPLE_SYNC_KIND,
            lettuce_sync::ASR_VOICE_EXAMPLE_SYNC_SCHEMA,
            lettuce_sync::ASR_VOICE_EXAMPLE_SYNC_VERSION,
        )
        | (
            lettuce_sync::USAGE_COST_SYNC_KIND,
            lettuce_sync::USAGE_COST_SYNC_SCHEMA,
            lettuce_sync::USAGE_COST_SYNC_VERSION,
        )
        | (
            lettuce_sync::JOB_USAGE_SYNC_KIND,
            lettuce_sync::JOB_USAGE_SYNC_SCHEMA,
            lettuce_sync::JOB_USAGE_SYNC_VERSION,
        )
        | (
            lettuce_sync::JOB_USAGE_COST_SYNC_KIND,
            lettuce_sync::JOB_USAGE_COST_SYNC_SCHEMA,
            lettuce_sync::JOB_USAGE_COST_SYNC_VERSION,
        )
        | (
            lettuce_sync::LEGACY_USAGE_SYNC_KIND,
            lettuce_sync::LEGACY_USAGE_SYNC_SCHEMA,
            lettuce_sync::LEGACY_USAGE_SYNC_VERSION,
        )
        | (
            lettuce_sync::PLAYGROUND_HISTORY_SYNC_KIND,
            lettuce_sync::PLAYGROUND_HISTORY_SYNC_SCHEMA,
            lettuce_sync::PLAYGROUND_HISTORY_SYNC_VERSION,
        )
        | (
            lettuce_sync::PLAYGROUND_IMAGE_SYNC_KIND,
            lettuce_sync::PLAYGROUND_IMAGE_SYNC_SCHEMA,
            lettuce_sync::PLAYGROUND_IMAGE_SYNC_VERSION,
        )
        | (
            lettuce_sync::CREATION_WORKFLOW_SYNC_KIND,
            lettuce_sync::CREATION_WORKFLOW_SYNC_SCHEMA,
            lettuce_sync::CREATION_WORKFLOW_SYNC_VERSION,
        )
        | (
            lettuce_sync::COMPANION_SOUL_SYNC_KIND,
            lettuce_sync::COMPANION_SOUL_SYNC_SCHEMA,
            lettuce_sync::COMPANION_SOUL_SYNC_VERSION,
        )
        | (
            lettuce_sync::COMPANION_RELATIONSHIP_SYNC_KIND,
            lettuce_sync::COMPANION_RELATIONSHIP_SYNC_SCHEMA,
            lettuce_sync::COMPANION_RELATIONSHIP_SYNC_VERSION,
        )
        | (
            lettuce_sync::COMPANION_SESSION_SYNC_KIND,
            lettuce_sync::COMPANION_SESSION_SYNC_SCHEMA,
            lettuce_sync::COMPANION_SESSION_SYNC_VERSION,
        )
        | (
            lettuce_sync::COMPANION_NOTE_SYNC_KIND,
            lettuce_sync::COMPANION_NOTE_SYNC_SCHEMA,
            lettuce_sync::COMPANION_NOTE_SYNC_VERSION,
        )
        | (
            lettuce_sync::MEMORY_ITEM_SYNC_KIND,
            lettuce_sync::MEMORY_ITEM_SYNC_SCHEMA,
            lettuce_sync::MEMORY_ITEM_SYNC_VERSION,
        )
        | (
            lettuce_sync::MEMORY_CURSOR_SYNC_KIND,
            lettuce_sync::MEMORY_CURSOR_SYNC_SCHEMA,
            lettuce_sync::MEMORY_CURSOR_SYNC_VERSION,
        )
        | (
            lettuce_sync::MEMORY_SUMMARY_SYNC_KIND,
            lettuce_sync::MEMORY_SUMMARY_SYNC_SCHEMA,
            lettuce_sync::MEMORY_SUMMARY_SYNC_VERSION,
        )
        | (
            lettuce_sync::CONVERSATION_BRANCH_SYNC_KIND,
            lettuce_sync::CONVERSATION_BRANCH_SYNC_SCHEMA,
            lettuce_sync::CONVERSATION_BRANCH_SYNC_VERSION,
        )
        | (
            lettuce_sync::CONVERSATION_MESSAGE_SYNC_KIND,
            lettuce_sync::CONVERSATION_MESSAGE_SYNC_SCHEMA,
            lettuce_sync::CONVERSATION_MESSAGE_SYNC_VERSION,
        )
        | (
            lettuce_sync::GROUP_SYNC_KIND,
            lettuce_sync::GROUP_SYNC_SCHEMA,
            lettuce_sync::GROUP_SYNC_VERSION,
        )
        | (
            lettuce_sync::PROMPT_SYNC_KIND,
            lettuce_sync::PROMPT_SYNC_SCHEMA,
            lettuce_sync::PROMPT_SYNC_VERSION,
        )
        | (
            lettuce_sync::CHARACTER_LOREBOOK_BINDINGS_SYNC_KIND
            | lettuce_sync::PERSONA_LOREBOOK_BINDINGS_SYNC_KIND
            | lettuce_sync::GROUP_LOREBOOK_BINDINGS_SYNC_KIND,
            lettuce_sync::LOREBOOK_BINDINGS_SYNC_SCHEMA,
            lettuce_sync::LOREBOOK_BINDINGS_SYNC_VERSION,
        ) => {
            let codec =
                snapshot_codec(change.entity().kind()).ok_or(IncomingChangeError::Corrupt)?;
            (codec.decode)(change.entity().id(), payload.bytes())
                .map_err(|_| IncomingChangeError::Corrupt)?;
            Ok(true)
        }
        (
            lettuce_sync::CONVERSATION_SNAPSHOT_SYNC_KIND,
            lettuce_sync::CONVERSATION_SNAPSHOT_SYNC_SCHEMA,
            lettuce_sync::CONVERSATION_SNAPSHOT_SYNC_VERSION,
        ) => {
            if change.operation() != ChangeOperation::Insert {
                return Err(IncomingChangeError::Corrupt);
            }
            serde_json::from_slice::<
                crate::conversation::conversation_artifact_adapter::SyncSnapshotArtifact,
            >(payload.bytes())
            .map_err(incoming_corrupt)?;
            Ok(true)
        }
        ("media_asset", MEDIA_ASSET_SYNC_SCHEMA, MEDIA_ASSET_SYNC_VERSION) => {
            let value: lettuce_media::SyncMediaAsset =
                serde_json::from_slice(payload.bytes()).map_err(incoming_corrupt)?;
            value.validate().map_err(incoming_corrupt)?;
            if value.asset.id.to_string() != change.entity().id()
                || change.operation() != ChangeOperation::Insert
            {
                return Err(IncomingChangeError::Corrupt);
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn incoming_wins(incoming: &CanonicalChange, current: &CanonicalChange) -> bool {
    (
        incoming.timestamp(),
        incoming.origin_device(),
        incoming.origin_sequence(),
        incoming.id(),
    ) > (
        current.timestamp(),
        current.origin_device(),
        current.origin_sequence(),
        current.id(),
    )
}

enum ApplyOneError {
    Pending,
    Corrupt,
    /// The local entity cannot be encoded as a canonical payload (it exceeds
    /// the payload limit).
    Unencodable,
    Storage,
}

fn payload_error(error: lettuce_sync::SyncChangeError) -> ApplyOneError {
    match error {
        lettuce_sync::SyncChangeError::InvalidPayloadSize { .. } => ApplyOneError::Unencodable,
        _ => ApplyOneError::Corrupt,
    }
}

/// Records, once until the user dismisses it, that an entity cannot be
/// encoded for sync on this device.
fn record_not_synced(
    connection: &Connection,
    kind: &str,
    id: &str,
    now: TimestampMillis,
) -> Result<(), ApplyOneError> {
    let entity = format!("{kind}/{id}");
    let recorded: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM purge_notices
             WHERE entity_kind = 'sync_entity' AND entity_id = ?1
               AND reason = 'not_synced' AND dismissed_at IS NULL)",
            [&entity],
            |row| row.get(0),
        )
        .map_err(|_| ApplyOneError::Storage)?;
    if !recorded {
        tracing::warn!(kind, "a synced entity is too large to sync");
        crate::purge::record_notice(
            connection,
            crate::PurgeNoticeEntity::SyncEntity,
            &entity,
            crate::PurgeNoticeReason::NotSynced,
            now,
        )
        .map_err(|_| ApplyOneError::Storage)?;
    }
    Ok(())
}

fn repository_apply_error(error: RepositoryError) -> ApplyOneError {
    match error {
        RepositoryError::NotFound
        | RepositoryError::Archived
        | RepositoryError::HasDependencies => ApplyOneError::Pending,
        RepositoryError::Storage => ApplyOneError::Storage,
        _ => ApplyOneError::Corrupt,
    }
}

fn insert_conflict(
    tx: &Transaction<'_>,
    change: &CanonicalChange,
    current: Option<&CanonicalChange>,
    current_payload: Option<&[u8]>,
    incoming_payload: &[u8],
    winner_is_incoming: bool,
    now: TimestampMillis,
) -> Result<(), ApplyOneError> {
    let conflict_id = Uuid::new_v5(&change.id().as_uuid(), b"entity-conflict");
    tx.execute(
        "INSERT INTO sync_conflicts (
           conflict_id, entity_kind, entity_id, current_change_id,
           incoming_change_id, winning_side, current_payload, incoming_payload,
           detected_at
         ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)
         ON CONFLICT(incoming_change_id) DO NOTHING",
        params![
            conflict_id.to_string(),
            change.entity().kind(),
            change.entity().id(),
            current.map(|value| value.id().as_uuid().to_string()),
            change.id().as_uuid().to_string(),
            if winner_is_incoming {
                "incoming"
            } else {
                "current"
            },
            current_payload,
            incoming_payload,
            now.get()
        ],
    )
    .map_err(|_| ApplyOneError::Storage)?;
    Ok(())
}

fn resolve_dominated_conflicts(
    connection: &Connection,
    winner: &CanonicalChange,
    now: TimestampMillis,
    skipped_conflict: Option<OperationId>,
) -> Result<(), ApplyOneError> {
    let mut statement = connection
        .prepare(
            "SELECT conflict_id, current_change_id, incoming_change_id
             FROM sync_conflicts
             WHERE status = 'unresolved' AND entity_kind = ?1 AND entity_id = ?2",
        )
        .map_err(|_| ApplyOneError::Storage)?;
    let rows = statement
        .query_map(
            params![winner.entity().kind(), winner.entity().id()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .map_err(|_| ApplyOneError::Storage)?;
    let mut resolved = Vec::new();
    for row in rows {
        let (conflict_id, current_id, incoming_id) = row.map_err(|_| ApplyOneError::Storage)?;
        if skipped_conflict.is_some_and(|skipped| skipped.to_string() == conflict_id) {
            continue;
        }
        let incoming_id = SyncChangeId::from_uuid(
            Uuid::parse_str(&incoming_id).map_err(|_| ApplyOneError::Corrupt)?,
        );
        let current = current_id
            .map(|current_id| {
                let current_id = SyncChangeId::from_uuid(
                    Uuid::parse_str(&current_id).map_err(|_| ApplyOneError::Corrupt)?,
                );
                load_change_by_id(connection, current_id)
                    .map_err(|error| match error {
                        IncomingChangeError::Storage => ApplyOneError::Storage,
                        _ => ApplyOneError::Corrupt,
                    })?
                    .ok_or(ApplyOneError::Corrupt)
            })
            .transpose()?;
        let incoming = load_change_by_id(connection, incoming_id)
            .map_err(|error| match error {
                IncomingChangeError::Storage => ApplyOneError::Storage,
                _ => ApplyOneError::Corrupt,
            })?
            .ok_or(ApplyOneError::Corrupt)?;
        if current
            .as_ref()
            .is_none_or(|current| winner.id() == current.id() || winner.observes(current))
            && (winner.id() == incoming.id() || winner.observes(&incoming))
        {
            resolved.push(conflict_id);
        }
    }
    drop(statement);
    for conflict_id in resolved {
        connection
            .execute(
                "UPDATE sync_conflicts SET status = 'resolved',
             resolution_choice = 'superseded', resolved_by_change_id = ?2,
             resolved_at = ?3 WHERE conflict_id = ?1 AND status = 'unresolved'",
                params![conflict_id, winner.id().as_uuid().to_string(), now.get()],
            )
            .map_err(|_| ApplyOneError::Storage)?;
    }
    Ok(())
}

/// How one synced aggregate kind reads its current canonical snapshot and
/// materializes an incoming one. `materialize` returns `false` when the
/// snapshot is valid but cannot be applied here (the change is journaled and
/// kept as a current-wins conflict).
type ScanIds = fn(&Connection) -> Result<Vec<String>, ApplyOneError>;
type SnapshotDelete =
    fn(&Transaction<'_>, &CanonicalChange, TimestampMillis) -> Result<bool, ApplyOneError>;

struct SnapshotCodec {
    kind: &'static str,
    decode: fn(&str, &[u8]) -> Result<(), ApplyOneError>,
    current: fn(&Transaction<'_>, &str) -> Result<Option<CanonicalPayload>, ApplyOneError>,
    materialize: fn(&Transaction<'_>, &str, &[u8]) -> Result<bool, ApplyOneError>,
    ids: Option<ScanIds>,
    delete: Option<SnapshotDelete>,
    empty: Option<fn() -> Result<CanonicalPayload, ApplyOneError>>,
    /// An untouched seed snapshot (a fresh device's defaults) that any edited
    /// snapshot replaces.
    seed: Option<fn(&[u8]) -> bool>,
    assets: fn(&[u8]) -> Vec<String>,
}

fn no_assets(_: &[u8]) -> Vec<String> {
    Vec::new()
}

const PERSONA_CODEC: SnapshotCodec = SnapshotCodec {
    empty: None,
    seed: None,
    assets: |bytes| {
        serde_json::from_slice::<Persona>(bytes)
            .map(|persona| {
                persona
                    .media
                    .links
                    .iter()
                    .map(|link| link.asset_id.to_string())
                    .collect()
            })
            .unwrap_or_default()
    },
    kind: "persona",
    ids: Some(|connection| {
        connection
            .prepare("SELECT id FROM personas ORDER BY id")
            .and_then(|mut statement| {
                statement
                    .query_map([], |row| row.get::<_, String>(0))?
                    .collect()
            })
            .map_err(|_| ApplyOneError::Storage)
    }),
    delete: None,
    decode: |id, bytes| {
        let incoming: Persona =
            serde_json::from_slice(bytes).map_err(|_| ApplyOneError::Corrupt)?;
        let id = id
            .parse::<PersonaId>()
            .map_err(|_| ApplyOneError::Corrupt)?;
        if incoming.id != id {
            return Err(ApplyOneError::Corrupt);
        }
        Ok(())
    },
    current: |connection, id| {
        let id = id
            .parse::<PersonaId>()
            .map_err(|_| ApplyOneError::Corrupt)?;
        load_persona(connection, id)
            .map_err(|_| ApplyOneError::Storage)?
            .as_ref()
            .map(canonical_persona_payload)
            .transpose()
            .map_err(|_| ApplyOneError::Corrupt)
    },
    materialize: |tx, _, bytes| {
        let incoming: Persona =
            serde_json::from_slice(bytes).map_err(|_| ApplyOneError::Corrupt)?;
        for link in &incoming.media.links {
            let present: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM media_assets WHERE id=?1)",
                    [link.asset_id.to_string()],
                    |row| row.get(0),
                )
                .map_err(|_| ApplyOneError::Storage)?;
            if !present {
                return Err(ApplyOneError::Pending);
            }
        }
        apply_synced_persona(tx, incoming).map_err(repository_apply_error)?;
        Ok(true)
    },
};

const PERSONA_DEFAULT_CODEC: SnapshotCodec = SnapshotCodec {
    empty: None,
    seed: Some(|bytes| {
        serde_json::from_slice::<PersonaDefaultState>(bytes)
            .is_ok_and(|state| state.revision == lettuce_types::Revision::INITIAL)
    }),
    assets: no_assets,
    kind: "persona_default",
    ids: Some(|_| Ok(vec!["application".to_owned()])),
    delete: None,
    decode: |_, bytes| {
        serde_json::from_slice::<PersonaDefaultState>(bytes)
            .map(|_| ())
            .map_err(|_| ApplyOneError::Corrupt)
    },
    current: |connection, _| {
        let current = read_default(connection).map_err(|_| ApplyOneError::Storage)?;
        canonical_persona_default_payload(&current)
            .map(Some)
            .map_err(|_| ApplyOneError::Corrupt)
    },
    materialize: |tx, _, bytes| {
        let incoming: PersonaDefaultState =
            serde_json::from_slice(bytes).map_err(|_| ApplyOneError::Corrupt)?;
        match apply_synced_persona_default(tx, incoming) {
            Ok(_) => Ok(true),
            Err(RepositoryError::Archived | RepositoryError::NotFound) => Ok(false),
            Err(error) => Err(repository_apply_error(error)),
        }
    },
};

fn model_apply_error(error: lettuce_models::ModelRepositoryError) -> ApplyOneError {
    match error {
        lettuce_models::ModelRepositoryError::AccountMissing
        | lettuce_models::ModelRepositoryError::NotFound => ApplyOneError::Pending,
        lettuce_models::ModelRepositoryError::Storage => ApplyOneError::Storage,
        _ => ApplyOneError::Corrupt,
    }
}

const PROVIDER_ACCOUNT_CODEC: SnapshotCodec = SnapshotCodec {
    assets: no_assets,
    kind: lettuce_sync::PROVIDER_ACCOUNT_SYNC_KIND,
    decode: |id, bytes| {
        let account: lettuce_models::ProviderAccount =
            serde_json::from_slice(bytes).map_err(|_| ApplyOneError::Corrupt)?;
        if account.id.to_string() != id {
            return Err(ApplyOneError::Corrupt);
        }
        Ok(())
    },
    current: |connection, id| {
        crate::sync_load_provider_account(connection, id)
            .map_err(model_apply_error)?
            .as_ref()
            .map(lettuce_sync::canonical_provider_account_payload)
            .transpose()
            .map_err(|_| ApplyOneError::Corrupt)
    },
    materialize: |tx, _, bytes| {
        let account: lettuce_models::ProviderAccount =
            serde_json::from_slice(bytes).map_err(|_| ApplyOneError::Corrupt)?;
        crate::sync_upsert_provider_account(tx, &account).map_err(model_apply_error)?;
        Ok(true)
    },
    ids: Some(|connection| {
        crate::sync_ids(connection, "provider_accounts").map_err(|_| ApplyOneError::Storage)
    }),
    delete: Some(|tx, change, _| {
        let id = change.entity().id();
        crate::sync_delete_provider_account(tx, id).map_err(model_apply_error)
    }),
    empty: None,
    seed: None,
};

const MODEL_PROFILE_CODEC: SnapshotCodec = SnapshotCodec {
    assets: no_assets,
    kind: lettuce_sync::MODEL_PROFILE_SYNC_KIND,
    decode: |id, bytes| {
        let profile: lettuce_models::ModelProfile =
            serde_json::from_slice(bytes).map_err(|_| ApplyOneError::Corrupt)?;
        if profile.id.to_string() != id {
            return Err(ApplyOneError::Corrupt);
        }
        Ok(())
    },
    current: |connection, id| {
        crate::sync_load_model_profile(connection, id)
            .map_err(model_apply_error)?
            .map(|profile| crate::sync_exchanged_model_profile(connection, profile))
            .transpose()
            .map_err(model_apply_error)?
            .as_ref()
            .map(lettuce_sync::canonical_model_profile_payload)
            .transpose()
            .map_err(|_| ApplyOneError::Corrupt)
    },
    materialize: |tx, _, bytes| {
        let profile: lettuce_models::ModelProfile =
            serde_json::from_slice(bytes).map_err(|_| ApplyOneError::Corrupt)?;
        match crate::sync_upsert_model_profile(tx, &profile) {
            Ok(()) => Ok(true),
            Err(lettuce_models::ModelRepositoryError::AccountMissing) => Ok(false),
            Err(error) => Err(model_apply_error(error)),
        }
    },
    ids: Some(|connection| {
        crate::sync_ids(connection, "model_profiles").map_err(|_| ApplyOneError::Storage)
    }),
    delete: Some(|tx, change, now| {
        let id = change.entity().id();
        crate::sync_delete_model_profile(tx, id, now).map_err(model_apply_error)
    }),
    empty: None,
    seed: None,
};

const CHARACTER_CODEC: SnapshotCodec = SnapshotCodec {
    assets: |bytes| {
        serde_json::from_slice::<lettuce_characters::CharacterDetails>(bytes)
            .map(|details| {
                crate::catalog::character_adapter::character_asset_ids(&details)
                    .into_iter()
                    .map(|id| id.to_string())
                    .collect()
            })
            .unwrap_or_default()
    },
    kind: lettuce_sync::CHARACTER_SYNC_KIND,
    decode: |id, bytes| {
        let details: lettuce_characters::CharacterDetails =
            serde_json::from_slice(bytes).map_err(|_| ApplyOneError::Corrupt)?;
        if details.character.id.to_string() != id || details.validate().is_err() {
            return Err(ApplyOneError::Corrupt);
        }
        Ok(())
    },
    current: |connection, id| {
        let id = id
            .parse::<lettuce_types::CharacterId>()
            .map_err(|_| ApplyOneError::Corrupt)?;
        crate::catalog::character_adapter::load_character_details(connection, id)
            .map_err(repository_apply_error)?
            .as_ref()
            .map(lettuce_sync::canonical_character_payload)
            .transpose()
            .map_err(|_| ApplyOneError::Corrupt)
    },
    materialize: |tx, _, bytes| {
        let details: lettuce_characters::CharacterDetails =
            serde_json::from_slice(bytes).map_err(|_| ApplyOneError::Corrupt)?;
        crate::catalog::character_adapter::sync_replace_character(tx, &details)
            .map_err(repository_apply_error)?;
        Ok(true)
    },
    ids: Some(|connection| {
        crate::catalog::character_adapter::sync_character_ids(connection)
            .map_err(repository_apply_error)
    }),
    delete: Some(|tx, change, now| {
        let id = change.entity().id();
        crate::purge::queue_purge(tx, crate::purge::PurgeKind::Character, id, change, now)
            .map_err(|_| ApplyOneError::Storage)?;
        Ok(true)
    }),
    empty: None,
    seed: None,
};

fn lorebook_apply_error(error: lettuce_context::LorebookRepositoryError) -> ApplyOneError {
    match error {
        lettuce_context::LorebookRepositoryError::NotFound => ApplyOneError::Pending,
        lettuce_context::LorebookRepositoryError::Failure(_) => ApplyOneError::Storage,
        _ => ApplyOneError::Corrupt,
    }
}

const LOREBOOK_CODEC: SnapshotCodec = SnapshotCodec {
    assets: |bytes| {
        serde_json::from_slice::<lettuce_context::LorebookDetails>(bytes)
            .ok()
            .and_then(|details| details.book.icon_asset_id)
            .map(|id| vec![id.to_string()])
            .unwrap_or_default()
    },
    kind: lettuce_sync::LOREBOOK_SYNC_KIND,
    decode: |id, bytes| {
        let details: lettuce_context::LorebookDetails =
            serde_json::from_slice(bytes).map_err(|_| ApplyOneError::Corrupt)?;
        if details.book.id.to_string() != id || details.validate().is_err() {
            return Err(ApplyOneError::Corrupt);
        }
        Ok(())
    },
    current: |connection, id| {
        let id = id
            .parse::<lettuce_types::LorebookId>()
            .map_err(|_| ApplyOneError::Corrupt)?;
        crate::lorebook::lorebook_adapter::load_details(connection, id)
            .map_err(|_| ApplyOneError::Storage)?
            .as_ref()
            .map(lettuce_sync::canonical_lorebook_payload)
            .transpose()
            .map_err(|_| ApplyOneError::Corrupt)
    },
    materialize: |tx, _, bytes| {
        let details: lettuce_context::LorebookDetails =
            serde_json::from_slice(bytes).map_err(|_| ApplyOneError::Corrupt)?;
        crate::lorebook::lorebook_adapter::sync_replace_lorebook(tx, &details)
            .map_err(lorebook_apply_error)?;
        Ok(true)
    },
    ids: Some(|connection| {
        crate::lorebook::lorebook_adapter::sync_lorebook_ids(connection)
            .map_err(|_| ApplyOneError::Storage)
    }),
    delete: None,
    empty: None,
    seed: None,
};

fn decode_bindings(bytes: &[u8]) -> Result<Vec<lettuce_context::LorebookBinding>, ApplyOneError> {
    let bindings: Vec<lettuce_context::LorebookBinding> =
        serde_json::from_slice(bytes).map_err(|_| ApplyOneError::Corrupt)?;
    if lettuce_context::validate_bindings(&bindings).is_err() {
        return Err(ApplyOneError::Corrupt);
    }
    Ok(bindings)
}

fn prompt_apply_error(error: lettuce_context::PromptRepositoryError) -> ApplyOneError {
    match error {
        lettuce_context::PromptRepositoryError::NotFound => ApplyOneError::Pending,
        lettuce_context::PromptRepositoryError::Failure(_) => ApplyOneError::Storage,
        _ => ApplyOneError::Corrupt,
    }
}

const PROMPT_CODEC: SnapshotCodec = SnapshotCodec {
    kind: lettuce_sync::PROMPT_SYNC_KIND,
    assets: no_assets,
    decode: |id, bytes| {
        let document: lettuce_context::PromptDocument =
            serde_json::from_slice(bytes).map_err(|_| ApplyOneError::Corrupt)?;
        if document.id.to_string() != id || document.validate().is_err() {
            return Err(ApplyOneError::Corrupt);
        }
        Ok(())
    },
    current: |connection, id| {
        let id = id
            .parse::<lettuce_types::PromptDocumentId>()
            .map_err(|_| ApplyOneError::Corrupt)?;
        crate::catalog::prompt_adapter::load_document(connection, id)
            .map_err(|_| ApplyOneError::Storage)?
            .as_ref()
            .map(lettuce_sync::canonical_prompt_payload)
            .transpose()
            .map_err(|_| ApplyOneError::Corrupt)
    },
    materialize: |tx, _, bytes| {
        let document: lettuce_context::PromptDocument =
            serde_json::from_slice(bytes).map_err(|_| ApplyOneError::Corrupt)?;
        crate::catalog::prompt_adapter::sync_replace_prompt(tx, &document)
            .map_err(prompt_apply_error)
    },
    ids: Some(|connection| {
        crate::catalog::prompt_adapter::sync_prompt_ids(connection)
            .map_err(|_| ApplyOneError::Storage)
    }),
    delete: None,
    empty: None,
    seed: None,
};

fn app_settings_payload(
    snapshot: &crate::SyncAppSettings,
) -> Result<CanonicalPayload, ApplyOneError> {
    CanonicalPayload::new(
        lettuce_sync::APP_SETTINGS_SYNC_SCHEMA,
        lettuce_sync::APP_SETTINGS_SYNC_VERSION,
        serde_json::to_vec(snapshot).map_err(|_| ApplyOneError::Corrupt)?,
    )
    .map_err(payload_error)
}

const APP_SETTINGS_CODEC: SnapshotCodec = SnapshotCodec {
    kind: lettuce_sync::APP_SETTINGS_SYNC_KIND,
    assets: no_assets,
    decode: |id, bytes| {
        let snapshot: crate::SyncAppSettings =
            serde_json::from_slice(bytes).map_err(|_| ApplyOneError::Corrupt)?;
        if id != "application"
            || snapshot.model_settings.validate().is_err()
            || !snapshot.settings.ui_preferences.within_bounds()
        {
            return Err(ApplyOneError::Corrupt);
        }
        Ok(())
    },
    current: |connection, _| {
        let snapshot =
            crate::sync_load_app_settings(connection).map_err(|_| ApplyOneError::Storage)?;
        app_settings_payload(&snapshot).map(Some)
    },
    materialize: |tx, _, bytes| {
        let snapshot: crate::SyncAppSettings =
            serde_json::from_slice(bytes).map_err(|_| ApplyOneError::Corrupt)?;
        crate::sync_write_app_settings(tx, &snapshot).map_err(|error| match error {
            rusqlite::Error::QueryReturnedNoRows => ApplyOneError::Pending,
            _ => ApplyOneError::Storage,
        })?;
        Ok(true)
    },
    ids: Some(|_| Ok(vec!["application".to_owned()])),
    delete: None,
    empty: None,
    seed: Some(|bytes| {
        serde_json::from_slice::<crate::SyncAppSettings>(bytes)
            .is_ok_and(|snapshot| snapshot.revision == lettuce_types::Revision::INITIAL)
    }),
};

macro_rules! binding_codec {
    ($name:ident, $kind:expr, $owner:expr) => {
        const $name: SnapshotCodec = SnapshotCodec {
            kind: $kind,
            assets: no_assets,
            decode: |_, bytes| decode_bindings(bytes).map(|_| ()),
            current: |connection, id| {
                crate::lorebook::lorebook_adapter::sync_load_bindings(connection, $owner, id)
                    .map_err(|_| ApplyOneError::Storage)?
                    .as_deref()
                    .map(lettuce_sync::canonical_lorebook_bindings_payload)
                    .transpose()
                    .map_err(|_| ApplyOneError::Corrupt)
            },
            materialize: |tx, id, bytes| {
                crate::lorebook::lorebook_adapter::sync_replace_bindings(
                    tx,
                    $owner,
                    id,
                    &decode_bindings(bytes)?,
                )
                .map_err(lorebook_apply_error)?;
                Ok(true)
            },
            ids: Some(|connection| {
                crate::lorebook::lorebook_adapter::sync_binding_owner_ids(connection, $owner)
                    .map_err(|_| ApplyOneError::Storage)
            }),
            delete: None,
            empty: Some(|| {
                lettuce_sync::canonical_lorebook_bindings_payload(&[])
                    .map_err(|_| ApplyOneError::Corrupt)
            }),
            seed: None,
        };
    };
}

binding_codec!(
    CHARACTER_BINDINGS_CODEC,
    lettuce_sync::CHARACTER_LOREBOOK_BINDINGS_SYNC_KIND,
    crate::lorebook::lorebook_adapter::OwnerKind::Character
);
const GROUP_CODEC: SnapshotCodec = SnapshotCodec {
    kind: lettuce_sync::GROUP_SYNC_KIND,
    assets: |bytes| {
        serde_json::from_slice::<lettuce_characters::GroupDetails>(bytes)
            .map(|details| {
                crate::catalog::group_adapter::group_asset_ids(&details)
                    .into_iter()
                    .map(|id| id.to_string())
                    .collect()
            })
            .unwrap_or_default()
    },
    decode: |id, bytes| {
        let details: lettuce_characters::GroupDetails =
            serde_json::from_slice(bytes).map_err(|_| ApplyOneError::Corrupt)?;
        if details.group.id.to_string() != id || details.validate().is_err() {
            return Err(ApplyOneError::Corrupt);
        }
        Ok(())
    },
    current: |connection, id| {
        let id = id
            .parse::<lettuce_types::GroupId>()
            .map_err(|_| ApplyOneError::Corrupt)?;
        crate::catalog::group_adapter::load_details(connection, id)
            .map_err(|_| ApplyOneError::Storage)?
            .as_ref()
            .map(lettuce_sync::canonical_group_payload)
            .transpose()
            .map_err(|_| ApplyOneError::Corrupt)
    },
    materialize: |tx, _, bytes| {
        let details: lettuce_characters::GroupDetails =
            serde_json::from_slice(bytes).map_err(|_| ApplyOneError::Corrupt)?;
        crate::catalog::group_adapter::sync_replace_group(tx, &details)
            .map_err(repository_apply_error)?;
        Ok(true)
    },
    ids: Some(|connection| {
        crate::catalog::group_adapter::sync_group_ids(connection).map_err(repository_apply_error)
    }),
    delete: None,
    empty: None,
    seed: None,
};

binding_codec!(
    GROUP_BINDINGS_CODEC,
    lettuce_sync::GROUP_LOREBOOK_BINDINGS_SYNC_KIND,
    crate::lorebook::lorebook_adapter::OwnerKind::Group
);
binding_codec!(
    PERSONA_BINDINGS_CODEC,
    lettuce_sync::PERSONA_LOREBOOK_BINDINGS_SYNC_KIND,
    crate::lorebook::lorebook_adapter::OwnerKind::Persona
);

fn conversation_apply_error(
    error: lettuce_conversations::ConversationRepositoryError,
) -> ApplyOneError {
    match error {
        lettuce_conversations::ConversationRepositoryError::NotFound => ApplyOneError::Pending,
        lettuce_conversations::ConversationRepositoryError::Storage => ApplyOneError::Storage,
        _ => ApplyOneError::Corrupt,
    }
}

const CONVERSATION_CODEC: SnapshotCodec = SnapshotCodec {
    kind: lettuce_sync::CONVERSATION_SYNC_KIND,
    assets: |bytes| {
        serde_json::from_slice::<crate::sync::conversation_sync_adapter::SyncConversationRoot>(
            bytes,
        )
        .ok()
        .and_then(|root| root.conversation.current_settings)
        .and_then(|settings| match settings.background {
            Some(lettuce_conversations::ConversationBackground::Image { asset_id }) => {
                Some(asset_id.to_string())
            }
            _ => None,
        })
        .into_iter()
        .collect()
    },
    empty: None,
    seed: None,
    decode: |id, bytes| {
        let root: crate::sync::conversation_sync_adapter::SyncConversationRoot =
            serde_json::from_slice(bytes).map_err(|_| ApplyOneError::Corrupt)?;
        if root.conversation.id.to_string() != id
            || root.root_branch.conversation_id != root.conversation.id
            || root.root_branch.parent_branch_id.is_some()
        {
            return Err(ApplyOneError::Corrupt);
        }
        Ok(())
    },
    current: |tx, id| {
        crate::sync::conversation_sync_adapter::sync_load_conversation_root(tx, id)
            .map_err(conversation_apply_error)?
            .map(|root| {
                CanonicalPayload::new(
                    lettuce_sync::CONVERSATION_SYNC_SCHEMA,
                    lettuce_sync::CONVERSATION_SYNC_VERSION,
                    serde_json::to_vec(&root).map_err(|_| ApplyOneError::Corrupt)?,
                )
                .map_err(payload_error)
            })
            .transpose()
    },
    materialize: |tx, _, bytes| {
        let root: crate::sync::conversation_sync_adapter::SyncConversationRoot =
            serde_json::from_slice(bytes).map_err(|_| ApplyOneError::Corrupt)?;
        crate::sync::conversation_sync_adapter::sync_replace_conversation_root(tx, &root)
            .map_err(conversation_apply_error)?;
        Ok(true)
    },
    ids: Some(|connection| {
        crate::sync::conversation_sync_adapter::sync_conversation_ids(connection)
            .map_err(|_| ApplyOneError::Storage)
    }),
    delete: Some(|tx, change, now| {
        let id = change.entity().id();
        crate::purge::queue_purge(tx, crate::purge::PurgeKind::Conversation, id, change, now)
            .map_err(|_| ApplyOneError::Storage)?;
        Ok(true)
    }),
};

fn memory_apply_error(error: lettuce_memory::MemoryRepositoryError) -> ApplyOneError {
    match error {
        lettuce_memory::MemoryRepositoryError::NotFound => ApplyOneError::Pending,
        lettuce_memory::MemoryRepositoryError::Failure(_) => ApplyOneError::Storage,
        _ => ApplyOneError::Corrupt,
    }
}

fn decode_memory_item(id: &str, bytes: &[u8]) -> Result<lettuce_memory::MemoryItem, ApplyOneError> {
    let item: lettuce_memory::MemoryItem =
        serde_json::from_slice(bytes).map_err(|_| ApplyOneError::Corrupt)?;
    match crate::sync::memory_sync_adapter::split_item_id(id) {
        Some((_, item_id)) if item_id == item.id => Ok(item),
        _ => Err(ApplyOneError::Corrupt),
    }
}

const MEMORY_ITEM_CODEC: SnapshotCodec = SnapshotCodec {
    kind: lettuce_sync::MEMORY_ITEM_SYNC_KIND,
    assets: no_assets,
    empty: None,
    seed: None,
    decode: |id, bytes| decode_memory_item(id, bytes).map(|_| ()),
    current: |tx, id| {
        crate::sync::memory_sync_adapter::sync_load_memory_item(tx, id)
            .map_err(memory_apply_error)?
            .map(|item| {
                json_payload(
                    lettuce_sync::MEMORY_ITEM_SYNC_SCHEMA,
                    lettuce_sync::MEMORY_ITEM_SYNC_VERSION,
                    &item,
                )
            })
            .transpose()
    },
    materialize: |tx, id, bytes| {
        let item = decode_memory_item(id, bytes)?;
        crate::sync::memory_sync_adapter::sync_put_memory_item(tx, id, &item)
            .map_err(memory_apply_error)
    },
    ids: Some(|connection| {
        crate::sync::memory_sync_adapter::sync_memory_item_ids(connection)
            .map_err(|_| ApplyOneError::Storage)
    }),
    delete: Some(|tx, change, _| {
        let id = change.entity().id();
        crate::sync::memory_sync_adapter::sync_delete_memory_item(tx, id)
            .map_err(memory_apply_error)
    }),
};

const MEMORY_CURSOR_CODEC: SnapshotCodec = SnapshotCodec {
    kind: lettuce_sync::MEMORY_CURSOR_SYNC_KIND,
    assets: no_assets,
    empty: None,
    seed: None,
    decode: |id, bytes| {
        parse_conversation(id)?;
        serde_json::from_slice::<u64>(bytes)
            .map(|_| ())
            .map_err(|_| ApplyOneError::Corrupt)
    },
    current: |tx, id| {
        crate::sync::memory_sync_adapter::sync_load_memory_cursor(tx, parse_conversation(id)?)
            .map_err(memory_apply_error)?
            .map(|cursor| {
                json_payload(
                    lettuce_sync::MEMORY_CURSOR_SYNC_SCHEMA,
                    lettuce_sync::MEMORY_CURSOR_SYNC_VERSION,
                    &cursor,
                )
            })
            .transpose()
    },
    materialize: |tx, id, bytes| {
        let cursor: u64 = serde_json::from_slice(bytes).map_err(|_| ApplyOneError::Corrupt)?;
        crate::sync::memory_sync_adapter::sync_store_memory_cursor(
            tx,
            parse_conversation(id)?,
            cursor,
        )
        .map_err(memory_apply_error)?;
        Ok(true)
    },
    ids: Some(|connection| {
        crate::sync::memory_sync_adapter::sync_memory_cursor_ids(connection)
            .map_err(|_| ApplyOneError::Storage)
    }),
    delete: None,
};

fn decode_memory_summary(
    id: &str,
    bytes: &[u8],
) -> Result<lettuce_memory::MemorySummary, ApplyOneError> {
    if !crate::sync::memory_sync_adapter::valid_owner(id) {
        return Err(ApplyOneError::Corrupt);
    }
    serde_json::from_slice(bytes).map_err(|_| ApplyOneError::Corrupt)
}

const MEMORY_SUMMARY_CODEC: SnapshotCodec = SnapshotCodec {
    kind: lettuce_sync::MEMORY_SUMMARY_SYNC_KIND,
    assets: no_assets,
    empty: None,
    seed: None,
    decode: |id, bytes| decode_memory_summary(id, bytes).map(|_| ()),
    current: |tx, id| {
        crate::sync::memory_sync_adapter::sync_load_memory_summary(tx, id)
            .map_err(memory_apply_error)?
            .map(|summary| {
                json_payload(
                    lettuce_sync::MEMORY_SUMMARY_SYNC_SCHEMA,
                    lettuce_sync::MEMORY_SUMMARY_SYNC_VERSION,
                    &summary,
                )
            })
            .transpose()
    },
    materialize: |tx, id, bytes| {
        let summary = decode_memory_summary(id, bytes)?;
        crate::sync::memory_sync_adapter::sync_replace_memory_summary(tx, id, &summary)
            .map_err(memory_apply_error)?;
        Ok(true)
    },
    ids: Some(|connection| {
        crate::sync::memory_sync_adapter::sync_memory_summary_owners(connection)
            .map_err(|_| ApplyOneError::Storage)
    }),
    delete: Some(|tx, change, _| {
        let id = change.entity().id();
        crate::sync::memory_sync_adapter::sync_delete_memory_summary(tx, id)
            .map_err(memory_apply_error)
    }),
};

fn json_payload<T: serde::Serialize>(
    schema: &str,
    version: u32,
    value: &T,
) -> Result<CanonicalPayload, ApplyOneError> {
    CanonicalPayload::new(
        schema,
        version,
        serde_json::to_vec(value).map_err(|_| ApplyOneError::Corrupt)?,
    )
    .map_err(payload_error)
}

fn parse_character(id: &str) -> Result<lettuce_types::CharacterId, ApplyOneError> {
    id.parse().map_err(|_| ApplyOneError::Corrupt)
}

/// A Soul sync id: the character id for its shared Soul, or
/// `character:conversation` for a conversation's own Soul.
fn parse_soul_owner(id: &str) -> Result<lettuce_companions::SoulOwner, ApplyOneError> {
    match id.split_once(':') {
        None => Ok(lettuce_companions::SoulOwner::Character(parse_character(
            id,
        )?)),
        Some((character, conversation)) => Ok(lettuce_companions::SoulOwner::Conversation {
            character_id: parse_character(character)?,
            conversation_id: conversation.parse().map_err(|_| ApplyOneError::Corrupt)?,
        }),
    }
}

const COMPANION_SOUL_CODEC: SnapshotCodec = SnapshotCodec {
    kind: lettuce_sync::COMPANION_SOUL_SYNC_KIND,
    assets: no_assets,
    empty: None,
    seed: Some(|bytes| {
        serde_json::from_slice::<Vec<lettuce_companions::SoulFact>>(bytes).is_ok_and(|facts| {
            facts.iter().all(|fact| {
                fact.kind == lettuce_companions::SoulFactKind::Authored
                    && fact.superseded_by.is_none()
            })
        })
    }),
    decode: |id, bytes| {
        parse_soul_owner(id)?;
        serde_json::from_slice::<Vec<lettuce_companions::SoulFact>>(bytes)
            .map(|_| ())
            .map_err(|_| ApplyOneError::Corrupt)
    },
    current: |tx, id| {
        crate::sync::companion_sync_adapter::sync_load_soul(tx, parse_soul_owner(id)?)
            .map_err(conversation_apply_error)?
            .map(|facts| {
                json_payload(
                    lettuce_sync::COMPANION_SOUL_SYNC_SCHEMA,
                    lettuce_sync::COMPANION_SOUL_SYNC_VERSION,
                    &facts,
                )
            })
            .transpose()
    },
    materialize: |tx, id, bytes| {
        let facts: Vec<lettuce_companions::SoulFact> =
            serde_json::from_slice(bytes).map_err(|_| ApplyOneError::Corrupt)?;
        crate::sync::companion_sync_adapter::sync_replace_soul(tx, parse_soul_owner(id)?, &facts)
            .map_err(conversation_apply_error)?;
        Ok(true)
    },
    ids: Some(|connection| {
        crate::sync::companion_sync_adapter::sync_soul_ids(connection)
            .map_err(|_| ApplyOneError::Storage)
    }),
    delete: None,
};

fn decode_relationship(
    id: &str,
    bytes: &[u8],
) -> Result<crate::sync::companion_sync_adapter::SyncCompanionRelationship, ApplyOneError> {
    let relationship: crate::sync::companion_sync_adapter::SyncCompanionRelationship =
        serde_json::from_slice(bytes).map_err(|_| ApplyOneError::Corrupt)?;
    if crate::sync::companion_sync_adapter::relationship_id(&relationship) != id {
        return Err(ApplyOneError::Corrupt);
    }
    Ok(relationship)
}

const COMPANION_RELATIONSHIP_CODEC: SnapshotCodec = SnapshotCodec {
    kind: lettuce_sync::COMPANION_RELATIONSHIP_SYNC_KIND,
    assets: no_assets,
    empty: None,
    seed: Some(|bytes| {
        serde_json::from_slice::<crate::sync::companion_sync_adapter::SyncCompanionRelationship>(
            bytes,
        )
        .is_ok_and(|relationship| relationship.state.interaction_count == 0)
    }),
    decode: |id, bytes| decode_relationship(id, bytes).map(|_| ()),
    current: |tx, id| {
        crate::sync::companion_sync_adapter::sync_load_relationship(tx, id)
            .map_err(conversation_apply_error)?
            .map(|relationship| {
                json_payload(
                    lettuce_sync::COMPANION_RELATIONSHIP_SYNC_SCHEMA,
                    lettuce_sync::COMPANION_RELATIONSHIP_SYNC_VERSION,
                    &relationship,
                )
            })
            .transpose()
    },
    materialize: |tx, id, bytes| {
        let relationship = decode_relationship(id, bytes)?;
        crate::sync::companion_sync_adapter::sync_replace_relationship(tx, &relationship)
            .map_err(conversation_apply_error)?;
        Ok(true)
    },
    ids: Some(|connection| {
        crate::sync::companion_sync_adapter::sync_relationship_ids(connection)
            .map_err(|_| ApplyOneError::Storage)
    }),
    delete: None,
};

fn parse_conversation(id: &str) -> Result<lettuce_types::ConversationId, ApplyOneError> {
    id.parse().map_err(|_| ApplyOneError::Corrupt)
}

const COMPANION_SESSION_CODEC: SnapshotCodec = SnapshotCodec {
    kind: lettuce_sync::COMPANION_SESSION_SYNC_KIND,
    assets: no_assets,
    empty: None,
    seed: None,
    decode: |id, bytes| {
        parse_conversation(id)?;
        serde_json::from_slice::<crate::sync::companion_sync_adapter::SyncCompanionSession>(bytes)
            .map(|_| ())
            .map_err(|_| ApplyOneError::Corrupt)
    },
    current: |tx, id| {
        crate::sync::companion_sync_adapter::sync_load_session(tx, parse_conversation(id)?)
            .map_err(conversation_apply_error)?
            .map(|session| {
                json_payload(
                    lettuce_sync::COMPANION_SESSION_SYNC_SCHEMA,
                    lettuce_sync::COMPANION_SESSION_SYNC_VERSION,
                    &session,
                )
            })
            .transpose()
    },
    materialize: |tx, id, bytes| {
        let session: crate::sync::companion_sync_adapter::SyncCompanionSession =
            serde_json::from_slice(bytes).map_err(|_| ApplyOneError::Corrupt)?;
        crate::sync::companion_sync_adapter::sync_replace_session(
            tx,
            parse_conversation(id)?,
            &session,
        )
        .map_err(conversation_apply_error)?;
        Ok(true)
    },
    ids: Some(|connection| {
        crate::sync::companion_sync_adapter::sync_session_ids(connection)
            .map_err(|_| ApplyOneError::Storage)
    }),
    delete: None,
};

fn decode_note(
    id: &str,
    bytes: &[u8],
) -> Result<lettuce_companions::CompanionScheduledNote, ApplyOneError> {
    let note: lettuce_companions::CompanionScheduledNote =
        serde_json::from_slice(bytes).map_err(|_| ApplyOneError::Corrupt)?;
    if note.id.to_string() != id {
        return Err(ApplyOneError::Corrupt);
    }
    Ok(note)
}

const COMPANION_NOTE_CODEC: SnapshotCodec = SnapshotCodec {
    kind: lettuce_sync::COMPANION_NOTE_SYNC_KIND,
    assets: no_assets,
    empty: None,
    seed: None,
    decode: |id, bytes| decode_note(id, bytes).map(|_| ()),
    current: |tx, id| {
        let id = Uuid::parse_str(id).map_err(|_| ApplyOneError::Corrupt)?;
        crate::sync::companion_sync_adapter::sync_load_note(tx, id)
            .map_err(conversation_apply_error)?
            .map(|note| {
                json_payload(
                    lettuce_sync::COMPANION_NOTE_SYNC_SCHEMA,
                    lettuce_sync::COMPANION_NOTE_SYNC_VERSION,
                    &note,
                )
            })
            .transpose()
    },
    materialize: |tx, id, bytes| {
        let note = decode_note(id, bytes)?;
        crate::sync::companion_sync_adapter::sync_replace_note(tx, &note)
            .map_err(conversation_apply_error)?;
        Ok(true)
    },
    ids: Some(|connection| {
        crate::sync::companion_sync_adapter::sync_note_ids(connection)
            .map_err(|_| ApplyOneError::Storage)
    }),
    delete: Some(|tx, change, _| {
        let id = change.entity().id();
        crate::sync::companion_sync_adapter::sync_delete_note(tx, id)
            .map_err(conversation_apply_error)
    }),
};

fn row_apply_error(error: crate::sync::row_sync_adapter::RowSyncError) -> ApplyOneError {
    match error {
        crate::sync::row_sync_adapter::RowSyncError::Pending => ApplyOneError::Pending,
        crate::sync::row_sync_adapter::RowSyncError::Corrupt => ApplyOneError::Corrupt,
        crate::sync::row_sync_adapter::RowSyncError::Storage => ApplyOneError::Storage,
    }
}

macro_rules! row_codec {
    ($name:ident, $kind:expr, $schema:expr, $version:expr, $spec:expr, $assets:expr) => {
        row_codec!(
            $name,
            $kind,
            $schema,
            $version,
            $spec,
            $assets,
            Some(|tx, change, _| {
                crate::sync::row_sync_adapter::row_delete(tx, &$spec, change.entity().id())
                    .map_err(row_apply_error)
            })
        );
    };
    ($name:ident, $kind:expr, $schema:expr, $version:expr, $spec:expr, $assets:expr, $delete:expr) => {
        const $name: SnapshotCodec = SnapshotCodec {
            kind: $kind,
            assets: $assets,
            empty: None,
            seed: None,
            decode: |_, bytes| {
                crate::sync::row_sync_adapter::row_decode(&$spec, bytes)
                    .map(|_| ())
                    .map_err(row_apply_error)
            },
            current: |tx, id| {
                crate::sync::row_sync_adapter::row_current(tx, &$spec, id)
                    .map_err(row_apply_error)?
                    .map(|bytes| {
                        CanonicalPayload::new($schema, $version, bytes).map_err(payload_error)
                    })
                    .transpose()
            },
            materialize: |tx, id, bytes| {
                crate::sync::row_sync_adapter::row_materialize(tx, &$spec, id, bytes)
                    .map_err(row_apply_error)
            },
            ids: Some(|connection| {
                crate::sync::row_sync_adapter::row_ids(connection, &$spec)
                    .map_err(|_| ApplyOneError::Storage)
            }),
            delete: $delete,
        };
    };
}

row_codec!(
    AUDIO_PROVIDER_CODEC,
    lettuce_sync::AUDIO_PROVIDER_SYNC_KIND,
    lettuce_sync::AUDIO_PROVIDER_SYNC_SCHEMA,
    lettuce_sync::AUDIO_PROVIDER_SYNC_VERSION,
    crate::sync::row_sync_adapter::AUDIO_PROVIDERS,
    no_assets
);

row_codec!(
    USER_VOICE_CODEC,
    lettuce_sync::USER_VOICE_SYNC_KIND,
    lettuce_sync::USER_VOICE_SYNC_SCHEMA,
    lettuce_sync::USER_VOICE_SYNC_VERSION,
    crate::sync::row_sync_adapter::USER_VOICES,
    no_assets
);

row_codec!(
    ASR_VOCABULARY_TERM_CODEC,
    lettuce_sync::ASR_VOCABULARY_TERM_SYNC_KIND,
    lettuce_sync::ASR_VOCABULARY_TERM_SYNC_SCHEMA,
    lettuce_sync::ASR_VOCABULARY_TERM_SYNC_VERSION,
    crate::sync::row_sync_adapter::ASR_VOCABULARY_TERMS,
    no_assets
);

row_codec!(
    ASR_CORRECTION_CODEC,
    lettuce_sync::ASR_CORRECTION_SYNC_KIND,
    lettuce_sync::ASR_CORRECTION_SYNC_SCHEMA,
    lettuce_sync::ASR_CORRECTION_SYNC_VERSION,
    crate::sync::row_sync_adapter::ASR_CORRECTIONS,
    no_assets
);

row_codec!(
    ASR_IGNORED_SUGGESTION_CODEC,
    lettuce_sync::ASR_IGNORED_SUGGESTION_SYNC_KIND,
    lettuce_sync::ASR_IGNORED_SUGGESTION_SYNC_SCHEMA,
    lettuce_sync::ASR_IGNORED_SUGGESTION_SYNC_VERSION,
    crate::sync::row_sync_adapter::ASR_IGNORED_SUGGESTIONS,
    no_assets
);

row_codec!(
    ASR_VOICE_EXAMPLE_CODEC,
    lettuce_sync::ASR_VOICE_EXAMPLE_SYNC_KIND,
    lettuce_sync::ASR_VOICE_EXAMPLE_SYNC_SCHEMA,
    lettuce_sync::ASR_VOICE_EXAMPLE_SYNC_VERSION,
    crate::sync::row_sync_adapter::ASR_VOICE_EXAMPLES,
    |bytes| {
        serde_json::from_slice::<serde_json::Map<String, serde_json::Value>>(bytes)
            .ok()
            .and_then(|row| row.get("audio_asset_id")?.as_str().map(str::to_owned))
            .into_iter()
            .collect()
    }
);

row_codec!(
    PLAYGROUND_HISTORY_CODEC,
    lettuce_sync::PLAYGROUND_HISTORY_SYNC_KIND,
    lettuce_sync::PLAYGROUND_HISTORY_SYNC_SCHEMA,
    lettuce_sync::PLAYGROUND_HISTORY_SYNC_VERSION,
    crate::sync::row_sync_adapter::PLAYGROUND_HISTORY,
    no_assets
);

row_codec!(
    PLAYGROUND_IMAGE_CODEC,
    lettuce_sync::PLAYGROUND_IMAGE_SYNC_KIND,
    lettuce_sync::PLAYGROUND_IMAGE_SYNC_SCHEMA,
    lettuce_sync::PLAYGROUND_IMAGE_SYNC_VERSION,
    crate::sync::row_sync_adapter::PLAYGROUND_HISTORY_IMAGES,
    |bytes| {
        serde_json::from_slice::<serde_json::Map<String, serde_json::Value>>(bytes)
            .ok()
            .and_then(|row| row.get("asset_id")?.as_str().map(str::to_owned))
            .into_iter()
            .collect()
    }
);

fn creation_apply_error(error: lettuce_creation::CreationRepositoryError) -> ApplyOneError {
    match error {
        lettuce_creation::CreationRepositoryError::Storage => ApplyOneError::Storage,
        _ => ApplyOneError::Corrupt,
    }
}

fn decode_creation_workflow(
    id: &str,
    bytes: &[u8],
) -> Result<crate::catalog::creation_adapter::SyncCreationWorkflow, ApplyOneError> {
    let workflow: crate::catalog::creation_adapter::SyncCreationWorkflow =
        serde_json::from_slice(bytes).map_err(|_| ApplyOneError::Corrupt)?;
    if workflow.workflow.id.to_string() != id {
        return Err(ApplyOneError::Corrupt);
    }
    Ok(workflow)
}

const CREATION_WORKFLOW_CODEC: SnapshotCodec = SnapshotCodec {
    kind: lettuce_sync::CREATION_WORKFLOW_SYNC_KIND,
    empty: None,
    seed: None,
    assets: no_assets,
    decode: |id, bytes| decode_creation_workflow(id, bytes).map(|_| ()),
    current: |tx, id| {
        crate::catalog::creation_adapter::sync_load_workflow(
            tx,
            id.parse().map_err(|_| ApplyOneError::Corrupt)?,
        )
        .map_err(creation_apply_error)?
        .map(|workflow| {
            json_payload(
                lettuce_sync::CREATION_WORKFLOW_SYNC_SCHEMA,
                lettuce_sync::CREATION_WORKFLOW_SYNC_VERSION,
                &workflow,
            )
        })
        .transpose()
    },
    materialize: |tx, id, bytes| {
        crate::catalog::creation_adapter::sync_merge_workflow(
            tx,
            &decode_creation_workflow(id, bytes)?,
        )
        .map_err(creation_apply_error)
    },
    ids: Some(|connection| {
        crate::catalog::creation_adapter::sync_workflow_ids(connection)
            .map_err(|_| ApplyOneError::Storage)
    }),
    delete: None,
};

row_codec!(
    USAGE_COST_CODEC,
    lettuce_sync::USAGE_COST_SYNC_KIND,
    lettuce_sync::USAGE_COST_SYNC_SCHEMA,
    lettuce_sync::USAGE_COST_SYNC_VERSION,
    crate::sync::row_sync_adapter::USAGE_COSTS,
    no_assets,
    None
);

row_codec!(
    JOB_USAGE_CODEC,
    lettuce_sync::JOB_USAGE_SYNC_KIND,
    lettuce_sync::JOB_USAGE_SYNC_SCHEMA,
    lettuce_sync::JOB_USAGE_SYNC_VERSION,
    crate::sync::row_sync_adapter::JOB_INFERENCE_USAGE,
    no_assets,
    None
);

row_codec!(
    JOB_USAGE_COST_CODEC,
    lettuce_sync::JOB_USAGE_COST_SYNC_KIND,
    lettuce_sync::JOB_USAGE_COST_SYNC_SCHEMA,
    lettuce_sync::JOB_USAGE_COST_SYNC_VERSION,
    crate::sync::row_sync_adapter::JOB_USAGE_COSTS,
    no_assets,
    None
);

row_codec!(
    LEGACY_USAGE_CODEC,
    lettuce_sync::LEGACY_USAGE_SYNC_KIND,
    lettuce_sync::LEGACY_USAGE_SYNC_SCHEMA,
    lettuce_sync::LEGACY_USAGE_SYNC_VERSION,
    crate::sync::row_sync_adapter::LEGACY_USAGE_RECORDS,
    no_assets,
    None
);

fn decode_branch(
    id: &str,
    bytes: &[u8],
) -> Result<lettuce_conversations::ConversationBranch, ApplyOneError> {
    let branch: lettuce_conversations::ConversationBranch =
        serde_json::from_slice(bytes).map_err(|_| ApplyOneError::Corrupt)?;
    if crate::sync::conversation_sync_adapter::sync_branch_id(id)
        != Some((branch.conversation_id, branch.id))
        || branch.parent_branch_id.is_none()
    {
        return Err(ApplyOneError::Corrupt);
    }
    Ok(branch)
}

const CONVERSATION_BRANCH_CODEC: SnapshotCodec = SnapshotCodec {
    kind: lettuce_sync::CONVERSATION_BRANCH_SYNC_KIND,
    assets: no_assets,
    empty: None,
    seed: None,
    decode: |id, bytes| decode_branch(id, bytes).map(|_| ()),
    current: |tx, id| {
        let (conversation_id, branch_id) =
            crate::sync::conversation_sync_adapter::sync_branch_id(id)
                .ok_or(ApplyOneError::Corrupt)?;
        crate::sync::conversation_sync_adapter::sync_load_branch(tx, conversation_id, branch_id)
            .map_err(conversation_apply_error)?
            .map(|branch| {
                CanonicalPayload::new(
                    lettuce_sync::CONVERSATION_BRANCH_SYNC_SCHEMA,
                    lettuce_sync::CONVERSATION_BRANCH_SYNC_VERSION,
                    serde_json::to_vec(&branch).map_err(|_| ApplyOneError::Corrupt)?,
                )
                .map_err(payload_error)
            })
            .transpose()
    },
    materialize: |tx, id, bytes| {
        let branch = decode_branch(id, bytes)?;
        crate::sync::conversation_sync_adapter::sync_insert_branch(tx, &branch)
            .map_err(conversation_apply_error)?;
        Ok(true)
    },
    ids: Some(|connection| {
        crate::sync::conversation_sync_adapter::sync_branch_ids(connection)
            .map_err(|_| ApplyOneError::Storage)
    }),
    delete: None,
};

fn decode_message(
    id: &str,
    bytes: &[u8],
) -> Result<crate::sync::conversation_sync_adapter::SyncConversationMessage, ApplyOneError> {
    let message: crate::sync::conversation_sync_adapter::SyncConversationMessage =
        serde_json::from_slice(bytes).map_err(|_| ApplyOneError::Corrupt)?;
    let inner = &message.message.message;
    if crate::sync::conversation_sync_adapter::sync_message_id(id)
        != Some((inner.conversation_id, inner.id))
    {
        return Err(ApplyOneError::Corrupt);
    }
    Ok(message)
}

const CONVERSATION_MESSAGE_CODEC: SnapshotCodec = SnapshotCodec {
    kind: lettuce_sync::CONVERSATION_MESSAGE_SYNC_KIND,
    assets: |bytes| {
        let Ok(message) = serde_json::from_slice::<
            crate::sync::conversation_sync_adapter::SyncConversationMessage,
        >(bytes) else {
            return Vec::new();
        };
        let backup = &message.message;
        backup
            .revisions
            .iter()
            .map(|revision| &revision.parts)
            .chain(backup.candidates.iter().map(|candidate| &candidate.parts))
            .filter_map(|parts| {
                crate::conversation::conversation_history_writer::media_parts(parts).ok()
            })
            .flatten()
            .map(|(_, asset, _)| asset)
            .collect()
    },
    empty: None,
    seed: None,
    decode: |id, bytes| decode_message(id, bytes).map(|_| ()),
    current: |tx, id| {
        let (conversation_id, message_id) =
            crate::sync::conversation_sync_adapter::sync_message_id(id)
                .ok_or(ApplyOneError::Corrupt)?;
        crate::sync::conversation_sync_adapter::sync_load_conversation_message(
            tx,
            conversation_id,
            message_id,
        )
        .map_err(conversation_apply_error)?
        .map(|message| {
            CanonicalPayload::new(
                lettuce_sync::CONVERSATION_MESSAGE_SYNC_SCHEMA,
                lettuce_sync::CONVERSATION_MESSAGE_SYNC_VERSION,
                serde_json::to_vec(&message).map_err(|_| ApplyOneError::Corrupt)?,
            )
            .map_err(payload_error)
        })
        .transpose()
    },
    materialize: |tx, id, bytes| {
        let message = decode_message(id, bytes)?;
        crate::sync::conversation_sync_adapter::sync_merge_conversation_message(tx, &message)
            .map_err(conversation_apply_error)?;
        Ok(true)
    },
    ids: Some(|connection| {
        crate::sync::conversation_sync_adapter::sync_message_ids(connection)
            .map_err(|_| ApplyOneError::Storage)
    }),
    delete: None,
};

/// Aggregates journaled by comparing their current state with the latest
/// journaled snapshot, in dependency order (deletes run in reverse).
const SCANNED_CODECS: [&SnapshotCodec; 35] = [
    &PROVIDER_ACCOUNT_CODEC,
    &MODEL_PROFILE_CODEC,
    &PERSONA_CODEC,
    &PERSONA_DEFAULT_CODEC,
    &PROMPT_CODEC,
    &APP_SETTINGS_CODEC,
    &CHARACTER_CODEC,
    &GROUP_CODEC,
    &LOREBOOK_CODEC,
    &CHARACTER_BINDINGS_CODEC,
    &PERSONA_BINDINGS_CODEC,
    &GROUP_BINDINGS_CODEC,
    &CONVERSATION_CODEC,
    &CONVERSATION_BRANCH_CODEC,
    &CONVERSATION_MESSAGE_CODEC,
    &MEMORY_ITEM_CODEC,
    &MEMORY_SUMMARY_CODEC,
    &MEMORY_CURSOR_CODEC,
    &COMPANION_SOUL_CODEC,
    &COMPANION_NOTE_CODEC,
    &COMPANION_RELATIONSHIP_CODEC,
    &COMPANION_SESSION_CODEC,
    &AUDIO_PROVIDER_CODEC,
    &USER_VOICE_CODEC,
    &ASR_VOCABULARY_TERM_CODEC,
    &ASR_CORRECTION_CODEC,
    &ASR_IGNORED_SUGGESTION_CODEC,
    &ASR_VOICE_EXAMPLE_CODEC,
    &PLAYGROUND_HISTORY_CODEC,
    &PLAYGROUND_IMAGE_CODEC,
    &CREATION_WORKFLOW_CODEC,
    &USAGE_COST_CODEC,
    &JOB_USAGE_CODEC,
    &JOB_USAGE_COST_CODEC,
    &LEGACY_USAGE_CODEC,
];

fn snapshot_codec(kind: &str) -> Option<&'static SnapshotCodec> {
    match kind {
        "persona" => Some(&PERSONA_CODEC),
        "persona_default" => Some(&PERSONA_DEFAULT_CODEC),
        _ => SCANNED_CODECS.into_iter().find(|codec| codec.kind == kind),
    }
}

/// The latest snapshot hash journaled for each entity of a kind (`None` once
/// deleted), skipping incoming changes that lost a conflict and so never
/// became the local state.
fn latest_journaled(
    connection: &Connection,
    kind: &str,
) -> Result<BTreeMap<String, Option<ContentHash>>, LocalChangeJournalError> {
    let mut statement = connection
        .prepare(
            "SELECT change.entity_id, change.payload_hash FROM sync_changes change
             WHERE change.entity_kind = ?1
               AND NOT EXISTS (
                 SELECT 1 FROM sync_conflicts conflict
                 WHERE conflict.incoming_change_id = change.change_id
                   AND conflict.winning_side = 'current'
               )
               AND NOT EXISTS (
                 SELECT 1 FROM sync_deferred_changes deferred
                 WHERE deferred.change_id = change.change_id
               )
             ORDER BY change.rowid",
        )
        .map_err(storage)?;
    let rows = statement
        .query_map([kind], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        })
        .map_err(storage)?;
    let mut latest = BTreeMap::new();
    for row in rows {
        let (id, hash) = row.map_err(storage)?;
        latest.insert(
            id,
            hash.map(ContentHash::parse).transpose().map_err(corrupt)?,
        );
    }
    Ok(latest)
}

/// Journals every launch snapshot artifact a conversation references that
/// was never journaled (immutable, insert only). An artifact whose encoded
/// payload exceeds the canonical payload limit is not journaled.
fn journal_referenced_snapshots(
    tx: &Transaction<'_>,
    now: TimestampMillis,
) -> Result<usize, LocalChangeJournalError> {
    let ids = crate::conversation::conversation_artifact_adapter::sync_snapshot_ids(tx)
        .map_err(storage)?;
    let mut journaled = 0;
    for id in ids {
        let exists: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sync_changes WHERE entity_kind = ?1 AND entity_id = ?2)",
                params![lettuce_sync::CONVERSATION_SNAPSHOT_SYNC_KIND, id],
                |row| row.get(0),
            )
            .map_err(storage)?;
        if exists {
            continue;
        }
        let Some(artifact) =
            crate::conversation::conversation_artifact_adapter::sync_load_snapshot(tx, &id)
                .map_err(storage)?
        else {
            continue;
        };
        let bytes = serde_json::to_vec(&artifact).map_err(corrupt)?;
        let Ok(payload) = CanonicalPayload::new(
            lettuce_sync::CONVERSATION_SNAPSHOT_SYNC_SCHEMA,
            lettuce_sync::CONVERSATION_SNAPSHOT_SYNC_VERSION,
            bytes,
        ) else {
            record_not_synced(tx, lettuce_sync::CONVERSATION_SNAPSHOT_SYNC_KIND, &id, now)
                .map_err(journal_apply_error)?;
            continue;
        };
        journal_state_change(
            tx,
            lettuce_sync::CONVERSATION_SNAPSHOT_SYNC_KIND,
            &id,
            ChangeOperation::Insert,
            None,
            Some(payload),
            now,
        )?;
        journaled += 1;
    }
    Ok(journaled)
}

pub(crate) fn media_asset_journaled(
    connection: &Connection,
    id: &str,
) -> Result<bool, LocalChangeJournalError> {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sync_changes
             WHERE entity_kind = 'media_asset' AND entity_id = ?1)",
            [id],
            |row| row.get(0),
        )
        .map_err(storage)
}

/// Journals every ready media asset a synced aggregate references and that
/// was never journaled. Assets are immutable in sync (insert only): later
/// local metadata changes such as retention stay device-local.
fn journal_referenced_media(
    tx: &Transaction<'_>,
    now: TimestampMillis,
) -> Result<usize, LocalChangeJournalError> {
    let ids = tx
        .prepare(
            "SELECT asset_id FROM persona_media
             UNION SELECT asset_id FROM character_media
             UNION SELECT asset_id FROM character_presentation_asset_refs
             UNION SELECT asset_id FROM scene_assets
             UNION SELECT icon_asset_id FROM lorebooks WHERE icon_asset_id IS NOT NULL
             UNION SELECT asset_id FROM group_presentation_asset_refs
             UNION SELECT background_asset_id FROM groups WHERE background_asset_id IS NOT NULL
             UNION SELECT asset_id FROM group_scene_assets
             UNION SELECT asset_id FROM revision_media_refs
             UNION SELECT asset_id FROM candidate_media_refs
             UNION SELECT background_asset_id FROM conversation_settings WHERE background_asset_id IS NOT NULL
             UNION SELECT audio_asset_id FROM asr_voice_examples
             UNION SELECT asset_id FROM playground_history_images WHERE asset_id IS NOT NULL
             ORDER BY asset_id",
        )
        .and_then(|mut statement| {
            statement
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .map_err(storage)?;
    let mut journaled = 0;
    for id in ids {
        if media_asset_journaled(tx, &id)? {
            continue;
        }
        let asset_id = id.parse::<lettuce_types::AssetId>().map_err(corrupt)?;
        let Some(asset) = crate::load_asset_with_blob(tx, asset_id).map_err(storage)? else {
            continue;
        };
        let blob = tx
            .query_row(
                &format!(
                    "SELECT {} FROM media_blobs WHERE id=?1",
                    crate::MEDIA_BLOB_COLUMNS
                ),
                [asset.blob_id.to_string()],
                crate::media_from_row,
            )
            .map_err(storage)?;
        if blob.state != lettuce_media::BlobState::Ready {
            continue;
        }
        let request =
            lettuce_sync::media_asset_insert_change(&lettuce_media::SyncMediaAsset { asset, blob })
                .map_err(corrupt)?;
        record_local_change_in(
            tx,
            lettuce_sync::media_asset_create_operation(asset_id),
            &request,
            now,
        )?;
        journaled += 1;
    }
    Ok(journaled)
}

fn journal_state_change(
    tx: &Transaction<'_>,
    kind: &str,
    id: &str,
    operation: ChangeOperation,
    base: Option<ContentHash>,
    payload: Option<CanonicalPayload>,
    now: TimestampMillis,
) -> Result<(), LocalChangeJournalError> {
    let source_time = payload
        .as_ref()
        .and_then(|payload| snapshot_source_time(kind, payload.bytes()));
    let request = NewCanonicalChange::new(
        SyncEntity::new(kind, id).map_err(corrupt)?,
        operation,
        base,
        payload,
    )
    .map_err(corrupt)?;
    record_local_change_in_skipping(tx, OperationId::new(), &request, now, None, source_time)?;
    Ok(())
}

/// When a snapshot's content last changed: for a memory item the latest of
/// its creation, last access and supersession; for a Soul the latest time a
/// fact became valid, was created or was superseded; for a relationship its
/// last interaction; for any other kind the latest `updated_at` (or
/// `updatedAt`) it records at any depth. A scanned insert or update is
/// stamped with it instead of the session time, so against a peer's version
/// of the same entity the more recently changed content wins rather than
/// whichever device scanned last. A stamp never falls behind a change this
/// device already journaled for the entity: it then takes the next counter
/// after that change, so an edit always supersedes what it was based on.
fn snapshot_source_time(kind: &str, bytes: &[u8]) -> Option<TimestampMillis> {
    let newest = |times: &mut dyn Iterator<Item = TimestampMillis>| {
        times.filter(|time| time.get() > 0).max()
    };
    if kind == lettuce_sync::MEMORY_ITEM_SYNC_KIND {
        let item = serde_json::from_slice::<lettuce_memory::MemoryItem>(bytes).ok()?;
        return newest(
            &mut [
                Some(item.created_at),
                Some(item.last_accessed_at),
                item.superseded_at,
            ]
            .into_iter()
            .flatten(),
        );
    }
    if kind == lettuce_sync::COMPANION_SOUL_SYNC_KIND {
        let facts = serde_json::from_slice::<Vec<lettuce_companions::SoulFact>>(bytes).ok()?;
        return newest(&mut facts.iter().flat_map(|fact| {
            [
                Some(fact.valid_from),
                Some(fact.created_at),
                fact.superseded_at,
            ]
            .into_iter()
            .flatten()
        }));
    }
    if kind == lettuce_sync::COMPANION_RELATIONSHIP_SYNC_KIND {
        let relationship = serde_json::from_slice::<
            crate::sync::companion_sync_adapter::SyncCompanionRelationship,
        >(bytes)
        .ok()?;
        return newest(&mut std::iter::once(relationship.state.last_interaction_at));
    }
    fn latest(value: &serde_json::Value) -> Option<i64> {
        match value {
            serde_json::Value::Object(map) => map
                .iter()
                .filter_map(|(key, value)| {
                    if key == "updated_at" || key == "updatedAt" {
                        value.as_i64().filter(|time| *time > 0)
                    } else {
                        latest(value)
                    }
                })
                .max(),
            serde_json::Value::Array(items) => items.iter().filter_map(latest).max(),
            _ => None,
        }
    }
    serde_json::from_slice::<serde_json::Value>(bytes)
        .ok()
        .as_ref()
        .and_then(latest)
        .map(TimestampMillis::new)
}

fn journal_apply_error(error: ApplyOneError) -> LocalChangeJournalError {
    match error {
        ApplyOneError::Storage => LocalChangeJournalError::Storage,
        ApplyOneError::Pending | ApplyOneError::Corrupt | ApplyOneError::Unencodable => {
            LocalChangeJournalError::Corrupt
        }
    }
}

/// A delete of a complete-snapshot entity. Concurrent with a local edit the
/// delete still wins (legacy) and the discarded edit is kept as conflict
/// evidence. A delete this device must refuse (the entity is still
/// referenced here) is journaled and nothing else happens now: the next state
/// scan sees the entity present after a journaled delete and journals a fresh
/// insert, in dependency order, so every device converges on keeping it.
fn settle_snapshot_delete(
    tx: &Transaction<'_>,
    change: &CanonicalChange,
    now: TimestampMillis,
    codec: &SnapshotCodec,
) -> Result<bool, ApplyOneError> {
    let delete = codec.delete.ok_or(ApplyOneError::Corrupt)?;
    let current_payload = (codec.current)(tx, change.entity().id())?;
    let current_change = current_payload
        .as_ref()
        .map(|value| load_materialized_change(tx, change.entity(), value.content_hash()))
        .transpose()
        .map_err(|error| match error {
            IncomingChangeError::Storage => ApplyOneError::Storage,
            _ => ApplyOneError::Corrupt,
        })?
        .flatten();
    let Some(current_payload) = current_payload else {
        resolve_dominated_conflicts(tx, change, now, None)?;
        return Ok(false);
    };
    let concurrent = change.base_revision() != Some(current_payload.content_hash());
    if let Some(kind) = crate::purge::PurgeKind::from_sync_kind(codec.kind)
        && keep_for_unseen_changes(tx, change, kind, now)?
    {
        return Ok(false);
    }
    if !delete(tx, change, now)? {
        return Ok(false);
    }
    if concurrent {
        insert_conflict(
            tx,
            change,
            current_change.as_ref(),
            Some(current_payload.bytes()),
            &[],
            true,
            now,
        )?;
    }
    resolve_dominated_conflicts(tx, change, now, None)?;
    Ok(concurrent)
}

/// The conversations a received delete of `id` takes with it.
fn purged_conversations(
    tx: &Transaction<'_>,
    kind: crate::purge::PurgeKind,
    id: &str,
) -> Result<Vec<String>, ApplyOneError> {
    match kind {
        crate::purge::PurgeKind::Conversation => Ok(vec![id.to_owned()]),
        crate::purge::PurgeKind::Character => tx
            .prepare(
                "SELECT DISTINCT conversation.id FROM conversations conversation
                 JOIN conversation_participants participant
                   ON participant.conversation_id = conversation.id
                 WHERE conversation.kind = 'direct' AND participant.source_kind = 'character'
                   AND participant.source_id = ?1
                 ORDER BY conversation.id",
            )
            .and_then(|mut statement| statement.query_map([id], |row| row.get(0))?.collect())
            .map_err(|_| ApplyOneError::Storage),
    }
}

/// Whether a synced entity id belongs to one of `owners` (a conversation or
/// character id): the owner itself, `<owner>:<part>` entities (messages,
/// branches, relationships, conversation Souls, bindings) and the memory of
/// `conversation:<owner>` or `pool:<owner>`.
fn owned_entity(owners: &[String], id: &str) -> bool {
    owners.iter().any(|owner| {
        id == owner
            || id
                .strip_prefix(owner.as_str())
                .is_some_and(|rest| rest.starts_with(':'))
            || id.starts_with(&format!("conversation:{owner}"))
            || id.starts_with(&format!("pool:{owner}"))
    })
}

/// What a received delete of a conversation or character takes with it.
struct PurgeScope {
    kind: crate::purge::PurgeKind,
    id: String,
    owners: Vec<String>,
    conversations: Vec<String>,
    notes: Vec<String>,
}

impl PurgeScope {
    fn load(
        tx: &Transaction<'_>,
        kind: crate::purge::PurgeKind,
        id: &str,
    ) -> Result<Self, ApplyOneError> {
        let conversations = purged_conversations(tx, kind, id)?;
        let mut owners = vec![id.to_owned()];
        owners.extend(conversations.iter().cloned());
        owners.dedup();
        let notes = tx
            .prepare(
                "SELECT id FROM companion_scheduled_notes
                 WHERE character_id IN (SELECT value FROM json_each(?1)) ORDER BY id",
            )
            .and_then(|mut statement| {
                statement
                    .query_map(
                        [serde_json::to_string(&owners).unwrap_or_default()],
                        |row| row.get(0),
                    )?
                    .collect()
            })
            .map_err(|_| ApplyOneError::Storage)?;
        Ok(Self {
            kind,
            id: id.to_owned(),
            owners,
            conversations,
            notes,
        })
    }

    fn owns(&self, kind: &str, id: &str) -> bool {
        owned_entity(&self.owners, id)
            || (kind == lettuce_sync::COMPANION_NOTE_SYNC_KIND
                && self.notes.iter().any(|note| note == id))
    }

    /// Every scanned entity the scope owns, in scan (dependency) order;
    /// messages in timeline order.
    fn entities(
        &self,
        tx: &Transaction<'_>,
    ) -> Result<Vec<(&'static SnapshotCodec, String)>, ApplyOneError> {
        let mut entities = Vec::new();
        for codec in SCANNED_CODECS {
            if codec.kind == lettuce_sync::CONVERSATION_MESSAGE_SYNC_KIND {
                for conversation in &self.conversations {
                    let messages: Vec<String> = tx
                        .prepare(
                            "SELECT conversation_id || ':' || id FROM conversation_messages
                             WHERE conversation_id = ?1 ORDER BY timeline_ordinal, id",
                        )
                        .and_then(|mut statement| {
                            statement
                                .query_map([conversation], |row| row.get(0))?
                                .collect()
                        })
                        .map_err(|_| ApplyOneError::Storage)?;
                    entities.extend(messages.into_iter().map(|id| (codec, id)));
                }
                continue;
            }
            let ids = codec.ids.ok_or(ApplyOneError::Corrupt)?;
            entities.extend(
                ids(tx)?
                    .into_iter()
                    .filter(|id| self.owns(codec.kind, id))
                    .map(|id| (codec, id)),
            );
        }
        Ok(entities
            .into_iter()
            .filter(|(codec, id)| SyncEntity::new(codec.kind, id.clone()).is_ok())
            .collect())
    }
}

/// What the device holds of a deleted entity that the delete did not see.
#[derive(Default)]
struct UnseenChanges {
    local: bool,
    remote: bool,
}

/// Compares the delete with everything this device holds of the scope: a
/// journaled change of an owned entity the delete does not observe (from this
/// device or another), the current content of every owned entity against
/// its latest journaled content (edits made since the last scan, including
/// during an exchange), and a generation still running. Untouched seeds, such
/// as the Soul a received companion starts with, do not count.
fn unseen_changes(
    tx: &Transaction<'_>,
    delete: &CanonicalChange,
    scope: &PurgeScope,
) -> Result<UnseenChanges, ApplyOneError> {
    let mut unseen = UnseenChanges::default();
    let device = local_device(tx).map_err(|_| ApplyOneError::Storage)?;
    for owner in &scope.owners {
        let mut statement = tx
            .prepare(
                "SELECT change.change_id, change.entity_kind, change.entity_id, change.payload_bytes
                 FROM sync_changes change
                 WHERE (change.entity_id = ?1 OR change.entity_id LIKE ?1 || ':%'
                        OR change.entity_id LIKE 'conversation:' || ?1 || '%'
                        OR change.entity_id LIKE 'pool:' || ?1 || '%')
                   AND change.change_id <> ?2
                   AND NOT EXISTS (
                     SELECT 1 FROM sync_conflicts conflict
                     WHERE conflict.incoming_change_id = change.change_id
                       AND conflict.winning_side = 'current')
                   AND NOT EXISTS (
                     SELECT 1 FROM sync_deferred_changes deferred
                     WHERE deferred.change_id = change.change_id)",
            )
            .map_err(|_| ApplyOneError::Storage)?;
        let rows = statement
            .query_map(params![owner, delete.id().as_uuid().to_string()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<Vec<u8>>>(3)?,
                ))
            })
            .map_err(|_| ApplyOneError::Storage)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|_| ApplyOneError::Storage)?;
        for (change_id, kind, entity, payload) in rows {
            if !scope.owns(&kind, &entity) || is_seed(&kind, payload.as_deref()) {
                continue;
            }
            let change = load_change_by_id(
                tx,
                SyncChangeId::from_uuid(
                    Uuid::parse_str(&change_id).map_err(|_| ApplyOneError::Corrupt)?,
                ),
            )
            .map_err(|_| ApplyOneError::Storage)?
            .ok_or(ApplyOneError::Corrupt)?;
            if delete.observes(&change) {
                continue;
            }
            if Some(change.origin_device()) == device {
                unseen.local = true;
            } else {
                unseen.remote = true;
            }
        }
    }
    for (codec, id) in scope.entities(tx)? {
        let payload = match (codec.current)(tx, &id) {
            Ok(Some(payload)) => payload,
            Ok(None) | Err(ApplyOneError::Corrupt) => continue,
            Err(ApplyOneError::Unencodable) => {
                unseen.local = true;
                continue;
            }
            Err(error) => return Err(error),
        };
        if codec.seed.is_some_and(|seed| seed(payload.bytes())) {
            continue;
        }
        let latest = latest_journaled_entity(tx, codec.kind, &id, delete)?;
        if latest.as_ref() != Some(payload.content_hash()) {
            unseen.local = true;
        }
    }
    for conversation in &scope.conversations {
        unseen.local |= tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM conversation_turns WHERE conversation_id = ?1
                   AND status NOT IN ('succeeded', 'failed', 'cancelled', 'interrupted'))",
                [conversation],
                |row| row.get::<_, bool>(0),
            )
            .map_err(|_| ApplyOneError::Storage)?;
    }
    Ok(unseen)
}

fn is_seed(kind: &str, payload: Option<&[u8]>) -> bool {
    snapshot_codec(kind)
        .and_then(|codec| codec.seed)
        .zip(payload)
        .is_some_and(|(seed, payload)| seed(payload))
}

/// The content hash of the latest journaled change of one entity that became
/// local state, other than the delete being decided (`None` when never
/// journaled or deleted).
fn latest_journaled_entity(
    tx: &Transaction<'_>,
    kind: &str,
    id: &str,
    delete: &CanonicalChange,
) -> Result<Option<ContentHash>, ApplyOneError> {
    let hash: Option<Option<String>> = tx
        .query_row(
            "SELECT change.payload_hash FROM sync_changes change
             WHERE change.entity_kind = ?1 AND change.entity_id = ?2 AND change.change_id <> ?3
               AND NOT EXISTS (
                 SELECT 1 FROM sync_conflicts conflict
                 WHERE conflict.incoming_change_id = change.change_id
                   AND conflict.winning_side = 'current')
               AND NOT EXISTS (
                 SELECT 1 FROM sync_deferred_changes deferred
                 WHERE deferred.change_id = change.change_id)
             ORDER BY change.rowid DESC LIMIT 1",
            params![kind, id, delete.id().as_uuid().to_string()],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| ApplyOneError::Storage)?;
    hash.flatten()
        .map(ContentHash::parse)
        .transpose()
        .map_err(|_| ApplyOneError::Corrupt)
}

/// A received delete of a conversation or character is refused when this
/// device holds changes to it the deleting device had not seen. Everything
/// the entity owns is then journaled again as fresh inserts that observe the
/// delete, so the deleting device receives it back whole whichever device it
/// syncs with next; the user gets a notice when some of those changes are
/// this device's own.
fn keep_for_unseen_changes(
    tx: &Transaction<'_>,
    delete: &CanonicalChange,
    kind: crate::purge::PurgeKind,
    now: TimestampMillis,
) -> Result<bool, ApplyOneError> {
    let scope = PurgeScope::load(tx, kind, delete.entity().id())?;
    let unseen = unseen_changes(tx, delete, &scope)?;
    if unseen.local {
        crate::purge::record_notice(
            tx,
            notice_entity(kind),
            &scope.id,
            crate::PurgeNoticeReason::KeptUnsentLocalChanges,
            now,
        )
        .map_err(|_| ApplyOneError::Storage)?;
    }
    if unseen.local || unseen.remote {
        rejournal_scope(tx, &scope, now)?;
    }
    Ok(unseen.local || unseen.remote)
}

const fn notice_entity(kind: crate::purge::PurgeKind) -> crate::PurgeNoticeEntity {
    match kind {
        crate::purge::PurgeKind::Conversation => crate::PurgeNoticeEntity::Conversation,
        crate::purge::PurgeKind::Character => crate::PurgeNoticeEntity::Character,
    }
}

/// Decides a delete that waited in the purge queue again, against what the
/// device holds now: `true` when the entity is kept.
pub(crate) fn keep_queued_delete(
    tx: &Transaction<'_>,
    kind: crate::purge::PurgeKind,
    change_id: &str,
    now: TimestampMillis,
) -> Result<bool, crate::PurgeError> {
    let Ok(uuid) = Uuid::parse_str(change_id) else {
        return Ok(false);
    };
    let Some(change) = load_change_by_id(tx, SyncChangeId::from_uuid(uuid))
        .map_err(|_| crate::PurgeError::Storage)?
    else {
        return Ok(false);
    };
    keep_for_unseen_changes(tx, &change, kind, now).map_err(|error| match error {
        ApplyOneError::Storage => crate::PurgeError::Storage,
        _ => crate::PurgeError::Integrity,
    })
}

/// How many times a re-journal waits for missing snapshots, media or content
/// before it is sent without them.
const MAX_REJOURNAL_ATTEMPTS: i64 = 5;

/// Journals the current state of everything the scope owns as fresh inserts:
/// the launch snapshots and media it references first, then every owned
/// entity in dependency order. When a snapshot or referenced asset is
/// missing, a media blob is not ready or a payload cannot be encoded, nothing
/// is sent yet: the scope waits in `purge_rejournals` (retried before every
/// scan, which journals nothing it owns meanwhile) and the user gets a
/// `rejournal_incomplete` notice. After `MAX_REJOURNAL_ATTEMPTS` the scope is
/// sent without what is still missing, with a `rejournal_dropped` notice
/// naming each asset or entity left out.
fn rejournal_scope(
    tx: &Transaction<'_>,
    scope: &PurgeScope,
    now: TimestampMillis,
) -> Result<bool, ApplyOneError> {
    let journal = |error: LocalChangeJournalError| match error {
        LocalChangeJournalError::Storage => ApplyOneError::Storage,
        _ => ApplyOneError::Corrupt,
    };
    let key = params![scope.kind.name(), scope.id];
    let attempts: i64 = tx
        .query_row(
            "SELECT attempts FROM purge_rejournals WHERE entity_kind = ?1 AND entity_id = ?2",
            key,
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| ApplyOneError::Storage)?
        .unwrap_or(0);
    let mut missing: Vec<(crate::PurgeNoticeEntity, String)> = Vec::new();
    let mut artifacts = Vec::new();
    for conversation in &scope.conversations {
        let ids: Vec<String> = tx
            .prepare(
                "SELECT artifact_id FROM conversation_snapshot_refs
                 WHERE conversation_id = ?1 ORDER BY artifact_id",
            )
            .and_then(|mut statement| {
                statement
                    .query_map([conversation], |row| row.get(0))?
                    .collect()
            })
            .map_err(|_| ApplyOneError::Storage)?;
        for artifact in ids {
            let payload = crate::conversation::conversation_artifact_adapter::sync_load_snapshot(
                tx, &artifact,
            )
            .map_err(|_| ApplyOneError::Storage)?
            .and_then(|snapshot| serde_json::to_vec(&snapshot).ok())
            .and_then(|bytes| {
                CanonicalPayload::new(
                    lettuce_sync::CONVERSATION_SNAPSHOT_SYNC_SCHEMA,
                    lettuce_sync::CONVERSATION_SNAPSHOT_SYNC_VERSION,
                    bytes,
                )
                .ok()
            });
            match payload {
                Some(payload) => artifacts.push((artifact, payload)),
                None => missing.push((
                    crate::PurgeNoticeEntity::SyncEntity,
                    format!(
                        "{}/{artifact}",
                        lettuce_sync::CONVERSATION_SNAPSHOT_SYNC_KIND
                    ),
                )),
            }
        }
    }
    let mut entities = Vec::new();
    let mut media = Vec::new();
    let mut media_ids = std::collections::BTreeSet::new();
    for (codec, id) in scope.entities(tx)? {
        let payload = match (codec.current)(tx, &id) {
            Ok(Some(payload)) => payload,
            Ok(None) => continue,
            Err(ApplyOneError::Corrupt | ApplyOneError::Unencodable) => {
                missing.push((
                    crate::PurgeNoticeEntity::SyncEntity,
                    format!("{}/{id}", codec.kind),
                ));
                continue;
            }
            Err(error) => return Err(error),
        };
        for asset in (codec.assets)(payload.bytes()) {
            if !media_ids.insert(asset.clone()) {
                continue;
            }
            match ready_media_asset(tx, &asset)? {
                Some(request) => media.push(request),
                None => missing.push((crate::PurgeNoticeEntity::MediaAsset, asset)),
            }
        }
        entities.push((codec, id, payload));
    }
    if !missing.is_empty() && attempts + 1 < MAX_REJOURNAL_ATTEMPTS {
        let inserted = tx
            .execute(
                "INSERT INTO purge_rejournals (entity_kind, entity_id, attempts) VALUES (?1, ?2, 1)
                 ON CONFLICT (entity_kind, entity_id) DO UPDATE SET attempts = attempts + 1",
                key,
            )
            .map_err(|_| ApplyOneError::Storage)?;
        if inserted == 1 && attempts == 0 {
            crate::purge::record_notice(
                tx,
                notice_entity(scope.kind),
                &scope.id,
                crate::PurgeNoticeReason::RejournalIncomplete,
                now,
            )
            .map_err(|_| ApplyOneError::Storage)?;
        }
        return Ok(false);
    }
    for (entity, id) in &missing {
        crate::purge::record_notice(
            tx,
            *entity,
            id,
            crate::PurgeNoticeReason::RejournalDropped,
            now,
        )
        .map_err(|_| ApplyOneError::Storage)?;
    }
    for (artifact, payload) in artifacts {
        journal_state_change(
            tx,
            lettuce_sync::CONVERSATION_SNAPSHOT_SYNC_KIND,
            &artifact,
            ChangeOperation::Insert,
            None,
            Some(payload),
            now,
        )
        .map_err(journal)?;
    }
    for request in media {
        record_local_change_in(tx, OperationId::new(), &request, now).map_err(journal)?;
    }
    for (codec, id, payload) in entities {
        journal_state_change(
            tx,
            codec.kind,
            &id,
            ChangeOperation::Insert,
            None,
            Some(payload),
            now,
        )
        .map_err(journal)?;
    }
    tx.execute(
        "DELETE FROM purge_rejournals WHERE entity_kind = ?1 AND entity_id = ?2",
        key,
    )
    .map_err(|_| ApplyOneError::Storage)?;
    Ok(missing.is_empty())
}

/// The scopes whose re-journal still waits; the scan journals nothing they
/// own until it is sent.
fn pending_rejournal_scopes(tx: &Transaction<'_>) -> Result<Vec<PurgeScope>, ApplyOneError> {
    let pending: Vec<(String, String)> = tx
        .prepare(
            "SELECT entity_kind, entity_id FROM purge_rejournals ORDER BY entity_kind, entity_id",
        )
        .and_then(|mut statement| {
            statement
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect()
        })
        .map_err(|_| ApplyOneError::Storage)?;
    pending
        .into_iter()
        .map(|(kind, id)| {
            let kind = if kind == crate::purge::PurgeKind::Character.name() {
                crate::purge::PurgeKind::Character
            } else {
                crate::purge::PurgeKind::Conversation
            };
            PurgeScope::load(tx, kind, &id)
        })
        .collect()
}

/// Retries the re-journals that waited for media or snapshots; one whose
/// entity is gone is dropped.
fn retry_pending_rejournals(
    tx: &Transaction<'_>,
    now: TimestampMillis,
) -> Result<(), ApplyOneError> {
    let pending: Vec<(String, String)> = tx
        .prepare(
            "SELECT entity_kind, entity_id FROM purge_rejournals ORDER BY entity_kind, entity_id",
        )
        .and_then(|mut statement| {
            statement
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect()
        })
        .map_err(|_| ApplyOneError::Storage)?;
    for (kind_name, id) in pending {
        let kind = if kind_name == crate::purge::PurgeKind::Character.name() {
            crate::purge::PurgeKind::Character
        } else {
            crate::purge::PurgeKind::Conversation
        };
        let table = match kind {
            crate::purge::PurgeKind::Conversation => "conversations",
            crate::purge::PurgeKind::Character => "characters",
        };
        let exists: bool = tx
            .query_row(
                &format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE id = ?1)"),
                [&id],
                |row| row.get(0),
            )
            .map_err(|_| ApplyOneError::Storage)?;
        if !exists {
            tx.execute(
                "DELETE FROM purge_rejournals WHERE entity_kind = ?1 AND entity_id = ?2",
                params![kind_name, id],
            )
            .map_err(|_| ApplyOneError::Storage)?;
            continue;
        }
        let scope = PurgeScope::load(tx, kind, &id)?;
        rejournal_scope(tx, &scope, now)?;
    }
    Ok(())
}

/// The insert change of a referenced asset whose blob is ready here.
fn ready_media_asset(
    tx: &Transaction<'_>,
    id: &str,
) -> Result<Option<NewCanonicalChange>, ApplyOneError> {
    let Ok(asset_id) = id.parse::<lettuce_types::AssetId>() else {
        return Ok(None);
    };
    let Some(asset) =
        crate::load_asset_with_blob(tx, asset_id).map_err(|_| ApplyOneError::Storage)?
    else {
        return Ok(None);
    };
    let blob = tx
        .query_row(
            &format!(
                "SELECT {} FROM media_blobs WHERE id=?1",
                crate::MEDIA_BLOB_COLUMNS
            ),
            [asset.blob_id.to_string()],
            crate::media_from_row,
        )
        .map_err(|_| ApplyOneError::Storage)?;
    if blob.state != lettuce_media::BlobState::Ready {
        return Ok(None);
    }
    lettuce_sync::media_asset_insert_change(&lettuce_media::SyncMediaAsset { asset, blob })
        .map(Some)
        .map_err(|_| ApplyOneError::Corrupt)
}

/// Whether the entity has a journal entry other than `change` that became
/// local state (an emptied binding list is then present, not absent).
fn journaled_before(tx: &Transaction<'_>, change: &CanonicalChange) -> Result<bool, ApplyOneError> {
    tx.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM sync_changes other
           WHERE other.entity_kind = ?1 AND other.entity_id = ?2 AND other.change_id <> ?3
             AND NOT EXISTS (
               SELECT 1 FROM sync_conflicts conflict
               WHERE conflict.incoming_change_id = other.change_id
                 AND conflict.winning_side = 'current'
             )
             AND NOT EXISTS (
               SELECT 1 FROM sync_deferred_changes deferred
               WHERE deferred.change_id = other.change_id
             )
         )",
        params![
            change.entity().kind(),
            change.entity().id(),
            change.id().as_uuid().to_string()
        ],
        |row| row.get(0),
    )
    .map_err(|_| ApplyOneError::Storage)
}

/// Whether the latest local journal entry for the entity is a delete the
/// incoming change did not observe: a concurrent delete beats the update.
fn deleted_concurrently(
    tx: &Transaction<'_>,
    change: &CanonicalChange,
) -> Result<bool, ApplyOneError> {
    let latest_other = tx
        .query_row(
            "SELECT change.operation FROM sync_changes change
             WHERE change.entity_kind = ?1 AND change.entity_id = ?2 AND change.change_id <> ?3
               AND NOT EXISTS (
                 SELECT 1 FROM sync_conflicts conflict
                 WHERE conflict.incoming_change_id = change.change_id
                   AND conflict.winning_side = 'current'
               )
               AND NOT EXISTS (
                 SELECT 1 FROM sync_deferred_changes deferred
                 WHERE deferred.change_id = change.change_id
               )
             ORDER BY change.rowid DESC LIMIT 1",
            params![
                change.entity().kind(),
                change.entity().id(),
                change.id().as_uuid().to_string()
            ],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|_| ApplyOneError::Storage)?;
    if latest_other.as_deref() != Some("delete") {
        return Ok(false);
    }
    let delete_id: String = tx
        .query_row(
            "SELECT change_id FROM sync_changes
             WHERE entity_kind = ?1 AND entity_id = ?2 AND operation = 'delete'
             ORDER BY rowid DESC LIMIT 1",
            params![change.entity().kind(), change.entity().id()],
            |row| row.get(0),
        )
        .map_err(|_| ApplyOneError::Storage)?;
    let delete = load_change_by_id(
        tx,
        SyncChangeId::from_uuid(Uuid::parse_str(&delete_id).map_err(|_| ApplyOneError::Corrupt)?),
    )
    .map_err(|_| ApplyOneError::Storage)?
    .ok_or(ApplyOneError::Corrupt)?;
    Ok(!change.observes(&delete))
}

/// One incoming complete-snapshot change: identical snapshots and clean
/// inserts/updates apply, an absent entity adopts the snapshot, anything
/// else is a concurrent edit settled by `incoming_wins` with both snapshots
/// kept as conflict evidence.
/// Settling runs after the change is journaled, so a deferred change can be
/// settled again later.
fn settle_snapshot_change(
    tx: &Transaction<'_>,
    change: &CanonicalChange,
    now: TimestampMillis,
    codec: &SnapshotCodec,
) -> Result<bool, ApplyOneError> {
    if change.operation() == ChangeOperation::Delete {
        return settle_snapshot_delete(tx, change, now, codec);
    }
    let payload = change.payload().ok_or(ApplyOneError::Corrupt)?;
    (codec.decode)(change.entity().id(), payload.bytes())?;
    let mut current_payload = (codec.current)(tx, change.entity().id())?;
    if current_payload.is_none()
        && let Some(empty) = codec.empty
        && journaled_before(tx, change)?
    {
        current_payload = Some(empty()?);
    }
    if current_payload.is_none() && deleted_concurrently(tx, change)? {
        insert_conflict(tx, change, None, Some(&[]), payload.bytes(), false, now)?;
        return Ok(true);
    }
    let same = current_payload
        .as_ref()
        .is_some_and(|value| value.content_hash() == payload.content_hash());
    let clean_update = current_payload.as_ref().is_some_and(|value| {
        change.operation() == ChangeOperation::Update
            && change.base_revision() == Some(value.content_hash())
    });
    let clean_insert = current_payload.is_none();
    let conflict = !same && !clean_update && !clean_insert;
    let current_change = current_payload
        .as_ref()
        .map(|value| load_materialized_change(tx, change.entity(), value.content_hash()))
        .transpose()
        .map_err(|error| match error {
            IncomingChangeError::Storage => ApplyOneError::Storage,
            _ => ApplyOneError::Corrupt,
        })?
        .flatten();
    let seeds = codec
        .seed
        .zip(current_payload.as_ref())
        .map(|(seed, current)| (seed(current.bytes()), seed(payload.bytes())));
    let winner_is_incoming = !conflict
        || match seeds {
            Some((true, false)) => true,
            Some((false, true)) => false,
            _ => current_change
                .as_ref()
                .is_some_and(|current| incoming_wins(change, current)),
        };
    let mut unmaterialized = false;
    if winner_is_incoming && !same {
        unmaterialized = !(codec.materialize)(tx, change.entity().id(), payload.bytes())?;
    }
    if unmaterialized && current_payload.is_none() {
        return Ok(false);
    }
    if conflict || unmaterialized {
        let current_bytes = current_payload
            .as_ref()
            .map(CanonicalPayload::bytes)
            .ok_or(ApplyOneError::Corrupt)?;
        insert_conflict(
            tx,
            change,
            current_change.as_ref(),
            Some(current_bytes),
            payload.bytes(),
            winner_is_incoming && !unmaterialized,
            now,
        )?;
    }
    if !unmaterialized {
        resolve_dominated_conflicts(tx, change, now, None)?;
    }
    Ok(conflict || unmaterialized)
}

fn settle_media_asset_change(
    tx: &Transaction<'_>,
    change: &CanonicalChange,
) -> Result<bool, ApplyOneError> {
    let payload = change.payload().ok_or(ApplyOneError::Corrupt)?;
    let incoming: lettuce_media::SyncMediaAsset =
        serde_json::from_slice(payload.bytes()).map_err(|_| ApplyOneError::Corrupt)?;
    incoming.validate().map_err(|_| ApplyOneError::Corrupt)?;
    let id = change
        .entity()
        .id()
        .parse::<lettuce_types::AssetId>()
        .map_err(|_| ApplyOneError::Corrupt)?;
    if incoming.asset.id != id {
        return Err(ApplyOneError::Corrupt);
    }
    let current = crate::load_asset_with_blob(tx, id)
        .map_err(|_| ApplyOneError::Storage)?
        .ok_or(ApplyOneError::Pending)?;
    let blob = tx
        .query_row(
            &format!(
                "SELECT {} FROM media_blobs WHERE id=?1",
                crate::MEDIA_BLOB_COLUMNS
            ),
            [current.blob_id.to_string()],
            crate::media_from_row,
        )
        .map_err(|_| ApplyOneError::Storage)?;
    let expected = incoming.asset;
    if current.kind != expected.kind
        || current.origin != expected.origin
        || current.created_at != expected.created_at
        || blob.content_hash != incoming.blob.content_hash
        || blob.kind != incoming.blob.kind
        || blob.mime_type != incoming.blob.mime_type
        || blob.byte_size != incoming.blob.byte_size
        || blob.width != incoming.blob.width
        || blob.height != incoming.blob.height
        || blob.duration_ms != incoming.blob.duration_ms
        || blob.validation_version != incoming.blob.validation_version
        || blob.state != lettuce_media::BlobState::Ready
    {
        return Err(ApplyOneError::Corrupt);
    }
    Ok(false)
}

fn settle_change(
    tx: &Transaction<'_>,
    change: &CanonicalChange,
    now: TimestampMillis,
) -> Result<bool, ApplyOneError> {
    match change.entity().kind() {
        "media_asset" => settle_media_asset_change(tx, change),
        lettuce_sync::CONVERSATION_SNAPSHOT_SYNC_KIND => {
            let payload = change.payload().ok_or(ApplyOneError::Corrupt)?;
            let artifact: crate::conversation::conversation_artifact_adapter::SyncSnapshotArtifact =
                serde_json::from_slice(payload.bytes()).map_err(|_| ApplyOneError::Corrupt)?;
            if artifact.reference.artifact_id.to_string() != change.entity().id() {
                return Err(ApplyOneError::Corrupt);
            }
            crate::conversation::conversation_artifact_adapter::sync_stage_snapshot(
                tx, &artifact, now,
            )
            .map_err(|error| match error {
                lettuce_conversations::ArtifactError::Storage => ApplyOneError::Storage,
                _ => ApplyOneError::Corrupt,
            })?;
            Ok(false)
        }
        kind => match snapshot_codec(kind) {
            Some(codec) => settle_snapshot_change(tx, change, now, codec),
            None => Err(ApplyOneError::Corrupt),
        },
    }
}

/// Whether an earlier change for the same entity is still deferred: changes
/// of one entity settle in journal order, so a later one waits behind it.
fn deferred_before(tx: &Transaction<'_>, change: &CanonicalChange) -> Result<bool, ApplyOneError> {
    tx.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM sync_deferred_changes deferred
           JOIN sync_changes earlier ON earlier.change_id = deferred.change_id
           WHERE deferred.entity_kind = ?1 AND deferred.entity_id = ?2
             AND deferred.change_id <> ?3
             AND earlier.rowid < (SELECT rowid FROM sync_changes WHERE change_id = ?3)
         )",
        params![
            change.entity().kind(),
            change.entity().id(),
            change.id().as_uuid().to_string()
        ],
        |row| row.get(0),
    )
    .map_err(|_| ApplyOneError::Storage)
}

/// Whether a change for this entity is deferred here: the entity is on its
/// way, so a reference to it must wait instead of being cleared.
pub(crate) fn entity_deferred(
    connection: &Connection,
    kind: &str,
    id: &str,
) -> Result<bool, rusqlite::Error> {
    connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sync_deferred_changes WHERE entity_kind = ?1 AND entity_id = ?2)",
        params![kind, id],
        |row| row.get(0),
    )
}

fn defer_change(
    tx: &Transaction<'_>,
    change: &CanonicalChange,
    now: TimestampMillis,
) -> Result<(), ApplyOneError> {
    tx.execute(
        "INSERT INTO sync_deferred_changes (change_id, entity_kind, entity_id, deferred_at)
         VALUES (?1, ?2, ?3, ?4) ON CONFLICT(change_id) DO NOTHING",
        params![
            change.id().as_uuid().to_string(),
            change.entity().kind(),
            change.entity().id(),
            now.get()
        ],
    )
    .map_err(|_| ApplyOneError::Storage)?;
    Ok(())
}

/// Settles a journaled change inside a savepoint. A change waiting for an
/// entity that is not here yet (its owner, derivation source or media), or
/// behind an earlier deferred change of the same entity, is rolled back and
/// queued per entity instead of holding the batch, so the origin's later
/// changes still apply; deferred changes are settled again after every batch
/// and media phase.
fn settle_or_defer(
    tx: &Transaction<'_>,
    change: &CanonicalChange,
    now: TimestampMillis,
) -> Result<Option<bool>, ApplyOneError> {
    if deferred_before(tx, change)? {
        defer_change(tx, change, now)?;
        return Ok(None);
    }
    tx.execute_batch("SAVEPOINT sync_settle")
        .map_err(|_| ApplyOneError::Storage)?;
    match settle_change(tx, change, now) {
        Ok(conflict) => {
            tx.execute_batch("RELEASE sync_settle")
                .map_err(|_| ApplyOneError::Storage)?;
            tx.execute(
                "DELETE FROM sync_deferred_changes WHERE change_id = ?1",
                [change.id().as_uuid().to_string()],
            )
            .map_err(|_| ApplyOneError::Storage)?;
            Ok(Some(conflict))
        }
        Err(error @ (ApplyOneError::Pending | ApplyOneError::Unencodable)) => {
            tx.execute_batch("ROLLBACK TO sync_settle; RELEASE sync_settle")
                .map_err(|_| ApplyOneError::Storage)?;
            defer_change(tx, change, now)?;
            if matches!(error, ApplyOneError::Unencodable) {
                record_not_synced(tx, change.entity().kind(), change.entity().id(), now)?;
            }
            Ok(None)
        }
        Err(error) => {
            let _ = tx.execute_batch("ROLLBACK TO sync_settle; RELEASE sync_settle");
            Err(error)
        }
    }
}

pub(crate) fn retry_deferred_changes_in(
    tx: &Transaction<'_>,
    now: TimestampMillis,
) -> Result<usize, ()> {
    retry_deferred_changes(tx, now).map_err(|_| ())
}

/// Settles deferred changes, in journal order per entity, until none makes
/// progress. A deferred change that now fails for another reason stays
/// deferred instead of failing every later batch.
fn retry_deferred_changes(
    tx: &Transaction<'_>,
    now: TimestampMillis,
) -> Result<usize, ApplyOneError> {
    let mut settled = 0;
    loop {
        let deferred = tx
            .prepare(
                "SELECT deferred.change_id FROM sync_deferred_changes deferred
                 JOIN sync_changes change ON change.change_id = deferred.change_id
                 ORDER BY change.rowid",
            )
            .and_then(|mut statement| {
                statement
                    .query_map([], |row| row.get::<_, String>(0))?
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .map_err(|_| ApplyOneError::Storage)?;
        let mut progress = false;
        for id in deferred {
            let id =
                SyncChangeId::from_uuid(Uuid::parse_str(&id).map_err(|_| ApplyOneError::Corrupt)?);
            let change = load_change_by_id(tx, id)
                .map_err(|_| ApplyOneError::Storage)?
                .ok_or(ApplyOneError::Corrupt)?;
            match settle_or_defer(tx, &change, now) {
                Ok(Some(_)) => {
                    progress = true;
                    settled += 1;
                }
                Ok(None)
                | Err(
                    ApplyOneError::Corrupt | ApplyOneError::Pending | ApplyOneError::Unencodable,
                ) => {}
                Err(ApplyOneError::Storage) => return Err(ApplyOneError::Storage),
            }
        }
        if !progress {
            return Ok(settled);
        }
    }
}

fn mark_batch_pending(
    connection: &Connection,
    batch_id: OperationId,
    reason: &str,
) -> Result<(), IncomingChangeError> {
    connection
        .execute(
            "UPDATE sync_incoming_batches
             SET state = 'pending', pending_reason = ?2
             WHERE batch_id = ?1 AND state <> 'committed'",
            params![batch_id.to_string(), reason],
        )
        .map_err(incoming_storage)?;
    Ok(())
}

fn conflict_storage(_: impl std::fmt::Debug) -> ConflictRepositoryError {
    ConflictRepositoryError::Storage
}

fn conflict_corrupt(_: impl std::fmt::Debug) -> ConflictRepositoryError {
    ConflictRepositoryError::Corrupt
}

fn conflict_value(
    entity_kind: &str,
    entity_id: &str,
    payload: &[u8],
) -> Result<PersonaConflictValue, ConflictRepositoryError> {
    match entity_kind {
        "persona" => {
            let persona: Persona = serde_json::from_slice(payload).map_err(conflict_corrupt)?;
            persona.validate().map_err(conflict_corrupt)?;
            if persona.id.to_string() != entity_id {
                return Err(ConflictRepositoryError::Corrupt);
            }
            Ok(PersonaConflictValue::Persona(persona))
        }
        "persona_default" => {
            if entity_id != "application" {
                return Err(ConflictRepositoryError::Corrupt);
            }
            let state: PersonaDefaultState =
                serde_json::from_slice(payload).map_err(conflict_corrupt)?;
            state.validate().map_err(conflict_corrupt)?;
            Ok(PersonaConflictValue::Default(state))
        }
        _ => Err(ConflictRepositoryError::Corrupt),
    }
}

fn conflict_candidate(
    connection: &Connection,
    change_id: Option<&str>,
    entity_kind: &str,
    entity_id: &str,
    payload: &[u8],
) -> Result<PersonaConflictCandidate, ConflictRepositoryError> {
    let change = change_id
        .map(|id| {
            let id = SyncChangeId::from_uuid(Uuid::parse_str(id).map_err(conflict_corrupt)?);
            load_change_by_id(connection, id)
                .map_err(|error| match error {
                    IncomingChangeError::Storage => ConflictRepositoryError::Storage,
                    _ => ConflictRepositoryError::Corrupt,
                })?
                .ok_or(ConflictRepositoryError::Corrupt)
        })
        .transpose()?;
    Ok(PersonaConflictCandidate {
        change_id: change.as_ref().map(CanonicalChange::id),
        device_id: change.as_ref().map(CanonicalChange::origin_device),
        timestamp: change.as_ref().map(|value| value.timestamp().wall_time()),
        value: conflict_value(entity_kind, entity_id, payload)?,
    })
}

fn resolution_value(
    selected: &PersonaConflictValue,
    current: &PersonaConflictValue,
    now: TimestampMillis,
) -> Result<PersonaConflictValue, ConflictRepositoryError> {
    match (selected, current) {
        (PersonaConflictValue::Persona(selected), PersonaConflictValue::Persona(current)) => {
            let mut resolved = selected.clone();
            resolved.id = current.id;
            resolved.revision = current.revision.next().map_err(conflict_corrupt)?;
            resolved.created_at = current.created_at;
            resolved.updated_at = now;
            resolved.validate().map_err(conflict_corrupt)?;
            Ok(PersonaConflictValue::Persona(resolved))
        }
        (PersonaConflictValue::Default(selected), PersonaConflictValue::Default(current)) => {
            let mut resolved = selected.clone();
            resolved.revision = current.revision.next().map_err(conflict_corrupt)?;
            resolved.created_at = current.created_at;
            resolved.updated_at = now;
            resolved.validate().map_err(conflict_corrupt)?;
            Ok(PersonaConflictValue::Default(resolved))
        }
        _ => Err(ConflictRepositoryError::Corrupt),
    }
}

fn resolution_request(
    entity_kind: &str,
    current: &PersonaConflictValue,
    resolved: &PersonaConflictValue,
) -> Result<NewCanonicalChange, ConflictRepositoryError> {
    let (entity, current_payload, resolved_payload) = match (current, resolved) {
        (PersonaConflictValue::Persona(current), PersonaConflictValue::Persona(resolved)) => (
            persona_sync_entity(current.id).map_err(conflict_corrupt)?,
            canonical_persona_payload(current).map_err(conflict_corrupt)?,
            canonical_persona_payload(resolved).map_err(conflict_corrupt)?,
        ),
        (PersonaConflictValue::Default(current), PersonaConflictValue::Default(resolved)) => (
            persona_default_sync_entity().map_err(conflict_corrupt)?,
            canonical_persona_default_payload(current).map_err(conflict_corrupt)?,
            canonical_persona_default_payload(resolved).map_err(conflict_corrupt)?,
        ),
        _ => return Err(ConflictRepositoryError::Corrupt),
    };
    if entity.kind() != entity_kind {
        return Err(ConflictRepositoryError::Corrupt);
    }
    NewCanonicalChange::new(
        entity,
        ChangeOperation::Update,
        Some(current_payload.content_hash().clone()),
        Some(resolved_payload),
    )
    .map_err(conflict_corrupt)
}

fn apply_resolution_value(
    tx: &Transaction<'_>,
    value: PersonaConflictValue,
) -> Result<(), ConflictRepositoryError> {
    let map_error = |error| match error {
        RepositoryError::Storage => ConflictRepositoryError::Storage,
        _ => ConflictRepositoryError::Conflict,
    };
    match value {
        PersonaConflictValue::Persona(persona) => {
            apply_synced_persona(tx, persona).map_err(map_error)?;
        }
        PersonaConflictValue::Default(state) => {
            apply_synced_persona_default(tx, state).map_err(map_error)?;
        }
    }
    Ok(())
}

fn materialized_conflict_value(
    connection: &Connection,
    entity_kind: &str,
    entity_id: &str,
) -> Result<PersonaConflictValue, ConflictRepositoryError> {
    match entity_kind {
        "persona" => {
            let id = entity_id.parse::<PersonaId>().map_err(conflict_corrupt)?;
            let persona = load_persona(connection, id)
                .map_err(conflict_storage)?
                .ok_or(ConflictRepositoryError::Conflict)?;
            Ok(PersonaConflictValue::Persona(persona))
        }
        "persona_default" if entity_id == "application" => read_default(connection)
            .map(PersonaConflictValue::Default)
            .map_err(conflict_storage),
        _ => Err(ConflictRepositoryError::Corrupt),
    }
}

fn conflict_choice_name(choice: ConflictChoice) -> &'static str {
    match choice {
        ConflictChoice::Current => "current",
        ConflictChoice::Other => "other",
    }
}

fn map_local_conflict_error(error: LocalChangeJournalError) -> ConflictRepositoryError {
    match error {
        LocalChangeJournalError::Conflict => ConflictRepositoryError::Conflict,
        _ => ConflictRepositoryError::Storage,
    }
}

struct StoredConflictRecord {
    entity_kind: String,
    entity_id: String,
    current_change_id: Option<String>,
    incoming_change_id: String,
    winning_side: String,
    current_payload: Vec<u8>,
    incoming_payload: Vec<u8>,
    detected_at: i64,
    status: String,
    resolution_choice: Option<String>,
    resolved_by_change_id: Option<String>,
}

impl StoredConflictRecord {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            entity_kind: row.get(0)?,
            entity_id: row.get(1)?,
            current_change_id: row.get(2)?,
            incoming_change_id: row.get(3)?,
            winning_side: row.get(4)?,
            current_payload: row.get(5)?,
            incoming_payload: row.get(6)?,
            detected_at: row.get(7)?,
            status: row.get(8)?,
            resolution_choice: row.get(9)?,
            resolved_by_change_id: row.get(10)?,
        })
    }
}

fn conflict_candidates(
    connection: &Connection,
    record: &StoredConflictRecord,
) -> Result<(PersonaConflictCandidate, PersonaConflictCandidate), ConflictRepositoryError> {
    let prior = conflict_candidate(
        connection,
        record.current_change_id.as_deref(),
        &record.entity_kind,
        &record.entity_id,
        &record.current_payload,
    )?;
    let incoming = conflict_candidate(
        connection,
        Some(&record.incoming_change_id),
        &record.entity_kind,
        &record.entity_id,
        &record.incoming_payload,
    )?;
    match record.winning_side.as_str() {
        "current" => Ok((prior, incoming)),
        "incoming" => Ok((incoming, prior)),
        _ => Err(ConflictRepositoryError::Corrupt),
    }
}

impl LocalChangeJournal for Database {
    fn journal_current_state(
        &self,
        now: TimestampMillis,
    ) -> Result<usize, LocalChangeJournalError> {
        let mut connection = self.connection().map_err(storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        rebaseline_journal_if_format_changed(&tx).map_err(storage)?;
        retry_pending_rejournals(&tx, now).map_err(journal_apply_error)?;
        let pending_scopes = pending_rejournal_scopes(&tx).map_err(journal_apply_error)?;
        let mut journaled = journal_referenced_media(&tx, now)?;
        journaled += journal_referenced_snapshots(&tx, now)?;
        let mut present = Vec::with_capacity(SCANNED_CODECS.len());
        for codec in SCANNED_CODECS {
            let ids = codec.ids.ok_or(LocalChangeJournalError::Corrupt)?;
            let ids = ids(&tx).map_err(journal_apply_error)?;
            let latest = latest_journaled(&tx, codec.kind)?;
            let mut skipped = Vec::new();
            for id in &ids {
                if SyncEntity::new(codec.kind, id.clone()).is_err() {
                    continue;
                }
                if entity_deferred(&tx, codec.kind, id).map_err(storage)?
                    || crate::purge::purge_queued(&tx, codec.kind, id).map_err(storage)?
                    || pending_scopes
                        .iter()
                        .any(|scope| scope.owns(codec.kind, id))
                {
                    skipped.push(id.clone());
                    continue;
                }
                let payload = match (codec.current)(&tx, id) {
                    Ok(Some(payload)) => payload,
                    Ok(None) => {
                        skipped.push(id.clone());
                        continue;
                    }
                    Err(ApplyOneError::Unencodable) => {
                        record_not_synced(&tx, codec.kind, id, now).map_err(journal_apply_error)?;
                        skipped.push(id.clone());
                        continue;
                    }
                    Err(ApplyOneError::Corrupt) => continue,
                    Err(error) => return Err(journal_apply_error(error)),
                };
                let base = latest.get(id).cloned().flatten();
                if base.as_ref() == Some(payload.content_hash()) {
                    continue;
                }
                let mut media_ready = true;
                for asset in (codec.assets)(payload.bytes()) {
                    media_ready &= media_asset_journaled(&tx, &asset)?;
                }
                if !media_ready {
                    continue;
                }
                let operation = if base.is_some() {
                    ChangeOperation::Update
                } else {
                    ChangeOperation::Insert
                };
                journal_state_change(&tx, codec.kind, id, operation, base, Some(payload), now)?;
                journaled += 1;
            }
            if codec.kind == lettuce_sync::CONVERSATION_MESSAGE_SYNC_KIND {
                crate::sync::conversation_sync_adapter::mark_messages_scanned(&tx, &ids, &skipped)
                    .map_err(storage)?;
            }
            present.push((codec, ids, latest));
        }
        for (codec, ids, latest) in present.into_iter().rev() {
            let empty = codec
                .empty
                .map(|empty| empty())
                .transpose()
                .map_err(journal_apply_error)?;
            if codec.delete.is_none() && empty.is_none() {
                continue;
            }
            for (id, base) in latest {
                let Some(base) = base else { continue };
                if ids.binary_search(&id).is_ok()
                    || entity_deferred(&tx, codec.kind, &id).map_err(storage)?
                {
                    continue;
                }
                match &empty {
                    Some(empty) if &base == empty.content_hash() => {}
                    Some(empty) => {
                        journal_state_change(
                            &tx,
                            codec.kind,
                            &id,
                            ChangeOperation::Update,
                            Some(base),
                            Some(empty.clone()),
                            now,
                        )?;
                        journaled += 1;
                    }
                    None => {
                        journal_state_change(
                            &tx,
                            codec.kind,
                            &id,
                            ChangeOperation::Delete,
                            Some(base),
                            None,
                            now,
                        )?;
                        journaled += 1;
                    }
                }
            }
        }
        tx.commit().map_err(storage)?;
        crate::purge::run_queued_purges_on(&mut connection, &self.foreign_keys_lost, now)
            .map_err(storage)?;
        Ok(journaled)
    }

    fn local_device_id(
        &self,
        now: TimestampMillis,
    ) -> Result<SyncDeviceId, LocalChangeJournalError> {
        let mut connection = self.connection().map_err(storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let device = ensure_local_device(&transaction, now)?;
        transaction.commit().map_err(storage)?;
        Ok(device)
    }

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
        if local_device(&connection)?.is_none() {
            if !local_frontier.is_empty() {
                return Err(LocalChangeJournalError::Corrupt);
            }
            return Ok(OutboundChangeBatch {
                changes: Vec::new(),
                payload_bytes: 0,
                has_more: false,
            });
        }
        let mut simulated = remote_frontier.clone();
        let mut cursors = BTreeMap::new();
        for (origin, local_sequence) in &local_frontier {
            let seen = remote_frontier
                .get(origin)
                .copied()
                .unwrap_or(0)
                .min(*local_sequence);
            if seen == 0 {
                simulated.remove(origin);
            } else {
                simulated.insert(*origin, seen);
            }
            if seen < *local_sequence {
                cursors.insert(*origin, (seen.saturating_add(1), *local_sequence));
            }
        }
        let max_changes = max_changes.min(MAX_OUTBOUND_CHANGES);
        let max_payload_bytes = max_payload_bytes.min(MAX_OUTBOUND_PAYLOAD_BYTES);
        let mut changes = Vec::new();
        let mut payload_bytes = 0usize;
        if max_changes == 0 || max_payload_bytes == 0 {
            return Ok(OutboundChangeBatch {
                changes,
                payload_bytes,
                has_more: !cursors.is_empty(),
            });
        }
        let mut heads = BTreeMap::new();
        while changes.len() < max_changes && !cursors.is_empty() {
            let mut next: Option<(CanonicalChange, SyncDeviceId)> = None;
            for (origin, (sequence, _)) in &cursors {
                let head = match heads.remove(origin) {
                    Some(head) => head,
                    None => change_for_sequence(&connection, *origin, *sequence)?,
                };
                let ready = head.base_frontier().iter().all(|(dependency, required)| {
                    simulated.get(dependency).copied().unwrap_or(0) >= *required
                });
                let earlier = next.as_ref().is_none_or(|(current, _)| {
                    (head.timestamp(), head.origin_device())
                        < (current.timestamp(), current.origin_device())
                });
                if ready && earlier {
                    if let Some((previous, previous_origin)) = next.take() {
                        heads.insert(previous_origin, previous);
                    }
                    next = Some((head, *origin));
                } else {
                    heads.insert(*origin, head);
                }
            }
            let Some((change, origin)) = next else {
                return Err(LocalChangeJournalError::UnsatisfiedDependencies);
            };
            let bytes = change.payload().map_or(0, |payload| payload.bytes().len());
            if !changes.is_empty() && payload_bytes.saturating_add(bytes) > max_payload_bytes {
                break;
            }
            payload_bytes += bytes;
            let (sequence, last) = cursors
                .get_mut(&origin)
                .ok_or(LocalChangeJournalError::Corrupt)?;
            simulated.insert(origin, *sequence);
            if *sequence == *last {
                cursors.remove(&origin);
            } else {
                *sequence += 1;
            }
            changes.push(change);
        }
        Ok(OutboundChangeBatch {
            changes,
            payload_bytes,
            has_more: !cursors.is_empty(),
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

impl lettuce_sync::ConversationForkRepository for Database {
    fn unresolved_conversation_forks(
        &self,
        limit: usize,
    ) -> Result<Vec<lettuce_sync::ConversationFork>, ConflictRepositoryError> {
        let connection = self.connection().map_err(conflict_storage)?;
        let mut statement = connection
            .prepare(
                "SELECT conversation_id, branch_id, holds_local, detected_at FROM sync_conversation_forks
                 WHERE resolved_at IS NULL ORDER BY detected_at DESC, conversation_id, branch_id LIMIT ?1",
            )
            .map_err(conflict_storage)?;
        let rows = statement
            .query_map(
                [i64::try_from(limit.min(MAX_UNRESOLVED_CONFLICTS)).map_err(conflict_corrupt)?],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                },
            )
            .map_err(conflict_storage)?;
        rows.map(|row| {
            let (conversation, branch, holds_local, detected_at) = row.map_err(conflict_storage)?;
            Ok(lettuce_sync::ConversationFork {
                conversation_id: conversation.parse().map_err(conflict_corrupt)?,
                branch_id: branch.parse().map_err(conflict_corrupt)?,
                holds_local: holds_local != 0,
                detected_at: TimestampMillis::new(detected_at),
            })
        })
        .collect()
    }

    fn resolve_conversation_fork(
        &self,
        conversation_id: lettuce_types::ConversationId,
        branch_id: lettuce_types::ConversationBranchId,
        now: TimestampMillis,
    ) -> Result<(), ConflictRepositoryError> {
        let connection = self.connection().map_err(conflict_storage)?;
        let changed = connection
            .execute(
                "UPDATE sync_conversation_forks SET resolved_at = ?3 WHERE conversation_id = ?1 AND branch_id = ?2 AND resolved_at IS NULL",
                params![conversation_id.to_string(), branch_id.to_string(), now.get()],
            )
            .map_err(conflict_storage)?;
        if changed == 0 {
            return Err(ConflictRepositoryError::NotFound);
        }
        Ok(())
    }
}

impl PersonaConflictRepository for Database {
    fn unresolved_persona_conflicts(
        &self,
        limit: usize,
    ) -> Result<Vec<PersonaConflict>, ConflictRepositoryError> {
        let connection = self.connection().map_err(conflict_storage)?;
        let mut statement = connection
            .prepare(
                "SELECT entity_kind, entity_id, current_change_id,
                        incoming_change_id, winning_side, current_payload,
                        incoming_payload, detected_at, status,
                        resolution_choice, resolved_by_change_id, conflict_id
                 FROM sync_conflicts
                 WHERE status = 'unresolved' AND entity_kind IN ('persona', 'persona_default')
                 ORDER BY detected_at DESC, conflict_id
                 LIMIT ?1",
            )
            .map_err(conflict_storage)?;
        let rows = statement
            .query_map(
                [i64::try_from(limit.min(MAX_UNRESOLVED_CONFLICTS)).map_err(conflict_corrupt)?],
                |row| {
                    Ok((
                        StoredConflictRecord::from_row(row)?,
                        row.get::<_, String>(11)?,
                    ))
                },
            )
            .map_err(conflict_storage)?;
        let mut conflicts = Vec::new();
        for row in rows {
            let (record, id) = row.map_err(conflict_storage)?;
            let (current, other) = conflict_candidates(&connection, &record)?;
            conflicts.push(PersonaConflict {
                id: id.parse().map_err(conflict_corrupt)?,
                entity_kind: record.entity_kind,
                entity_id: record.entity_id,
                detected_at: TimestampMillis::new(record.detected_at),
                current,
                other,
            });
        }
        Ok(conflicts)
    }

    fn resolve_persona_conflict(
        &self,
        conflict_id: OperationId,
        expected_current_change: Option<SyncChangeId>,
        choice: ConflictChoice,
        resolution_id: OperationId,
        now: TimestampMillis,
    ) -> Result<CanonicalChange, ConflictRepositoryError> {
        let mut connection = self.connection().map_err(conflict_storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(conflict_storage)?;
        let record = transaction
            .query_row(
                "SELECT entity_kind, entity_id, current_change_id,
                        incoming_change_id, winning_side, current_payload,
                        incoming_payload, detected_at, status,
                        resolution_choice, resolved_by_change_id
                 FROM sync_conflicts WHERE conflict_id = ?1",
                [conflict_id.to_string()],
                StoredConflictRecord::from_row,
            )
            .optional()
            .map_err(conflict_storage)?
            .ok_or(ConflictRepositoryError::NotFound)?;
        let (current, other) = conflict_candidates(&transaction, &record)?;
        if current.change_id != expected_current_change {
            return Err(ConflictRepositoryError::Conflict);
        }
        if record.status == "resolved" {
            let resolved_id = record
                .resolved_by_change_id
                .ok_or(ConflictRepositoryError::Corrupt)?;
            if record.resolution_choice.as_deref() != Some(conflict_choice_name(choice)) {
                return Err(ConflictRepositoryError::Conflict);
            }
            let change = load_local_change_in(&transaction, resolution_id)
                .map_err(map_local_conflict_error)?
                .ok_or(ConflictRepositoryError::Conflict)?;
            if change.id().as_uuid().to_string() != resolved_id {
                return Err(ConflictRepositoryError::Conflict);
            }
            transaction.commit().map_err(conflict_storage)?;
            return Ok(change);
        }
        if record.status != "unresolved"
            || record.resolution_choice.is_some()
            || record.resolved_by_change_id.is_some()
        {
            return Err(ConflictRepositoryError::Corrupt);
        }
        let materialized =
            materialized_conflict_value(&transaction, &record.entity_kind, &record.entity_id)?;
        if materialized != current.value {
            return Err(ConflictRepositoryError::Conflict);
        }
        let selected = match choice {
            ConflictChoice::Current => &current.value,
            ConflictChoice::Other => &other.value,
        };
        let resolved = resolution_value(selected, &materialized, now)?;
        let request = resolution_request(&record.entity_kind, &materialized, &resolved)?;
        let admission = record_local_change_in_skipping(
            &transaction,
            resolution_id,
            &request,
            now,
            Some(conflict_id),
            None,
        )
        .map_err(map_local_conflict_error)?;
        if !admission.created {
            return Err(ConflictRepositoryError::Conflict);
        }
        apply_resolution_value(&transaction, resolved)?;
        let changed = transaction
            .execute(
                "UPDATE sync_conflicts SET status = 'resolved',
                   resolution_choice = ?2, resolved_by_change_id = ?3,
                   resolved_at = ?4 WHERE conflict_id = ?1 AND status = 'unresolved'",
                params![
                    conflict_id.to_string(),
                    conflict_choice_name(choice),
                    admission.change.id().as_uuid().to_string(),
                    now.get()
                ],
            )
            .map_err(conflict_storage)?;
        if changed != 1 {
            return Err(ConflictRepositoryError::Conflict);
        }
        resolve_dominated_conflicts(&transaction, &admission.change, now, None).map_err(
            |error| match error {
                ApplyOneError::Storage => ConflictRepositoryError::Storage,
                _ => ConflictRepositoryError::Corrupt,
            },
        )?;
        transaction.commit().map_err(conflict_storage)?;
        Ok(admission.change)
    }
}

impl IncomingChangeRepository for Database {
    fn stage_incoming_batch(
        &self,
        peer: SyncDeviceId,
        batch_id: OperationId,
        declared_hash: &ContentHash,
        changes: &[CanonicalChange],
        now: TimestampMillis,
    ) -> Result<IncomingBatchAdmission, IncomingChangeError> {
        let payload_bytes = changes.iter().try_fold(0usize, |total, change| {
            total
                .checked_add(change.payload().map_or(0, |payload| payload.bytes().len()))
                .ok_or(IncomingChangeError::InvalidBatch)
        })?;
        if changes.is_empty()
            || changes.len() > MAX_INCOMING_CHANGES
            || (changes.len() > 1 && payload_bytes > MAX_INCOMING_PAYLOAD_BYTES)
        {
            return Err(IncomingChangeError::InvalidBatch);
        }
        if &canonical_batch_hash(changes) != declared_hash {
            return Err(IncomingChangeError::InvalidBatchHash);
        }
        let mut encoded = Vec::with_capacity(changes.len());
        for change in changes {
            let document = serde_json::to_vec(&StoredIncomingChange::from_change(change))
                .map_err(incoming_corrupt)?;
            if document.len() > 131_072 {
                return Err(IncomingChangeError::InvalidBatch);
            }
            encoded.push(document);
        }
        let mut connection = self.connection().map_err(incoming_storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(incoming_storage)?;
        if let Some((stored_peer, stored_hash, stored_count, state)) = transaction
            .query_row(
                "SELECT peer_device_id, batch_hash, change_count, state
                 FROM sync_incoming_batches WHERE batch_id = ?1",
                [batch_id.to_string()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()
            .map_err(incoming_storage)?
        {
            if stored_peer != peer.as_uuid().to_string()
                || stored_hash != declared_hash.as_str()
                || stored_count != i64::try_from(changes.len()).map_err(incoming_corrupt)?
            {
                return Err(IncomingChangeError::Conflict);
            }
            let state = match state.as_str() {
                "staged" => IncomingBatchState::Staged,
                "pending" => IncomingBatchState::Pending,
                "committed" => IncomingBatchState::Committed,
                _ => return Err(IncomingChangeError::Corrupt),
            };
            transaction.commit().map_err(incoming_storage)?;
            return Ok(IncomingBatchAdmission {
                state,
                created: false,
            });
        }
        transaction
            .execute(
                "INSERT INTO sync_incoming_batches (
                   batch_id, peer_device_id, batch_hash, change_count,
                   payload_bytes, state, created_at
                 ) VALUES (?1,?2,?3,?4,?5,'staged',?6)",
                params![
                    batch_id.to_string(),
                    peer.as_uuid().to_string(),
                    declared_hash.as_str(),
                    i64::try_from(changes.len()).map_err(incoming_corrupt)?,
                    i64::try_from(payload_bytes).map_err(incoming_corrupt)?,
                    now.get()
                ],
            )
            .map_err(incoming_storage)?;
        for (ordinal, (change, document)) in changes.iter().zip(encoded).enumerate() {
            transaction
                .execute(
                    "INSERT INTO sync_incoming_changes (
                       batch_id, change_id, ordinal, fingerprint, document, payload_bytes
                     ) VALUES (?1,?2,?3,?4,?5,?6)",
                    params![
                        batch_id.to_string(),
                        change.id().as_uuid().to_string(),
                        i64::try_from(ordinal).map_err(incoming_corrupt)?,
                        change.fingerprint().as_str(),
                        document,
                        change.payload().map(CanonicalPayload::bytes)
                    ],
                )
                .map_err(incoming_storage)?;
        }
        transaction.commit().map_err(incoming_storage)?;
        Ok(IncomingBatchAdmission {
            state: IncomingBatchState::Staged,
            created: true,
        })
    }

    fn apply_incoming_batch(
        &self,
        batch_id: OperationId,
        now: TimestampMillis,
    ) -> Result<IncomingBatchResult, IncomingChangeError> {
        let mut connection = self.connection().map_err(incoming_storage)?;
        let (stored_hash, expected_count, state) = connection
            .query_row(
                "SELECT batch_hash, change_count, state
                 FROM sync_incoming_batches WHERE batch_id = ?1",
                [batch_id.to_string()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(incoming_storage)?
            .ok_or(IncomingChangeError::NotFound)?;
        if state == "committed" {
            return Ok(IncomingBatchResult {
                state: IncomingBatchState::Committed,
                applied: 0,
                duplicates: usize::try_from(expected_count).map_err(incoming_corrupt)?,
                conflicts: 0,
                frontier: load_frontier(&connection).map_err(incoming_corrupt)?,
            });
        }
        if !matches!(state.as_str(), "staged" | "pending") {
            return Err(IncomingChangeError::Corrupt);
        }
        let changes = load_staged_changes(&connection, batch_id)?;
        if changes.len() != usize::try_from(expected_count).map_err(incoming_corrupt)?
            || canonical_batch_hash(&changes).as_str() != stored_hash
        {
            return Err(IncomingChangeError::Corrupt);
        }
        let mut has_unsupported_change = false;
        for change in &changes {
            has_unsupported_change |= !supported_change(change)?;
        }
        if has_unsupported_change {
            mark_batch_pending(&connection, batch_id, "unsupported_schema")?;
            return Ok(IncomingBatchResult {
                state: IncomingBatchState::Pending,
                applied: 0,
                duplicates: 0,
                conflicts: 0,
                frontier: load_frontier(&connection).map_err(incoming_corrupt)?,
            });
        }
        let mut simulated = load_frontier(&connection).map_err(incoming_corrupt)?;
        let mut duplicates = 0usize;
        for change in &changes {
            if let Some(existing) = load_change_by_id(&connection, change.id())? {
                if existing != *change {
                    return Err(IncomingChangeError::Conflict);
                }
                duplicates += 1;
                continue;
            }
            let expected = simulated
                .get(&change.origin_device())
                .copied()
                .unwrap_or(0)
                .saturating_add(1);
            if change.origin_sequence() != expected
                || change.base_frontier().iter().any(|(origin, required)| {
                    simulated.get(origin).copied().unwrap_or(0) < *required
                })
            {
                mark_batch_pending(&connection, batch_id, "causal_dependency")?;
                return Ok(IncomingBatchResult {
                    state: IncomingBatchState::Pending,
                    applied: 0,
                    duplicates,
                    conflicts: 0,
                    frontier: load_frontier(&connection).map_err(incoming_corrupt)?,
                });
            }
            simulated.insert(change.origin_device(), change.origin_sequence());
        }

        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(incoming_storage)?;
        let mut applied = 0usize;
        let mut conflicts = 0usize;
        for change in &changes {
            if load_change_by_id(&transaction, change.id())?.is_some() {
                continue;
            }
            observe_remote_clock(&transaction, change.timestamp(), now)
                .map_err(|_| IncomingChangeError::Storage)?;
            insert_change(&transaction, None, change, now)
                .map_err(|_| IncomingChangeError::Storage)?;
            let result =
                settle_or_defer(&transaction, change, now).map(|settled| settled.unwrap_or(false));
            match result {
                Ok(conflict) => {
                    applied += 1;
                    conflicts += usize::from(conflict);
                }
                Err(
                    ApplyOneError::Corrupt | ApplyOneError::Pending | ApplyOneError::Unencodable,
                ) => {
                    return Err(IncomingChangeError::Corrupt);
                }
                Err(ApplyOneError::Storage) => return Err(IncomingChangeError::Storage),
            }
        }
        retry_deferred_changes(&transaction, now).map_err(|error| match error {
            ApplyOneError::Corrupt => IncomingChangeError::Corrupt,
            _ => IncomingChangeError::Storage,
        })?;
        transaction
            .execute(
                "UPDATE sync_incoming_batches
                 SET state = 'committed', pending_reason = NULL, committed_at = ?2
                 WHERE batch_id = ?1",
                params![batch_id.to_string(), now.get()],
            )
            .map_err(incoming_storage)?;
        transaction
            .execute(
                "DELETE FROM sync_incoming_changes WHERE batch_id = ?1",
                [batch_id.to_string()],
            )
            .map_err(incoming_storage)?;
        transaction.commit().map_err(incoming_storage)?;
        if let Err(error) =
            crate::purge::run_queued_purges_on(&mut connection, &self.foreign_keys_lost, now)
        {
            tracing::warn!(%error, "received deletes stay queued");
        }
        Ok(IncomingBatchResult {
            state: IncomingBatchState::Committed,
            applied,
            duplicates,
            conflicts,
            frontier: simulated,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lettuce_characters::{
        Persona, PersonaArchiveRequest, PersonaDraftUpdate, PersonaRepository,
    };
    use lettuce_sync::persona_sync_entity;
    use lettuce_types::{PersonaId, Revision};

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
    fn local_device_identity_survives_reopen_before_the_first_change() {
        let path = std::env::temp_dir().join(format!(
            "sync-device-identity-{}.sqlite3",
            OperationId::new()
        ));
        let first_device = {
            let database = Database::open(&path).expect("database");
            let device = database
                .local_device_id(TimestampMillis::new(100))
                .expect("device identity");
            assert!(database.local_frontier().expect("frontier").is_empty());
            device
        };
        let database = Database::open(&path).expect("reopen");
        assert_eq!(
            database
                .local_device_id(TimestampMillis::new(200))
                .expect("reopened device identity"),
            first_device
        );
        let first_change = database
            .record_local_change(
                OperationId::new(),
                request(b"first"),
                TimestampMillis::new(200),
            )
            .expect("first change")
            .change;
        assert_eq!(first_change.origin_device(), first_device);
        assert_eq!(first_change.origin_sequence(), 1);
        drop(database);
        std::fs::remove_file(path).expect("remove database");
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

    fn stage_and_apply(target: &Database, change: CanonicalChange, at: i64) -> IncomingBatchState {
        let id = OperationId::new();
        target
            .stage_incoming_batch(
                SyncDeviceId::new(),
                id,
                &canonical_batch_hash(std::slice::from_ref(&change)),
                std::slice::from_ref(&change),
                TimestampMillis::new(at),
            )
            .expect("stage batch");
        target
            .apply_incoming_batch(id, TimestampMillis::new(at + 1))
            .expect("apply batch")
            .state
    }

    #[test]
    fn an_update_for_a_persona_that_predates_the_journal_materializes_its_snapshot() {
        let target = Database::open_in_memory().expect("target");
        let persona = Persona::new(
            PersonaId::new(),
            "Older".into(),
            "Created before sync".into(),
            TimestampMillis::new(10),
        )
        .expect("persona");
        let change = CanonicalChange::new(
            SyncChangeId::new(),
            SyncDeviceId::new(),
            1,
            HybridTimestamp::new(TimestampMillis::new(20), 0),
            CausalFrontier::new(),
            persona_sync_entity(persona.id).expect("entity"),
            ChangeOperation::Update,
            Some(ContentHash::parse("44".repeat(32)).expect("base hash")),
            Some(canonical_persona_payload(&persona).expect("payload")),
        )
        .expect("change");

        assert_eq!(
            stage_and_apply(&target, change, 30),
            IncomingBatchState::Committed
        );
        assert_eq!(
            PersonaRepository::get(&target, persona.id).expect("persona"),
            Some(persona)
        );
    }

    #[test]
    fn a_default_for_a_locally_archived_persona_commits_as_a_conflict() {
        let target = Database::open_in_memory().expect("target");
        let persona_id = PersonaId::new();
        let created = PersonaRepository::create(
            &target,
            Persona::new(
                persona_id,
                "Archived".into(),
                "Archived here".into(),
                TimestampMillis::new(10),
            )
            .expect("persona"),
        )
        .expect("create persona");
        PersonaRepository::archive(
            &target,
            PersonaArchiveRequest {
                persona_id,
                expected_persona_revision: created.revision,
                expected_default_revision: None,
                now: TimestampMillis::new(20),
            },
        )
        .expect("archive persona");
        let before = PersonaRepository::get_default_snapshot(&target)
            .expect("default")
            .state;
        let incoming = lettuce_characters::PersonaDefaultState {
            persona_id: Some(persona_id),
            revision: before.revision.next().expect("next revision"),
            created_at: before.created_at,
            updated_at: TimestampMillis::new(30),
        };
        let change = CanonicalChange::new(
            SyncChangeId::new(),
            SyncDeviceId::new(),
            1,
            HybridTimestamp::new(TimestampMillis::new(30), 0),
            CausalFrontier::new(),
            persona_default_sync_entity().expect("entity"),
            ChangeOperation::Update,
            Some(
                canonical_persona_default_payload(&before)
                    .expect("base")
                    .content_hash()
                    .clone(),
            ),
            Some(canonical_persona_default_payload(&incoming).expect("payload")),
        )
        .expect("change");

        assert_eq!(
            stage_and_apply(&target, change, 40),
            IncomingBatchState::Committed
        );
        assert_eq!(
            PersonaRepository::get_default_snapshot(&target)
                .expect("default")
                .state,
            before
        );
        let (count, side): (i64, String) = target
            .connection()
            .expect("connection")
            .query_row(
                "SELECT COUNT(*), MAX(winning_side) FROM sync_conflicts WHERE status = 'unresolved'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("conflicts");
        assert_eq!((count, side.as_str()), (1, "current"));

        let other = PersonaRepository::create(
            &target,
            Persona::new(
                PersonaId::new(),
                "Active".into(),
                "Still active".into(),
                TimestampMillis::new(50),
            )
            .expect("persona"),
        )
        .expect("create active persona");
        PersonaRepository::set_default(
            &target,
            other.id,
            before.revision,
            TimestampMillis::new(60),
        )
        .expect("local default change");
        let status: String = target
            .connection()
            .expect("connection")
            .query_row("SELECT resolution_choice FROM sync_conflicts", [], |row| {
                row.get(0)
            })
            .expect("conflict status");
        assert_eq!(status, "superseded");
    }

    #[test]
    fn local_edits_after_a_lower_remote_winner_get_fresh_operation_ids() {
        let source = Database::open_in_memory().expect("source");
        let target = Database::open_in_memory().expect("target");
        let send = |from: &Database, to: &Database, at: i64| {
            let batch = from
                .outbound_changes(
                    &to.local_frontier().expect("frontier"),
                    MAX_OUTBOUND_CHANGES,
                    MAX_OUTBOUND_PAYLOAD_BYTES,
                )
                .expect("outbound");
            for change in batch.changes {
                stage_and_apply(to, change, at);
            }
        };
        let draft = |title: &str| PersonaDraftUpdate {
            title: title.into(),
            description: "Edited".into(),
            nickname: None,
            design_description: None,
            avatar_crop: None,
            image_recommendation: None,
        };
        let persona_id = PersonaId::new();
        PersonaRepository::create(
            &source,
            Persona::new(
                persona_id,
                "Shared".into(),
                "Both".into(),
                TimestampMillis::new(1),
            )
            .expect("persona"),
        )
        .expect("create");
        send(&source, &target, 2);
        let first = PersonaRepository::revise(
            &target,
            persona_id,
            Revision::INITIAL,
            draft("Local one"),
            TimestampMillis::new(10),
        )
        .expect("first local edit");
        PersonaRepository::revise(
            &target,
            persona_id,
            first.revision,
            draft("Local two"),
            TimestampMillis::new(11),
        )
        .expect("second local edit");
        std::thread::sleep(std::time::Duration::from_millis(5));
        PersonaRepository::revise(
            &source,
            persona_id,
            Revision::INITIAL,
            draft("Remote"),
            TimestampMillis::new(100),
        )
        .expect("remote edit");
        send(&source, &target, 200);
        let current = PersonaRepository::get(&target, persona_id)
            .expect("persona")
            .expect("present");
        assert_eq!(current.title, "Remote");
        assert_eq!(current.revision, first.revision);

        let edited = PersonaRepository::revise(
            &target,
            persona_id,
            current.revision,
            draft("Local three"),
            TimestampMillis::new(300),
        )
        .expect("edit after the remote winner");
        assert_eq!(edited.title, "Local three");
    }

    #[test]
    fn a_restored_old_backup_loses_to_the_newer_peer_edit() {
        use lettuce_transfer::{ProviderBackupRestoreWriter, ProviderBackupSource};

        let device = Database::open_in_memory().expect("device");
        let peer = Database::open_in_memory().expect("peer");
        let send = |from: &Database, to: &Database, at: i64| {
            let batch = from
                .outbound_changes(
                    &to.local_frontier().expect("frontier"),
                    MAX_OUTBOUND_CHANGES,
                    MAX_OUTBOUND_PAYLOAD_BYTES,
                )
                .expect("outbound");
            for change in batch.changes {
                stage_and_apply(to, change, at);
            }
        };
        let persona_id = PersonaId::new();
        PersonaRepository::create(
            &device,
            Persona::new(
                persona_id,
                "Backed up".into(),
                "Old".into(),
                TimestampMillis::new(1),
            )
            .expect("persona"),
        )
        .expect("create");
        send(&device, &peer, 2);
        let mut backup = device.read_provider_backup_graph().expect("backup");
        lettuce_transfer::canonicalize_and_validate(&mut backup).expect("canonical backup");
        PersonaRepository::revise(
            &peer,
            persona_id,
            Revision::INITIAL,
            PersonaDraftUpdate {
                title: "Peer edit".into(),
                description: "Newer".into(),
                nickname: None,
                design_description: None,
                avatar_crop: None,
                image_recommendation: None,
            },
            TimestampMillis::new(100),
        )
        .expect("peer edit");

        let restored = Database::open_in_memory().expect("restored");
        restored
            .restore_provider_backup_graph(&backup, &[])
            .expect("restore");
        restored
            .journal_current_state(TimestampMillis::now().expect("clock"))
            .expect("scan the restored state");
        send(&restored, &peer, 300);
        send(&peer, &restored, 400);

        for database in [&peer, &restored] {
            let persona = PersonaRepository::get(database, persona_id)
                .expect("persona")
                .expect("present");
            assert_eq!(persona.title, "Peer edit");
        }
    }

    #[test]
    fn memory_soul_and_relationship_snapshots_carry_their_latest_change_time() {
        let memory_id = lettuce_types::MemoryId::new();
        let item = lettuce_memory::MemoryItem {
            id: memory_id,
            short_id: lettuce_memory::MemoryShortId::derived(memory_id),
            text: "Likes tea".into(),
            category: lettuce_memory::MemoryCategory::Other,
            source_message_id: None,
            source_role: None,
            observed_at: None,
            observed_time_precision: None,
            superseded_by: None,
            superseded_at: Some(TimestampMillis::new(20)),
            supersedes: Vec::new(),
            token_count: 3,
            is_cold: false,
            is_pinned: false,
            importance: lettuce_memory::Score::FULL,
            persistence_importance: lettuce_memory::Score::FULL,
            prompt_importance: lettuce_memory::Score::FULL,
            volatility: lettuce_memory::Score::LEGACY_VOLATILITY,
            access_count: 1,
            created_at: TimestampMillis::new(5),
            last_accessed_at: TimestampMillis::new(30),
        };
        assert_eq!(
            snapshot_source_time(
                lettuce_sync::MEMORY_ITEM_SYNC_KIND,
                &serde_json::to_vec(&item).expect("memory item")
            ),
            Some(TimestampMillis::new(30))
        );
        let soul = serde_json::json!([
            {"id": "a", "category": "likes", "value": "tea", "policy": "current", "slot": "drink", "validFrom": 40, "createdAt": 10},
            {"id": "b", "category": "likes", "value": "rain", "policy": "current", "slot": "weather", "validFrom": 15, "createdAt": 12, "supersededAt": 50}
        ]);
        assert_eq!(
            snapshot_source_time(
                lettuce_sync::COMPANION_SOUL_SYNC_KIND,
                &serde_json::to_vec(&soul).expect("soul")
            ),
            Some(TimestampMillis::new(50))
        );
        let relationship = crate::sync::companion_sync_adapter::SyncCompanionRelationship {
            character_id: lettuce_types::CharacterId::new(),
            persona_id: None,
            state: lettuce_companions::RelationshipState {
                closeness: 0.1,
                trust: 0.1,
                affection: 0.1,
                tension: 0.0,
                stability: 0.5,
                interaction_count: 3,
                last_interaction_at: TimestampMillis::new(44),
            },
        };
        assert_eq!(
            snapshot_source_time(
                lettuce_sync::COMPANION_RELATIONSHIP_SYNC_KIND,
                &serde_json::to_vec(&relationship).expect("relationship")
            ),
            Some(TimestampMillis::new(44))
        );
    }

    #[test]
    fn an_entity_too_large_to_sync_is_reported_once_until_dismissed() {
        let database = Database::open_in_memory().expect("database");
        let oversized = CanonicalPayload::new(
            "lorebook.snapshot",
            1,
            vec![b'x'; lettuce_sync::MAX_CANONICAL_PAYLOAD_BYTES + 1],
        )
        .map_err(payload_error);
        assert!(matches!(oversized, Err(ApplyOneError::Unencodable)));
        {
            let connection = database.connection().expect("connection");
            for at in [10, 20] {
                assert!(
                    record_not_synced(&connection, "lorebook", "book", TimestampMillis::new(at))
                        .is_ok()
                );
            }
        }
        let notices = database.purge_notices().expect("notices");
        assert_eq!(notices.len(), 1);
        assert_eq!(notices[0].reason, crate::PurgeNoticeReason::NotSynced);
        assert_eq!(notices[0].entity_id, "lorebook/book");
        database
            .dismiss_purge_notice(notices[0].id, TimestampMillis::new(30))
            .expect("dismiss");
        let connection = database.connection().expect("connection");
        assert!(
            record_not_synced(&connection, "lorebook", "book", TimestampMillis::new(40)).is_ok()
        );
        drop(connection);
        assert_eq!(database.purge_notices().expect("notices").len(), 1);
    }

    #[test]
    fn a_losing_remote_change_keeps_local_retries_replayable() {
        let source = Database::open_in_memory().expect("source");
        let target = Database::open_in_memory().expect("target");
        let persona_id = PersonaId::new();
        PersonaRepository::create(
            &source,
            Persona::new(
                persona_id,
                "Shared".into(),
                "Both".into(),
                TimestampMillis::new(1),
            )
            .expect("persona"),
        )
        .expect("create");
        for change in source
            .outbound_changes(
                &CausalFrontier::new(),
                MAX_OUTBOUND_CHANGES,
                MAX_OUTBOUND_PAYLOAD_BYTES,
            )
            .expect("insert")
            .changes
        {
            stage_and_apply(&target, change, 2);
        }
        let draft = |title: &str| PersonaDraftUpdate {
            title: title.into(),
            description: "Edited".into(),
            nickname: None,
            design_description: None,
            avatar_crop: None,
            image_recommendation: None,
        };
        PersonaRepository::revise(
            &source,
            persona_id,
            Revision::INITIAL,
            draft("Earlier remote"),
            TimestampMillis::new(5),
        )
        .expect("remote edit");
        std::thread::sleep(std::time::Duration::from_millis(5));
        let local = PersonaRepository::revise(
            &target,
            persona_id,
            Revision::INITIAL,
            draft("Later local"),
            TimestampMillis::new(10),
        )
        .expect("local edit");
        let frontier = target.local_frontier().expect("frontier");
        for change in source
            .outbound_changes(&frontier, MAX_OUTBOUND_CHANGES, MAX_OUTBOUND_PAYLOAD_BYTES)
            .expect("remote edit batch")
            .changes
        {
            stage_and_apply(&target, change, 20);
        }
        assert_eq!(
            PersonaRepository::get(&target, persona_id)
                .expect("persona")
                .expect("present")
                .title,
            "Later local"
        );

        let retry = PersonaRepository::revise(
            &target,
            persona_id,
            Revision::INITIAL,
            draft("Later local"),
            TimestampMillis::new(10),
        )
        .expect("exact retry replays");
        assert_eq!(retry, local);
    }

    #[test]
    fn a_peer_relays_changes_it_received_from_a_third_device() {
        let paths = ["a", "b", "c"].map(|name| {
            std::env::temp_dir().join(format!("sync-relay-{name}-{}.sqlite3", OperationId::new()))
        });
        let [a, b, c] = paths
            .each_ref()
            .map(|path| Database::open(path).expect("database"));
        let transfer = |from: &Database, to: &Database, at: i64| {
            let batch = from
                .outbound_changes(
                    &to.local_frontier().expect("peer frontier"),
                    MAX_OUTBOUND_CHANGES,
                    MAX_OUTBOUND_PAYLOAD_BYTES,
                )
                .expect("outbound batch");
            let id = OperationId::new();
            to.stage_incoming_batch(
                SyncDeviceId::new(),
                id,
                &canonical_batch_hash(&batch.changes),
                &batch.changes,
                TimestampMillis::new(at),
            )
            .expect("stage batch");
            let applied = to
                .apply_incoming_batch(id, TimestampMillis::new(at + 1))
                .expect("apply batch");
            assert_eq!(applied.state, IncomingBatchState::Committed);
            batch.changes
        };
        let from_a = PersonaId::new();
        PersonaRepository::create(
            &a,
            Persona::new(from_a, "A".into(), "First".into(), TimestampMillis::new(10))
                .expect("persona"),
        )
        .expect("create on a");
        transfer(&a, &b, 20);
        let from_b = PersonaId::new();
        PersonaRepository::create(
            &b,
            Persona::new(
                from_b,
                "B".into(),
                "Second".into(),
                TimestampMillis::new(30),
            )
            .expect("persona"),
        )
        .expect("create on b");

        let relayed = transfer(&b, &c, 40);

        let a_device = a
            .local_frontier()
            .expect("a frontier")
            .into_keys()
            .next()
            .expect("a device");
        assert_eq!(relayed.len(), 2);
        assert_eq!(relayed[0].origin_device(), a_device);
        assert_ne!(relayed[1].origin_device(), a_device);
        assert!(
            PersonaRepository::get(&c, from_a)
                .expect("relayed")
                .is_some()
        );
        assert!(
            PersonaRepository::get(&c, from_b)
                .expect("direct")
                .is_some()
        );
        assert_eq!(
            c.local_frontier().expect("c"),
            b.local_frontier().expect("b")
        );
        assert!(
            b.outbound_changes(
                &c.local_frontier().expect("c"),
                MAX_OUTBOUND_CHANGES,
                MAX_OUTBOUND_PAYLOAD_BYTES,
            )
            .expect("caught up")
            .changes
            .is_empty()
        );
        drop((a, b, c));
        for path in paths {
            std::fs::remove_file(path).expect("remove test database");
        }
    }

    #[test]
    fn incoming_persona_batches_apply_replay_and_preserve_pending_conflicts() {
        let source_path = std::env::temp_dir().join(format!(
            "sync-incoming-source-{}.sqlite3",
            OperationId::new()
        ));
        let target_path = std::env::temp_dir().join(format!(
            "sync-incoming-target-{}.sqlite3",
            OperationId::new()
        ));
        let peer = SyncDeviceId::new();
        let persona_id = PersonaId::new();
        let source = Database::open(&source_path).expect("source database");
        let created = PersonaRepository::create(
            &source,
            Persona::new(
                persona_id,
                "Writer".into(),
                "Writes careful prose".into(),
                TimestampMillis::new(10),
            )
            .expect("persona"),
        )
        .expect("create source persona");
        PersonaRepository::set_default(
            &source,
            persona_id,
            Revision::INITIAL,
            TimestampMillis::new(20),
        )
        .expect("set source default");
        let revised = PersonaRepository::revise(
            &source,
            persona_id,
            created.revision,
            PersonaDraftUpdate {
                title: "Editor".into(),
                description: "Edits careful prose".into(),
                nickname: Some("E".into()),
                design_description: None,
                avatar_crop: None,
                image_recommendation: None,
            },
            TimestampMillis::new(30),
        )
        .expect("revise source persona");
        let archived = PersonaRepository::archive(
            &source,
            PersonaArchiveRequest {
                persona_id,
                expected_persona_revision: revised.revision,
                expected_default_revision: Some(Revision::new(2)),
                now: TimestampMillis::new(40),
            },
        )
        .expect("archive source persona");
        let initial = source
            .outbound_changes(
                &CausalFrontier::new(),
                MAX_OUTBOUND_CHANGES,
                MAX_OUTBOUND_PAYLOAD_BYTES,
            )
            .expect("initial outbound batch");
        assert_eq!(initial.changes.len(), 5);

        let initial_batch_id = OperationId::new();
        let initial_hash = canonical_batch_hash(&initial.changes);
        {
            let target = Database::open(&target_path).expect("target database");
            assert!(
                target
                    .stage_incoming_batch(
                        peer,
                        initial_batch_id,
                        &initial_hash,
                        &initial.changes,
                        TimestampMillis::new(50),
                    )
                    .expect("stage initial batch")
                    .created
            );
        }
        let target = Database::open(&target_path).expect("reopen target database");
        let applied = target
            .apply_incoming_batch(initial_batch_id, TimestampMillis::new(51))
            .expect("apply initial batch");
        assert_eq!(applied.state, IncomingBatchState::Committed);
        assert_eq!(applied.applied, 5);
        assert_eq!(applied.conflicts, 0);
        assert_eq!(
            PersonaRepository::get(&target, persona_id).expect("target persona"),
            Some(archived.persona.clone())
        );
        assert_eq!(
            PersonaRepository::get_default_snapshot(&target)
                .expect("target default")
                .state,
            archived.default
        );
        let replay = target
            .apply_incoming_batch(initial_batch_id, TimestampMillis::new(52))
            .expect("replay initial batch");
        assert_eq!(replay.state, IncomingBatchState::Committed);
        assert_eq!(replay.duplicates, 5);
        let staged_replay = target
            .stage_incoming_batch(
                peer,
                initial_batch_id,
                &initial_hash,
                &initial.changes,
                TimestampMillis::new(53),
            )
            .expect("replay staged input");
        assert_eq!(staged_replay.state, IncomingBatchState::Committed);
        assert!(!staged_replay.created);

        let target_restored = PersonaRepository::restore(
            &target,
            persona_id,
            archived.persona.revision,
            TimestampMillis::new(60),
        )
        .expect("restore target persona");
        let source_restored = PersonaRepository::restore(
            &source,
            persona_id,
            archived.persona.revision,
            TimestampMillis::new(100),
        )
        .expect("restore source persona");
        assert_ne!(target_restored, source_restored);
        let source_device = initial.changes[0].origin_device();
        let (&target_device, _) = target
            .local_frontier()
            .expect("target frontier")
            .iter()
            .find(|(device, _)| {
                !initial
                    .changes
                    .iter()
                    .any(|change| change.origin_device() == **device)
            })
            .expect("target device");
        let target_restore_batch = target
            .outbound_changes(
                &CausalFrontier::from([(source_device, 6)]),
                MAX_OUTBOUND_CHANGES,
                MAX_OUTBOUND_PAYLOAD_BYTES,
            )
            .expect("target restore batch");
        assert_eq!(target_restore_batch.changes.len(), 1);
        let target_restore_batch_id = OperationId::new();
        source
            .stage_incoming_batch(
                peer,
                target_restore_batch_id,
                &canonical_batch_hash(&target_restore_batch.changes),
                &target_restore_batch.changes,
                TimestampMillis::new(100),
            )
            .expect("stage target restore");
        assert_eq!(
            source
                .apply_incoming_batch(target_restore_batch_id, TimestampMillis::new(101))
                .expect("apply target restore")
                .conflicts,
            1
        );
        let conflict_batch = source
            .outbound_changes(
                &CausalFrontier::from([(source_device, 5), (target_device, 1)]),
                MAX_OUTBOUND_CHANGES,
                MAX_OUTBOUND_PAYLOAD_BYTES,
            )
            .expect("conflicting outbound batch");
        assert_eq!(conflict_batch.changes.len(), 1);
        let conflict_batch_id = OperationId::new();
        target
            .stage_incoming_batch(
                peer,
                conflict_batch_id,
                &canonical_batch_hash(&conflict_batch.changes),
                &conflict_batch.changes,
                TimestampMillis::new(101),
            )
            .expect("stage conflict");
        let conflict = target
            .apply_incoming_batch(conflict_batch_id, TimestampMillis::new(102))
            .expect("apply conflict");
        assert_eq!(conflict.state, IncomingBatchState::Committed);
        assert_eq!(conflict.conflicts, 1);
        assert_eq!(
            PersonaRepository::get(&target, persona_id).expect("target winner"),
            Some(source_restored.clone())
        );

        let target_conflicts = target
            .unresolved_persona_conflicts(MAX_UNRESOLVED_CONFLICTS)
            .expect("target conflicts");
        assert_eq!(target_conflicts.len(), 1);
        assert_eq!(
            target.resolve_persona_conflict(
                target_conflicts[0].id,
                None,
                ConflictChoice::Other,
                OperationId::new(),
                TimestampMillis::new(129),
            ),
            Err(ConflictRepositoryError::Conflict)
        );
        let first_resolution_id = OperationId::new();
        let first_resolution = target
            .resolve_persona_conflict(
                target_conflicts[0].id,
                target_conflicts[0].current.change_id,
                ConflictChoice::Other,
                first_resolution_id,
                TimestampMillis::new(130),
            )
            .expect("choose other persona");
        drop(target);
        let target = Database::open(&target_path).expect("reopen resolved target");
        assert_eq!(
            target
                .resolve_persona_conflict(
                    target_conflicts[0].id,
                    target_conflicts[0].current.change_id,
                    ConflictChoice::Other,
                    first_resolution_id,
                    TimestampMillis::new(999),
                )
                .expect("replay persona resolution"),
            first_resolution
        );
        assert_eq!(
            target.resolve_persona_conflict(
                target_conflicts[0].id,
                target_conflicts[0].current.change_id,
                ConflictChoice::Current,
                first_resolution_id,
                TimestampMillis::new(999),
            ),
            Err(ConflictRepositoryError::Conflict)
        );
        assert!(
            target
                .unresolved_persona_conflicts(MAX_UNRESOLVED_CONFLICTS)
                .expect("resolved target conflicts")
                .is_empty()
        );
        let first_resolution_batch = target
            .outbound_changes(
                &CausalFrontier::from([(source_device, 6), (target_device, 1)]),
                MAX_OUTBOUND_CHANGES,
                MAX_OUTBOUND_PAYLOAD_BYTES,
            )
            .expect("persona resolution batch");
        assert_eq!(first_resolution_batch.changes, vec![first_resolution]);
        let first_resolution_batch_id = OperationId::new();
        source
            .stage_incoming_batch(
                peer,
                first_resolution_batch_id,
                &canonical_batch_hash(&first_resolution_batch.changes),
                &first_resolution_batch.changes,
                TimestampMillis::new(131),
            )
            .expect("stage persona resolution");
        source
            .apply_incoming_batch(first_resolution_batch_id, TimestampMillis::new(132))
            .expect("apply persona resolution");
        assert!(
            source
                .unresolved_persona_conflicts(MAX_UNRESOLVED_CONFLICTS)
                .expect("superseded source conflicts")
                .is_empty()
        );

        PersonaRepository::set_default(
            &target,
            persona_id,
            Revision::new(3),
            TimestampMillis::new(200),
        )
        .expect("set target default");
        PersonaRepository::set_default(
            &source,
            persona_id,
            Revision::new(3),
            TimestampMillis::new(210),
        )
        .expect("set source default");
        let target_default_batch = target
            .outbound_changes(
                &CausalFrontier::from([(source_device, 6), (target_device, 2)]),
                MAX_OUTBOUND_CHANGES,
                MAX_OUTBOUND_PAYLOAD_BYTES,
            )
            .expect("target default batch");
        let target_default_batch_id = OperationId::new();
        source
            .stage_incoming_batch(
                peer,
                target_default_batch_id,
                &canonical_batch_hash(&target_default_batch.changes),
                &target_default_batch.changes,
                TimestampMillis::new(211),
            )
            .expect("stage target default");
        source
            .apply_incoming_batch(target_default_batch_id, TimestampMillis::new(212))
            .expect("apply target default");
        let source_default_batch = source
            .outbound_changes(
                &CausalFrontier::from([(source_device, 6), (target_device, 2)]),
                MAX_OUTBOUND_CHANGES,
                MAX_OUTBOUND_PAYLOAD_BYTES,
            )
            .expect("source default batch");
        let source_default_batch_id = OperationId::new();
        target
            .stage_incoming_batch(
                peer,
                source_default_batch_id,
                &canonical_batch_hash(&source_default_batch.changes),
                &source_default_batch.changes,
                TimestampMillis::new(213),
            )
            .expect("stage source default");
        target
            .apply_incoming_batch(source_default_batch_id, TimestampMillis::new(214))
            .expect("apply source default");
        let default_conflicts = target
            .unresolved_persona_conflicts(MAX_UNRESOLVED_CONFLICTS)
            .expect("default conflicts");
        assert_eq!(default_conflicts.len(), 1);
        let second_resolution_id = OperationId::new();
        let second_resolution = target
            .resolve_persona_conflict(
                default_conflicts[0].id,
                default_conflicts[0].current.change_id,
                ConflictChoice::Current,
                second_resolution_id,
                TimestampMillis::new(220),
            )
            .expect("choose current default");
        let second_resolution_batch = target
            .outbound_changes(
                &CausalFrontier::from([(source_device, 7), (target_device, 3)]),
                MAX_OUTBOUND_CHANGES,
                MAX_OUTBOUND_PAYLOAD_BYTES,
            )
            .expect("default resolution batch");
        assert_eq!(second_resolution_batch.changes, vec![second_resolution]);
        let second_resolution_batch_id = OperationId::new();
        source
            .stage_incoming_batch(
                peer,
                second_resolution_batch_id,
                &canonical_batch_hash(&second_resolution_batch.changes),
                &second_resolution_batch.changes,
                TimestampMillis::new(221),
            )
            .expect("stage default resolution");
        source
            .apply_incoming_batch(second_resolution_batch_id, TimestampMillis::new(222))
            .expect("apply default resolution");
        assert!(
            source
                .unresolved_persona_conflicts(MAX_UNRESOLVED_CONFLICTS)
                .expect("resolved source default conflict")
                .is_empty()
        );

        let future_origin = SyncDeviceId::new();
        let future_change = CanonicalChange::new(
            SyncChangeId::new(),
            future_origin,
            1,
            HybridTimestamp::new(TimestampMillis::new(110), 0),
            CausalFrontier::new(),
            persona_sync_entity(persona_id).expect("future entity"),
            ChangeOperation::Update,
            Some(ContentHash::parse("11".repeat(32)).expect("base hash")),
            Some(
                CanonicalPayload::new("persona.future", 2, b"future".to_vec())
                    .expect("future payload"),
            ),
        )
        .expect("future change");
        let future_batch_id = OperationId::new();
        target
            .stage_incoming_batch(
                peer,
                future_batch_id,
                &canonical_batch_hash(std::slice::from_ref(&future_change)),
                std::slice::from_ref(&future_change),
                TimestampMillis::new(111),
            )
            .expect("stage future change");
        assert_eq!(
            target
                .apply_incoming_batch(future_batch_id, TimestampMillis::new(112))
                .expect("retain future change")
                .state,
            IncomingBatchState::Pending
        );

        let missing_change = CanonicalChange::new(
            SyncChangeId::new(),
            SyncDeviceId::new(),
            2,
            HybridTimestamp::new(TimestampMillis::new(120), 0),
            CausalFrontier::new(),
            persona_sync_entity(persona_id).expect("missing entity"),
            ChangeOperation::Update,
            Some(ContentHash::parse("22".repeat(32)).expect("base hash")),
            Some(canonical_persona_payload(&target_restored).expect("missing payload")),
        )
        .expect("missing change");
        let missing_batch_id = OperationId::new();
        target
            .stage_incoming_batch(
                peer,
                missing_batch_id,
                &canonical_batch_hash(std::slice::from_ref(&missing_change)),
                std::slice::from_ref(&missing_change),
                TimestampMillis::new(121),
            )
            .expect("stage missing sequence");
        assert_eq!(
            target
                .apply_incoming_batch(missing_batch_id, TimestampMillis::new(122))
                .expect("retain missing sequence")
                .state,
            IncomingBatchState::Pending
        );

        let resolution_choices = target
            .connection()
            .expect("connection")
            .prepare(
                "SELECT resolution_choice
                 FROM sync_conflicts
                 WHERE status = 'resolved'
                 ORDER BY resolution_choice ASC",
            )
            .expect("prepare resolved conflicts")
            .query_map([], |row| row.get::<_, String>(0))
            .expect("query resolved conflicts")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect resolved conflicts");
        assert_eq!(resolution_choices, vec!["current", "other"]);
        drop(target);
        drop(source);
        std::fs::remove_file(target_path).expect("remove target database");
        std::fs::remove_file(source_path).expect("remove source database");
    }
}
