use rusqlite::OptionalExtension;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::Manager;

use super::db::{db_path, open_db};
use super::legacy::storage_root;
use crate::utils::now_millis;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageUsageSummary {
    pub file_count: usize,
    pub estimated_sessions: usize,
    pub last_updated_ms: Option<u64>,
}

#[tauri::command]
pub fn storage_clear_all(app: tauri::AppHandle) -> Result<(), String> {
    let dir = storage_root(&app)?;
    if dir.exists() {
        fs::remove_dir_all(&dir)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }
    fs::create_dir_all(&dir)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

fn delete_dir_if_exists(path: &Path) {
    if path.exists() {
        let _ = fs::remove_dir_all(path);
    }
}

fn is_within_root(path: &Path, root: &Path) -> bool {
    path == root || path.starts_with(root)
}

#[tauri::command]
pub async fn storage_reset_database(app: tauri::AppHandle) -> Result<(), String> {
    let lettuce_dir = crate::infra::utils::ensure_lettuce_dir(&app)?;
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let whisper_models_dir = lettuce_dir.join("models").join("whisper");
    let default_kokoro_root = lettuce_dir.join("kokoro");
    let mut kokoro_asset_roots = HashSet::<PathBuf>::from([default_kokoro_root.clone()]);

    if let Ok(mut conn) = open_db(&app) {
        let _ = conn.busy_timeout(Duration::from_secs(2));
        let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA foreign_keys=OFF;");

        if let Ok(mut stmt) = conn.prepare(
            "SELECT asset_root FROM audio_providers WHERE provider_type = 'kokoro' AND asset_root IS NOT NULL",
        ) {
            if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
                for row in rows.flatten() {
                    let trimmed = row.trim();
                    if !trimmed.is_empty() {
                        kokoro_asset_roots.insert(PathBuf::from(trimmed));
                    }
                }
            }
        }

        let tables: Vec<String> = {
            let mut stmt = conn
                .prepare(
                    "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
                )
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            let table_iter = stmt
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

            let mut out = Vec::new();
            for table in table_iter {
                out.push(
                    table.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
                );
            }
            out
        };

        let tx = conn
            .transaction()
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        for table in tables {
            let safe_name = table.replace('"', "\"\"");
            let sql = format!("DELETE FROM \"{}\";", safe_name);
            tx.execute(&sql, [])
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        }
        tx.commit()
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        let _ = conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA wal_checkpoint(TRUNCATE);");
        let _ = conn.execute_batch("VACUUM;");
    }

    let storage = storage_root(&app)?;
    let dirs_to_clear = ["images", "avatars", "attachments", "sessions"];
    for dir_name in dirs_to_clear {
        let dir_path = storage.join(dir_name);
        if dir_path.exists() {
            let _ = fs::remove_dir_all(&dir_path);
        }
    }

    let generated_images_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .join("generated_images");
    if generated_images_dir.exists() {
        let _ = fs::remove_dir_all(&generated_images_dir);
    }

    let model_dir = lettuce_dir.join("models").join("embedding");
    delete_dir_if_exists(&model_dir);
    delete_dir_if_exists(&whisper_models_dir);

    delete_dir_if_exists(&lettuce_dir.join("diffusion"));
    delete_dir_if_exists(&lettuce_dir.join("models").join("diffusion"));
    delete_dir_if_exists(&lettuce_dir.join("models").join("loras"));
    for asset_root in kokoro_asset_roots {
        if is_within_root(&asset_root, &lettuce_dir) || is_within_root(&asset_root, &app_data_dir) {
            delete_dir_if_exists(&asset_root);
        }
    }
    crate::embedding::reset_download_state().await;

    Ok(())
}

#[tauri::command]
pub fn storage_usage_summary(app: tauri::AppHandle) -> Result<StorageUsageSummary, String> {
    let mut file_count = 0usize;
    let mut latest: Option<u64> = None;
    let db = db_path(&app)?;
    if db.exists() {
        file_count += 1;
        if let Ok(metadata) = fs::metadata(&db) {
            if let Ok(modified) = metadata.modified() {
                if let Ok(ms) = modified.duration_since(std::time::UNIX_EPOCH) {
                    let ts = ms.as_millis() as u64;
                    latest = Some(latest.map_or(ts, |cur| cur.max(ts)));
                }
            }
        }
    }
    let conn = open_db(&app)?;
    let session_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))
        .unwrap_or(0);
    let settings_updated: Option<i64> = conn
        .query_row("SELECT updated_at FROM settings WHERE id = 1", [], |r| {
            r.get(0)
        })
        .optional()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    if let Some(ts) = settings_updated {
        latest = Some(latest.map_or(ts as u64, |cur| cur.max(ts as u64)));
    }
    for (table, sql) in [
        ("characters", "SELECT MAX(updated_at) FROM characters"),
        ("personas", "SELECT MAX(updated_at) FROM personas"),
        ("sessions", "SELECT MAX(updated_at) FROM sessions"),
    ] {
        let max_ts: Option<i64> =
            conn.query_row(sql, [], |r| r.get(0))
                .optional()
                .map_err(|e| {
                    crate::utils::err_msg(module_path!(), line!(), format!("{}: {}", table, e))
                })?;
        if let Some(ts) = max_ts {
            latest = Some(latest.map_or(ts as u64, |cur| cur.max(ts as u64)));
        }
    }
    let last_updated_ms = latest.or_else(|| now_millis().ok());
    Ok(StorageUsageSummary {
        file_count,
        estimated_sessions: session_count as usize,
        last_updated_ms,
    })
}
