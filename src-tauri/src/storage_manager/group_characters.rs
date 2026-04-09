use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;

use super::db::{now_ms, SwappablePool};
use crate::storage_manager::group_sessions;
use crate::storage_manager::lorebook::Lorebook;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub id: String,
    pub name: String,
    pub character_ids: Vec<String>,
    #[serde(default)]
    pub muted_character_ids: Vec<String>,
    pub persona_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default)]
    pub archived: bool,
    #[serde(default = "default_chat_type")]
    pub chat_type: String,
    #[serde(default)]
    pub starting_scene: Option<serde_json::Value>,
    #[serde(default)]
    pub background_image_path: Option<String>,
    #[serde(default)]
    pub lorebook_ids: Vec<String>,
    #[serde(default)]
    pub disable_character_lorebooks: bool,
    #[serde(default = "default_speaker_selection_method")]
    pub speaker_selection_method: String,
    #[serde(default = "default_memory_type")]
    pub memory_type: String,
}

fn default_memory_type() -> String {
    "manual".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupPreview {
    pub id: String,
    pub name: String,
    pub character_ids: Vec<String>,
    pub updated_at: i64,
    pub last_message: Option<String>,
    pub message_count: i64,
    pub archived: bool,
    pub chat_type: String,
}

fn default_chat_type() -> String {
    "conversation".to_string()
}

fn default_speaker_selection_method() -> String {
    "llm".to_string()
}

fn ensure_participation_records(
    conn: &Connection,
    session_id: &str,
    character_ids: &[String],
) -> Result<(), String> {
    for character_id in character_ids {
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(1) FROM group_participation WHERE session_id = ?1 AND character_id = ?2",
                params![session_id, character_id],
                |row| row.get(0),
            )
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        if count == 0 {
            conn.execute(
                "INSERT INTO group_participation (id, session_id, character_id, speak_count, last_spoke_turn, last_spoke_at)
                 VALUES (?1, ?2, ?3, 0, NULL, NULL)",
                params![Uuid::new_v4().to_string(), session_id, character_id],
            )
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        }
    }

    Ok(())
}

fn read_group(conn: &Connection, id: &str) -> Result<Option<Group>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name, character_ids, muted_character_ids, persona_id, created_at, updated_at,
                    COALESCE(archived, 0), COALESCE(chat_type, 'conversation'), starting_scene,
                    background_image_path, COALESCE(lorebook_ids, '[]'),
                    COALESCE(disable_character_lorebooks, 0),
                    COALESCE(speaker_selection_method, 'llm'),
                    COALESCE(memory_type, 'manual')
             FROM group_characters
             WHERE id = ?1",
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let mut rows = stmt
        .query(params![id])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    if let Some(row) = rows
        .next()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
    {
        let character_ids_json: String = row
            .get(2)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let character_ids: Vec<String> =
            serde_json::from_str(&character_ids_json).unwrap_or_default();

        let muted_character_ids_json: String = row
            .get::<_, Option<String>>(3)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
            .unwrap_or_else(|| "[]".to_string());
        let mut muted_character_ids: Vec<String> =
            serde_json::from_str(&muted_character_ids_json).unwrap_or_default();
        muted_character_ids.retain(|cid| character_ids.contains(cid));

        let starting_scene_json: Option<String> = row
            .get(9)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let starting_scene: Option<serde_json::Value> =
            starting_scene_json.and_then(|s| serde_json::from_str(&s).ok());
        let lorebook_ids_json: String = row
            .get::<_, Option<String>>(11)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
            .unwrap_or_else(|| "[]".to_string());
        let lorebook_ids: Vec<String> =
            serde_json::from_str(&lorebook_ids_json).unwrap_or_default();

        Ok(Some(Group {
            id: row
                .get(0)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
            name: row
                .get(1)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
            character_ids,
            muted_character_ids,
            persona_id: row
                .get(4)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
            created_at: row
                .get(5)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
            updated_at: row
                .get(6)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
            archived: row
                .get::<_, i64>(7)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
                != 0,
            chat_type: row
                .get(8)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
            starting_scene,
            background_image_path: row
                .get(10)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
            lorebook_ids,
            disable_character_lorebooks: row
                .get::<_, i64>(12)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
                != 0,
            speaker_selection_method: row
                .get(13)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
            memory_type: row
                .get(14)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
        }))
    } else {
        Ok(None)
    }
}

fn load_lorebooks_for_ids(
    conn: &Connection,
    lorebook_ids: &[String],
) -> Result<Vec<Lorebook>, String> {
    let mut lorebooks = Vec::new();
    for lorebook_id in lorebook_ids {
        if let Some(lorebook) = conn
            .query_row(
                "SELECT id, name, avatar_path, keyword_detection_mode, created_at, updated_at FROM lorebooks WHERE id = ?1",
                params![lorebook_id],
                |row| {
                    Ok(Lorebook {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        avatar_path: row.get(2)?,
                        keyword_detection_mode:
                            crate::storage_manager::lorebook::LorebookKeywordDetectionMode::from_db_value(
                                row.get(3)?,
                            ),
                        created_at: row.get(4)?,
                        updated_at: row.get(5)?,
                    })
                },
            )
            .optional()
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        {
            lorebooks.push(lorebook);
        }
    }
    Ok(lorebooks)
}

#[tauri::command]
pub fn groups_list(pool: State<'_, SwappablePool>) -> Result<String, String> {
    let conn = pool.get_connection()?;

    let mut stmt = conn
        .prepare(
            "SELECT gc.id,
                    gc.name,
                    gc.character_ids,
                    COALESCE((SELECT MAX(gs.updated_at) FROM group_sessions gs WHERE gs.group_character_id = gc.id), gc.updated_at) as effective_updated_at,
                    (SELECT gm.content
                     FROM group_messages gm
                     JOIN group_sessions gs2 ON gm.session_id = gs2.id
                     WHERE gs2.group_character_id = gc.id
                     ORDER BY gm.created_at DESC
                     LIMIT 1) as last_message,
                    (SELECT COUNT(*)
                     FROM group_messages gm
                     JOIN group_sessions gs3 ON gm.session_id = gs3.id
                     WHERE gs3.group_character_id = gc.id) as message_count,
                    COALESCE(gc.archived, 0) as archived,
                    COALESCE(gc.chat_type, 'conversation') as chat_type
             FROM group_characters gc
             WHERE COALESCE(gc.archived, 0) = 0
             ORDER BY effective_updated_at DESC",
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let mut rows = stmt
        .query([])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let mut items: Vec<GroupPreview> = Vec::new();

    while let Some(row) = rows
        .next()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
    {
        let character_ids_json: String = row
            .get(2)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let character_ids: Vec<String> =
            serde_json::from_str(&character_ids_json).unwrap_or_default();

        items.push(GroupPreview {
            id: row
                .get(0)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
            name: row
                .get(1)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
            character_ids,
            updated_at: row
                .get(3)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
            last_message: row
                .get(4)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
            message_count: row
                .get(5)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
            archived: row
                .get::<_, i64>(6)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
                != 0,
            chat_type: row
                .get(7)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
        });
    }

    serde_json::to_string(&items)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

#[tauri::command]
pub fn group_create(
    name: String,
    character_ids_json: String,
    persona_id: Option<String>,
    chat_type: Option<String>,
    starting_scene_json: Option<String>,
    background_image_path: Option<String>,
    speaker_selection_method: Option<String>,
    app: tauri::AppHandle,
    pool: State<'_, SwappablePool>,
) -> Result<String, String> {
    let conn = pool.get_connection()?;
    let now = now_ms() as i64;
    let id = Uuid::new_v4().to_string();

    let character_ids: Vec<String> = serde_json::from_str(&character_ids_json)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let final_persona_id = if persona_id.is_none() {
        super::personas::default_persona_id(app.clone())
    } else {
        persona_id
    };

    let chat_type_value = chat_type.unwrap_or_else(default_chat_type);
    let selection_method =
        speaker_selection_method.unwrap_or_else(default_speaker_selection_method);
    let lorebook_ids_json = "[]".to_string();

    conn.execute(
        "INSERT INTO group_characters (id, name, character_ids, muted_character_ids, persona_id, created_at, updated_at, archived, chat_type, starting_scene, background_image_path, lorebook_ids, disable_character_lorebooks, speaker_selection_method)
         VALUES (?1, ?2, ?3, '[]', ?4, ?5, ?5, 0, ?6, ?7, ?8, ?9, 0, ?10)",
        params![
            id,
            name,
            character_ids_json,
            final_persona_id.as_deref(),
            now,
            chat_type_value,
            starting_scene_json.as_deref(),
            background_image_path,
            lorebook_ids_json,
            selection_method
        ],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let item = Group {
        id,
        name,
        character_ids,
        muted_character_ids: vec![],
        persona_id: final_persona_id,
        created_at: now,
        updated_at: now,
        archived: false,
        chat_type: chat_type_value,
        starting_scene: starting_scene_json.and_then(|s| serde_json::from_str(&s).ok()),
        background_image_path,
        lorebook_ids: Vec::new(),
        disable_character_lorebooks: false,
        speaker_selection_method: selection_method,
        memory_type: "manual".to_string(),
    };

    serde_json::to_string(&item)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

#[tauri::command]
pub fn group_get(id: String, pool: State<'_, SwappablePool>) -> Result<String, String> {
    let conn = pool.get_connection()?;
    match read_group(&conn, &id)? {
        Some(item) => serde_json::to_string(&item)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e)),
        None => Ok("null".to_string()),
    }
}

#[tauri::command]
pub fn group_lorebooks_list(id: String, pool: State<'_, SwappablePool>) -> Result<String, String> {
    let conn = pool.get_connection()?;
    let group = read_group(&conn, &id)?
        .ok_or_else(|| crate::utils::err_msg(module_path!(), line!(), "Group not found"))?;
    let lorebooks = load_lorebooks_for_ids(&conn, &group.lorebook_ids)?;
    serde_json::to_string(&lorebooks)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

#[tauri::command]
pub fn group_lorebooks_set(
    id: String,
    lorebook_ids_json: String,
    pool: State<'_, SwappablePool>,
) -> Result<String, String> {
    let conn = pool.get_connection()?;
    let now = now_ms() as i64;
    let lorebook_ids: Vec<String> = serde_json::from_str(&lorebook_ids_json)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let lorebook_ids_json = serde_json::to_string(&lorebook_ids)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    conn.execute(
        "UPDATE group_characters SET lorebook_ids = ?1, updated_at = ?2 WHERE id = ?3",
        params![lorebook_ids_json, now, id],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    group_get(id, pool)
}

#[tauri::command]
pub fn group_update_disable_character_lorebooks(
    id: String,
    disable_character_lorebooks: bool,
    pool: State<'_, SwappablePool>,
) -> Result<String, String> {
    let conn = pool.get_connection()?;
    let now = now_ms() as i64;
    conn.execute(
        "UPDATE group_characters SET disable_character_lorebooks = ?1, updated_at = ?2 WHERE id = ?3",
        params![disable_character_lorebooks as i64, now, id],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    group_get(id, pool)
}

#[tauri::command]
pub fn group_update(
    id: String,
    name: String,
    character_ids_json: String,
    muted_character_ids_json: Option<String>,
    persona_id: Option<String>,
    chat_type: Option<String>,
    starting_scene_json: Option<String>,
    background_image_path: Option<String>,
    speaker_selection_method: Option<String>,
    pool: State<'_, SwappablePool>,
) -> Result<String, String> {
    let conn = pool.get_connection()?;
    let now = now_ms() as i64;

    let character_ids: Vec<String> = serde_json::from_str(&character_ids_json)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let existing = read_group(&conn, &id)?.ok_or_else(|| {
        crate::utils::err_msg(module_path!(), line!(), "Group character not found")
    })?;

    let mut muted_character_ids = match muted_character_ids_json {
        Some(json) => serde_json::from_str(&json)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
        None => existing.muted_character_ids,
    };
    muted_character_ids.retain(|cid| character_ids.contains(cid));
    muted_character_ids.sort();
    muted_character_ids.dedup();
    let muted_character_ids_json = serde_json::to_string(&muted_character_ids)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let next_chat_type = chat_type.unwrap_or(existing.chat_type);
    let next_speaker_selection_method =
        speaker_selection_method.unwrap_or(existing.speaker_selection_method);
    let lorebook_ids_json = serde_json::to_string(&existing.lorebook_ids)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    conn.execute(
        "UPDATE group_characters
         SET name = ?1,
             character_ids = ?2,
             muted_character_ids = ?3,
             persona_id = ?4,
             chat_type = ?5,
             starting_scene = ?6,
             background_image_path = ?7,
             lorebook_ids = ?8,
             disable_character_lorebooks = ?9,
             speaker_selection_method = ?10,
             updated_at = ?11
         WHERE id = ?12",
        params![
            name,
            character_ids_json,
            muted_character_ids_json,
            persona_id,
            next_chat_type,
            starting_scene_json,
            background_image_path,
            lorebook_ids_json,
            existing.disable_character_lorebooks as i64,
            next_speaker_selection_method,
            now,
            id
        ],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let starting_scene: Option<serde_json::Value> =
        starting_scene_json.and_then(|s| serde_json::from_str(&s).ok());

    let updated = Group {
        id,
        name,
        character_ids,
        muted_character_ids,
        persona_id,
        created_at: existing.created_at,
        updated_at: now,
        archived: existing.archived,
        chat_type: next_chat_type,
        starting_scene,
        background_image_path,
        lorebook_ids: existing.lorebook_ids,
        disable_character_lorebooks: existing.disable_character_lorebooks,
        speaker_selection_method: next_speaker_selection_method,
        memory_type: existing.memory_type,
    };

    serde_json::to_string(&updated)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

#[tauri::command]
pub fn group_update_name(
    id: String,
    name: String,
    pool: State<'_, SwappablePool>,
) -> Result<(), String> {
    let conn = pool.get_connection()?;
    let now = now_ms() as i64;
    conn.execute(
        "UPDATE group_characters SET name = ?1, updated_at = ?2 WHERE id = ?3",
        params![name, now, id],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

#[tauri::command]
pub fn group_update_persona(
    id: String,
    persona_id: Option<String>,
    pool: State<'_, SwappablePool>,
) -> Result<(), String> {
    let conn = pool.get_connection()?;
    let now = now_ms() as i64;
    conn.execute(
        "UPDATE group_characters SET persona_id = ?1, updated_at = ?2 WHERE id = ?3",
        params![persona_id, now, id],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

#[tauri::command]
pub fn group_update_speaker_selection_method(
    id: String,
    speaker_selection_method: String,
    pool: State<'_, SwappablePool>,
) -> Result<(), String> {
    let conn = pool.get_connection()?;
    let now = now_ms() as i64;
    conn.execute(
        "UPDATE group_characters SET speaker_selection_method = ?1, updated_at = ?2 WHERE id = ?3",
        params![speaker_selection_method, now, id],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

#[tauri::command]
pub fn group_update_memory_type(
    id: String,
    memory_type: String,
    pool: State<'_, SwappablePool>,
) -> Result<(), String> {
    let conn = pool.get_connection()?;
    let now = now_ms() as i64;
    conn.execute(
        "UPDATE group_characters SET memory_type = ?1, updated_at = ?2 WHERE id = ?3",
        params![memory_type, now, id],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

#[tauri::command]
pub fn group_update_background_image(
    id: String,
    background_image_path: Option<String>,
    pool: State<'_, SwappablePool>,
) -> Result<(), String> {
    let conn = pool.get_connection()?;
    let now = now_ms() as i64;
    conn.execute(
        "UPDATE group_characters SET background_image_path = ?1, updated_at = ?2 WHERE id = ?3",
        params![background_image_path, now, id],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

#[tauri::command]
pub fn group_update_character_ids(
    id: String,
    character_ids_json: String,
    pool: State<'_, SwappablePool>,
) -> Result<(), String> {
    let conn = pool.get_connection()?;
    let now = now_ms() as i64;

    let character_ids: Vec<String> = serde_json::from_str(&character_ids_json)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    // Read current muted_character_ids and prune to only IDs still in the new list
    let muted_json: Option<String> = conn
        .query_row(
            "SELECT muted_character_ids FROM group_characters WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let mut muted: Vec<String> = muted_json
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();
    muted.retain(|cid| character_ids.contains(cid));
    let muted_out = serde_json::to_string(&muted)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    conn.execute(
        "UPDATE group_characters SET character_ids = ?1, muted_character_ids = ?2, updated_at = ?3 WHERE id = ?4",
        params![character_ids_json, muted_out, now, id],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

#[tauri::command]
pub fn group_update_muted_character_ids(
    id: String,
    muted_character_ids_json: String,
    pool: State<'_, SwappablePool>,
) -> Result<(), String> {
    let conn = pool.get_connection()?;
    let now = now_ms() as i64;

    let mut muted: Vec<String> = serde_json::from_str(&muted_character_ids_json)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    // Read current character_ids to validate
    let char_json: String = conn
        .query_row(
            "SELECT character_ids FROM group_characters WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let character_ids: Vec<String> = serde_json::from_str(&char_json).unwrap_or_default();

    muted.retain(|cid| character_ids.contains(cid));
    muted.sort();
    muted.dedup();

    // Ensure at least 1 active
    if !character_ids.is_empty() && muted.len() >= character_ids.len() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "At least one participant must remain active",
        ));
    }

    let muted_out = serde_json::to_string(&muted)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    conn.execute(
        "UPDATE group_characters SET muted_character_ids = ?1, updated_at = ?2 WHERE id = ?3",
        params![muted_out, now, id],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

#[tauri::command]
pub fn group_update_starting_scene(
    id: String,
    starting_scene_json: Option<String>,
    pool: State<'_, SwappablePool>,
) -> Result<(), String> {
    let conn = pool.get_connection()?;
    let now = now_ms() as i64;
    conn.execute(
        "UPDATE group_characters SET starting_scene = ?1, updated_at = ?2 WHERE id = ?3",
        params![starting_scene_json, now, id],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

#[tauri::command]
pub fn group_delete(id: String, pool: State<'_, SwappablePool>) -> Result<(), String> {
    let conn = pool.get_connection()?;

    conn.execute(
        "DELETE FROM group_sessions WHERE group_character_id = ?1",
        params![id],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    conn.execute("DELETE FROM group_characters WHERE id = ?1", params![id])
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    Ok(())
}

#[tauri::command]
pub fn group_create_session(
    group_id: String,
    pool: State<'_, SwappablePool>,
) -> Result<String, String> {
    let conn = pool.get_connection()?;
    let now = now_ms() as i64;

    let config = read_group(&conn, &group_id)?
        .ok_or_else(|| crate::utils::err_msg(module_path!(), line!(), "Group not found"))?;

    let session_id = Uuid::new_v4().to_string();
    let character_ids_json = serde_json::to_string(&config.character_ids)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let muted_character_ids_json = serde_json::to_string(&config.muted_character_ids)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let starting_scene_json = config
        .starting_scene
        .as_ref()
        .and_then(|s| serde_json::to_string(s).ok());

    conn.execute(
        "INSERT INTO group_sessions (id, group_character_id, name, character_ids, muted_character_ids, persona_id, created_at, updated_at, archived, chat_type, starting_scene, background_image_path, lorebook_ids, disable_character_lorebooks, speaker_selection_method, memory_type)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, 0, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        params![
            session_id,
            group_id,
            &config.name,
            character_ids_json,
            muted_character_ids_json,
            &config.persona_id,
            now,
            &config.chat_type,
            starting_scene_json,
            &config.background_image_path,
            serde_json::to_string(&config.lorebook_ids)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
            config.disable_character_lorebooks as i64,
            &config.speaker_selection_method,
            &config.memory_type,
        ],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    ensure_participation_records(&conn, &session_id, &config.character_ids)?;

    if config.chat_type == "roleplay" {
        if let Some(scene) = config.starting_scene {
            if let Some(content) = scene.get("content").and_then(|v| v.as_str()) {
                if !content.trim().is_empty() {
                    conn.execute(
                        "INSERT INTO group_messages (id, session_id, role, content, speaker_character_id, turn_number, created_at, is_pinned, attachments)
                         VALUES (?1, ?2, 'scene', ?3, NULL, 0, ?4, 0, '[]')",
                        params![Uuid::new_v4().to_string(), session_id, content, now],
                    )
                    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
                }
            }
        }
    }

    group_sessions::group_session_get_internal(&conn, &session_id)
}
