use std::collections::BTreeMap;

use lettuce_characters::{Persona, PersonaDefaultState, RepositoryError};
use lettuce_sync::{
    CANONICAL_CHANGE_VERSION, CanonicalChange, CanonicalPayload, CausalFrontier, ChangeOperation,
    ConflictChoice, ConflictRepositoryError, HybridTimestamp, IncomingBatchAdmission,
    IncomingBatchResult, IncomingBatchState, IncomingChangeError, IncomingChangeRepository,
    LocalChangeAdmission, LocalChangeJournal, LocalChangeJournalError, MAX_FRONTIER_DEVICES,
    MAX_INCOMING_CHANGES, MAX_INCOMING_PAYLOAD_BYTES, MAX_OUTBOUND_CHANGES,
    MAX_OUTBOUND_PAYLOAD_BYTES, MAX_UNRESOLVED_CONFLICTS, NewCanonicalChange, OutboundChangeBatch,
    PERSONA_DEFAULT_SYNC_SCHEMA, PERSONA_DEFAULT_SYNC_VERSION, PERSONA_SYNC_SCHEMA,
    PERSONA_SYNC_VERSION, PersonaConflict, PersonaConflictCandidate, PersonaConflictRepository,
    PersonaConflictValue, SyncChangeId, SyncDeviceId, SyncEntity, canonical_batch_hash,
    canonical_persona_default_payload, canonical_persona_payload, persona_default_sync_entity,
    persona_sync_entity,
};
use lettuce_types::{ContentHash, OperationId, PersonaId, TimestampMillis};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::Database;
use crate::persona_adapter::{
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
    record_local_change_in_skipping(connection, operation_id, request, now, None)
}

fn record_local_change_in_skipping(
    connection: &Connection,
    operation_id: OperationId,
    request: &NewCanonicalChange,
    now: TimestampMillis,
    skipped_conflict: Option<OperationId>,
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
        return Ok(false);
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
                || change.operation() != ChangeOperation::Update
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
    Storage,
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
             WHERE status = 'unresolved' AND entity_kind = ?1 AND entity_id = ?2
               AND current_change_id IS NOT NULL",
        )
        .map_err(|_| ApplyOneError::Storage)?;
    let rows = statement
        .query_map(
            params![winner.entity().kind(), winner.entity().id()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
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
        let current_id = SyncChangeId::from_uuid(
            Uuid::parse_str(&current_id).map_err(|_| ApplyOneError::Corrupt)?,
        );
        let incoming_id = SyncChangeId::from_uuid(
            Uuid::parse_str(&incoming_id).map_err(|_| ApplyOneError::Corrupt)?,
        );
        let current = load_change_by_id(connection, current_id)
            .map_err(|error| match error {
                IncomingChangeError::Storage => ApplyOneError::Storage,
                _ => ApplyOneError::Corrupt,
            })?
            .ok_or(ApplyOneError::Corrupt)?;
        let incoming = load_change_by_id(connection, incoming_id)
            .map_err(|error| match error {
                IncomingChangeError::Storage => ApplyOneError::Storage,
                _ => ApplyOneError::Corrupt,
            })?
            .ok_or(ApplyOneError::Corrupt)?;
        if (winner.id() == current.id() || winner.observes(&current))
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

fn apply_persona_change(
    tx: &Transaction<'_>,
    change: &CanonicalChange,
    now: TimestampMillis,
) -> Result<bool, ApplyOneError> {
    let payload = change.payload().ok_or(ApplyOneError::Corrupt)?;
    let incoming: Persona =
        serde_json::from_slice(payload.bytes()).map_err(|_| ApplyOneError::Corrupt)?;
    let id = change
        .entity()
        .id()
        .parse::<PersonaId>()
        .map_err(|_| ApplyOneError::Corrupt)?;
    if incoming.id != id {
        return Err(ApplyOneError::Corrupt);
    }
    let current = load_persona(tx, id).map_err(|_| ApplyOneError::Storage)?;
    let current_payload = current
        .as_ref()
        .map(canonical_persona_payload)
        .transpose()
        .map_err(|_| ApplyOneError::Corrupt)?;
    let same = current_payload
        .as_ref()
        .is_some_and(|value| value.content_hash() == payload.content_hash());
    let clean_update = current_payload.as_ref().is_some_and(|value| {
        change.operation() == ChangeOperation::Update
            && change.base_revision() == Some(value.content_hash())
    });
    let clean_insert = current.is_none() && change.operation() == ChangeOperation::Insert;
    if current.is_none() && !clean_insert {
        return Err(ApplyOneError::Pending);
    }
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
    let winner_is_incoming = !conflict
        || current_change
            .as_ref()
            .is_some_and(|current| incoming_wins(change, current));
    observe_remote_clock(tx, change.timestamp(), now).map_err(|_| ApplyOneError::Storage)?;
    insert_change(tx, None, change, now).map_err(|_| ApplyOneError::Storage)?;
    if winner_is_incoming && !same {
        apply_synced_persona(tx, incoming).map_err(repository_apply_error)?;
    }
    if conflict {
        insert_conflict(
            tx,
            change,
            current_change.as_ref(),
            current_payload.as_ref().map(CanonicalPayload::bytes),
            payload.bytes(),
            winner_is_incoming,
            now,
        )?;
    }
    resolve_dominated_conflicts(tx, change, now, None)?;
    Ok(conflict)
}

fn apply_persona_default_change(
    tx: &Transaction<'_>,
    change: &CanonicalChange,
    now: TimestampMillis,
) -> Result<bool, ApplyOneError> {
    let payload = change.payload().ok_or(ApplyOneError::Corrupt)?;
    let incoming: PersonaDefaultState =
        serde_json::from_slice(payload.bytes()).map_err(|_| ApplyOneError::Corrupt)?;
    let current = read_default(tx).map_err(|_| ApplyOneError::Storage)?;
    let current_payload =
        canonical_persona_default_payload(&current).map_err(|_| ApplyOneError::Corrupt)?;
    let same = current_payload.content_hash() == payload.content_hash();
    let clean_update = change.base_revision() == Some(current_payload.content_hash());
    let conflict = !same && !clean_update;
    let current_change =
        load_materialized_change(tx, change.entity(), current_payload.content_hash()).map_err(
            |error| match error {
                IncomingChangeError::Storage => ApplyOneError::Storage,
                _ => ApplyOneError::Corrupt,
            },
        )?;
    let winner_is_incoming = !conflict
        || current_change
            .as_ref()
            .is_some_and(|current| incoming_wins(change, current));
    observe_remote_clock(tx, change.timestamp(), now).map_err(|_| ApplyOneError::Storage)?;
    insert_change(tx, None, change, now).map_err(|_| ApplyOneError::Storage)?;
    if winner_is_incoming && !same {
        apply_synced_persona_default(tx, incoming).map_err(repository_apply_error)?;
    }
    if conflict {
        insert_conflict(
            tx,
            change,
            current_change.as_ref(),
            Some(current_payload.bytes()),
            payload.bytes(),
            winner_is_incoming,
            now,
        )?;
    }
    resolve_dominated_conflicts(tx, change, now, None)?;
    Ok(conflict)
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
                 FROM sync_conflicts WHERE status = 'unresolved'
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
            || payload_bytes > MAX_INCOMING_PAYLOAD_BYTES
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
            let result = match change.entity().kind() {
                "persona" => apply_persona_change(&transaction, change, now),
                "persona_default" => apply_persona_default_change(&transaction, change, now),
                _ => Err(ApplyOneError::Corrupt),
            };
            match result {
                Ok(conflict) => {
                    applied += 1;
                    conflicts += usize::from(conflict);
                }
                Err(ApplyOneError::Pending) => {
                    drop(transaction);
                    mark_batch_pending(&connection, batch_id, "materialization_dependency")?;
                    return Ok(IncomingBatchResult {
                        state: IncomingBatchState::Pending,
                        applied: 0,
                        duplicates,
                        conflicts: 0,
                        frontier: load_frontier(&connection).map_err(incoming_corrupt)?,
                    });
                }
                Err(ApplyOneError::Corrupt) => return Err(IncomingChangeError::Corrupt),
                Err(ApplyOneError::Storage) => return Err(IncomingChangeError::Storage),
            }
        }
        transaction
            .execute(
                "UPDATE sync_incoming_batches
                 SET state = 'committed', pending_reason = NULL, committed_at = ?2
                 WHERE batch_id = ?1",
                params![batch_id.to_string(), now.get()],
            )
            .map_err(incoming_storage)?;
        transaction.commit().map_err(incoming_storage)?;
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
                &CausalFrontier::from([(source_device, 5)]),
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
