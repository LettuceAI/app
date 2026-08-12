use std::collections::BTreeMap;

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

const USER_RESOLVABLE_TABLES: &[&str] = &[
    "characters",
    "personas",
    "lorebooks",
    "lorebook_entries",
    "prompt_templates",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
enum StoredValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredRow {
    columns: Vec<String>,
    primary_key: Vec<StoredValue>,
    values: Option<Vec<StoredValue>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictField {
    pub key: String,
    pub current: serde_json::Value,
    pub other: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncConflict {
    pub conflict_id: String,
    pub entity_type: String,
    pub entity_name: String,
    pub operation: String,
    pub detected_at: i64,
    pub current_change_id: String,
    pub current_device_id: String,
    pub current_timestamp: i64,
    pub other_change_id: String,
    pub other_device_id: String,
    pub other_timestamp: i64,
    pub fields: Vec<ConflictField>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConflictChoice {
    Current,
    Other,
}

#[derive(Debug, thiserror::Error)]
pub enum ConflictError {
    #[error("sync v2 database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("conflict snapshot is invalid: {0}")]
    InvalidSnapshot(String),
    #[error("conflict {0} was not found")]
    NotFound(String),
    #[error("conflicts for table {0} require automatic resolution")]
    NotUserResolvable(String),
    #[error("conflict {0} is already resolved")]
    AlreadyResolved(String),
    #[error("resolving conflict did not create a tracked revision")]
    NoRevision,
}

pub(crate) fn encode_row_snapshot(
    columns: &[String],
    primary_key: &[Value],
    values: Option<&[Value]>,
) -> Result<Vec<u8>, rusqlite::Error> {
    let stored = StoredRow {
        columns: columns.to_vec(),
        primary_key: primary_key.iter().map(StoredValue::from).collect(),
        values: values.map(|values| values.iter().map(StoredValue::from).collect()),
    };
    bincode::serialize(&stored)
        .map_err(|error| rusqlite::Error::InvalidParameterName(error.to_string()))
}

pub fn list_unresolved_conflicts(
    conn: &Connection,
) -> Result<Vec<SyncConflict>, ConflictError> {
    let mut statement = conn.prepare(
        "SELECT c.conflict_id, c.table_name, c.operation, c.detected_at,
                c.local_change_id, c.incoming_change_id, c.local_row, c.incoming_row,
                local.origin_device_id, local.hlc_wall_time_ms,
                incoming.origin_device_id, incoming.hlc_wall_time_ms
         FROM sync_v2_conflicts c
         JOIN sync_v2_changes local ON local.change_id = c.local_change_id
         JOIN sync_v2_changes incoming ON incoming.change_id = c.incoming_change_id
         WHERE c.status = 'unresolved'
           AND c.local_row IS NOT NULL
           AND c.incoming_row IS NOT NULL
         ORDER BY c.detected_at DESC",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, Vec<u8>>(6)?,
            row.get::<_, Vec<u8>>(7)?,
            row.get::<_, String>(8)?,
            row.get::<_, i64>(9)?,
            row.get::<_, String>(10)?,
            row.get::<_, i64>(11)?,
        ))
    })?;

    let mut conflicts = Vec::new();
    for row in rows {
        let (
            conflict_id,
            table_name,
            operation,
            detected_at,
            local_change_id,
            incoming_change_id,
            local_blob,
            incoming_blob,
            local_device_id,
            local_timestamp,
            incoming_device_id,
            incoming_timestamp,
        ) = row?;
        if !is_user_resolvable(&table_name) {
            continue;
        }
        let local = decode_snapshot(&local_blob)?;
        let incoming = decode_snapshot(&incoming_blob)?;
        let Some(winning_change_id) = winning_change_id(conn, &table_name, &local.primary_key)? else {
            continue;
        };
        let (
            current_change_id,
            current_device_id,
            current_timestamp,
            current,
            other_change_id,
            other_device_id,
            other_timestamp,
            other,
        ) = if winning_change_id == local_change_id {
            (
                local_change_id,
                local_device_id,
                local_timestamp,
                local,
                incoming_change_id,
                incoming_device_id,
                incoming_timestamp,
                incoming,
            )
        } else if winning_change_id == incoming_change_id {
            (
                incoming_change_id,
                incoming_device_id,
                incoming_timestamp,
                incoming,
                local_change_id,
                local_device_id,
                local_timestamp,
                local,
            )
        } else {
            continue;
        };
        if current.values.is_none() {
            continue;
        }
        let fields = visible_differences(&table_name, &current, &other);
        if fields.is_empty() {
            continue;
        }
        let entity_name = entity_name(&table_name, &current, &other);
        conflicts.push(SyncConflict {
            conflict_id,
            entity_type: table_name,
            entity_name,
            operation,
            detected_at,
            current_change_id,
            current_device_id,
            current_timestamp,
            other_change_id,
            other_device_id,
            other_timestamp,
            fields,
        });
    }
    Ok(conflicts)
}

pub fn resolve_conflict(
    conn: &Connection,
    conflict_id: &str,
    choice: ConflictChoice,
    now_ms: i64,
) -> Result<(), ConflictError> {
    let record = conn
        .query_row(
            "SELECT table_name, status, local_change_id, incoming_change_id,
                    local_row, incoming_row
             FROM sync_v2_conflicts
             WHERE conflict_id = ?1",
            params![conflict_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Vec<u8>>(4)?,
                    row.get::<_, Vec<u8>>(5)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| ConflictError::NotFound(conflict_id.to_string()))?;
    let (table_name, status, local_change_id, incoming_change_id, local_blob, incoming_blob) =
        record;
    if status != "unresolved" {
        return Err(ConflictError::AlreadyResolved(conflict_id.to_string()));
    }
    if !is_user_resolvable(&table_name) {
        return Err(ConflictError::NotUserResolvable(table_name));
    }
    let local = decode_snapshot(&local_blob)?;
    let incoming = decode_snapshot(&incoming_blob)?;
    let winning = winning_change_id(conn, &table_name, &local.primary_key)?
        .ok_or_else(|| ConflictError::NotFound(conflict_id.to_string()))?;
    let selected = match choice {
        ConflictChoice::Current if winning == local_change_id => local,
        ConflictChoice::Current if winning == incoming_change_id => incoming,
        ConflictChoice::Other if winning == local_change_id => incoming,
        ConflictChoice::Other if winning == incoming_change_id => local,
        _ => return Err(ConflictError::AlreadyResolved(conflict_id.to_string())),
    };

    let captured = super::capture_local_transaction(conn, now_ms, |tx| {
        restore_selected_row(tx, &table_name, &selected, now_ms)
    })
    .map_err(|error| ConflictError::InvalidSnapshot(error.to_string()))?;
    if captured.revision.is_none() {
        return Err(ConflictError::NoRevision);
    }
    Ok(())
}

fn is_user_resolvable(table: &str) -> bool {
    USER_RESOLVABLE_TABLES.contains(&table)
}

fn decode_snapshot(bytes: &[u8]) -> Result<StoredRow, ConflictError> {
    bincode::deserialize(bytes).map_err(|error| ConflictError::InvalidSnapshot(error.to_string()))
}

fn winning_change_id(
    conn: &Connection,
    table_name: &str,
    primary_key: &[StoredValue],
) -> Result<Option<String>, ConflictError> {
    let values = primary_key
        .iter()
        .map(Value::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    let primary_key_hash = super::changeset::row_identity_hash(table_name, &values);
    conn.query_row(
        "SELECT winning_change_id
         FROM sync_v2_row_versions
         WHERE table_name = ?1 AND primary_key_hash = ?2",
        params![table_name, primary_key_hash],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(ConflictError::from)
}

fn restore_selected_row(
    conn: &Connection,
    table_name: &str,
    snapshot: &StoredRow,
    now_ms: i64,
) -> Result<(), rusqlite::Error> {
    let (_, primary_key_columns) = table_columns(conn, table_name)?;
    let primary_key = snapshot
        .primary_key
        .iter()
        .map(Value::try_from)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| rusqlite::Error::InvalidParameterName(error.to_string()))?;
    let quoted_table = quote_identifier(table_name);
    let Some(stored_values) = &snapshot.values else {
        let predicate = primary_key_columns
            .iter()
            .map(|column| format!("{} IS ?", quote_identifier(column)))
            .collect::<Vec<_>>()
            .join(" AND ");
        conn.execute(
            &format!("DELETE FROM {quoted_table} WHERE {predicate}"),
            params_from_iter(primary_key.iter()),
        )?;
        return Ok(());
    };

    let mut values = stored_values
        .iter()
        .map(Value::try_from)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| rusqlite::Error::InvalidParameterName(error.to_string()))?;
    if let Some(index) = snapshot.columns.iter().position(|column| column == "updated_at") {
        let previous = match values.get(index) {
            Some(Value::Integer(value)) => *value,
            _ => 0,
        };
        values[index] = Value::Integer(now_ms.max(previous.saturating_add(1)));
    }
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
    conn.execute(
        &format!(
            "INSERT INTO {quoted_table} ({columns}) VALUES ({placeholders})
             ON CONFLICT DO UPDATE SET {updates}"
        ),
        params_from_iter(values.iter()),
    )?;
    Ok(())
}

fn visible_differences(
    table_name: &str,
    current: &StoredRow,
    other: &StoredRow,
) -> Vec<ConflictField> {
    let current_values = row_map(current);
    let other_values = row_map(other);
    visible_fields(table_name)
        .iter()
        .filter_map(|key| {
            let current = current_values.get(*key).cloned().unwrap_or(serde_json::Value::Null);
            let other = other_values.get(*key).cloned().unwrap_or(serde_json::Value::Null);
            (current != other).then(|| ConflictField {
                key: (*key).to_string(),
                current,
                other,
            })
        })
        .collect()
}

fn visible_fields(table_name: &str) -> &'static [&'static str] {
    match table_name {
        "characters" => &[
            "name",
            "nickname",
            "description",
            "definition",
            "scenario",
            "creator_notes",
            "tags",
            "system_prompt",
        ],
        "personas" => &["title", "nickname", "description"],
        "lorebooks" => &["name", "keyword_detection_mode"],
        "lorebook_entries" => &[
            "title",
            "content",
            "keywords",
            "enabled",
            "always_active",
            "priority",
        ],
        "prompt_templates" => &["name", "content", "entries"],
        _ => &[],
    }
}

fn row_map(snapshot: &StoredRow) -> BTreeMap<String, serde_json::Value> {
    let Some(values) = &snapshot.values else {
        return BTreeMap::new();
    };
    snapshot
        .columns
        .iter()
        .cloned()
        .zip(values.iter().map(StoredValue::display_value))
        .collect()
}

fn entity_name(table_name: &str, current: &StoredRow, other: &StoredRow) -> String {
    let current = row_map(current);
    let other = row_map(other);
    let key = if table_name == "personas" || table_name == "lorebook_entries" {
        "title"
    } else {
        "name"
    };
    current
        .get(key)
        .or_else(|| other.get(key))
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("")
        .to_string()
}

fn table_columns(
    conn: &Connection,
    table_name: &str,
) -> Result<(Vec<String>, Vec<String>), rusqlite::Error> {
    let escaped = table_name.replace('\'', "''");
    let mut statement = conn.prepare(&format!(
        "SELECT name, pk FROM pragma_table_info('{escaped}') ORDER BY cid"
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

fn quote_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

impl From<&Value> for StoredValue {
    fn from(value: &Value) -> Self {
        match value {
            Value::Null => Self::Null,
            Value::Integer(value) => Self::Integer(*value),
            Value::Real(value) => Self::Real(*value),
            Value::Text(value) => Self::Text(value.clone()),
            Value::Blob(value) => Self::Blob(BASE64.encode(value)),
        }
    }
}

impl TryFrom<&StoredValue> for Value {
    type Error = ConflictError;

    fn try_from(value: &StoredValue) -> Result<Self, Self::Error> {
        match value {
            StoredValue::Null => Ok(Self::Null),
            StoredValue::Integer(value) => Ok(Self::Integer(*value)),
            StoredValue::Real(value) => Ok(Self::Real(*value)),
            StoredValue::Text(value) => Ok(Self::Text(value.clone())),
            StoredValue::Blob(value) => BASE64
                .decode(value)
                .map(Self::Blob)
                .map_err(|error| ConflictError::InvalidSnapshot(error.to_string())),
        }
    }
}

impl StoredValue {
    fn display_value(&self) -> serde_json::Value {
        match self {
            Self::Null => serde_json::Value::Null,
            Self::Integer(value) => (*value).into(),
            Self::Real(value) => serde_json::Number::from_f64(*value)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null),
            Self::Text(value) => value.clone().into(),
            Self::Blob(_) => "[binary data]".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use rusqlite::{params, Connection};

    use super::{list_unresolved_conflicts, resolve_conflict, ConflictChoice};
    use crate::sync::v2::{
        apply_remote_revision, capture_transaction, create_schema, load_revision,
    };

    fn connection() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE characters (
               id TEXT PRIMARY KEY,
               name TEXT NOT NULL,
               description TEXT,
               created_at INTEGER NOT NULL,
               updated_at INTEGER NOT NULL
             );",
        )
        .unwrap();
        create_schema(&conn).unwrap();
        conn
    }

    fn description(conn: &Connection) -> String {
        conn.query_row(
            "SELECT description FROM characters WHERE id = 'character-1'",
            [],
            |row| row.get(0),
        )
        .unwrap()
    }

    fn set_device_id(conn: &Connection, device_id: &str) {
        conn.execute(
            "INSERT INTO sync_v2_local_state (key, value) VALUES ('device_id', ?1)",
            params![device_id],
        )
        .unwrap();
    }

    #[test]
    fn user_choice_is_captured_and_resolves_on_every_replica() {
        let left = connection();
        let right = connection();
        set_device_id(&left, "device-a");
        set_device_id(&right, "device-b");
        let base = capture_transaction(&left, "device-a", 100, |tx| {
            tx.execute(
                "INSERT INTO characters
                   (id, name, description, created_at, updated_at)
                 VALUES ('character-1', 'Base', 'Base description', 100, 100)",
                [],
            )
        })
        .unwrap()
        .revision
        .unwrap();
        apply_remote_revision(&right, &base, 110).unwrap();

        let left_edit = capture_transaction(&left, "device-a", 200, |tx| {
            tx.execute(
                "UPDATE characters
                 SET name = 'Left name', updated_at = 200
                 WHERE id = 'character-1'",
                [],
            )
        })
        .unwrap()
        .revision
        .unwrap();
        let right_edit = capture_transaction(&right, "device-b", 300, |tx| {
            tx.execute(
                "UPDATE characters
                 SET description = 'Right description', updated_at = 300
                 WHERE id = 'character-1'",
                [],
            )
        })
        .unwrap()
        .revision
        .unwrap();
        apply_remote_revision(&left, &right_edit, 310).unwrap();
        apply_remote_revision(&right, &left_edit, 320).unwrap();

        let conflicts = list_unresolved_conflicts(&left).unwrap();
        assert_eq!(conflicts.len(), 1);
        assert!(conflicts[0]
            .fields
            .iter()
            .any(|field| field.key == "description"));

        resolve_conflict(
            &left,
            &conflicts[0].conflict_id,
            ConflictChoice::Other,
            400,
        )
        .unwrap();
        assert_eq!(description(&left), "Base description");

        let resolution_id = left
            .query_row(
                "SELECT resolved_by_change_id
                 FROM sync_v2_conflicts
                 WHERE conflict_id = ?1",
                params![conflicts[0].conflict_id],
                |row| row.get::<_, String>(0),
            )
            .unwrap();
        let resolution = load_revision(&left, &resolution_id).unwrap().unwrap();
        apply_remote_revision(&right, &resolution, 410).unwrap();

        for conn in [&left, &right] {
            assert_eq!(description(conn), "Base description");
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
    fn choosing_current_still_creates_a_resolution_revision() {
        let conn = connection();
        let peer = connection();
        set_device_id(&conn, "device-a");
        set_device_id(&peer, "device-b");
        let base = capture_transaction(&conn, "device-a", 100, |tx| {
            tx.execute(
                "INSERT INTO characters
                   (id, name, description, created_at, updated_at)
                 VALUES ('character-1', 'Base', 'Base', 100, 100)",
                [],
            )
        })
        .unwrap()
        .revision
        .unwrap();
        apply_remote_revision(&peer, &base, 110).unwrap();
        let local = capture_transaction(&conn, "device-a", 200, |tx| {
            tx.execute(
                "UPDATE characters SET name = 'Local', updated_at = 200
                 WHERE id = 'character-1'",
                [],
            )
        })
        .unwrap()
        .revision
        .unwrap();
        let remote = capture_transaction(&peer, "device-b", 300, |tx| {
            tx.execute(
                "UPDATE characters SET name = 'Remote', updated_at = 300
                 WHERE id = 'character-1'",
                [],
            )
        })
        .unwrap()
        .revision
        .unwrap();
        apply_remote_revision(&conn, &remote, 310).unwrap();
        apply_remote_revision(&peer, &local, 320).unwrap();

        let conflict = list_unresolved_conflicts(&conn).unwrap().remove(0);
        resolve_conflict(&conn, &conflict.conflict_id, ConflictChoice::Current, 400).unwrap();
        assert_eq!(
            conn.query_row(
                "SELECT updated_at FROM characters WHERE id = 'character-1'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
            400
        );
    }
}
