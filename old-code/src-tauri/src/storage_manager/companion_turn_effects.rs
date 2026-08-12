use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::AppHandle;
use uuid::Uuid;

use crate::storage_manager::db::open_db;
use crate::utils::now_millis;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanionTurnEffect {
    pub id: String,
    pub session_id: String,
    pub user_message_id: Option<String>,
    pub assistant_message_id: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub status: String,
    pub summary: Option<String>,
    pub relationship_delta: Value,
    pub emotion_delta: Value,
    pub signal_changes: Value,
    pub memory_changes: Value,
    pub source_window: Value,
}

#[derive(Debug, Clone)]
pub struct CompanionTurnEffectSeed {
    pub relationship_delta: Value,
    pub emotion_delta: Value,
    pub signal_changes: Value,
}

impl Default for CompanionTurnEffectSeed {
    fn default() -> Self {
        Self {
            relationship_delta: json!({}),
            emotion_delta: json!({}),
            signal_changes: json!({ "added": [], "removed": [] }),
        }
    }
}

impl Default for CompanionTurnEffect {
    fn default() -> Self {
        Self {
            id: String::new(),
            session_id: String::new(),
            user_message_id: None,
            assistant_message_id: String::new(),
            created_at: 0,
            updated_at: 0,
            status: "processing".to_string(),
            summary: None,
            relationship_delta: json!({}),
            emotion_delta: json!({}),
            signal_changes: json!({ "added": [], "removed": [] }),
            memory_changes: json!({ "added": [], "updated": [], "superseded": [] }),
            source_window: json!({}),
        }
    }
}

pub fn create_processing_effect(
    app: &AppHandle,
    session_id: &str,
    user_message_id: Option<&str>,
    assistant_message_id: &str,
    seed: CompanionTurnEffectSeed,
) -> Result<CompanionTurnEffect, String> {
    let now = now_millis()?;
    let effect = CompanionTurnEffect {
        id: Uuid::new_v4().to_string(),
        session_id: session_id.to_string(),
        user_message_id: user_message_id.map(|value| value.to_string()),
        assistant_message_id: assistant_message_id.to_string(),
        created_at: now,
        updated_at: now,
        status: "processing".to_string(),
        summary: None,
        relationship_delta: seed.relationship_delta,
        emotion_delta: seed.emotion_delta,
        signal_changes: seed.signal_changes,
        memory_changes: json!({ "added": [], "updated": [], "superseded": [] }),
        source_window: json!({}),
    };
    upsert_effect(app, &effect)?;
    Ok(effect)
}

pub fn mark_effect_ready(
    app: &AppHandle,
    session_id: &str,
    assistant_message_id: &str,
    summary: Option<String>,
    memory_changes: Value,
    source_window: Value,
) -> Result<(), String> {
    let mut conn = open_db(app)?;
    let now = now_millis()?;
    let tx = conn
        .transaction()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    tx.execute(
        r#"
        UPDATE companion_turn_effects
        SET status = 'ready',
            summary = ?1,
            memory_changes = ?2,
            source_window = ?3,
            updated_at = ?4
        WHERE session_id = ?5 AND assistant_message_id = ?6
        "#,
        params![
            summary,
            memory_changes.to_string(),
            source_window.to_string(),
            now as i64,
            session_id,
            assistant_message_id
        ],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    tx.commit()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

pub fn mark_effect_failed(
    app: &AppHandle,
    session_id: &str,
    assistant_message_id: &str,
    error: &str,
) -> Result<(), String> {
    let conn = open_db(app)?;
    let now = now_millis()?;
    conn.execute(
        r#"
        UPDATE companion_turn_effects
        SET status = 'failed',
            summary = ?1,
            updated_at = ?2
        WHERE session_id = ?3 AND assistant_message_id = ?4
        "#,
        params![error, now as i64, session_id, assistant_message_id],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

pub fn get_effect_for_message(
    app: &AppHandle,
    session_id: &str,
    assistant_message_id: &str,
) -> Result<Option<CompanionTurnEffect>, String> {
    let conn = open_db(app)?;
    conn.query_row(
        r#"
        SELECT id, session_id, user_message_id, assistant_message_id, created_at, updated_at,
               status, summary, relationship_delta, emotion_delta, signal_changes,
               memory_changes, source_window
        FROM companion_turn_effects
        WHERE session_id = ?1 AND assistant_message_id = ?2
        ORDER BY updated_at DESC
        LIMIT 1
        "#,
        params![session_id, assistant_message_id],
        effect_from_row,
    )
    .optional()
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

#[tauri::command]
pub fn get_message_companion_effect(
    app: AppHandle,
    session_id: String,
    assistant_message_id: String,
) -> Result<Option<String>, String> {
    get_effect_for_message(&app, &session_id, &assistant_message_id)?
        .map(|effect| {
            serde_json::to_string(&effect)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
        })
        .transpose()
}

fn upsert_effect(app: &AppHandle, effect: &CompanionTurnEffect) -> Result<(), String> {
    let conn = open_db(app)?;
    conn.execute(
        r#"
        INSERT INTO companion_turn_effects (
            id, session_id, user_message_id, assistant_message_id, created_at, updated_at,
            status, summary, relationship_delta, emotion_delta, signal_changes,
            memory_changes, source_window
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
        ON CONFLICT(session_id, assistant_message_id) DO UPDATE SET
            user_message_id = excluded.user_message_id,
            updated_at = excluded.updated_at,
            status = excluded.status,
            summary = excluded.summary,
            relationship_delta = excluded.relationship_delta,
            emotion_delta = excluded.emotion_delta,
            signal_changes = excluded.signal_changes,
            memory_changes = excluded.memory_changes,
            source_window = excluded.source_window
        "#,
        params![
            effect.id,
            effect.session_id,
            effect.user_message_id,
            effect.assistant_message_id,
            effect.created_at as i64,
            effect.updated_at as i64,
            effect.status,
            effect.summary,
            effect.relationship_delta.to_string(),
            effect.emotion_delta.to_string(),
            effect.signal_changes.to_string(),
            effect.memory_changes.to_string(),
            effect.source_window.to_string(),
        ],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

fn effect_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CompanionTurnEffect> {
    let relationship_delta: String = row.get(8)?;
    let emotion_delta: String = row.get(9)?;
    let signal_changes: String = row.get(10)?;
    let memory_changes: String = row.get(11)?;
    let source_window: String = row.get(12)?;

    Ok(CompanionTurnEffect {
        id: row.get(0)?,
        session_id: row.get(1)?,
        user_message_id: row.get(2)?,
        assistant_message_id: row.get(3)?,
        created_at: row.get::<_, i64>(4)?.max(0) as u64,
        updated_at: row.get::<_, i64>(5)?.max(0) as u64,
        status: row.get(6)?,
        summary: row.get(7)?,
        relationship_delta: serde_json::from_str(&relationship_delta).unwrap_or_else(|_| json!({})),
        emotion_delta: serde_json::from_str(&emotion_delta).unwrap_or_else(|_| json!({})),
        signal_changes: serde_json::from_str(&signal_changes)
            .unwrap_or_else(|_| json!({ "added": [], "removed": [] })),
        memory_changes: serde_json::from_str(&memory_changes)
            .unwrap_or_else(|_| json!({ "added": [], "updated": [], "superseded": [] })),
        source_window: serde_json::from_str(&source_window).unwrap_or_else(|_| json!({})),
    })
}
