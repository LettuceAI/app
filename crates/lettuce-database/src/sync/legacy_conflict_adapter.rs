use std::collections::BTreeMap;

use lettuce_sync::ConflictRepositoryError;
use lettuce_transfer::{LegacyConflictSnapshot, LegacyIdScope, decode_legacy_conflict_snapshot};
use lettuce_types::{LegacyImportRunId, TimestampMillis};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;

use crate::{ApiOperationTransaction, Database};

#[derive(Debug, Clone, PartialEq)]
pub enum LegacyConflictCurrent {
    Value(Value),
    NotMapped,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyConflictField {
    pub key: String,
    pub current: LegacyConflictCurrent,
    pub local: Value,
    pub incoming: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LegacySyncConflict {
    pub run_id: LegacyImportRunId,
    pub source_key: String,
    pub entity_kind: String,
    pub entity_id: String,
    pub detected_at: TimestampMillis,
    pub fields: Vec<LegacyConflictField>,
}

fn storage(_: impl std::fmt::Debug) -> ConflictRepositoryError {
    ConflictRepositoryError::Storage
}

fn corrupt(_: impl std::fmt::Debug) -> ConflictRepositoryError {
    ConflictRepositoryError::Corrupt
}

fn fields(table: &str) -> Option<(&'static str, &'static [&'static str])> {
    match table {
        "characters" => Some((
            "character",
            &[
                "name",
                "nickname",
                "description",
                "definition",
                "scenario",
                "creator_notes",
                "tags",
                "system_prompt",
            ],
        )),
        "personas" => Some(("persona", &["title", "nickname", "description"])),
        "lorebooks" => Some(("lorebook", &["name", "keyword_detection_mode"])),
        "lorebook_entries" => Some((
            "lorebook_entry",
            &[
                "title",
                "content",
                "keywords",
                "enabled",
                "always_active",
                "priority",
            ],
        )),
        "prompt_templates" => Some(("prompt", &["name", "content", "entries"])),
        _ => None,
    }
}

fn snapshot(row: &Value, key: &str) -> Result<LegacyConflictSnapshot, ConflictRepositoryError> {
    row.get(key)
        .and_then(|value| value.get("hex"))
        .and_then(Value::as_str)
        .and_then(decode_legacy_conflict_snapshot)
        .ok_or(ConflictRepositoryError::Corrupt)
}

fn current_fields(
    connection: &Connection,
    run: &str,
    kind: &str,
    source: &str,
) -> Result<BTreeMap<String, LegacyConflictCurrent>, ConflictRepositoryError> {
    let id = if kind == "character" {
        Some(LegacyIdScope::new().source(kind, source).to_string())
    } else {
        connection.query_row("SELECT destination_id FROM legacy_import_assignments WHERE run_id=?1 AND source_kind=?2 AND source_key=?3 AND source_detail=''", params![run, kind, source], |row| row.get::<_, String>(0)).optional().map_err(storage)?
    };
    let Some(id) = id else {
        return Ok(BTreeMap::new());
    };
    let mut values = BTreeMap::<String, Value>::new();
    match kind {
        "character" => {
            if let Some(details) = crate::catalog::character_adapter::load_character_details(
                connection,
                id.parse().map_err(corrupt)?,
            )
            .map_err(storage)?
            {
                let character = details.character;
                let profile = serde_json::to_value(character.profile).map_err(corrupt)?;
                for key in ["name", "nickname", "description", "definition", "scenario"] {
                    values.insert(key.into(), profile[key].clone());
                }
                values.insert(
                    "creator_notes".into(),
                    serde_json::to_value(character.provenance.creator_notes).map_err(corrupt)?,
                );
                values.insert(
                    "tags".into(),
                    serde_json::to_string(&character.provenance.tags)
                        .map_err(corrupt)?
                        .into(),
                );
            }
        }
        "persona" => {
            if let Some(persona) = crate::catalog::persona_adapter::load_persona(
                connection,
                id.parse().map_err(corrupt)?,
            )
            .map_err(storage)?
            {
                values.insert("title".into(), persona.title.into());
                values.insert(
                    "nickname".into(),
                    serde_json::to_value(persona.nickname).map_err(corrupt)?,
                );
                values.insert("description".into(), persona.description.into());
            }
        }
        "lorebook" => {
            if let Some(details) = crate::lorebook::lorebook_adapter::load_details(
                connection,
                id.parse().map_err(corrupt)?,
            )
            .map_err(storage)?
            {
                values.insert("name".into(), details.book.name.into());
                let detection = match details.book.detection_policy {
                    lettuce_context::DetectionPolicy::RecentMessageWindow => {
                        "recent_message_window"
                    }
                    lettuce_context::DetectionPolicy::LatestUserMessage => "latest_user_message",
                };
                values.insert("keyword_detection_mode".into(), detection.into());
            }
        }
        "lorebook_entry" => {
            let parent = connection
                .query_row(
                    "SELECT lorebook_id FROM lorebook_entries WHERE id=?1",
                    [&id],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(storage)?;
            if let Some(parent) = parent {
                let details = crate::lorebook::lorebook_adapter::load_details(
                    connection,
                    parent.parse().map_err(corrupt)?,
                )
                .map_err(storage)?
                .ok_or(ConflictRepositoryError::Corrupt)?;
                let entry = details
                    .entries
                    .into_iter()
                    .find(|entry| entry.id.to_string() == id)
                    .ok_or(ConflictRepositoryError::Corrupt)?;
                values.insert("title".into(), entry.title.into());
                values.insert("content".into(), entry.content.into());
                values.insert(
                    "keywords".into(),
                    serde_json::to_string(&entry.keywords)
                        .map_err(corrupt)?
                        .into(),
                );
                values.insert("enabled".into(), i64::from(entry.enabled).into());
                values.insert(
                    "always_active".into(),
                    i64::from(entry.always_active).into(),
                );
                values.insert("priority".into(), entry.priority.into());
            }
        }
        "prompt" => {
            if let Some(document) = crate::catalog::prompt_adapter::load_document(
                connection,
                id.parse().map_err(corrupt)?,
            )
            .map_err(storage)?
            {
                values.insert("name".into(), document.name.into());
                values.insert(
                    "entries".into(),
                    serde_json::to_value(document.entries).map_err(corrupt)?,
                );
            }
        }
        _ => return Err(ConflictRepositoryError::Corrupt),
    }
    let mut mapped = values
        .into_iter()
        .map(|(key, value)| (key, LegacyConflictCurrent::Value(value)))
        .collect::<BTreeMap<_, _>>();
    if kind == "character" {
        mapped.insert("system_prompt".into(), LegacyConflictCurrent::NotMapped);
    }
    if kind == "prompt" {
        mapped.insert("content".into(), LegacyConflictCurrent::NotMapped);
    }
    Ok(mapped)
}

impl Database {
    pub fn unresolved_legacy_conflicts(
        &self,
    ) -> Result<Vec<LegacySyncConflict>, ConflictRepositoryError> {
        let connection = self.connection().map_err(storage)?;
        let rows = connection.prepare("SELECT run_id,source_key,row_json FROM legacy_import_preserved_rows WHERE source_table='sync_v2_conflicts' AND dismissed_at IS NULL ORDER BY json_extract(row_json,'$.detected_at') DESC,run_id,source_key").map_err(storage)?
            .query_map([], |row| Ok((row.get::<_, String>(0)?,row.get::<_, String>(1)?,row.get::<_, String>(2)?))).map_err(storage)?.collect::<rusqlite::Result<Vec<_>>>().map_err(storage)?;
        let mut conflicts = Vec::new();
        for (run, source_key, json) in rows {
            let row: Value = serde_json::from_str(&json).map_err(corrupt)?;
            let status = row
                .get("status")
                .and_then(Value::as_str)
                .ok_or(ConflictRepositoryError::Corrupt)?;
            if status == "resolved" {
                continue;
            }
            if status != "unresolved" {
                return Err(ConflictRepositoryError::Corrupt);
            }
            let table = row
                .get("table_name")
                .and_then(Value::as_str)
                .ok_or(ConflictRepositoryError::Corrupt)?;
            let Some((kind, keys)) = fields(table) else {
                continue;
            };
            if row.get("local_row").is_none_or(Value::is_null)
                || row.get("incoming_row").is_none_or(Value::is_null)
            {
                continue;
            }
            let local = snapshot(&row, "local_row")?;
            let incoming = snapshot(&row, "incoming_row")?;
            if local.primary_key != incoming.primary_key || local.primary_key.len() != 1 {
                return Err(ConflictRepositoryError::Corrupt);
            }
            let source = local.primary_key[0]
                .as_str()
                .ok_or(ConflictRepositoryError::Corrupt)?;
            let current = current_fields(&connection, &run, kind, source)?;
            let local = local.values.unwrap_or_default();
            let incoming = incoming.values.unwrap_or_default();
            let mut diff = Vec::new();
            for key in keys {
                if !local.contains_key(*key)
                    && !incoming.contains_key(*key)
                    && !(kind == "prompt" && *key == "entries")
                {
                    continue;
                }
                let local = local.get(*key).cloned().unwrap_or(Value::Null);
                let incoming = incoming.get(*key).cloned().unwrap_or(Value::Null);
                let current = current
                    .get(*key)
                    .cloned()
                    .unwrap_or(LegacyConflictCurrent::NotMapped);
                if current != LegacyConflictCurrent::Value(local.clone()) || local != incoming {
                    diff.push(LegacyConflictField {
                        key: (*key).into(),
                        current,
                        local,
                        incoming,
                    });
                }
            }
            conflicts.push(LegacySyncConflict {
                run_id: run.parse().map_err(corrupt)?,
                source_key,
                entity_kind: kind.into(),
                entity_id: source.into(),
                detected_at: TimestampMillis::new(
                    row.get("detected_at")
                        .and_then(Value::as_i64)
                        .ok_or(ConflictRepositoryError::Corrupt)?,
                ),
                fields: diff,
            });
        }
        Ok(conflicts)
    }
}

impl ApiOperationTransaction<'_, '_> {
    pub fn dismiss_legacy_conflict(
        &self,
        run: LegacyImportRunId,
        key: &str,
        now: TimestampMillis,
    ) -> Result<(), ConflictRepositoryError> {
        let row: Option<String> = self.transaction.query_row("SELECT row_json FROM legacy_import_preserved_rows WHERE run_id=?1 AND source_table='sync_v2_conflicts' AND source_key=?2 AND dismissed_at IS NULL", params![run.to_string(),key], |row| row.get(0)).optional().map_err(storage)?;
        let row: Value = serde_json::from_str(&row.ok_or(ConflictRepositoryError::NotFound)?)
            .map_err(corrupt)?;
        if row.get("status").and_then(Value::as_str) != Some("unresolved")
            || row
                .get("table_name")
                .and_then(Value::as_str)
                .and_then(fields)
                .is_none()
        {
            return Err(ConflictRepositoryError::NotFound);
        }
        if row.get("local_row").is_none_or(Value::is_null)
            || row.get("incoming_row").is_none_or(Value::is_null)
        {
            return Err(ConflictRepositoryError::NotFound);
        }
        let changed = self.transaction.execute("UPDATE legacy_import_preserved_rows SET dismissed_at=?3 WHERE run_id=?1 AND source_table='sync_v2_conflicts' AND source_key=?2 AND dismissed_at IS NULL", params![run.to_string(), key, now.get()]).map_err(storage)?;
        if changed != 1 {
            return Err(ConflictRepositoryError::Conflict);
        }
        Ok(())
    }
}
