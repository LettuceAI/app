use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaygroundGenerationRecord {
    pub id: String,
    pub created_at: u64,
    pub provider_id: String,
    pub model_id: String,
    #[serde(default)]
    pub model_name: String,
    pub prompt: String,
    #[serde(default)]
    pub negative_prompt: Option<String>,
    #[serde(default)]
    pub seed: Option<u32>,
    #[serde(default = "default_params_json")]
    pub params_json: String,
    #[serde(default = "default_status")]
    pub status: String,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default = "default_images_json")]
    pub images_json: String,
}

fn default_params_json() -> String {
    "{}".to_string()
}

fn default_status() -> String {
    "pending".to_string()
}

fn default_images_json() -> String {
    "[]".to_string()
}

fn record_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PlaygroundGenerationRecord> {
    Ok(PlaygroundGenerationRecord {
        id: row.get(0)?,
        created_at: row.get::<_, i64>(1)?.max(0) as u64,
        provider_id: row.get(2)?,
        model_id: row.get(3)?,
        model_name: row.get(4)?,
        prompt: row.get(5)?,
        negative_prompt: row.get(6)?,
        seed: row.get::<_, Option<i64>>(7)?.and_then(|value| u32::try_from(value).ok()),
        params_json: row.get(8)?,
        status: row.get(9)?,
        error: row.get(10)?,
        images_json: row.get(11)?,
    })
}

const RECORD_COLUMNS: &str = "id, created_at, provider_id, model_id, model_name, prompt, negative_prompt, seed, params_json, status, error, images_json";

#[tauri::command]
pub fn playground_history_list(
    app: AppHandle,
    limit: Option<u32>,
    before: Option<u64>,
) -> Result<Vec<PlaygroundGenerationRecord>, String> {
    let conn = crate::storage_manager::db::open_db(&app)?;
    let limit = limit.unwrap_or(30).clamp(1, 200) as i64;
    let mut records = Vec::new();
    match before {
        Some(before) => {
            let mut statement = conn
                .prepare(&format!(
                    "SELECT {RECORD_COLUMNS} FROM playground_generations WHERE created_at < ?1 ORDER BY created_at DESC, id DESC LIMIT ?2"
                ))
                .map_err(|error| crate::utils::err_to_string(module_path!(), line!(), error))?;
            let rows = statement
                .query_map(params![before.min(i64::MAX as u64) as i64, limit], record_from_row)
                .map_err(|error| crate::utils::err_to_string(module_path!(), line!(), error))?;
            for row in rows {
                records.push(
                    row.map_err(|error| {
                        crate::utils::err_to_string(module_path!(), line!(), error)
                    })?,
                );
            }
        }
        None => {
            let mut statement = conn
                .prepare(&format!(
                    "SELECT {RECORD_COLUMNS} FROM playground_generations ORDER BY created_at DESC, id DESC LIMIT ?1"
                ))
                .map_err(|error| crate::utils::err_to_string(module_path!(), line!(), error))?;
            let rows = statement
                .query_map(params![limit], record_from_row)
                .map_err(|error| crate::utils::err_to_string(module_path!(), line!(), error))?;
            for row in rows {
                records.push(
                    row.map_err(|error| {
                        crate::utils::err_to_string(module_path!(), line!(), error)
                    })?,
                );
            }
        }
    }
    Ok(records)
}

#[tauri::command]
pub fn playground_history_save(
    app: AppHandle,
    entry: PlaygroundGenerationRecord,
) -> Result<(), String> {
    if entry.id.trim().is_empty() {
        return Err("The playground history entry needs an id.".to_string());
    }
    let conn = crate::storage_manager::db::open_db(&app)?;
    conn.execute(
        &format!(
            "INSERT OR REPLACE INTO playground_generations ({RECORD_COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)"
        ),
        params![
            entry.id,
            entry.created_at.min(i64::MAX as u64) as i64,
            entry.provider_id,
            entry.model_id,
            entry.model_name,
            entry.prompt,
            entry.negative_prompt,
            entry.seed.map(|value| value as i64),
            entry.params_json,
            entry.status,
            entry.error,
            entry.images_json,
        ],
    )
    .map_err(|error| crate::utils::err_to_string(module_path!(), line!(), error))?;
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredImageRef {
    #[serde(default)]
    asset_id: Option<String>,
}

#[tauri::command]
pub fn playground_history_delete(
    app: AppHandle,
    id: String,
    delete_images: bool,
) -> Result<(), String> {
    let conn = crate::storage_manager::db::open_db(&app)?;
    if delete_images {
        let images_json: Option<String> = conn
            .query_row(
                "SELECT images_json FROM playground_generations WHERE id = ?1",
                params![&id],
                |row| row.get(0),
            )
            .ok();
        if let Some(images_json) = images_json {
            if let Ok(images) = serde_json::from_str::<Vec<StoredImageRef>>(&images_json) {
                for image in images {
                    if let Some(asset_id) = image.asset_id.filter(|value| !value.trim().is_empty())
                    {
                        let _ = crate::storage_manager::media::storage_delete_image(
                            app.clone(),
                            asset_id,
                        );
                    }
                }
            }
        }
    }
    conn.execute(
        "DELETE FROM playground_generations WHERE id = ?1",
        params![id],
    )
    .map_err(|error| crate::utils::err_to_string(module_path!(), line!(), error))?;
    Ok(())
}
