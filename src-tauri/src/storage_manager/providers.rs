use rusqlite::{params, OptionalExtension};
use serde_json::{Map as JsonMap, Value as JsonValue};

use super::db::open_db;

#[tauri::command]
pub fn provider_upsert(app: tauri::AppHandle, credential_json: String) -> Result<String, String> {
    let conn = open_db(&app)?;
    let cred: JsonValue = serde_json::from_str(&credential_json)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let id = cred
        .get("id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let provider_id = cred
        .get("providerId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "providerId is required".to_string())?;
    let label = cred
        .get("label")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "label is required".to_string())?;
    let api_key = cred
        .get("apiKey")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let base_url = cred
        .get("baseUrl")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let default_model = cred
        .get("defaultModel")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let headers = cred
        .get("headers")
        .map(|v| serde_json::to_string(v).unwrap_or("null".into()));
    let config = cred
        .get("config")
        .map(|v| serde_json::to_string(v).unwrap_or("null".into()));
    conn.execute(
        r#"INSERT INTO provider_credentials (id, provider_id, label, api_key, base_url, default_model, headers, config)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                provider_id = excluded.provider_id,
                label = excluded.label,
                api_key = excluded.api_key,
                base_url = excluded.base_url,
                default_model = excluded.default_model,
                headers = excluded.headers,
                config = excluded.config"#,
        params![id, provider_id, label, api_key, base_url, default_model, headers, config],
    ).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let mut out = JsonMap::new();
    out.insert("id".into(), JsonValue::String(id));
    out.insert(
        "providerId".into(),
        JsonValue::String(provider_id.to_string()),
    );
    out.insert("label".into(), JsonValue::String(label.to_string()));
    if let Some(v) = cred.get("apiKey").and_then(|v| v.as_str()) {
        out.insert("apiKey".into(), JsonValue::String(v.to_string()));
    }
    if let Some(v) = cred
        .get("baseUrl")
        .and_then(|v| v.as_str())
        .map(|s| JsonValue::String(s.to_string()))
    {
        out.insert("baseUrl".into(), v);
    }
    if let Some(v) = cred
        .get("defaultModel")
        .and_then(|v| v.as_str())
        .map(|s| JsonValue::String(s.to_string()))
    {
        out.insert("defaultModel".into(), v);
    }
    if let Some(v) = cred.get("headers").cloned() {
        if !v.is_null() {
            out.insert("headers".into(), v);
        }
    }
    if let Some(v) = cred.get("config").cloned() {
        if !v.is_null() {
            out.insert("config".into(), v);
        }
    }
    serde_json::to_string(&JsonValue::Object(out))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

#[tauri::command]
pub fn provider_delete(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let conn = open_db(&app)?;
    conn.execute("DELETE FROM provider_credentials WHERE id = ?", params![id])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

pub fn get_provider_credential(
    app: &tauri::AppHandle,
    id: &str,
) -> Result<crate::chat_manager::types::ProviderCredential, String> {
    let conn = open_db(app).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let mut stmt = conn
        .prepare("SELECT id, provider_id, label, api_key, base_url, default_model, headers, config FROM provider_credentials WHERE id = ?")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let row = stmt
        .query_row(params![id], |r| {
            let id: String = r.get(0)?;
            let provider_id: String = r.get(1)?;
            let label: String = r.get(2)?;
            let api_key: Option<String> = r.get(3)?;
            let base_url: Option<String> = r.get(4)?;
            let default_model: Option<String> = r.get(5)?;
            let headers_json: Option<String> = r.get(6)?;

            let headers = if let Some(s) = headers_json {
                serde_json::from_str(&s).ok()
            } else {
                None
            };

            let config_json: Option<String> = r.get(7)?;
            let config = if let Some(s) = config_json {
                serde_json::from_str(&s).ok()
            } else {
                None
            };

            Ok(crate::chat_manager::types::ProviderCredential {
                id,
                provider_id,
                label,
                api_key,
                base_url,
                default_model,
                headers,
                config,
            })
        })
        .optional()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    row.ok_or_else(|| "Provider credential not found".to_string())
}
