//! Plain rows exchanged by sync.
//!
//! Tables whose rows stand alone (ASR learning data, audio providers, user
//! voices and the playground history with its images) are exchanged row by
//! row: the payload is the row's columns as a
//! JSON object keyed by column name, without the local revision. An update
//! bumps the local revision and never moves `updated_at` backwards. A row
//! whose required parent has not arrived waits; an optional reference to a
//! row that is gone is cleared. When a table has a natural identity beside
//! its id, two devices that created the same thing under different ids keep
//! the lower id.

use rusqlite::types::Value;
use rusqlite::{Connection, Transaction, params_from_iter};
use serde_json::{Map, Number};

pub(crate) struct RowReference {
    pub column: &'static str,
    pub table: &'static str,
    pub required: bool,
}

pub(crate) struct RowTable {
    pub table: &'static str,
    pub key: &'static [&'static str],
    pub immutable: bool,
    pub columns: &'static [&'static str],
    pub revision: bool,
    pub identity: &'static [&'static str],
    pub references: &'static [RowReference],
    /// Rows the scan leaves out while it holds (a pending playground entry
    /// is still being generated); a row never returns to it once journaled.
    pub scan_filter: Option<&'static str>,
}

#[derive(Debug)]
pub(crate) enum RowSyncError {
    Pending,
    Corrupt,
    Storage,
}

fn storage(_: impl std::fmt::Debug) -> RowSyncError {
    RowSyncError::Storage
}

fn key_expression(spec: &RowTable) -> String {
    spec.key.join(" || ':' || ")
}

fn key_condition(spec: &RowTable) -> String {
    spec.key
        .iter()
        .map(|column| format!("{column} = ?"))
        .collect::<Vec<_>>()
        .join(" AND ")
}

fn key_values(spec: &RowTable, id: &str) -> Result<Vec<Value>, RowSyncError> {
    let mut values = Vec::with_capacity(spec.key.len());
    let mut rest = id;
    for index in 0..spec.key.len() {
        if index + 1 == spec.key.len() {
            values.push(Value::Text(rest.to_owned()));
        } else {
            let (value, tail) = rest.split_once(':').ok_or(RowSyncError::Corrupt)?;
            values.push(Value::Text(value.to_owned()));
            rest = tail;
        }
    }
    Ok(values)
}

pub(crate) fn row_ids(connection: &Connection, spec: &RowTable) -> rusqlite::Result<Vec<String>> {
    connection
        .prepare(&format!(
            "SELECT {key} FROM {} WHERE {} ORDER BY {key}",
            spec.table,
            spec.scan_filter.unwrap_or("1"),
            key = key_expression(spec)
        ))?
        .query_map([], |row| row.get(0))?
        .collect()
}

fn to_json(value: Value) -> Result<serde_json::Value, RowSyncError> {
    Ok(match value {
        Value::Null => serde_json::Value::Null,
        Value::Integer(value) => serde_json::Value::Number(value.into()),
        Value::Real(value) => {
            serde_json::Value::Number(Number::from_f64(value).ok_or(RowSyncError::Corrupt)?)
        }
        Value::Text(value) => serde_json::Value::String(value),
        Value::Blob(_) => return Err(RowSyncError::Corrupt),
    })
}

fn to_sql(value: &serde_json::Value) -> Result<Value, RowSyncError> {
    Ok(match value {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(value) => Value::Integer(i64::from(*value)),
        serde_json::Value::Number(number) => match number.as_i64() {
            Some(value) => Value::Integer(value),
            None => Value::Real(number.as_f64().ok_or(RowSyncError::Corrupt)?),
        },
        serde_json::Value::String(value) => Value::Text(value.clone()),
        _ => return Err(RowSyncError::Corrupt),
    })
}

pub(crate) fn row_current(
    transaction: &Transaction<'_>,
    spec: &RowTable,
    id: &str,
) -> Result<Option<Vec<u8>>, RowSyncError> {
    let sql = format!(
        "SELECT {} FROM {} WHERE {}",
        spec.columns.join(", "),
        spec.table,
        key_condition(spec)
    );
    let mut statement = transaction.prepare(&sql).map_err(storage)?;
    let mut rows = statement
        .query(params_from_iter(key_values(spec, id)?))
        .map_err(storage)?;
    let Some(row) = rows.next().map_err(storage)? else {
        return Ok(None);
    };
    let mut object = Map::new();
    for (index, column) in spec.columns.iter().enumerate() {
        object.insert(
            (*column).to_owned(),
            to_json(row.get::<_, Value>(index).map_err(storage)?)?,
        );
    }
    serde_json::to_vec(&object).map(Some).map_err(storage)
}

pub(crate) fn row_decode(
    spec: &RowTable,
    bytes: &[u8],
) -> Result<Map<String, serde_json::Value>, RowSyncError> {
    let object: Map<String, serde_json::Value> =
        serde_json::from_slice(bytes).map_err(|_| RowSyncError::Corrupt)?;
    if object.len() != spec.columns.len()
        || spec
            .columns
            .iter()
            .any(|column| !object.contains_key(*column))
    {
        return Err(RowSyncError::Corrupt);
    }
    for value in object.values() {
        to_sql(value)?;
    }
    Ok(object)
}

fn exists(transaction: &Transaction<'_>, table: &str, id: &Value) -> Result<bool, RowSyncError> {
    transaction
        .query_row(
            &format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE id = ?1)"),
            [id],
            |row| row.get(0),
        )
        .map_err(storage)
}

/// Writes a synced row. Returns `false` when a row with the same natural
/// identity and a lower id already exists here, so the incoming one is not
/// kept. A row whose identity columns are all empty has no natural identity
/// (like a SQL unique constraint over nulls).
pub(crate) fn row_materialize(
    transaction: &Transaction<'_>,
    spec: &RowTable,
    id: &str,
    bytes: &[u8],
) -> Result<bool, RowSyncError> {
    let object = row_decode(spec, bytes)?;
    let mut values = spec
        .columns
        .iter()
        .map(|column| to_sql(&object[*column]))
        .collect::<Result<Vec<_>, _>>()?;
    let keys = key_values(spec, id)?;
    for reference in spec.references {
        if let Some(index) = spec
            .key
            .iter()
            .position(|column| *column == reference.column)
        {
            if !exists(transaction, reference.table, &keys[index])? {
                return Err(RowSyncError::Pending);
            }
            continue;
        }
        let index = spec
            .columns
            .iter()
            .position(|column| *column == reference.column)
            .ok_or(RowSyncError::Corrupt)?;
        if values[index] == Value::Null || exists(transaction, reference.table, &values[index])? {
            continue;
        }
        if reference.required {
            return Err(RowSyncError::Pending);
        }
        values[index] = Value::Null;
    }
    if !spec.identity.is_empty() {
        let condition = spec
            .identity
            .iter()
            .map(|column| format!("coalesce({column}, '') = coalesce(?, '')"))
            .collect::<Vec<_>>()
            .join(" AND ");
        let identity = spec
            .identity
            .iter()
            .map(|column| {
                spec.columns
                    .iter()
                    .position(|name| name == column)
                    .map(|index| values[index].clone())
                    .ok_or(RowSyncError::Corrupt)
            })
            .collect::<Result<Vec<_>, _>>()?;
        if identity.iter().all(|value| *value == Value::Null) {
            return insert_or_update(transaction, spec, id, bytes, values);
        }
        let twins = transaction
            .prepare(&format!(
                "SELECT id FROM {} WHERE id <> ? AND {condition}",
                spec.table
            ))
            .map_err(storage)?
            .query_map(
                params_from_iter(std::iter::once(Value::Text(id.to_owned())).chain(identity)),
                |row| row.get::<_, String>(0),
            )
            .map_err(storage)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(storage)?;
        if twins.iter().any(|twin| twin.as_str() < id) {
            return Ok(false);
        }
        for twin in twins {
            transaction
                .execute(&format!("DELETE FROM {} WHERE id = ?1", spec.table), [twin])
                .map_err(storage)?;
        }
    }
    insert_or_update(transaction, spec, id, bytes, values)
}

fn insert_or_update(
    transaction: &Transaction<'_>,
    spec: &RowTable,
    id: &str,
    bytes: &[u8],
    values: Vec<Value>,
) -> Result<bool, RowSyncError> {
    let present: bool = transaction
        .query_row(
            &format!(
                "SELECT EXISTS(SELECT 1 FROM {} WHERE {})",
                spec.table,
                key_condition(spec)
            ),
            params_from_iter(key_values(spec, id)?),
            |row| row.get(0),
        )
        .map_err(storage)?;
    if present && spec.immutable {
        return Ok(row_current(transaction, spec, id)?.as_deref() == Some(bytes));
    }
    if present {
        let assignments = spec
            .columns
            .iter()
            .filter(|column| **column != "created_at")
            .map(|column| {
                if *column == "updated_at" && spec.revision {
                    "updated_at = max(updated_at, ?)".to_owned()
                } else {
                    format!("{column} = ?")
                }
            })
            .chain(spec.revision.then(|| "revision = revision + 1".to_owned()))
            .collect::<Vec<_>>()
            .join(", ");
        let bound = spec
            .columns
            .iter()
            .zip(values)
            .filter(|(column, _)| **column != "created_at")
            .map(|(_, value)| value)
            .chain(key_values(spec, id)?);
        transaction
            .execute(
                &format!(
                    "UPDATE {} SET {assignments} WHERE {}",
                    spec.table,
                    key_condition(spec)
                ),
                params_from_iter(bound),
            )
            .map_err(storage)?;
    } else {
        let columns = spec
            .key
            .iter()
            .copied()
            .chain(spec.columns.iter().copied())
            .chain(spec.revision.then_some("revision"))
            .collect::<Vec<_>>();
        let placeholders = vec!["?"; columns.len()].join(", ");
        let bound = key_values(spec, id)?
            .into_iter()
            .chain(values)
            .chain(spec.revision.then_some(Value::Integer(1)));
        transaction
            .execute(
                &format!(
                    "INSERT INTO {} ({}) VALUES ({placeholders})",
                    spec.table,
                    columns.join(", ")
                ),
                params_from_iter(bound),
            )
            .map_err(storage)?;
    }
    Ok(true)
}

pub(crate) fn row_delete(
    transaction: &Transaction<'_>,
    spec: &RowTable,
    id: &str,
) -> Result<bool, RowSyncError> {
    transaction
        .execute(
            &format!("DELETE FROM {} WHERE {}", spec.table, key_condition(spec)),
            params_from_iter(key_values(spec, id)?),
        )
        .map_err(storage)?;
    Ok(true)
}

pub(crate) const AUDIO_PROVIDERS: RowTable = RowTable {
    key: &["id"],
    immutable: false,
    table: "audio_providers",
    columns: &[
        "secret_owner_id",
        "provider_kind",
        "label",
        "api_key_secret_ref",
        "config_json",
        "created_at",
        "updated_at",
    ],
    revision: true,
    identity: &[],
    references: &[],
    scan_filter: None,
};

pub(crate) const USER_VOICES: RowTable = RowTable {
    key: &["id"],
    immutable: false,
    table: "user_voices",
    columns: &[
        "provider_id",
        "name",
        "model_id",
        "voice_id",
        "prompt",
        "created_at",
        "updated_at",
    ],
    revision: true,
    identity: &[],
    references: &[RowReference {
        column: "provider_id",
        table: "audio_providers",
        required: true,
    }],
    scan_filter: None,
};

pub(crate) const ASR_VOCABULARY_TERMS: RowTable = RowTable {
    key: &["id"],
    immutable: false,
    table: "asr_vocabulary_terms",
    columns: &[
        "term",
        "normalized_term",
        "language",
        "category",
        "scope",
        "priority",
        "use_count",
        "created_at",
        "updated_at",
    ],
    revision: false,
    identity: &[],
    references: &[],
    scan_filter: None,
};

pub(crate) const ASR_CORRECTIONS: RowTable = RowTable {
    key: &["id"],
    immutable: false,
    table: "asr_corrections",
    columns: &[
        "wrong",
        "normalized_wrong",
        "correct",
        "normalized_correct",
        "language",
        "scope",
        "confidence",
        "use_count",
        "accepted_count",
        "rejected_count",
        "seen_count",
        "last_seen_at",
        "user_approved",
        "created_at",
        "updated_at",
    ],
    revision: false,
    identity: &[],
    references: &[],
    scan_filter: None,
};

pub(crate) const ASR_IGNORED_SUGGESTIONS: RowTable = RowTable {
    key: &["id"],
    immutable: false,
    table: "asr_ignored_suggestions",
    columns: &[
        "wrong",
        "normalized_wrong",
        "correct",
        "normalized_correct",
        "language",
        "scope",
        "ignored_count",
        "last_ignored_at",
        "created_at",
        "updated_at",
    ],
    revision: false,
    identity: &[
        "normalized_wrong",
        "normalized_correct",
        "language",
        "scope",
    ],
    references: &[],
    scan_filter: None,
};

pub(crate) const ASR_VOICE_EXAMPLES: RowTable = RowTable {
    key: &["id"],
    immutable: false,
    table: "asr_voice_examples",
    columns: &[
        "audio_asset_id",
        "audio_blob_kind",
        "expected_text",
        "normalized_expected_text",
        "whisper_output",
        "normalized_whisper_output",
        "language",
        "scope",
        "vocabulary_term_id",
        "correction_id",
        "created_at",
        "updated_at",
    ],
    revision: false,
    identity: &[],
    references: &[
        RowReference {
            column: "audio_asset_id",
            table: "media_assets",
            required: true,
        },
        RowReference {
            column: "vocabulary_term_id",
            table: "asr_vocabulary_terms",
            required: false,
        },
        RowReference {
            column: "correction_id",
            table: "asr_corrections",
            required: false,
        },
    ],
    scan_filter: None,
};

pub(crate) const USAGE_COSTS: RowTable = RowTable {
    key: &["event_id"],
    immutable: true,
    table: "usage_costs",
    columns: &["basis_json"],
    revision: false,
    identity: &[],
    references: &[RowReference {
        column: "event_id",
        table: "usage_events",
        required: true,
    }],
    scan_filter: None,
};

pub(crate) const JOB_INFERENCE_USAGE: RowTable = RowTable {
    key: &["id"],
    immutable: false,
    table: "job_inference_usage",
    columns: &["job_id", "admitted_at", "record_json", "result_json"],
    revision: false,
    identity: &[],
    references: &[],
    scan_filter: None,
};

pub(crate) const JOB_USAGE_COSTS: RowTable = RowTable {
    key: &["event_id"],
    immutable: true,
    table: "job_usage_costs",
    columns: &["basis_json"],
    revision: false,
    identity: &[],
    references: &[RowReference {
        column: "event_id",
        table: "job_inference_usage",
        required: true,
    }],
    scan_filter: None,
};

pub(crate) const LEGACY_USAGE_RECORDS: RowTable = RowTable {
    key: &["run_id", "source_id"],
    immutable: true,
    table: "legacy_usage_records",
    columns: &[
        "recorded_at",
        "session_source_id",
        "character_source_id",
        "character_name",
        "model_source_id",
        "model_profile_id",
        "model_name",
        "provider_source_id",
        "provider_label",
        "operation_type",
        "finish_reason",
        "prompt_tokens",
        "completion_tokens",
        "total_tokens",
        "memory_tokens",
        "summary_tokens",
        "reasoning_tokens",
        "image_tokens",
        "audio_tokens",
        "prompt_cost",
        "completion_cost",
        "total_cost",
        "success",
        "error_message",
        "metadata_json",
    ],
    revision: false,
    identity: &[],
    references: &[],
    scan_filter: None,
};

pub(crate) const PLAYGROUND_HISTORY: RowTable = RowTable {
    key: &["id"],
    immutable: false,
    table: "playground_history",
    columns: &[
        "origin",
        "job_id",
        "import_run_id",
        "source_id",
        "created_at",
        "provider_kind",
        "source_model_id",
        "model_profile_id",
        "model_name",
        "prompt",
        "negative_prompt",
        "seed",
        "params_json",
        "status",
        "error",
    ],
    revision: false,
    identity: &["import_run_id", "source_id"],
    references: &[],
    scan_filter: Some("status <> 'pending'"),
};

pub(crate) const PLAYGROUND_HISTORY_IMAGES: RowTable = RowTable {
    key: &["history_id", "ordinal"],
    immutable: true,
    table: "playground_history_images",
    columns: &[
        "asset_id",
        "source_asset_id",
        "mime_type",
        "url",
        "width",
        "height",
    ],
    revision: false,
    identity: &[],
    references: &[
        RowReference {
            column: "history_id",
            table: "playground_history",
            required: true,
        },
        RowReference {
            column: "asset_id",
            table: "media_assets",
            required: true,
        },
    ],
    scan_filter: None,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Database;

    #[test]
    fn composite_key_immutable_rows_round_trip_once() {
        let source = Database::open_in_memory().expect("source");
        let target = Database::open_in_memory().expect("target");
        source
            .connection()
            .expect("connection")
            .execute(
                "INSERT INTO legacy_usage_records (run_id, source_id, recorded_at, session_source_id, character_source_id, character_name, model_source_id, model_profile_id, model_name, provider_source_id, provider_label, operation_type, finish_reason, prompt_tokens, completion_tokens, total_tokens, memory_tokens, summary_tokens, reasoning_tokens, image_tokens, audio_tokens, prompt_cost, completion_cost, total_cost, success, error_message, metadata_json) VALUES ('run', 'usage:1', 5, 's', 'c', 'Ada', 'm', NULL, 'Model', 'p', 'Provider', 'chat', 'stop', 10, 4, 14, NULL, NULL, NULL, NULL, NULL, 0.25, 0.5, 0.75, 1, NULL, '{}')",
                [],
            )
            .expect("legacy usage");
        let mut connection = source.connection().expect("connection");
        let transaction = connection.transaction().expect("transaction");
        let ids = row_ids(&transaction, &LEGACY_USAGE_RECORDS).expect("ids");
        assert_eq!(ids, vec!["run:usage:1".to_owned()]);
        let bytes = row_current(&transaction, &LEGACY_USAGE_RECORDS, &ids[0])
            .expect("current")
            .expect("present");
        drop(transaction);
        let mut connection = target.connection().expect("connection");
        let transaction = connection.transaction().expect("transaction");
        for _ in 0..2 {
            assert!(
                row_materialize(&transaction, &LEGACY_USAGE_RECORDS, &ids[0], &bytes)
                    .expect("materialize")
            );
        }
        assert_eq!(
            row_current(&transaction, &LEGACY_USAGE_RECORDS, &ids[0]).expect("current"),
            Some(bytes)
        );
    }
}
