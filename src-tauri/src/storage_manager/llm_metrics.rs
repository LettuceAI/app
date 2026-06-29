use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value as JsonValue};

use super::db::{now_ms, open_db};

const DEFAULT_RETENTION: i64 = 500;

pub fn llm_metrics_insert(
    app: &tauri::AppHandle,
    id: &str,
    model_name: Option<&str>,
    summary: &JsonValue,
    samples: &JsonValue,
) -> Result<(), String> {
    let conn = open_db(app)?;
    let summary_json = serde_json::to_string(summary)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let samples_json = serde_json::to_string(samples)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    conn.execute(
        "INSERT OR REPLACE INTO llm_generation_metrics
            (id, created_at, model_name, summary_json, samples_json)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, now_ms(), model_name, summary_json, samples_json],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    conn.execute(
        "DELETE FROM llm_generation_metrics
         WHERE id NOT IN (
            SELECT id FROM llm_generation_metrics
            ORDER BY created_at DESC
            LIMIT ?1
         )",
        params![DEFAULT_RETENTION],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    Ok(())
}

#[tauri::command]
pub fn llm_metrics_list(app: tauri::AppHandle, limit: Option<i64>) -> Result<JsonValue, String> {
    let conn = open_db(&app)?;
    let limit = limit.unwrap_or(DEFAULT_RETENTION).clamp(1, 5000);

    let mut stmt = conn
        .prepare(
            "SELECT id, created_at, summary_json
             FROM llm_generation_metrics
             ORDER BY created_at DESC
             LIMIT ?1",
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let rows = stmt
        .query_map(params![limit], |row| {
            let id: String = row.get(0)?;
            let created_at: i64 = row.get(1)?;
            let summary_json: String = row.get(2)?;
            Ok((id, created_at, summary_json))
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let mut out = Vec::new();
    for row in rows {
        let (id, created_at, summary_json) =
            row.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let mut summary = serde_json::from_str::<JsonValue>(&summary_json).unwrap_or_else(|_| json!({}));
        if let Some(obj) = summary.as_object_mut() {
            obj.insert("id".into(), json!(id));
            obj.insert("createdAt".into(), json!(created_at));
        }
        out.push(summary);
    }

    Ok(JsonValue::Array(out))
}

#[tauri::command]
pub fn llm_metrics_get(app: tauri::AppHandle, id: String) -> Result<Option<JsonValue>, String> {
    let conn = open_db(&app)?;
    let row: Option<(i64, String, String)> = conn
        .query_row(
            "SELECT created_at, summary_json, samples_json
             FROM llm_generation_metrics
             WHERE id = ?1",
            params![id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let Some((created_at, summary_json, samples_json)) = row else {
        return Ok(None);
    };

    let mut summary = serde_json::from_str::<JsonValue>(&summary_json).unwrap_or_else(|_| json!({}));
    let samples = serde_json::from_str::<JsonValue>(&samples_json).unwrap_or_else(|_| json!([]));
    if let Some(obj) = summary.as_object_mut() {
        obj.insert("id".into(), json!(id));
        obj.insert("createdAt".into(), json!(created_at));
        obj.insert("samples".into(), samples);
    }

    Ok(Some(summary))
}

#[tauri::command]
pub fn llm_metrics_attach_message(
    app: tauri::AppHandle,
    id: String,
    message_id: String,
) -> Result<(), String> {
    let conn = open_db(&app)?;
    conn.execute(
        "UPDATE llm_generation_metrics SET message_id = ?2 WHERE id = ?1",
        params![id, message_id],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

#[tauri::command]
pub fn llm_metrics_get_by_message(
    app: tauri::AppHandle,
    message_id: String,
) -> Result<Option<JsonValue>, String> {
    let conn = open_db(&app)?;
    let row: Option<(String, i64, String, String)> = conn
        .query_row(
            "SELECT id, created_at, summary_json, samples_json
             FROM llm_generation_metrics
             WHERE message_id = ?1
             ORDER BY created_at DESC
             LIMIT 1",
            params![message_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let Some((id, created_at, summary_json, samples_json)) = row else {
        return Ok(None);
    };

    let mut summary = serde_json::from_str::<JsonValue>(&summary_json).unwrap_or_else(|_| json!({}));
    let samples = serde_json::from_str::<JsonValue>(&samples_json).unwrap_or_else(|_| json!([]));
    if let Some(obj) = summary.as_object_mut() {
        obj.insert("id".into(), json!(id));
        obj.insert("createdAt".into(), json!(created_at));
        obj.insert("samples".into(), samples);
    }

    Ok(Some(summary))
}

#[tauri::command]
pub fn llm_metrics_clear(app: tauri::AppHandle) -> Result<(), String> {
    let conn = open_db(&app)?;
    conn.execute("DELETE FROM llm_generation_metrics", [])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}
