use serde_json::Value;
use tauri::AppHandle;

use crate::chat_manager::prompts;
use crate::chat_manager::types::PromptScope;
use crate::storage_manager::{settings::storage_read_settings, settings::storage_write_settings};
use crate::utils::log_info;

/// Current migration version
pub const CURRENT_MIGRATION_VERSION: u32 = 36;

pub fn run_migrations(app: &AppHandle) -> Result<(), String> {
    log_info(app, "migrations", "Starting migration check");

    let current_version = get_migration_version(app)?;

    if current_version >= CURRENT_MIGRATION_VERSION {
        migrate_v29_to_v30(app)?;
        migrate_v30_to_v31(app)?;
        migrate_v31_to_v32(app)?;
        migrate_v32_to_v33(app)?;
        migrate_v33_to_v34(app)?;
        migrate_v34_to_v35(app)?;
        migrate_v35_to_v36(app)?;
        log_info(
            app,
            "migrations",
            format!(
                "No migrations needed (current: {}, latest: {})",
                current_version, CURRENT_MIGRATION_VERSION
            ),
        );
        return Ok(());
    }

    log_info(
        app,
        "migrations",
        format!(
            "Running migrations from version {} to {}",
            current_version, CURRENT_MIGRATION_VERSION
        ),
    );

    // Run migrations sequentially
    let mut version = current_version;

    if version < 1 {
        log_info(
            app,
            "migrations",
            "Running migration v0 -> v1: Add custom prompt fields",
        );
        migrate_v0_to_v1(app)?;
        version = 1;
    }

    if version < 2 {
        log_info(
            app,
            "migrations",
            "Running migration v1 -> v2: Convert prompts to template system",
        );
        migrate_v1_to_v2(app)?;
        version = 2;
    }

    if version < 3 {
        log_info(
            app,
            "migrations",
            "Running migration v2 -> v3: Normalize templates to global prompts (no scopes)",
        );
        migrate_v2_to_v3(app)?;
        version = 3;
    }

    // Future migrations go here:
    if version < 4 {
        log_info(
            app,
            "migrations",
            "Running migration v3 -> v4: Move secrets to SQLite (from secrets.json)",
        );
        migrate_v3_to_v4(app)?;
        version = 4;
    }

    if version < 5 {
        log_info(
            app,
            "migrations",
            "Running migration v4 -> v5: Move prompt templates to SQLite (from prompt_templates.json)",
        );
        migrate_v4_to_v5(app)?;
        version = 5;
    }

    if version < 6 {
        log_info(
            app,
            "migrations",
            "Running migration v5 -> v6: Move model pricing cache to SQLite (from models_cache.json)",
        );
        migrate_v5_to_v6(app)?;
        version = 6;
    }

    if version < 7 {
        log_info(
            app,
            "migrations",
            "Running migration v6 -> v7: Add api_key column to provider_credentials and backfill",
        );
        migrate_v6_to_v7(app)?;
        version = 7;
    }

    if version < 8 {
        log_info(
            app,
            "migrations",
            "Running migration v7 -> v8: Add memories column to sessions table",
        );
        migrate_v7_to_v8(app)?;
        version = 8;
    }

    if version < 9 {
        log_info(
            app,
            "migrations",
            "Running migration v8 -> v9: Add advanced_settings column to settings table",
        );
        migrate_v8_to_v9(app)?;
        version = 9;
    }

    if version < 10 {
        log_info(
            app,
            "migrations",
            "Running migration v9 -> v10: Add memory_type to characters",
        );
        migrate_v9_to_v10(app)?;
        version = 10;
    }

    if version < 11 {
        log_info(
            app,
            "migrations",
            "Running migration v10 -> v11: Add memory_embeddings to sessions",
        );
        migrate_v10_to_v11(app)?;
        version = 11;
    }

    if version < 12 {
        log_info(
            app,
            "migrations",
            "Running migration v11 -> v12: Add memory summary and tool events to sessions",
        );
        migrate_v11_to_v12(app)?;
        version = 12;
    }

    if version < 13 {
        log_info(
            app,
            "migrations",
            "Running migration v12 -> v13: Add operation_type to usage_records",
        );
        migrate_v12_to_v13(app)?;
        version = 13;
    }

    if version < 14 {
        log_info(
            app,
            "migrations",
            "Running migration v13 -> v14: Add model_type to models",
        );
        migrate_v13_to_v14(app)?;
        version = 14;
    }

    if version < 15 {
        log_info(
            app,
            "migrations",
            "Running migration v14 -> v15: Add attachments column to messages",
        );
        migrate_v14_to_v15(app)?;
        version = 15;
    }

    if version < 16 {
        log_info(
            app,
            "migrations",
            "Running migration v15 -> v16: Backfill token_count for existing memory embeddings and add usage token breakdown",
        );
        migrate_v15_to_v16(app)?;
        version = 16;
    }

    if version < 17 {
        log_info(
            app,
            "migrations",
            "Running migration v16 -> v17: Add memory_tokens and summary_tokens to usage_records",
        );
        migrate_v16_to_v17(app)?;
        version = 17;
    }

    if version < 18 {
        log_info(
            app,
            "migrations",
            "Running migration v17 -> v18: Add custom gradient columns to characters",
        );
        migrate_v17_to_v18(app)?;
        version = 18;
    }

    if version < 19 {
        log_info(
            app,
            "migrations",
            "Running migration v18 -> v19: Add model input/output scopes",
        );
        migrate_v18_to_v19(app)?;
        version = 19;
    }

    if version < 20 {
        log_info(
            app,
            "migrations",
            "Running migration v19 -> v20: Convert lorebooks to app-level",
        );
        migrate_v19_to_v20(app)?;
        version = 20;
    }

    if version < 21 {
        log_info(
            app,
            "migrations",
            "Running migration v20 -> v21: Add config column to provider_credentials",
        );
        migrate_v20_to_v21(app)?;
        version = 21;
    }

    if version < 22 {
        log_info(
            app,
            "migrations",
            "Running migration v21 -> v22: Add direction column to scenes and scene_variants",
        );
        migrate_v21_to_v22(app)?;
        version = 22;
    }

    if version < 23 {
        log_info(
            app,
            "migrations",
            "Running migration v22 -> v23: Add finish_reason column to usage_records",
        );
        migrate_v22_to_v23(app)?;
        version = 23;
    }

    if version < 24 {
        log_info(
            app,
            "migrations",
            "Running migration v23 -> v24: Add memory columns to group_sessions",
        );
        migrate_v23_to_v24(app)?;
        version = 24;
    }

    if version < 25 {
        log_info(
            app,
            "migrations",
            "Running migration v24 -> v25: Add archived column to group_sessions",
        );
        migrate_v24_to_v25(app)?;
        version = 25;
    }

    if version < 26 {
        log_info(
            app,
            "migrations",
            "Running migration v25 -> v26: Add group session memory tool events",
        );
        migrate_v25_to_v26(app)?;
        version = 26;
    }

    if version < 27 {
        log_info(
            app,
            "migrations",
            "Running migration v26 -> v27: Add model_id to group messages",
        );
        migrate_v26_to_v27(app)?;
        version = 27;
    }

    if version < 28 {
        log_info(
            app,
            "migrations",
            "Running migration v27 -> v28: Add chat_type and starting_scene to group_sessions",
        );
        migrate_v27_to_v28(app)?;
        version = 28;
    }

    if version < 29 {
        log_info(
            app,
            "migrations",
            "Running migration v28 -> v29: Add background_image_path to group_sessions",
        );
        migrate_v28_to_v29(app)?;
        version = 29;
    }

    if version < 30 {
        log_info(
            app,
            "migrations",
            "Running migration v29 -> v30: Add definition column to characters",
        );
        migrate_v29_to_v30(app)?;
        version = 30;
    }

    if version < 31 {
        log_info(
            app,
            "migrations",
            "Running migration v30 -> v31: Add avatar crop columns",
        );
        migrate_v30_to_v31(app)?;
        version = 31;
    }

    if version < 32 {
        log_info(
            app,
            "migrations",
            "Running migration v31 -> v32: Remove model-level prompts",
        );
        migrate_v31_to_v32(app)?;
        version = 32;
    }

    if version < 33 {
        log_info(
            app,
            "migrations",
            "Running migration v32 -> v33: Add Smart Creator session persistence table",
        );
        migrate_v32_to_v33(app)?;
        version = 33;
    }

    if version < 34 {
        log_info(
            app,
            "migrations",
            "Running migration v33 -> v34: Add character metadata columns",
        );
        migrate_v33_to_v34(app)?;
        version = 34;
    }

    if version < 35 {
        log_info(
            app,
            "migrations",
            "Running migration v34 -> v35: Add speaker_selection_method to group_sessions",
        );
        migrate_v34_to_v35(app)?;
        version = 35;
    }

    if version < 36 {
        log_info(
            app,
            "migrations",
            "Running migration v35 -> v36: Add chat_appearance to characters",
        );
        migrate_v35_to_v36(app)?;
        version = 36;
    }

    // Update the stored version
    set_migration_version(app, version)?;

    log_info(
        app,
        "migrations",
        format!(
            "Migrations completed successfully. Now at version {}",
            version
        ),
    );

    cleanup_legacy_files(app);

    Ok(())
}

fn cleanup_legacy_files(app: &AppHandle) {
    use std::fs;
    if let Ok(dir) = crate::utils::ensure_lettuce_dir(app) {
        let candidates = ["secrets.json", "prompt_templates.json"];
        for name in candidates.iter() {
            let path = dir.join(name);
            if path.exists() {
                let _ = fs::remove_file(&path);
            }
        }
    }
}

/// Get the current migration version
fn get_migration_version(app: &AppHandle) -> Result<u32, String> {
    use crate::storage_manager::db::open_db;

    let conn = open_db(app)?;

    // Check if settings table exists first (it should if init_db ran)
    let count: i32 = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='settings'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if count == 0 {
        return Ok(0);
    }

    let version: u32 = conn
        .query_row(
            "SELECT migration_version FROM settings WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    Ok(version)
}

/// Set the migration version
fn set_migration_version(app: &AppHandle, version: u32) -> Result<(), String> {
    use crate::storage_manager::db::{now_ms, open_db};
    use rusqlite::params;

    let conn = open_db(app)?;
    let now = now_ms();

    // Ensure row exists (it should)
    conn.execute(
        "UPDATE settings SET migration_version = ?1, updated_at = ?2 WHERE id = 1",
        params![version, now],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    Ok(())
}

/// Migration v0 -> v1: Add system_prompt field to Settings, Model, and Character
///
/// This migration ensures all existing data structures have the new optional
/// system_prompt field. Since Rust uses #[serde(default)], this field will
/// automatically deserialize as None for old data, but we update the settings
/// file to explicitly include it for consistency.
fn migrate_v0_to_v1(app: &AppHandle) -> Result<(), String> {
    // Settings migration - add systemPrompt field if missing
    if let Ok(Some(settings_json)) = storage_read_settings(app.clone()) {
        let mut settings: Value = serde_json::from_str(&settings_json).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to parse settings: {}", e),
            )
        })?;

        let mut changed = false;

        // Add systemPrompt to root settings if not present
        if let Some(obj) = settings.as_object_mut() {
            if !obj.contains_key("systemPrompt") {
                obj.insert("systemPrompt".to_string(), Value::Null);
                changed = true;
                log_info(app, "migrations", "Added systemPrompt to settings");
            }

            // Add systemPrompt to all models if not present
            if let Some(models) = obj.get_mut("models").and_then(|v| v.as_array_mut()) {
                for model in models.iter_mut() {
                    if let Some(model_obj) = model.as_object_mut() {
                        if !model_obj.contains_key("systemPrompt") {
                            model_obj.insert("systemPrompt".to_string(), Value::Null);
                            changed = true;
                        }
                    }
                }
                if changed {
                    log_info(
                        app,
                        "migrations",
                        format!("Added systemPrompt to {} models", models.len()),
                    );
                }
            }
        }

        if changed {
            storage_write_settings(
                app.clone(),
                serde_json::to_string(&settings)
                    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
            )?;
            log_info(app, "migrations", "Settings migration completed");
        }
    }

    // Characters migration - add systemPrompt field if missing
    // Note: Characters are stored individually, so we'd need to iterate through all character files
    // Since Rust's serde will handle missing fields with #[serde(default)], we rely on that
    // The field will be automatically added when characters are saved next time
    log_info(
        app,
        "migrations",
        "Character systemPrompt fields will be added on next save (handled by serde defaults)",
    );

    Ok(())
}

/// Migration v3 -> v4: move secrets from JSON file to SQLite `secrets` table
fn migrate_v3_to_v4(app: &AppHandle) -> Result<(), String> {
    use rusqlite::params;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;
    use std::fs;

    use crate::storage_manager::db::{now_ms, open_db};
    use crate::utils::lettuce_dir;

    #[derive(Serialize, Deserialize, Default)]
    struct SecretsFile {
        entries: HashMap<String, String>,
    }

    // Locate old JSON file
    let dir = lettuce_dir(app)?;
    let old_path = dir.join("secrets.json");
    if !old_path.exists() {
        // Nothing to migrate
        return Ok(());
    }

    // Read and parse JSON
    let raw = fs::read_to_string(&old_path)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    if raw.trim().is_empty() {
        // Empty file; safe to remove
        let _ = fs::remove_file(&old_path);
        return Ok(());
    }
    let secrets: SecretsFile = serde_json::from_str(&raw)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    // Upsert into DB
    let mut conn = open_db(app)?;
    let now = now_ms();
    let tx = conn
        .transaction()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    for (k, v) in secrets.entries.iter() {
        // keys are formatted as "service|account"
        if let Some((service, account)) = k.split_once('|') {
            tx.execute(
                "INSERT INTO secrets (service, account, value, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?4)
                 ON CONFLICT(service, account) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
                params![service, account, v, now],
            )
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        }
    }
    tx.commit()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    // Backup old file
    let _ = fs::rename(&old_path, dir.join("secrets.json.bak"));
    Ok(())
}

/// Migration v4 -> v5: move prompt templates from JSON file to SQLite table
fn migrate_v4_to_v5(app: &AppHandle) -> Result<(), String> {
    use rusqlite::params;
    use std::fs;

    use crate::chat_manager::types::{PromptScope, SystemPromptTemplate};
    use crate::storage_manager::db::open_db;
    use crate::utils::ensure_lettuce_dir;

    // JSON file path
    let path = ensure_lettuce_dir(app)?.join("prompt_templates.json");
    if !path.exists() {
        return Ok(());
    }

    // The JSON file format: { templates: SystemPromptTemplate[] }
    #[derive(serde::Deserialize)]
    struct PromptTemplatesFile {
        templates: Vec<SystemPromptTemplate>,
    }

    let content = fs::read_to_string(&path)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let file: PromptTemplatesFile = serde_json::from_str(&content)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let mut conn = open_db(app)?;
    let tx = conn
        .transaction()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    for t in file.templates.iter() {
        let scope_str = match t.scope {
            PromptScope::AppWide => "AppWide",
            PromptScope::ModelSpecific => "ModelSpecific",
            PromptScope::CharacterSpecific => "CharacterSpecific",
        };
        let target_ids_json = serde_json::to_string(&t.target_ids)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        tx.execute(
            "INSERT OR REPLACE INTO prompt_templates (id, name, scope, target_ids, content, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                t.id,
                t.name,
                scope_str,
                target_ids_json,
                t.content,
                t.created_at,
                t.updated_at
            ],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    tx.commit()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    // Backup the old JSON file
    let _ = fs::rename(&path, path.with_extension("json.bak"));
    Ok(())
}

/// Migration v5 -> v6: move pricing cache from models_cache.json to SQLite table
fn migrate_v5_to_v6(app: &AppHandle) -> Result<(), String> {
    use rusqlite::params;
    use std::collections::HashMap;
    use std::fs;

    use crate::models::ModelPricing;
    use crate::storage_manager::db::open_db;
    use crate::utils::ensure_lettuce_dir;

    #[derive(serde::Deserialize)]
    struct ModelsCacheEntry {
        _id: String,
        pricing: Option<ModelPricing>,
        cached_at: u64,
    }

    #[derive(serde::Deserialize, Default)]
    struct ModelsCacheFile {
        models: HashMap<String, ModelsCacheEntry>,
        _last_updated: u64,
    }

    let path = ensure_lettuce_dir(app)?.join("models_cache.json");
    if !path.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(&path)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    if content.trim().is_empty() {
        let _ = fs::remove_file(&path);
        return Ok(());
    }
    let file: ModelsCacheFile = serde_json::from_str(&content)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let mut conn = open_db(app)?;
    let tx = conn
        .transaction()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    for (model_id, entry) in file.models.iter() {
        let pricing_json = match &entry.pricing {
            Some(p) => Some(
                serde_json::to_string(p)
                    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
            ),
            None => None,
        };
        tx.execute(
            "INSERT OR REPLACE INTO model_pricing_cache (model_id, pricing_json, cached_at) VALUES (?1, ?2, ?3)",
            params![model_id, pricing_json, entry.cached_at],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }
    tx.commit()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let _ = fs::rename(&path, path.with_extension("json.bak"));
    Ok(())
}

/// Migration v6 -> v7: add provider_credentials.api_key and backfill from secrets
fn migrate_v6_to_v7(app: &AppHandle) -> Result<(), String> {
    use crate::storage_manager::db::open_db;
    use rusqlite::{params, OptionalExtension};

    let conn = open_db(app)?;
    // Add column if it doesn't exist
    let _ = conn.execute(
        "ALTER TABLE provider_credentials ADD COLUMN api_key TEXT",
        [],
    );

    // Backfill using secrets table convention: service = 'lettuceai:apiKey', account = '{provider_id}:{cred_id}'
    // For each credential row, attempt to set api_key from secrets if missing
    let mut stmt = conn
        .prepare("SELECT id, provider_id FROM provider_credentials WHERE api_key IS NULL OR api_key = ''")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    for row in rows {
        let (cred_id, provider_id) =
            row.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let account = format!("{}:{}", provider_id, cred_id);
        let key_opt: Option<String> = conn
            .query_row(
                "SELECT value FROM secrets WHERE service = 'lettuceai:apiKey' AND account = ?1",
                params![account],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        if let Some(key) = key_opt {
            conn.execute(
                "UPDATE provider_credentials SET api_key = ?1 WHERE id = ?2",
                params![key, cred_id],
            )
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        }
    }

    Ok(())
}

/// Migration v7 -> v8: add memories column to sessions table
fn migrate_v7_to_v8(app: &AppHandle) -> Result<(), String> {
    use crate::storage_manager::db::open_db;

    let conn = open_db(app)?;
    // Add column with default empty JSON array if it doesn't exist
    let _ = conn.execute(
        "ALTER TABLE sessions ADD COLUMN memories TEXT NOT NULL DEFAULT '[]'",
        [],
    );

    Ok(())
}

/// Migration v8 -> v9: add advanced_settings column to settings table
fn migrate_v8_to_v9(app: &AppHandle) -> Result<(), String> {
    use crate::storage_manager::db::open_db;

    let conn = open_db(app)?;
    // Add column with default null if it doesn't exist
    let _ = conn.execute("ALTER TABLE settings ADD COLUMN advanced_settings TEXT", []);

    Ok(())
}

/// Migration v9 -> v10: add memory_type column to characters table
fn migrate_v9_to_v10(app: &AppHandle) -> Result<(), String> {
    use crate::storage_manager::db::open_db;

    let conn = open_db(app)?;

    // Check if column already exists
    let mut has_column = false;
    let mut stmt = conn
        .prepare("PRAGMA table_info(characters)")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    for col in rows {
        let name = col.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        if name == "memory_type" {
            has_column = true;
            break;
        }
    }

    if !has_column {
        let _ = conn.execute(
            "ALTER TABLE characters ADD COLUMN memory_type TEXT DEFAULT 'manual'",
            [],
        );
    }

    // Ensure all rows have a value
    let _ = conn.execute(
        "UPDATE characters SET memory_type = 'manual' WHERE memory_type IS NULL",
        [],
    );

    Ok(())
}

/// Migration v10 -> v11: add memory_embeddings column to sessions table
fn migrate_v10_to_v11(app: &AppHandle) -> Result<(), String> {
    use crate::storage_manager::db::open_db;

    let conn = open_db(app)?;

    // Check for existing column
    let mut has_column = false;
    let mut stmt = conn
        .prepare("PRAGMA table_info(sessions)")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    for col in rows {
        let name = col.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        if name == "memory_embeddings" {
            has_column = true;
            break;
        }
    }

    if !has_column {
        let _ = conn.execute(
            "ALTER TABLE sessions ADD COLUMN memory_embeddings TEXT DEFAULT '[]'",
            [],
        );
    }

    let _ = conn.execute(
        "UPDATE sessions SET memory_embeddings = '[]' WHERE memory_embeddings IS NULL",
        [],
    );

    Ok(())
}

/// Migration v11 -> v12: add memory_summary and memory_tool_events columns to sessions
fn migrate_v11_to_v12(app: &AppHandle) -> Result<(), String> {
    use crate::storage_manager::db::open_db;

    let conn = open_db(app)?;

    // Add memory_summary if missing
    let mut has_summary = false;
    let mut has_events = false;
    let mut stmt = conn
        .prepare("PRAGMA table_info(sessions)")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    for col in rows {
        let name = col.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        if name == "memory_summary" {
            has_summary = true;
        }
        if name == "memory_tool_events" {
            has_events = true;
        }
    }

    if !has_summary {
        let _ = conn.execute("ALTER TABLE sessions ADD COLUMN memory_summary TEXT", []);
    }

    if !has_events {
        let _ = conn.execute(
            "ALTER TABLE sessions ADD COLUMN memory_tool_events TEXT DEFAULT '[]'",
            [],
        );
    }

    let _ = conn.execute(
        "UPDATE sessions SET memory_tool_events = coalesce(memory_tool_events, '[]')",
        [],
    );

    Ok(())
}

/// Migration v6 -> v7: move per-credential model list cache from models-cache.json to SQLite table
// migrate_v6_to_v7 removed (feature dropped)
// We keep the same storage file/format (no new file), but update all templates so they no longer
// carry meaningful scope assignments:
// - Set `scope` to AppWide for all non-default templates
// - Clear `targetIds` for all templates
//
// Notes:
// - App Default template already uses AppWide scope; we don't change its scope (and updates are
//   prevented by prompts::update_template anyway)
// - Character/Model/Settings continue to reference templates by ID; behavior is unchanged because
//   runtime selection uses explicit references, not scope matching
fn migrate_v2_to_v3(app: &AppHandle) -> Result<(), String> {
    use crate::chat_manager::prompts;
    use crate::chat_manager::types::PromptScope;

    // Ensure App Default exists (idempotent)
    let _ = prompts::ensure_app_default_template(app);

    let templates = prompts::load_templates(app)?;
    let mut changed = 0usize;

    for t in templates.iter() {
        // Skip changing scope for App Default; it is already AppWide
        if prompts::is_app_default_template(&t.id) {
            // We still clear target IDs if any lingered (should be empty by design)
            if !t.target_ids.is_empty() {
                let _ = prompts::update_template(
                    app,
                    t.id.clone(),
                    None,
                    None,             // keep scope as-is for App Default
                    Some(Vec::new()), // clear target ids
                    None,
                    None,
                    None,
                )?;
                changed += 1;
            }
            continue;
        }

        let mut need_update = false;
        let new_scope = if t.scope != PromptScope::AppWide {
            need_update = true;
            PromptScope::AppWide
        } else {
            t.scope.clone()
        };

        let new_target_ids = if !t.target_ids.is_empty() {
            need_update = true;
            Vec::new()
        } else {
            t.target_ids.clone()
        };

        if need_update {
            let _ = prompts::update_template(
                app,
                t.id.clone(),
                None,
                Some(new_scope),
                Some(new_target_ids),
                None,
                None,
                None,
            )?;
            changed += 1;
        }
    }

    if changed > 0 {
        log_info(
            app,
            "migrations",
            format!("Migrated {} templates to AppWide scope", changed),
        );
    }

    Ok(())
}

/// Migration v1 -> v2: Convert systemPrompt strings to prompt template references
///
/// This migration converts the old systemPrompt field (direct string) to the new
/// prompt template system. It creates prompt templates for each unique custom prompt
/// and updates references in Settings, Models, and Characters.
fn migrate_v1_to_v2(app: &AppHandle) -> Result<(), String> {
    use std::collections::HashMap;

    let mut prompt_map: HashMap<String, String> = HashMap::new(); // content -> template_id
    let mut templates_created = 0;

    // Ensure "App Default" template exists
    let _app_default_id = prompts::ensure_app_default_template(app)?;

    // Migrate Settings app-wide prompt
    if let Ok(Some(settings_json)) = storage_read_settings(app.clone()) {
        let mut settings: Value = serde_json::from_str(&settings_json).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to parse settings: {}", e),
            )
        })?;

        let mut changed = false;

        if let Some(obj) = settings.as_object_mut() {
            // Migrate app-wide system prompt
            if let Some(Value::String(prompt_content)) = obj.get("systemPrompt") {
                if !prompt_content.is_empty() {
                    let template_id = if let Some(id) = prompt_map.get(prompt_content) {
                        id.clone()
                    } else {
                        let template = prompts::create_template(
                            app,
                            "App-wide Prompt".to_string(),
                            PromptScope::AppWide,
                            vec![],
                            prompt_content.clone(),
                            None,
                            None,
                        )?;
                        prompt_map.insert(prompt_content.clone(), template.id.clone());
                        templates_created += 1;
                        template.id
                    };

                    obj.insert("promptTemplateId".to_string(), Value::String(template_id));
                    obj.remove("systemPrompt");
                    changed = true;
                }
            }

            // Migrate model-specific prompts
            if let Some(models) = obj.get_mut("models").and_then(|v| v.as_array_mut()) {
                for (idx, model) in models.iter_mut().enumerate() {
                    if let Some(model_obj) = model.as_object_mut() {
                        if let Some(Value::String(prompt_content)) = model_obj.get("systemPrompt") {
                            if !prompt_content.is_empty() {
                                let model_id_default = format!("model_{}", idx);
                                let model_id = model_obj
                                    .get("id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or(&model_id_default);

                                let template_id = if let Some(id) = prompt_map.get(prompt_content) {
                                    id.clone()
                                } else {
                                    let template = prompts::create_template(
                                        app,
                                        format!("Model {} Prompt", model_id),
                                        PromptScope::ModelSpecific,
                                        vec![model_id.to_string()],
                                        prompt_content.clone(),
                                        None,
                                        None,
                                    )?;
                                    prompt_map.insert(prompt_content.clone(), template.id.clone());
                                    templates_created += 1;
                                    template.id
                                };

                                model_obj.insert(
                                    "promptTemplateId".to_string(),
                                    Value::String(template_id),
                                );
                                model_obj.remove("systemPrompt");
                                changed = true;
                            }
                        }
                    }
                }
            }
        }

        if changed {
            storage_write_settings(
                app.clone(),
                serde_json::to_string(&settings)
                    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
            )?;
            log_info(
                app,
                "migrations",
                format!(
                    "Migrated settings prompts, created {} templates",
                    templates_created
                ),
            );
        }
    }

    // Character prompt migration for legacy files skipped; characters moved to DB

    log_info(
        app,
        "migrations",
        format!(
            "v1->v2 migration completed. Total prompt templates created: {}",
            templates_created
        ),
    );

    Ok(())
}

/// Migration v12 -> v13: add operation_type column to usage_records
fn migrate_v12_to_v13(app: &AppHandle) -> Result<(), String> {
    use crate::storage_manager::db::open_db;

    let conn = open_db(app)?;

    // Check if column already exists
    let mut has_column = false;
    let mut stmt = conn
        .prepare("PRAGMA table_info(usage_records)")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    for col in rows {
        let name = col.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        if name == "operation_type" {
            has_column = true;
            break;
        }
    }

    if !has_column {
        let _ = conn.execute(
            "ALTER TABLE usage_records ADD COLUMN operation_type TEXT DEFAULT 'chat'",
            [],
        );
    }

    // Ensure all existing rows have a value
    let _ = conn.execute(
        "UPDATE usage_records SET operation_type = 'chat' WHERE operation_type IS NULL",
        [],
    );

    Ok(())
}

fn migrate_v26_to_v27(app: &AppHandle) -> Result<(), String> {
    use crate::storage_manager::db::open_db;

    let conn = open_db(app)?;

    // Check if model_id column exists in group_messages
    let mut has_model_id_messages = false;
    let mut stmt = conn
        .prepare("PRAGMA table_info(group_messages)")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    for col in rows {
        let name = col.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        if name == "model_id" {
            has_model_id_messages = true;
            break;
        }
    }

    // Add model_id column to group_messages
    if !has_model_id_messages {
        let _ = conn.execute("ALTER TABLE group_messages ADD COLUMN model_id TEXT", []);
    }

    // Check if model_id column exists in group_message_variants
    let mut has_model_id_variants = false;
    let mut stmt = conn
        .prepare("PRAGMA table_info(group_message_variants)")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    for col in rows {
        let name = col.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        if name == "model_id" {
            has_model_id_variants = true;
            break;
        }
    }

    // Add model_id column to group_message_variants
    if !has_model_id_variants {
        let _ = conn.execute(
            "ALTER TABLE group_message_variants ADD COLUMN model_id TEXT",
            [],
        );
    }

    Ok(())
}

fn migrate_v28_to_v29(app: &AppHandle) -> Result<(), String> {
    use crate::storage_manager::db::open_db;

    let conn = open_db(app)?;

    // Check if background_image_path column exists in group_sessions
    let mut has_background_image_path = false;
    let mut stmt = conn
        .prepare("PRAGMA table_info(group_sessions)")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    for col in rows {
        let name = col.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        if name == "background_image_path" {
            has_background_image_path = true;
            break;
        }
    }

    // Add background_image_path column to group_sessions
    if !has_background_image_path {
        let _ = conn.execute(
            "ALTER TABLE group_sessions ADD COLUMN background_image_path TEXT",
            [],
        );
    }

    Ok(())
}

fn migrate_v29_to_v30(app: &AppHandle) -> Result<(), String> {
    use crate::storage_manager::db::open_db;

    let conn = open_db(app)?;

    let mut has_definition = false;
    let mut stmt = conn
        .prepare("PRAGMA table_info(characters)")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    for col in rows {
        let name = col.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        if name == "definition" {
            has_definition = true;
            break;
        }
    }

    if !has_definition {
        let _ = conn.execute("ALTER TABLE characters ADD COLUMN definition TEXT", []);
    }

    let _ = conn.execute(
        "UPDATE characters SET definition = description WHERE (definition IS NULL OR definition = '') AND description IS NOT NULL",
        [],
    );

    Ok(())
}

fn migrate_v30_to_v31(app: &AppHandle) -> Result<(), String> {
    use crate::storage_manager::db::open_db;

    let conn = open_db(app)?;

    let mut has_avatar_crop_x = false;
    let mut has_avatar_crop_y = false;
    let mut has_avatar_crop_scale = false;
    let mut stmt = conn
        .prepare("PRAGMA table_info(characters)")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    for col in rows {
        let name = col.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        match name.as_str() {
            "avatar_crop_x" => has_avatar_crop_x = true,
            "avatar_crop_y" => has_avatar_crop_y = true,
            "avatar_crop_scale" => has_avatar_crop_scale = true,
            _ => {}
        }
    }

    if !has_avatar_crop_x {
        let _ = conn.execute("ALTER TABLE characters ADD COLUMN avatar_crop_x REAL", []);
    }
    if !has_avatar_crop_y {
        let _ = conn.execute("ALTER TABLE characters ADD COLUMN avatar_crop_y REAL", []);
    }
    if !has_avatar_crop_scale {
        let _ = conn.execute(
            "ALTER TABLE characters ADD COLUMN avatar_crop_scale REAL",
            [],
        );
    }

    let mut has_persona_avatar_crop_x = false;
    let mut has_persona_avatar_crop_y = false;
    let mut has_persona_avatar_crop_scale = false;
    let mut stmt = conn
        .prepare("PRAGMA table_info(personas)")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    for col in rows {
        let name = col.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        match name.as_str() {
            "avatar_crop_x" => has_persona_avatar_crop_x = true,
            "avatar_crop_y" => has_persona_avatar_crop_y = true,
            "avatar_crop_scale" => has_persona_avatar_crop_scale = true,
            _ => {}
        }
    }

    if !has_persona_avatar_crop_x {
        let _ = conn.execute("ALTER TABLE personas ADD COLUMN avatar_crop_x REAL", []);
    }
    if !has_persona_avatar_crop_y {
        let _ = conn.execute("ALTER TABLE personas ADD COLUMN avatar_crop_y REAL", []);
    }
    if !has_persona_avatar_crop_scale {
        let _ = conn.execute("ALTER TABLE personas ADD COLUMN avatar_crop_scale REAL", []);
    }

    Ok(())
}

fn migrate_v31_to_v32(app: &AppHandle) -> Result<(), String> {
    use crate::storage_manager::db::open_db;

    let conn = open_db(app)?;
    conn.execute(
        "UPDATE models SET prompt_template_id = NULL, system_prompt = NULL",
        [],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

fn migrate_v32_to_v33(app: &AppHandle) -> Result<(), String> {
    use crate::storage_manager::db::open_db;

    let conn = open_db(app)?;
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS creation_helper_sessions (
          id TEXT PRIMARY KEY,
          creation_goal TEXT NOT NULL,
          status TEXT NOT NULL,
          session_json TEXT NOT NULL,
          uploaded_images_json TEXT NOT NULL DEFAULT '{}',
          created_at INTEGER NOT NULL,
          updated_at INTEGER NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_creation_helper_sessions_goal_updated
          ON creation_helper_sessions(creation_goal, updated_at DESC);
        "#,
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

fn migrate_v33_to_v34(app: &AppHandle) -> Result<(), String> {
    use crate::storage_manager::db::open_db;

    let conn = open_db(app)?;

    let mut has_nickname = false;
    let mut has_scenario = false;
    let mut has_creator_notes = false;
    let mut has_creator = false;
    let mut has_creator_notes_multilingual = false;
    let mut has_source = false;
    let mut has_tags = false;

    let mut stmt = conn
        .prepare("PRAGMA table_info(characters)")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    for col in rows {
        let name = col.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        match name.as_str() {
            "nickname" => has_nickname = true,
            "scenario" => has_scenario = true,
            "creator_notes" => has_creator_notes = true,
            "creator" => has_creator = true,
            "creator_notes_multilingual" => has_creator_notes_multilingual = true,
            "source" => has_source = true,
            "tags" => has_tags = true,
            _ => {}
        }
    }

    if !has_nickname {
        let _ = conn.execute("ALTER TABLE characters ADD COLUMN nickname TEXT", []);
    }
    if !has_scenario {
        let _ = conn.execute("ALTER TABLE characters ADD COLUMN scenario TEXT", []);
    }
    if !has_creator_notes {
        let _ = conn.execute("ALTER TABLE characters ADD COLUMN creator_notes TEXT", []);
    }
    if !has_creator {
        let _ = conn.execute("ALTER TABLE characters ADD COLUMN creator TEXT", []);
    }
    if !has_creator_notes_multilingual {
        let _ = conn.execute(
            "ALTER TABLE characters ADD COLUMN creator_notes_multilingual TEXT",
            [],
        );
    }
    if !has_source {
        let _ = conn.execute("ALTER TABLE characters ADD COLUMN source TEXT", []);
    }
    if !has_tags {
        let _ = conn.execute("ALTER TABLE characters ADD COLUMN tags TEXT", []);
    }

    Ok(())
}

fn migrate_v27_to_v28(app: &AppHandle) -> Result<(), String> {
    use crate::storage_manager::db::open_db;

    let conn = open_db(app)?;

    // Check if chat_type column exists in group_sessions
    let mut has_chat_type = false;
    let mut stmt = conn
        .prepare("PRAGMA table_info(group_sessions)")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    for col in rows {
        let name = col.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        if name == "chat_type" {
            has_chat_type = true;
            break;
        }
    }

    // Add chat_type column to group_sessions
    if !has_chat_type {
        let _ = conn.execute(
            "ALTER TABLE group_sessions ADD COLUMN chat_type TEXT NOT NULL DEFAULT 'conversation'",
            [],
        );
    }

    // Check if starting_scene column exists in group_sessions
    let mut has_starting_scene = false;
    let mut stmt = conn
        .prepare("PRAGMA table_info(group_sessions)")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    for col in rows {
        let name = col.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        if name == "starting_scene" {
            has_starting_scene = true;
            break;
        }
    }

    // Add starting_scene column to group_sessions
    if !has_starting_scene {
        let _ = conn.execute(
            "ALTER TABLE group_sessions ADD COLUMN starting_scene TEXT",
            [],
        );
    }

    Ok(())
}

/// Migration v13 -> v14: add model_type column to models table
fn migrate_v13_to_v14(app: &AppHandle) -> Result<(), String> {
    use crate::storage_manager::db::open_db;

    let conn = open_db(app)?;

    // Check if column already exists
    let mut has_column = false;
    let mut stmt = conn
        .prepare("PRAGMA table_info(models)")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    for col in rows {
        let name = col.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        if name == "model_type" {
            has_column = true;
            break;
        }
    }

    if !has_column {
        let _ = conn.execute(
            "ALTER TABLE models ADD COLUMN model_type TEXT DEFAULT 'chat'",
            [],
        );
    }

    // Ensure all existing rows have a value
    let _ = conn.execute(
        "UPDATE models SET model_type = 'chat' WHERE model_type IS NULL",
        [],
    );

    Ok(())
}

/// Migration v14 -> v15: add attachments column to messages table
fn migrate_v14_to_v15(app: &AppHandle) -> Result<(), String> {
    use crate::storage_manager::db::open_db;

    let conn = open_db(app)?;

    // Check if column already exists
    let mut has_column = false;
    let mut stmt = conn
        .prepare("PRAGMA table_info(messages)")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    for col in rows {
        let name = col.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        if name == "attachments" {
            has_column = true;
            break;
        }
    }

    if !has_column {
        let _ = conn.execute(
            "ALTER TABLE messages ADD COLUMN attachments TEXT DEFAULT '[]'",
            [],
        );
    }

    Ok(())
}

/// Migration v15 -> v16: backfill token_count for existing memory embeddings and add memory_summary_token_count
fn migrate_v15_to_v16(app: &AppHandle) -> Result<(), String> {
    use crate::storage_manager::db::open_db;
    use serde_json::Value;

    let conn = open_db(app)?;

    // Add memory_summary_token_count column if it doesn't exist
    let mut has_column = false;
    let mut stmt = conn
        .prepare("PRAGMA table_info(sessions)")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    for col in rows {
        let name = col.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        if name == "memory_summary_token_count" {
            has_column = true;
            break;
        }
    }

    if !has_column {
        let _ = conn.execute(
            "ALTER TABLE sessions ADD COLUMN memory_summary_token_count INTEGER NOT NULL DEFAULT 0",
            [],
        );
    }

    // Try to backfill token counts only if tokenizer is available
    // If tokenizer isn't available (embedding model not downloaded), skip backfill
    // Token counts will be calculated when memories/summaries are created
    let tokenizer_available = {
        use crate::embedding_model::embedding_model_dir;
        let model_dir = embedding_model_dir(app).ok();
        model_dir
            .map(|dir| dir.join("tokenizer.json").exists())
            .unwrap_or(false)
    };

    if !tokenizer_available {
        return Ok(());
    }

    use crate::tokenizer::count_tokens;

    // Backfill token counts for memory_embeddings
    let mut stmt = conn
        .prepare("SELECT id, memory_embeddings FROM sessions WHERE memory_embeddings IS NOT NULL AND memory_embeddings != '[]'")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let session_rows: Vec<(String, String)> = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    // Process each session
    for (session_id, embeddings_json) in session_rows {
        let mut embeddings: Vec<Value> = serde_json::from_str(&embeddings_json).map_err(|e| {
            format!(
                "Failed to parse memory_embeddings for session {}: {}",
                session_id, e
            )
        })?;

        let mut updated = false;

        for embedding in &mut embeddings {
            // Check if tokenCount already exists
            if embedding.get("tokenCount").is_some() {
                continue;
            }

            // Get the text field
            if let Some(text) = embedding.get("text").and_then(|v| v.as_str()) {
                // Calculate token count
                let token_count = count_tokens(app, text).unwrap_or(0);

                // Add tokenCount field
                if let Value::Object(map) = embedding {
                    map.insert("tokenCount".to_string(), Value::Number(token_count.into()));
                    updated = true;
                }
            }
        }

        // Update the session if any embeddings were modified
        if updated {
            let updated_json = serde_json::to_string(&embeddings).map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to serialize updated embeddings: {}", e),
                )
            })?;

            conn.execute(
                "UPDATE sessions SET memory_embeddings = ?1 WHERE id = ?2",
                [&updated_json, &session_id],
            )
            .map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to update session {}: {}", session_id, e),
                )
            })?;
        }
    }

    // Backfill token counts for memory_summary
    let mut stmt = conn
        .prepare("SELECT id, memory_summary FROM sessions WHERE memory_summary IS NOT NULL AND memory_summary != '' AND memory_summary_token_count = 0")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let summary_rows: Vec<(String, String)> = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    for (session_id, summary) in summary_rows {
        let token_count = count_tokens(app, &summary).unwrap_or(0);

        conn.execute(
            "UPDATE sessions SET memory_summary_token_count = ?1 WHERE id = ?2",
            [&token_count.to_string(), &session_id],
        )
        .map_err(|e| {
            format!(
                "Failed to update summary token count for session {}: {}",
                session_id, e
            )
        })?;
    }

    Ok(())
}

/// Migration v16 -> v17: add memory_tokens and summary_tokens columns to usage_records
fn migrate_v16_to_v17(app: &AppHandle) -> Result<(), String> {
    use crate::storage_manager::db::open_db;

    let conn = open_db(app)?;

    // Check if memory_tokens column exists
    let mut has_memory_tokens = false;
    let mut has_summary_tokens = false;
    let mut stmt = conn
        .prepare("PRAGMA table_info(usage_records)")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    for col in rows {
        let name = col.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        if name == "memory_tokens" {
            has_memory_tokens = true;
        }
        if name == "summary_tokens" {
            has_summary_tokens = true;
        }
    }

    if !has_memory_tokens {
        let _ = conn.execute(
            "ALTER TABLE usage_records ADD COLUMN memory_tokens INTEGER",
            [],
        );
    }

    if !has_summary_tokens {
        let _ = conn.execute(
            "ALTER TABLE usage_records ADD COLUMN summary_tokens INTEGER",
            [],
        );
    }

    Ok(())
}

/// Migration v17 -> v18: add custom gradient columns to characters table
fn migrate_v17_to_v18(app: &AppHandle) -> Result<(), String> {
    use crate::storage_manager::db::open_db;
    use crate::utils::log_info;

    log_info(app, "migrations", "Starting v17->v18 migration");

    let conn = open_db(app)?;

    // Check which columns exist
    let mut has_custom_gradient_colors = false;
    let mut has_custom_text_color = false;
    let mut has_custom_text_secondary = false;
    let mut stmt = conn
        .prepare("PRAGMA table_info(characters)")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    for col in rows {
        let name = col.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        match name.as_str() {
            "custom_gradient_colors" => has_custom_gradient_colors = true,
            "custom_text_color" => has_custom_text_color = true,
            "custom_text_secondary" => has_custom_text_secondary = true,
            _ => {}
        }
    }

    log_info(
        app,
        "migrations",
        format!(
        "Column check: custom_gradient_colors={}, custom_text_color={}, custom_text_secondary={}",
        has_custom_gradient_colors, has_custom_text_color, has_custom_text_secondary
    ),
    );

    if !has_custom_gradient_colors {
        log_info(app, "migrations", "Adding custom_gradient_colors column");
        conn.execute(
            "ALTER TABLE characters ADD COLUMN custom_gradient_colors TEXT",
            [],
        )
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to add custom_gradient_colors: {}", e),
            )
        })?;
    }

    if !has_custom_text_color {
        log_info(app, "migrations", "Adding custom_text_color column");
        conn.execute(
            "ALTER TABLE characters ADD COLUMN custom_text_color TEXT",
            [],
        )
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to add custom_text_color: {}", e),
            )
        })?;
    }

    if !has_custom_text_secondary {
        log_info(app, "migrations", "Adding custom_text_secondary column");
        conn.execute(
            "ALTER TABLE characters ADD COLUMN custom_text_secondary TEXT",
            [],
        )
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to add custom_text_secondary: {}", e),
            )
        })?;
    }

    log_info(app, "migrations", "v17->v18 migration completed");
    Ok(())
}

/// Migration v18 -> v19: add input_scopes and output_scopes to models table and migrate legacy multimodel.
fn migrate_v18_to_v19(app: &AppHandle) -> Result<(), String> {
    use crate::storage_manager::db::open_db;
    use crate::utils::log_info;

    log_info(app, "migrations", "Starting v18->v19 migration");

    let conn = open_db(app)?;

    // Check which columns exist
    let mut has_input_scopes = false;
    let mut has_output_scopes = false;
    let mut stmt = conn
        .prepare("PRAGMA table_info(models)")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    for col in rows {
        let name = col.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        match name.as_str() {
            "input_scopes" => has_input_scopes = true,
            "output_scopes" => has_output_scopes = true,
            _ => {}
        }
    }

    if !has_input_scopes {
        let _ = conn.execute("ALTER TABLE models ADD COLUMN input_scopes TEXT", []);
    }

    if !has_output_scopes {
        let _ = conn.execute("ALTER TABLE models ADD COLUMN output_scopes TEXT", []);
    }

    // Migrate legacy multimodel -> scopes
    let _ = conn.execute(
        "UPDATE models SET input_scopes = '[\"text\",\"image\"]' WHERE model_type = 'multimodel' AND (input_scopes IS NULL OR input_scopes = '')",
        [],
    );
    let _ = conn.execute(
        "UPDATE models SET output_scopes = '[\"text\"]' WHERE model_type = 'multimodel' AND (output_scopes IS NULL OR output_scopes = '')",
        [],
    );
    // Normalize model_type away from legacy "multimodel"
    let _ = conn.execute(
        "UPDATE models SET model_type = 'chat' WHERE model_type = 'multimodel'",
        [],
    );

    // Backfill defaults where scopes are missing
    let _ = conn.execute(
        "UPDATE models SET input_scopes = '[\"text\"]' WHERE input_scopes IS NULL OR input_scopes = ''",
        [],
    );
    let _ = conn.execute(
        "UPDATE models SET output_scopes = '[\"text\"]' WHERE output_scopes IS NULL OR output_scopes = ''",
        [],
    );

    Ok(())
}

/// Migration v19 -> v20: convert character-level lorebook_entries into app-level lorebooks.
fn migrate_v19_to_v20(app: &AppHandle) -> Result<(), String> {
    use crate::storage_manager::db::open_db;
    use crate::utils::{log_info, now_millis};
    use rusqlite::{params, OptionalExtension};
    use uuid::Uuid;

    log_info(app, "migrations", "Starting v19->v20 migration");

    let conn = open_db(app)?;

    // Ensure new tables exist (fresh installs already have these from init_db).
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS lorebooks (
          id TEXT PRIMARY KEY,
          name TEXT NOT NULL,
          created_at INTEGER NOT NULL,
          updated_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS character_lorebooks (
          character_id TEXT NOT NULL,
          lorebook_id TEXT NOT NULL,
          enabled INTEGER NOT NULL DEFAULT 1,
          display_order INTEGER NOT NULL DEFAULT 0,
          created_at INTEGER NOT NULL,
          updated_at INTEGER NOT NULL,
          PRIMARY KEY(character_id, lorebook_id),
          FOREIGN KEY(character_id) REFERENCES characters(id) ON DELETE CASCADE,
          FOREIGN KEY(lorebook_id) REFERENCES lorebooks(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_character_lorebooks_character ON character_lorebooks(character_id);
        "#,
    )
    .map_err(|e| crate::utils::err_msg(module_path!(), line!(), format!("Failed to ensure lorebook tables: {}", e)))?;

    // If lorebook_entries already uses lorebook_id, nothing to do.
    let entries_table_exists: i32 = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='lorebook_entries'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if entries_table_exists == 0 {
        // Ensure the v2 entries table exists and return.
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS lorebook_entries (
              id TEXT PRIMARY KEY,
              lorebook_id TEXT NOT NULL,
              enabled INTEGER NOT NULL DEFAULT 1,
              always_active INTEGER NOT NULL DEFAULT 0,
              keywords TEXT NOT NULL DEFAULT '[]',
              case_sensitive INTEGER NOT NULL DEFAULT 0,
              content TEXT NOT NULL,
              priority INTEGER NOT NULL DEFAULT 0,
              display_order INTEGER NOT NULL DEFAULT 0,
              created_at INTEGER NOT NULL,
              updated_at INTEGER NOT NULL,
              FOREIGN KEY(lorebook_id) REFERENCES lorebooks(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_lorebook_entries_lorebook ON lorebook_entries(lorebook_id);
            CREATE INDEX IF NOT EXISTS idx_lorebook_entries_enabled ON lorebook_entries(lorebook_id, enabled);
            "#,
        )
        .map_err(|e| crate::utils::err_msg(module_path!(), line!(), format!("Failed to create lorebook_entries: {}", e)))?;
        return Ok(());
    }

    // Detect legacy character-level schema.
    let mut has_character_id = false;
    let mut has_lorebook_id = false;
    let mut stmt = conn
        .prepare("PRAGMA table_info(lorebook_entries)")
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to read lorebook_entries schema: {}", e),
            )
        })?;
    let cols = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to query lorebook_entries schema: {}", e),
            )
        })?;
    for col in cols {
        let name = col.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        match name.as_str() {
            "character_id" => has_character_id = true,
            "lorebook_id" => has_lorebook_id = true,
            _ => {}
        }
    }

    if has_lorebook_id {
        return Ok(());
    }

    if !has_character_id {
        // Unexpected schema; do not attempt destructive migration.
        return Ok(());
    }

    // Rename legacy table and create v2 table.
    let legacy_exists: i32 = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='lorebook_entries_v1'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if legacy_exists == 0 {
        conn.execute(
            "ALTER TABLE lorebook_entries RENAME TO lorebook_entries_v1",
            [],
        )
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to rename legacy lorebook_entries: {}", e),
            )
        })?;
    }

    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS lorebook_entries (
          id TEXT PRIMARY KEY,
          lorebook_id TEXT NOT NULL,
          enabled INTEGER NOT NULL DEFAULT 1,
          always_active INTEGER NOT NULL DEFAULT 0,
          keywords TEXT NOT NULL DEFAULT '[]',
          case_sensitive INTEGER NOT NULL DEFAULT 0,
          content TEXT NOT NULL,
          priority INTEGER NOT NULL DEFAULT 0,
          display_order INTEGER NOT NULL DEFAULT 0,
          created_at INTEGER NOT NULL,
          updated_at INTEGER NOT NULL,
          FOREIGN KEY(lorebook_id) REFERENCES lorebooks(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_lorebook_entries_lorebook ON lorebook_entries(lorebook_id);
        CREATE INDEX IF NOT EXISTS idx_lorebook_entries_enabled ON lorebook_entries(lorebook_id, enabled);
        "#,
    )
    .map_err(|e| crate::utils::err_msg(module_path!(), line!(), format!("Failed to create v2 lorebook_entries: {}", e)))?;

    // Create a default lorebook per character that has legacy entries and map it to the character.
    let mut stmt = conn
        .prepare("SELECT DISTINCT character_id FROM lorebook_entries_v1")
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to read legacy lorebook entries: {}", e),
            )
        })?;
    let character_ids = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to query legacy character ids: {}", e),
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to collect legacy character ids: {}", e),
            )
        })?;

    for character_id in character_ids {
        let name: Option<String> = conn
            .query_row(
                "SELECT name FROM characters WHERE id = ?1",
                params![character_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to read character name: {}", e),
                )
            })?;

        let lorebook_id = Uuid::new_v4().to_string();
        let now = now_millis()? as i64;
        let lorebook_name = match name {
            Some(n) if !n.trim().is_empty() => format!("{} Lorebook", n.trim()),
            _ => "Lorebook".to_string(),
        };

        conn.execute(
            "INSERT INTO lorebooks (id, name, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
            params![lorebook_id, lorebook_name, now, now],
        )
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to create migrated lorebook: {}", e),
            )
        })?;

        conn.execute(
            r#"
            INSERT INTO character_lorebooks (character_id, lorebook_id, enabled, display_order, created_at, updated_at)
            VALUES (?1, ?2, 1, 0, ?3, ?3)
            "#,
            params![character_id, lorebook_id, now],
        )
        .map_err(|e| crate::utils::err_msg(module_path!(), line!(), format!("Failed to map character to migrated lorebook: {}", e)))?;

        conn.execute(
            r#"
            INSERT INTO lorebook_entries (
              id, lorebook_id, enabled, always_active, keywords, case_sensitive,
              content, priority, display_order, created_at, updated_at
            )
            SELECT
              id, ?2, enabled, always_active, keywords, case_sensitive,
              content, priority, display_order, created_at, updated_at
            FROM lorebook_entries_v1
            WHERE character_id = ?1
            "#,
            params![character_id, lorebook_id],
        )
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to migrate lorebook entries: {}", e),
            )
        })?;
    }

    log_info(app, "migrations", "v19->v20 migration completed");
    Ok(())
}

/// Migration v20 -> v21: Add config column to provider_credentials
fn migrate_v20_to_v21(app: &AppHandle) -> Result<(), String> {
    use crate::storage_manager::db::open_db;

    let conn = open_db(app)?;

    // Add config column if it doesn't exist
    let _ = conn.execute(
        "ALTER TABLE provider_credentials ADD COLUMN config TEXT",
        [],
    );

    Ok(())
}

fn migrate_v21_to_v22(app: &AppHandle) -> Result<(), String> {
    use crate::storage_manager::db::open_db;

    let conn = open_db(app)?;

    // Add direction column to scenes if it doesn't exist
    let _ = conn.execute("ALTER TABLE scenes ADD COLUMN direction TEXT", []);

    // Add direction column to scene_variants if it doesn't exist
    let _ = conn.execute("ALTER TABLE scene_variants ADD COLUMN direction TEXT", []);

    Ok(())
}

fn migrate_v22_to_v23(app: &AppHandle) -> Result<(), String> {
    use crate::storage_manager::db::open_db;

    let conn = open_db(app)?;

    // Add finish_reason column to usage_records if it doesn't exist
    let _ = conn.execute(
        "ALTER TABLE usage_records ADD COLUMN finish_reason TEXT",
        [],
    );

    Ok(())
}

/// Migration v23 -> v24: Add memory columns to group_sessions
fn migrate_v23_to_v24(app: &AppHandle) -> Result<(), String> {
    use crate::storage_manager::db::open_db;

    let conn = open_db(app)?;

    // Check for existing columns
    let mut has_memories = false;
    let mut has_memory_embeddings = false;
    let mut has_memory_summary = false;
    let mut has_memory_summary_token_count = false;

    let mut stmt = conn
        .prepare("PRAGMA table_info(group_sessions)")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    for col in rows {
        let name = col.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        match name.as_str() {
            "memories" => has_memories = true,
            "memory_embeddings" => has_memory_embeddings = true,
            "memory_summary" => has_memory_summary = true,
            "memory_summary_token_count" => has_memory_summary_token_count = true,
            _ => {}
        }
    }

    // Add memories column (manual memories - array of strings)
    if !has_memories {
        let _ = conn.execute(
            "ALTER TABLE group_sessions ADD COLUMN memories TEXT NOT NULL DEFAULT '[]'",
            [],
        );
    }

    // Add memory_embeddings column (dynamic memories with embeddings)
    if !has_memory_embeddings {
        let _ = conn.execute(
            "ALTER TABLE group_sessions ADD COLUMN memory_embeddings TEXT NOT NULL DEFAULT '[]'",
            [],
        );
    }

    // Add memory_summary column (compressed summary for context)
    if !has_memory_summary {
        let _ = conn.execute(
            "ALTER TABLE group_sessions ADD COLUMN memory_summary TEXT NOT NULL DEFAULT ''",
            [],
        );
    }

    // Add memory_summary_token_count column
    if !has_memory_summary_token_count {
        let _ = conn.execute(
            "ALTER TABLE group_sessions ADD COLUMN memory_summary_token_count INTEGER NOT NULL DEFAULT 0",
            [],
        );
    }

    Ok(())
}

fn migrate_v24_to_v25(app: &AppHandle) -> Result<(), String> {
    use crate::storage_manager::db::open_db;

    let conn = open_db(app)?;

    // Check if archived column exists
    let mut has_archived = false;

    let mut stmt = conn
        .prepare("PRAGMA table_info(group_sessions)")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    for col in rows {
        let name = col.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        if name == "archived" {
            has_archived = true;
            break;
        }
    }

    // Add archived column
    if !has_archived {
        let _ = conn.execute(
            "ALTER TABLE group_sessions ADD COLUMN archived INTEGER NOT NULL DEFAULT 0",
            [],
        );
    }

    Ok(())
}

fn migrate_v25_to_v26(app: &AppHandle) -> Result<(), String> {
    use crate::storage_manager::db::open_db;

    let conn = open_db(app)?;

    // Check if memory_tool_events column exists
    let mut has_memory_tool_events = false;

    let mut stmt = conn
        .prepare("PRAGMA table_info(group_sessions)")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    for col in rows {
        let name = col.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        if name == "memory_tool_events" {
            has_memory_tool_events = true;
            break;
        }
    }

    // Add memory_tool_events column
    if !has_memory_tool_events {
        let _ = conn.execute(
            "ALTER TABLE group_sessions ADD COLUMN memory_tool_events TEXT NOT NULL DEFAULT '[]'",
            [],
        );
    }

    Ok(())
}

fn migrate_v34_to_v35(app: &AppHandle) -> Result<(), String> {
    use crate::storage_manager::db::open_db;

    let conn = open_db(app)?;

    let _ = conn.execute(
        "ALTER TABLE group_sessions ADD COLUMN speaker_selection_method TEXT NOT NULL DEFAULT 'llm'",
        [],
    );

    Ok(())
}

fn migrate_v35_to_v36(app: &AppHandle) -> Result<(), String> {
    use crate::storage_manager::db::open_db;

    let conn = open_db(app)?;

    let _ = conn.execute("ALTER TABLE characters ADD COLUMN chat_appearance TEXT", []);

    Ok(())
}
