use chrono::Local;
use rusqlite::{params, OptionalExtension};
use serde_json::{Map as JsonMap, Value as JsonValue};

use super::{db::now_ms, db::open_db, legacy::read_encrypted_file, legacy::settings_path};
use crate::utils::{log_error, log_info};

fn summarize_app_state_for_logs(app_state: &JsonValue) -> String {
    let onboarding = app_state
        .get("onboarding")
        .and_then(|value| value.as_object());
    let completed = onboarding
        .and_then(|value| value.get("completed"))
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let skipped = onboarding
        .and_then(|value| value.get("skipped"))
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let provider_setup_completed = onboarding
        .and_then(|value| value.get("providerSetupCompleted"))
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let model_setup_completed = onboarding
        .and_then(|value| value.get("modelSetupCompleted"))
        .and_then(|value| value.as_bool())
        .unwrap_or(false);

    format!(
        "onboarding.completed={} onboarding.skipped={} onboarding.providerSetupCompleted={} onboarding.modelSetupCompleted={}",
        completed, skipped, provider_setup_completed, model_setup_completed
    )
}

fn summarize_settings_json_for_logs(json: &JsonValue) -> String {
    let provider_count = json
        .get("providerCredentials")
        .and_then(|value| value.as_array())
        .map(|value| value.len())
        .unwrap_or(0);
    let model_count = json
        .get("models")
        .and_then(|value| value.as_array())
        .map(|value| value.len())
        .unwrap_or(0);
    let default_provider = json
        .get("defaultProviderCredentialId")
        .and_then(|value| value.as_str())
        .unwrap_or("none");
    let default_model = json
        .get("defaultModelId")
        .and_then(|value| value.as_str())
        .unwrap_or("none");
    let advanced_settings_present = json.get("advancedSettings").is_some();
    let app_state_summary = json
        .get("appState")
        .map(summarize_app_state_for_logs)
        .unwrap_or_else(|| "appState=missing".to_string());

    format!(
        "providers={} models={} default_provider={} default_model={} advanced_settings_present={} {}",
        provider_count,
        model_count,
        default_provider,
        default_model,
        advanced_settings_present,
        app_state_summary
    )
}

fn pure_mode_level_from_app_state(app_state: &JsonValue) -> crate::content_filter::PureModeLevel {
    crate::content_filter::level_from_app_state(Some(app_state))
}

fn sync_content_filter_from_app_state(app: &tauri::AppHandle, app_state: &JsonValue) {
    use tauri::Manager;
    if let Some(filter) = app.try_state::<crate::content_filter::ContentFilter>() {
        filter.set_level(pure_mode_level_from_app_state(app_state));
    }
}

fn db_read_settings_json(app: &tauri::AppHandle) -> Result<Option<String>, String> {
    log_info(app, "settings", "Reading settings from DB");
    let conn = open_db(app)?;

    let exists: Option<i64> = conn
        .query_row("SELECT COUNT(*) FROM settings", [], |r| r.get(0))
        .optional()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    log_info(
        app,
        "settings",
        format!("settings row count query returned {}", exists.unwrap_or(0)),
    );
    if exists.unwrap_or(0) == 0 {
        log_info(app, "settings", "settings table is empty; returning None");
        return Ok(None);
    }

    let row = conn
        .query_row(
            "SELECT default_provider_credential_id, default_model_id, app_state, advanced_model_settings, prompt_template_id, system_prompt, migration_version, advanced_settings FROM settings WHERE id = 1",
            [],
            |r| {
                Ok((
                    r.get::<_, Option<String>>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, Option<String>>(5)?,
                    r.get::<_, i64>(6)? as u32,
                    r.get::<_, Option<String>>(7)?,
                ))
            },
        )
        .optional()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let Some((
        default_provider_credential_id,
        default_model_id,
        app_state_json,
        advanced_model_settings_json,
        prompt_template_id,
        system_prompt,
        migration_version,
        advanced_settings_json,
    )) = row
    else {
        log_info(app, "settings", "settings row id=1 not found; returning None");
        return Ok(None);
    };

    // Provider credentials
    let mut stmt = conn
        .prepare("SELECT id, provider_id, label, api_key, base_url, default_model, headers, config FROM provider_credentials")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let creds_iter = stmt
        .query_map([], |r| {
            let id: String = r.get(0)?;
            let provider_id: String = r.get(1)?;
            let label: String = r.get(2)?;
            let api_key: Option<String> = r.get(3)?;
            let base_url: Option<String> = r.get(4)?;
            let default_model: Option<String> = r.get(5)?;
            let headers: Option<String> = r.get(6)?;
            let config: Option<String> = r.get(7)?;
            let mut obj = JsonMap::new();
            obj.insert("id".into(), JsonValue::String(id));
            obj.insert("providerId".into(), JsonValue::String(provider_id));
            obj.insert("label".into(), JsonValue::String(label));
            if let Some(k) = api_key {
                obj.insert("apiKey".into(), JsonValue::String(k));
            }
            if let Some(s) = base_url {
                obj.insert("baseUrl".into(), JsonValue::String(s));
            }
            if let Some(s) = default_model {
                obj.insert("defaultModel".into(), JsonValue::String(s));
            }
            if let Some(s) = headers {
                if let Ok(v) = serde_json::from_str::<JsonValue>(&s) {
                    if !v.is_null() {
                        obj.insert("headers".into(), v);
                    }
                }
            }
            if let Some(s) = config {
                if let Ok(v) = serde_json::from_str::<JsonValue>(&s) {
                    if !v.is_null() {
                        obj.insert("config".into(), v);
                    }
                }
            }
            Ok(JsonValue::Object(obj))
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let mut provider_credentials: Vec<JsonValue> = Vec::new();
    for item in creds_iter {
        provider_credentials
            .push(item.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?);
    }
    log_info(
        app,
        "settings",
        format!("loaded {} provider credentials from DB", provider_credentials.len()),
    );

    // Models
    let mut stmt = conn
        .prepare("SELECT id, name, provider_id, provider_credential_id, provider_label, display_name, created_at, input_scopes, output_scopes, advanced_model_settings, prompt_template_id, system_prompt FROM models ORDER BY created_at ASC")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let models_iter = stmt
        .query_map([], |r| {
            let id: String = r.get(0)?;
            let name: String = r.get(1)?;
            let provider_id: String = r.get(2)?;
            let provider_credential_id: Option<String> = r.get(3)?;
            let provider_label: String = r.get(4)?;
            let display_name: String = r.get(5)?;
            let created_at: i64 = r.get(6)?;
            let input_scopes: Option<String> = r.get(7)?;
            let output_scopes: Option<String> = r.get(8)?;
            let advanced: Option<String> = r.get(9)?;
            let prompt_template_id: Option<String> = r.get(10)?;
            let system_prompt: Option<String> = r.get(11)?;
            let mut obj = JsonMap::new();
            obj.insert("id".into(), JsonValue::String(id));
            obj.insert("name".into(), JsonValue::String(name));
            obj.insert("providerId".into(), JsonValue::String(provider_id));
            if let Some(s) = provider_credential_id {
                obj.insert("providerCredentialId".into(), JsonValue::String(s));
            }
            obj.insert("providerLabel".into(), JsonValue::String(provider_label));
            obj.insert("displayName".into(), JsonValue::String(display_name));
            obj.insert("createdAt".into(), JsonValue::from(created_at));
            if let Some(s) = input_scopes {
                if let Ok(v) = serde_json::from_str::<JsonValue>(&s) {
                    if v.is_array() {
                        obj.insert("inputScopes".into(), v);
                    }
                }
            }
            if let Some(s) = output_scopes {
                if let Ok(v) = serde_json::from_str::<JsonValue>(&s) {
                    if v.is_array() {
                        obj.insert("outputScopes".into(), v);
                    }
                }
            }
            if let Some(s) = advanced {
                if let Ok(v) = serde_json::from_str::<JsonValue>(&s) {
                    obj.insert("advancedModelSettings".into(), v);
                }
            }
            if let Some(s) = prompt_template_id {
                obj.insert("promptTemplateId".into(), JsonValue::String(s));
            }
            if let Some(s) = system_prompt {
                obj.insert("systemPrompt".into(), JsonValue::String(s));
            }
            Ok(JsonValue::Object(obj))
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let mut models: Vec<JsonValue> = Vec::new();
    for item in models_iter {
        models.push(item.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?);
    }
    log_info(
        app,
        "settings",
        format!("loaded {} models from DB", models.len()),
    );

    let app_state: JsonValue = serde_json::from_str(&app_state_json).unwrap_or_else(|_| serde_json::json!({
        "onboarding": {"completed": false, "skipped": false, "providerSetupCompleted": false, "modelSetupCompleted": false},
        "theme": "light",
        "tooltips": {},
        "pureModeEnabled": true,
        "pureModeLevel": "standard",
        "analyticsEnabled": true,
        "appActiveUsageMs": 0,
        "appActiveUsageByDayMs": {}
    }));

    let mut root = JsonMap::new();
    root.insert("$version".into(), JsonValue::from(2));
    root.insert(
        "defaultProviderCredentialId".into(),
        default_provider_credential_id
            .map(JsonValue::String)
            .unwrap_or(JsonValue::Null),
    );
    root.insert(
        "defaultModelId".into(),
        default_model_id
            .map(JsonValue::String)
            .unwrap_or(JsonValue::Null),
    );
    root.insert(
        "providerCredentials".into(),
        JsonValue::Array(provider_credentials),
    );
    root.insert("models".into(), JsonValue::Array(models));
    root.insert("appState".into(), app_state);
    if let Some(s) = advanced_model_settings_json {
        if let Ok(v) = serde_json::from_str::<JsonValue>(&s) {
            if !v.is_null() {
                root.insert("advancedModelSettings".into(), v);
            }
        }
    }
    if let Some(id) = prompt_template_id {
        root.insert("promptTemplateId".into(), JsonValue::String(id));
    }
    if let Some(sp) = system_prompt {
        root.insert("systemPrompt".into(), JsonValue::String(sp));
    }
    root.insert(
        "migrationVersion".into(),
        JsonValue::from(migration_version),
    );
    if let Some(s) = advanced_settings_json {
        if let Ok(v) = serde_json::from_str::<JsonValue>(&s) {
            if !v.is_null() {
                root.insert("advancedSettings".into(), v);
            }
        }
    }

    let root_value = JsonValue::Object(root);
    log_info(
        app,
        "settings",
        format!(
            "assembled settings payload from DB: {}",
            summarize_settings_json_for_logs(&root_value)
        ),
    );

    Ok(Some(
        serde_json::to_string(&root_value)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
    ))
}

fn db_write_settings_json(app: &tauri::AppHandle, data: String) -> Result<(), String> {
    log_info(app, "settings", "Writing settings to DB");
    let mut conn = open_db(app)?;
    let now = now_ms() as i64;
    let json: JsonValue = serde_json::from_str(&data).map_err(|e| {
        log_error(
            app,
            "settings",
            format!("Failed to parse settings JSON: {}", e),
        );
        e.to_string()
    })?;
    log_info(
        app,
        "settings",
        format!(
            "parsed settings JSON for write: {}",
            summarize_settings_json_for_logs(&json)
        ),
    );

    let default_provider_credential_id = json
        .get("defaultProviderCredentialId")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let default_model_id = json
        .get("defaultModelId")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let app_state_str = json
        .get("appState")
        .map(|v| serde_json::to_string(v).unwrap_or("{}".into()))
        .unwrap_or("{}".into());
    let advanced_model_settings = json.get("advancedModelSettings").and_then(|v| {
        if v.is_null() {
            None
        } else {
            Some(serde_json::to_string(v).unwrap_or("null".into()))
        }
    });
    let prompt_template_id = json
        .get("promptTemplateId")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let system_prompt = json
        .get("systemPrompt")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let migration_version = json
        .get("migrationVersion")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as i64;
    let adv_settings = json.get("advancedSettings").and_then(|v| {
        if v.is_null() {
            None
        } else {
            Some(serde_json::to_string(v).unwrap_or("null".into()))
        }
    });

    let tx = conn
        .transaction()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let settings_rows = tx.execute(
        r#"INSERT INTO settings (id, default_provider_credential_id, default_model_id, app_state, advanced_model_settings, prompt_template_id, system_prompt, migration_version, advanced_settings, created_at, updated_at)
            VALUES (1, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
              default_provider_credential_id=excluded.default_provider_credential_id,
              default_model_id=excluded.default_model_id,
              app_state=excluded.app_state,
              advanced_model_settings=excluded.advanced_model_settings,
              prompt_template_id=excluded.prompt_template_id,
              system_prompt=excluded.system_prompt,
              migration_version=excluded.migration_version,
              advanced_settings=excluded.advanced_settings,
              updated_at=excluded.updated_at"#,
        params![
            default_provider_credential_id,
            default_model_id,
            app_state_str,
            advanced_model_settings,
            prompt_template_id,
            system_prompt,
            migration_version,
            adv_settings,
            now,
            now,
        ],
    ).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    log_info(
        app,
        "settings",
        format!("settings table upsert affected {} row(s)", settings_rows),
    );

    let deleted_provider_rows = tx.execute("DELETE FROM provider_credentials", [])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    log_info(
        app,
        "settings",
        format!(
            "deleted {} existing provider credential row(s) before rewrite",
            deleted_provider_rows
        ),
    );
    if let Some(creds) = json.get("providerCredentials").and_then(|v| v.as_array()) {
        log_info(
            app,
            "settings",
            format!("rewriting {} provider credential row(s)", creds.len()),
        );
        for c in creds {
            let id = c.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let provider_id = c.get("providerId").and_then(|v| v.as_str()).unwrap_or("");
            let label = c.get("label").and_then(|v| v.as_str()).unwrap_or("");
            let api_key = c
                .get("apiKey")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let base_url = c
                .get("baseUrl")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let default_model = c
                .get("defaultModel")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let headers = c
                .get("headers")
                .map(|v| serde_json::to_string(v).unwrap_or("null".into()));
            let config = c
                .get("config")
                .map(|v| serde_json::to_string(v).unwrap_or("null".into()));
            tx.execute(
                "INSERT INTO provider_credentials (id, provider_id, label, api_key, base_url, default_model, headers, config) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                params![id, provider_id, label, api_key, base_url, default_model, headers, config],
            ).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            log_info(
                app,
                "settings",
                format!(
                    "wrote provider credential id={} provider_id={} label={} has_api_key={} has_base_url={}",
                    id,
                    provider_id,
                    label,
                    c.get("apiKey").and_then(|v| v.as_str()).is_some(),
                    c.get("baseUrl").and_then(|v| v.as_str()).is_some()
                ),
            );
        }
    }

    let deleted_model_rows = tx.execute("DELETE FROM models", [])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    log_info(
        app,
        "settings",
        format!("deleted {} existing model row(s) before rewrite", deleted_model_rows),
    );
    if let Some(models) = json.get("models").and_then(|v| v.as_array()) {
        log_info(
            app,
            "settings",
            format!("rewriting {} model row(s)", models.len()),
        );
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

        for m in models {
            let id = m.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let name = m.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let provider_id = m.get("providerId").and_then(|v| v.as_str()).unwrap_or("");
            let provider_credential_id = m
                .get("providerCredentialId")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let provider_label = m
                .get("providerLabel")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let display_name = m
                .get("displayName")
                .and_then(|v| v.as_str())
                .unwrap_or(name);
            let created_at = m.get("createdAt").and_then(|v| v.as_i64()).unwrap_or(now);
            let legacy_model_type = m.get("modelType").and_then(|v| v.as_str());
            let input_scopes = m.get("inputScopes").cloned().unwrap_or_else(|| {
                if legacy_model_type == Some("multimodel") {
                    serde_json::json!(["text", "image"])
                } else {
                    serde_json::json!(["text"])
                }
            });
            let output_scopes = m.get("outputScopes").cloned().unwrap_or_else(|| {
                if legacy_model_type == Some("imagegeneration") {
                    serde_json::json!(["text", "image"])
                } else {
                    serde_json::json!(["text"])
                }
            });
            let input_scopes = normalize_scopes(input_scopes);
            let output_scopes = normalize_scopes(output_scopes);
            let adv = m
                .get("advancedModelSettings")
                .map(|v| serde_json::to_string(v).unwrap_or("null".into()));
            let prompt_template_id = m
                .get("promptTemplateId")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let system_prompt = m
                .get("systemPrompt")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            tx.execute(
                "INSERT INTO models (id, name, provider_id, provider_credential_id, provider_label, display_name, created_at, model_type, input_scopes, output_scopes, advanced_model_settings, prompt_template_id, system_prompt) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    id,
                    name,
                    provider_id,
                    provider_credential_id,
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
            log_info(
                app,
                "settings",
                format!(
                    "wrote model id={} provider_id={} provider_credential_id={} name={}",
                    id,
                    provider_id,
                    provider_credential_id.as_deref().unwrap_or("none"),
                    name
                ),
            );
        }
    }
    tx.commit()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    log_info(app, "settings", "settings DB transaction committed successfully");
    Ok(())
}

pub fn read_settings_typed<T>(app: &tauri::AppHandle) -> Result<Option<T>, String>
where
    T: serde::de::DeserializeOwned,
{
    db_read_settings_json(app)?
        .map(|data| {
            serde_json::from_str(&data)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
        })
        .transpose()
}

pub fn write_settings_typed<T>(app: &tauri::AppHandle, value: &T) -> Result<(), String>
where
    T: serde::Serialize,
{
    let data = serde_json::to_string(value)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    db_write_settings_json(app, data.clone())?;
    if let Ok(json) = serde_json::from_str::<JsonValue>(&data) {
        if let Some(app_state) = json.get("appState") {
            sync_content_filter_from_app_state(app, app_state);
        }
    }
    Ok(())
}

fn ensure_settings_row(conn: &rusqlite::Connection) -> Result<(), String> {
    let now = now_ms() as i64;
    conn.execute(
        r#"INSERT INTO settings (id, app_state, created_at, updated_at)
            VALUES (1, '{}', ?, ?)
            ON CONFLICT(id) DO NOTHING"#,
        params![now, now],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

pub fn settings_increment_app_active_usage_ms(
    app: &tauri::AppHandle,
    delta_ms: u64,
) -> Result<(), String> {
    if delta_ms == 0 {
        return Ok(());
    }

    let conn = open_db(app)?;
    ensure_settings_row(&conn)?;

    let app_state_json: String = conn
        .query_row("SELECT app_state FROM settings WHERE id = 1", [], |r| {
            r.get(0)
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let mut app_state: JsonValue = serde_json::from_str(&app_state_json).unwrap_or(JsonValue::Null);
    let app_state_obj = if let Some(obj) = app_state.as_object_mut() {
        obj
    } else {
        app_state = JsonValue::Object(JsonMap::new());
        app_state
            .as_object_mut()
            .ok_or_else(|| "failed to initialize app_state object".to_string())?
    };

    let now = now_ms() as u64;
    let current_total = app_state_obj
        .get("appActiveUsageMs")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    app_state_obj.insert(
        "appActiveUsageMs".into(),
        JsonValue::from(current_total.saturating_add(delta_ms)),
    );
    let day_key = Local::now().format("%Y-%m-%d").to_string();
    let day_obj = app_state_obj
        .entry("appActiveUsageByDayMs")
        .or_insert_with(|| JsonValue::Object(JsonMap::new()));
    let day_map = if let Some(map) = day_obj.as_object_mut() {
        map
    } else {
        *day_obj = JsonValue::Object(JsonMap::new());
        day_obj
            .as_object_mut()
            .ok_or_else(|| "failed to initialize appActiveUsageByDayMs object".to_string())?
    };
    let current_day_total = day_map.get(&day_key).and_then(|v| v.as_u64()).unwrap_or(0);
    day_map.insert(
        day_key,
        JsonValue::from(current_day_total.saturating_add(delta_ms)),
    );

    app_state_obj
        .entry("appActiveUsageStartedAtMs")
        .or_insert_with(|| JsonValue::from(now));
    app_state_obj.insert("appActiveUsageLastUpdatedAtMs".into(), JsonValue::from(now));

    let now_i64 = now as i64;
    let next_app_state_json = serde_json::to_string(&app_state)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    conn.execute(
        "UPDATE settings SET app_state = ?, updated_at = ? WHERE id = 1",
        params![next_app_state_json, now_i64],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    crate::utils::log_info(
        app,
        "app_active_usage",
        format!(
            "Incremented active usage by {}ms, new total is {}ms",
            delta_ms,
            current_total.saturating_add(delta_ms)
        ),
    );

    Ok(())
}

#[tauri::command]
pub fn storage_read_settings(app: tauri::AppHandle) -> Result<Option<String>, String> {
    if let Some(v) = db_read_settings_json(&app)? {
        return Ok(Some(v));
    }
    let path = settings_path(&app)?;
    if let Some(json) = read_encrypted_file(&path)? {
        let _ = db_write_settings_json(&app, json.clone());
        return db_read_settings_json(&app);
    }
    Ok(None)
}

#[tauri::command]
pub fn storage_write_settings(app: tauri::AppHandle, data: String) -> Result<(), String> {
    db_write_settings_json(&app, data.clone())?;
    if let Ok(json) = serde_json::from_str::<JsonValue>(&data) {
        if let Some(app_state) = json.get("appState") {
            sync_content_filter_from_app_state(&app, app_state);
        }
    }
    Ok(())
}

#[tauri::command]
pub fn analytics_is_available() -> bool {
    std::env::var("APTABASE_KEY")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .or_else(|| option_env!("APTABASE_KEY").map(|v| v.to_string()))
        .is_some()
}

// Internal helper used by some backend modules
pub fn internal_read_settings(app: &tauri::AppHandle) -> Result<Option<String>, String> {
    db_read_settings_json(app)
}

#[tauri::command]
pub fn settings_set_defaults(
    app: tauri::AppHandle,
    default_provider_credential_id: Option<String>,
    default_model_id: Option<String>,
) -> Result<(), String> {
    let conn = open_db(&app)?;
    ensure_settings_row(&conn)?;
    let now = now_ms() as i64;
    conn.execute(
        "UPDATE settings SET default_provider_credential_id = ?, default_model_id = ?, updated_at = ? WHERE id = 1",
        params![default_provider_credential_id, default_model_id, now],
    ).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

#[tauri::command]
pub fn settings_set_advanced(app: tauri::AppHandle, advanced_json: String) -> Result<(), String> {
    let conn = open_db(&app)?;
    ensure_settings_row(&conn)?;
    let now = now_ms() as i64;
    let db_val: Option<String> = {
        let s = advanced_json.trim();
        if s.is_empty() || s == "null" {
            None
        } else {
            Some(s.to_string())
        }
    };
    conn.execute(
        "UPDATE settings SET advanced_settings = ?, updated_at = ? WHERE id = 1",
        params![db_val, now],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

#[tauri::command]
pub fn settings_set_advanced_model_settings(
    app: tauri::AppHandle,
    advanced_json: String,
) -> Result<(), String> {
    let conn = open_db(&app)?;
    ensure_settings_row(&conn)?;
    let now = now_ms() as i64;
    let db_val: Option<String> = {
        let s = advanced_json.trim();
        if s.is_empty() || s == "null" {
            None
        } else {
            Some(s.to_string())
        }
    };
    conn.execute(
        "UPDATE settings SET advanced_model_settings = ?, updated_at = ? WHERE id = 1",
        params![db_val, now],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

#[tauri::command]
pub fn settings_set_default_provider(
    app: tauri::AppHandle,
    id: Option<String>,
) -> Result<(), String> {
    let conn = open_db(&app)?;
    ensure_settings_row(&conn)?;
    let now = now_ms() as i64;
    conn.execute(
        "UPDATE settings SET default_provider_credential_id = ?, updated_at = ? WHERE id = 1",
        params![id, now],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

#[tauri::command]
pub fn settings_set_default_model(app: tauri::AppHandle, id: Option<String>) -> Result<(), String> {
    let conn = open_db(&app)?;
    ensure_settings_row(&conn)?;
    let now = now_ms() as i64;
    conn.execute(
        "UPDATE settings SET default_model_id = ?, updated_at = ? WHERE id = 1",
        params![id, now],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

#[tauri::command]
pub fn settings_set_app_state(app: tauri::AppHandle, state_json: String) -> Result<(), String> {
    let conn = open_db(&app)?;
    ensure_settings_row(&conn)?;

    let merged_state_json = {
        let incoming = serde_json::from_str::<JsonValue>(&state_json).ok();
        let existing_json: Option<String> = conn
            .query_row("SELECT app_state FROM settings WHERE id = 1", [], |r| {
                r.get(0)
            })
            .optional()
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let existing = existing_json
            .as_deref()
            .and_then(|raw| serde_json::from_str::<JsonValue>(raw).ok());

        match (incoming, existing) {
            (Some(mut incoming_value), Some(existing_value)) => {
                if let (Some(incoming_obj), Some(existing_obj)) =
                    (incoming_value.as_object_mut(), existing_value.as_object())
                {
                    for key in [
                        "appActiveUsageMs",
                        "appActiveUsageByDayMs",
                        "appActiveUsageStartedAtMs",
                        "appActiveUsageLastUpdatedAtMs",
                    ] {
                        if !incoming_obj.contains_key(key) {
                            if let Some(existing_value) = existing_obj.get(key) {
                                incoming_obj.insert(key.into(), existing_value.clone());
                            }
                        }
                    }
                }
                serde_json::to_string(&incoming_value)
                    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
            }
            _ => state_json,
        }
    };

    let now = now_ms() as i64;
    conn.execute(
        "UPDATE settings SET app_state = ?, updated_at = ? WHERE id = 1",
        params![merged_state_json, now],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    if let Ok(app_state) = serde_json::from_str::<JsonValue>(&merged_state_json) {
        sync_content_filter_from_app_state(&app, &app_state);
    }
    Ok(())
}

#[tauri::command]
pub fn settings_set_prompt_template(
    app: tauri::AppHandle,
    id: Option<String>,
) -> Result<(), String> {
    let conn = open_db(&app)?;
    ensure_settings_row(&conn)?;
    let now = now_ms() as i64;
    conn.execute(
        "UPDATE settings SET prompt_template_id = ?, updated_at = ? WHERE id = 1",
        params![id, now],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

#[tauri::command]
pub fn settings_set_system_prompt(
    app: tauri::AppHandle,
    prompt: Option<String>,
) -> Result<(), String> {
    let conn = open_db(&app)?;
    ensure_settings_row(&conn)?;
    let now = now_ms() as i64;
    conn.execute(
        "UPDATE settings SET system_prompt = ?, updated_at = ? WHERE id = 1",
        params![prompt, now],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

#[tauri::command]
pub fn settings_set_migration_version(app: tauri::AppHandle, version: i64) -> Result<(), String> {
    let conn = open_db(&app)?;
    ensure_settings_row(&conn)?;
    let now = now_ms() as i64;
    conn.execute(
        "UPDATE settings SET migration_version = ?, updated_at = ? WHERE id = 1",
        params![version, now],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}
