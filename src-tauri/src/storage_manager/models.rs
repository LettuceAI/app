use rusqlite::{params, OptionalExtension};
use serde_json::{Map as JsonMap, Value as JsonValue};

use super::db::{now_ms, open_db};

pub fn model_set_llama_runtime_report(
    app: &tauri::AppHandle,
    model_path: &str,
    report: Option<&JsonValue>,
) -> Result<bool, String> {
    let conn = open_db(app)?;
    let row: Option<(String, Option<String>)> = conn
        .query_row(
            "SELECT id, advanced_model_settings
             FROM models
             WHERE provider_id = 'llamacpp' AND name = ?1
             ORDER BY created_at DESC
             LIMIT 1",
            params![model_path],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let Some((model_id, advanced_json)) = row else {
        return Ok(false);
    };

    let mut advanced_value = advanced_json
        .as_deref()
        .and_then(|value| serde_json::from_str::<JsonValue>(value).ok())
        .unwrap_or_else(|| JsonValue::Object(JsonMap::new()));
    if !advanced_value.is_object() {
        advanced_value = JsonValue::Object(JsonMap::new());
    }
    let advanced_map = advanced_value.as_object_mut().ok_or_else(|| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            "Failed to prepare model advanced settings object",
        )
    })?;

    match report {
        Some(report) => {
            advanced_map.insert("llamaLastRuntimeReport".into(), report.clone());
        }
        None => {
            advanced_map.remove("llamaLastRuntimeReport");
        }
    }

    let updated_advanced_json = serde_json::to_string(&advanced_value)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    conn.execute(
        "UPDATE models SET advanced_model_settings = ?1 WHERE id = ?2",
        params![updated_advanced_json, model_id],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    Ok(true)
}

pub fn model_get_llama_runtime_report(
    app: &tauri::AppHandle,
    model_path: &str,
) -> Result<Option<JsonValue>, String> {
    let conn = open_db(app)?;
    let advanced_json: Option<String> = conn
        .query_row(
            "SELECT advanced_model_settings
             FROM models
             WHERE provider_id = 'llamacpp' AND name = ?1
             ORDER BY created_at DESC
             LIMIT 1",
            params![model_path],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let Some(advanced_json) = advanced_json else {
        return Ok(None);
    };

    let advanced_value = serde_json::from_str::<JsonValue>(&advanced_json)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(advanced_value
        .get("llamaLastRuntimeReport")
        .cloned()
        .filter(|value| !value.is_null()))
}

#[tauri::command]
pub fn model_upsert(app: tauri::AppHandle, model_json: String) -> Result<String, String> {
    let conn = open_db(&app)?;
    let model: JsonValue = serde_json::from_str(&model_json)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let id = model
        .get("id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let name = model
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "name is required".to_string())?;
    let provider_id = model
        .get("providerId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "providerId is required".to_string())?;
    let provider_label = model
        .get("providerLabel")
        .and_then(|v| v.as_str())
        .unwrap_or(provider_id);
    let provider_credential_id = model
        .get("providerCredentialId")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let display_name = model
        .get("displayName")
        .and_then(|v| v.as_str())
        .unwrap_or(name);
    let legacy_model_type = model.get("modelType").and_then(|v| v.as_str());
    let normalize_scopes = |value: JsonValue| -> JsonValue {
        let scope_order = ["text", "image", "audio"];
        let mut scopes: Vec<String> = vec![];
        if let Some(arr) = value.as_array() {
            for v in arr {
                if let Some(s) = v.as_str() {
                    scopes.push(s.to_string());
                }
            }
        }
        if !scopes.iter().any(|s| s.eq_ignore_ascii_case("text")) {
            scopes.push("text".to_string());
        }
        scopes.sort_by_key(|s| {
            scope_order
                .iter()
                .position(|o| o.eq_ignore_ascii_case(s))
                .unwrap_or(scope_order.len())
        });
        scopes.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        JsonValue::Array(scopes.into_iter().map(JsonValue::String).collect())
    };
    let input_scopes = model.get("inputScopes").cloned().unwrap_or_else(|| {
        if legacy_model_type == Some("multimodel") {
            serde_json::json!(["text", "image"])
        } else {
            serde_json::json!(["text"])
        }
    });
    let output_scopes = model.get("outputScopes").cloned().unwrap_or_else(|| {
        if legacy_model_type == Some("imagegeneration") {
            serde_json::json!(["text", "image"])
        } else {
            serde_json::json!(["text"])
        }
    });
    let input_scopes = normalize_scopes(input_scopes);
    let output_scopes = normalize_scopes(output_scopes);
    let adv = model
        .get("advancedModelSettings")
        .map(|v| serde_json::to_string(v).unwrap_or("null".into()));
    let prompt_template_id = model
        .get("promptTemplateId")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let system_prompt = model
        .get("systemPrompt")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let existing_created: Option<i64> = conn
        .query_row(
            "SELECT created_at FROM models WHERE id = ?",
            params![&id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let created_at = existing_created.unwrap_or(now_ms() as i64);
    let provider_credential_id = provider_credential_id.or_else(|| {
        conn.query_row(
            "SELECT id FROM provider_credentials WHERE provider_id = ? AND label = ? LIMIT 1",
            params![provider_id, provider_label],
            |r| r.get::<_, String>(0),
        )
        .optional()
        .ok()
        .flatten()
    });
    let provider_credential_id_for_db = provider_credential_id.clone();
    conn.execute(
        r#"INSERT INTO models (id, name, provider_id, provider_credential_id, provider_label, display_name, created_at, model_type, input_scopes, output_scopes, advanced_model_settings, prompt_template_id, system_prompt)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
              name=excluded.name,
              provider_id=excluded.provider_id,
              provider_credential_id=excluded.provider_credential_id,
              provider_label=excluded.provider_label,
              display_name=excluded.display_name,
              input_scopes=excluded.input_scopes,
              output_scopes=excluded.output_scopes,
              advanced_model_settings=excluded.advanced_model_settings,
              prompt_template_id=excluded.prompt_template_id,
              system_prompt=excluded.system_prompt"#,
        params![
            id,
            name,
            provider_id,
            provider_credential_id_for_db,
            provider_label,
            display_name,
            created_at,
            "chat",
            serde_json::to_string(&input_scopes).unwrap_or("[\"text\"]".into()),
            serde_json::to_string(&output_scopes).unwrap_or("[\"text\"]".into()),
            adv,
            prompt_template_id,
            system_prompt
        ],
    ).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let mut out = JsonMap::new();
    out.insert("id".into(), JsonValue::String(id));
    out.insert("name".into(), JsonValue::String(name.to_string()));
    out.insert(
        "providerId".into(),
        JsonValue::String(provider_id.to_string()),
    );
    if let Some(v) = model
        .get("providerCredentialId")
        .and_then(|v| v.as_str())
        .map(|s| JsonValue::String(s.to_string()))
    {
        out.insert("providerCredentialId".into(), v);
    } else if let Some(id) = provider_credential_id {
        out.insert("providerCredentialId".into(), JsonValue::String(id));
    }
    out.insert(
        "providerLabel".into(),
        JsonValue::String(provider_label.to_string()),
    );
    out.insert(
        "displayName".into(),
        JsonValue::String(display_name.to_string()),
    );
    out.insert("createdAt".into(), JsonValue::from(created_at));
    out.insert("inputScopes".into(), input_scopes);
    out.insert("outputScopes".into(), output_scopes);
    if let Some(v) = model.get("advancedModelSettings").cloned() {
        if !v.is_null() {
            out.insert("advancedModelSettings".into(), v);
        }
    }
    if let Some(v) = model
        .get("promptTemplateId")
        .and_then(|v| v.as_str())
        .map(|s| JsonValue::String(s.to_string()))
    {
        out.insert("promptTemplateId".into(), v);
    }
    if let Some(v) = model
        .get("systemPrompt")
        .and_then(|v| v.as_str())
        .map(|s| JsonValue::String(s.to_string()))
    {
        out.insert("systemPrompt".into(), v);
    }
    Ok(serde_json::to_string(&JsonValue::Object(out))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?)
}

#[tauri::command]
pub fn model_delete(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let conn = open_db(&app)?;
    conn.execute("DELETE FROM models WHERE id = ?", params![id])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

#[tauri::command]
pub fn model_export_as_usc(model_json: String) -> Result<String, String> {
    let model: crate::chat_manager::types::Model =
        serde_json::from_str(&model_json).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Invalid model JSON for export: {}", e),
            )
        })?;
    let card = crate::storage_manager::system_cards::create_model_profile_usc(&model);

    serde_json::to_string_pretty(&card).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to serialize USC model export: {}", e),
        )
    })
}
