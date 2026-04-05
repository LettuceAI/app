use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value as JsonValue};
use std::collections::BTreeMap;
use uuid::Uuid;

use super::db::DbConnection;
use crate::utils::now_millis;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Lorebook {
    pub id: String,
    pub name: String,
    pub avatar_path: Option<String>,
    #[serde(default)]
    pub keyword_detection_mode: LorebookKeywordDetectionMode,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum LorebookKeywordDetectionMode {
    #[default]
    RecentMessageWindow,
    LatestUserMessage,
}

impl LorebookKeywordDetectionMode {
    pub(crate) fn from_db_value(value: Option<String>) -> Self {
        match value.as_deref() {
            Some("latest_user_message") => Self::LatestUserMessage,
            _ => Self::RecentMessageWindow,
        }
    }

    pub(crate) fn as_db_value(self) -> &'static str {
        match self {
            Self::RecentMessageWindow => "recent_message_window",
            Self::LatestUserMessage => "latest_user_message",
        }
    }
}

impl Lorebook {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Lorebook {
            id: row.get(0)?,
            name: row.get(1)?,
            avatar_path: row.get(2)?,
            keyword_detection_mode: LorebookKeywordDetectionMode::from_db_value(row.get(3)?),
            created_at: row.get(4)?,
            updated_at: row.get(5)?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LorebookEntry {
    pub id: String,
    pub lorebook_id: String,
    pub title: String,
    pub enabled: bool,
    pub always_active: bool,
    pub keywords: Vec<String>,
    pub case_sensitive: bool,
    pub content: String,
    pub priority: i32,
    pub display_order: i32,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone)]
pub struct LorebookEntryActivationContext {
    pub entry: LorebookEntry,
    pub keyword_detection_mode: LorebookKeywordDetectionMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
struct WorldInfoExport {
    name: String,
    description: String,
    is_creation: bool,
    scan_depth: i64,
    token_budget: i64,
    recursive_scanning: bool,
    #[serde(default)]
    extensions: JsonValue,
    entries: BTreeMap<String, WorldInfoExportEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
struct WorldInfoExportEntry {
    uid: i64,
    #[serde(rename = "key")]
    key: Vec<String>,
    #[serde(default)]
    keysecondary: Vec<String>,
    comment: String,
    content: String,
    constant: bool,
    selective: bool,
    #[serde(rename = "selectiveLogic")]
    selective_logic: i32,
    order: i32,
    position: i32,
    disable: bool,
    #[serde(rename = "addMemo")]
    add_memo: bool,
    #[serde(rename = "excludeRecursion")]
    exclude_recursion: bool,
    probability: i32,
    #[serde(rename = "displayIndex")]
    display_index: i32,
    #[serde(rename = "useProbability")]
    use_probability: bool,
    secondary_keys: Vec<String>,
    keys: Vec<String>,
    id: i64,
    priority: i32,
    insertion_order: i32,
    enabled: bool,
    name: String,
    #[serde(default)]
    extensions: JsonValue,
    case_sensitive: bool,
    depth: i32,
    #[serde(default)]
    character_filter: Option<JsonValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
struct WorldInfoImport {
    name: String,
    #[serde(default)]
    extensions: JsonValue,
    #[serde(default)]
    entries: JsonValue,
}

impl LorebookEntry {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        let keywords_json: String = row.get(5)?;
        let keywords: Vec<String> = serde_json::from_str(&keywords_json).unwrap_or_default();

        Ok(LorebookEntry {
            id: row.get(0)?,
            lorebook_id: row.get(1)?,
            title: row.get(2)?,
            enabled: row.get::<_, i32>(3)? != 0,
            always_active: row.get::<_, i32>(4)? != 0,
            keywords,
            case_sensitive: row.get::<_, i32>(6)? != 0,
            content: row.get(7)?,
            priority: row.get(8)?,
            display_order: row.get(9)?,
            created_at: row.get(10)?,
            updated_at: row.get(11)?,
        })
    }
}

// ============================================================================
// Lorebooks (app-level)
// ============================================================================

pub fn list_lorebooks(conn: &DbConnection) -> Result<Vec<Lorebook>, String> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, name, avatar_path, keyword_detection_mode, created_at, updated_at
            FROM lorebooks
            ORDER BY updated_at DESC
            "#,
        )
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to prepare lorebooks list: {}", e),
            )
        })?;

    let items = stmt
        .query_map([], Lorebook::from_row)
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to query lorebooks: {}", e),
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to collect lorebooks: {}", e),
            )
        })?;

    Ok(items)
}

pub fn get_lorebook(conn: &DbConnection, lorebook_id: &str) -> Result<Option<Lorebook>, String> {
    conn.query_row(
        "SELECT id, name, avatar_path, keyword_detection_mode, created_at, updated_at FROM lorebooks WHERE id = ?1",
        params![lorebook_id],
        Lorebook::from_row,
    )
    .optional()
    .map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to query lorebook: {}", e),
        )
    })
}

pub fn upsert_lorebook(conn: &DbConnection, lorebook: &Lorebook) -> Result<Lorebook, String> {
    let now = now_millis()? as i64;

    let exists: bool = conn
        .query_row(
            "SELECT 1 FROM lorebooks WHERE id = ?1",
            params![lorebook.id],
            |_| Ok(true),
        )
        .optional()
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to check lorebook existence: {}", e),
            )
        })?
        .unwrap_or(false);

    if exists {
        conn.execute(
            "UPDATE lorebooks SET name = ?2, avatar_path = ?3, keyword_detection_mode = ?4, updated_at = ?5 WHERE id = ?1",
            params![
                lorebook.id,
                lorebook.name,
                lorebook.avatar_path,
                lorebook.keyword_detection_mode.as_db_value(),
                now
            ],
        )
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to update lorebook: {}", e),
            )
        })?;
    } else {
        conn.execute(
            "INSERT INTO lorebooks (id, name, avatar_path, keyword_detection_mode, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                lorebook.id,
                lorebook.name,
                lorebook.avatar_path,
                lorebook.keyword_detection_mode.as_db_value(),
                lorebook.created_at,
                now
            ],
        )
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to insert lorebook: {}", e),
            )
        })?;
    }

    get_lorebook(conn, &lorebook.id)?
        .ok_or_else(|| "Failed to retrieve lorebook after upsert".to_string())
}

pub fn delete_lorebook(conn: &DbConnection, lorebook_id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM lorebooks WHERE id = ?1", params![lorebook_id])
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to delete lorebook: {}", e),
            )
        })?;
    Ok(())
}

pub fn list_character_lorebooks(
    conn: &DbConnection,
    character_id: &str,
) -> Result<Vec<Lorebook>, String> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT l.id, l.name, l.avatar_path, l.keyword_detection_mode, l.created_at, l.updated_at
            FROM character_lorebooks cl
            JOIN lorebooks l ON l.id = cl.lorebook_id
            WHERE cl.character_id = ?1 AND cl.enabled = 1
            ORDER BY cl.display_order ASC, l.updated_at DESC
            "#,
        )
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to prepare character lorebooks list: {}", e),
            )
        })?;

    let items = stmt
        .query_map(params![character_id], Lorebook::from_row)
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to query character lorebooks: {}", e),
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to collect character lorebooks: {}", e),
            )
        })?;

    Ok(items)
}

pub fn set_character_lorebooks(
    conn: &mut DbConnection,
    character_id: &str,
    lorebook_ids: &[String],
) -> Result<(), String> {
    let now = now_millis()? as i64;
    let tx = conn.transaction().map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to start transaction: {}", e),
        )
    })?;

    tx.execute(
        "DELETE FROM character_lorebooks WHERE character_id = ?1",
        params![character_id],
    )
    .map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to clear character lorebooks: {}", e),
        )
    })?;

    for (idx, lorebook_id) in lorebook_ids.iter().enumerate() {
        tx.execute(
            r#"
            INSERT INTO character_lorebooks (character_id, lorebook_id, enabled, display_order, created_at, updated_at)
            VALUES (?1, ?2, 1, ?3, ?4, ?4)
            "#,
            params![character_id, lorebook_id, idx as i32, now],
        )
        .map_err(|e| crate::utils::err_msg(module_path!(), line!(), format!("Failed to set character lorebook mapping: {}", e)))?;
    }

    tx.commit().map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to commit character lorebooks: {}", e),
        )
    })?;

    Ok(())
}

// ============================================================================
// Lorebook entries
// ============================================================================

pub fn get_lorebook_entries(
    conn: &DbConnection,
    lorebook_id: &str,
) -> Result<Vec<LorebookEntry>, String> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, lorebook_id, title, enabled, always_active, keywords,
                   case_sensitive, content, priority, display_order,
                   created_at, updated_at
            FROM lorebook_entries
            WHERE lorebook_id = ?1
            ORDER BY display_order ASC, created_at ASC
            "#,
        )
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to prepare entries query: {}", e),
            )
        })?;

    let entries = stmt
        .query_map(params![lorebook_id], LorebookEntry::from_row)
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to execute entries query: {}", e),
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to collect entries: {}", e),
            )
        })?;

    Ok(entries)
}

pub fn get_enabled_character_lorebook_entry_contexts(
    conn: &DbConnection,
    character_id: &str,
) -> Result<Vec<LorebookEntryActivationContext>, String> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT e.id, e.lorebook_id, e.title, e.enabled, e.always_active, e.keywords,
                   e.case_sensitive, e.content, e.priority, e.display_order,
                   e.created_at, e.updated_at, l.keyword_detection_mode
            FROM lorebook_entries e
            JOIN character_lorebooks cl ON cl.lorebook_id = e.lorebook_id
            JOIN lorebooks l ON l.id = e.lorebook_id
            WHERE cl.character_id = ?1 AND cl.enabled = 1 AND e.enabled = 1
            ORDER BY cl.display_order ASC, e.display_order ASC, e.created_at ASC
            "#,
        )
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to prepare enabled entries query: {}", e),
            )
        })?;

    let entries = stmt
        .query_map(params![character_id], |row| {
            Ok(LorebookEntryActivationContext {
                entry: LorebookEntry::from_row(row)?,
                keyword_detection_mode: LorebookKeywordDetectionMode::from_db_value(row.get(12)?),
            })
        })
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to execute enabled entries query: {}", e),
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to collect enabled entries: {}", e),
            )
        })?;

    Ok(entries)
}

pub fn get_enabled_lorebook_entry_contexts_for_ids(
    conn: &DbConnection,
    lorebook_ids: &[String],
) -> Result<Vec<LorebookEntryActivationContext>, String> {
    if lorebook_ids.is_empty() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    for lorebook_id in lorebook_ids {
        let lorebook_entries = get_lorebook_entries(conn, lorebook_id)?;
        let keyword_detection_mode = get_lorebook(conn, lorebook_id)?
            .map(|lorebook| lorebook.keyword_detection_mode)
            .unwrap_or_default();
        for entry in lorebook_entries.into_iter().filter(|entry| entry.enabled) {
            entries.push(LorebookEntryActivationContext {
                entry,
                keyword_detection_mode,
            });
        }
    }

    entries.sort_by(|a, b| {
        let a_idx = lorebook_ids
            .iter()
            .position(|id| id == &a.entry.lorebook_id)
            .unwrap_or(usize::MAX);
        let b_idx = lorebook_ids
            .iter()
            .position(|id| id == &b.entry.lorebook_id)
            .unwrap_or(usize::MAX);
        a_idx
            .cmp(&b_idx)
            .then_with(|| a.entry.display_order.cmp(&b.entry.display_order))
            .then_with(|| a.entry.created_at.cmp(&b.entry.created_at))
    });

    Ok(entries)
}

pub fn get_lorebook_entry(
    conn: &DbConnection,
    entry_id: &str,
) -> Result<Option<LorebookEntry>, String> {
    conn.query_row(
        r#"
        SELECT id, lorebook_id, title, enabled, always_active, keywords,
               case_sensitive, content, priority, display_order,
               created_at, updated_at
        FROM lorebook_entries
        WHERE id = ?1
        "#,
        params![entry_id],
        LorebookEntry::from_row,
    )
    .optional()
    .map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to query entry: {}", e),
        )
    })
}

pub fn upsert_lorebook_entry(
    conn: &DbConnection,
    entry: &LorebookEntry,
) -> Result<LorebookEntry, String> {
    let keywords_json = serde_json::to_string(&entry.keywords).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to serialize keywords: {}", e),
        )
    })?;

    let exists: bool = conn
        .query_row(
            "SELECT 1 FROM lorebook_entries WHERE id = ?1",
            params![entry.id],
            |_| Ok(true),
        )
        .optional()
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to check entry existence: {}", e),
            )
        })?
        .unwrap_or(false);

    let now = now_millis()? as i64;

    if exists {
        conn.execute(
            r#"
            UPDATE lorebook_entries
            SET lorebook_id = ?2, title = ?3, enabled = ?4, always_active = ?5, keywords = ?6,
                case_sensitive = ?7, content = ?8, priority = ?9, display_order = ?10,
                updated_at = ?11
            WHERE id = ?1
            "#,
            params![
                entry.id,
                entry.lorebook_id,
                entry.title,
                entry.enabled as i32,
                entry.always_active as i32,
                keywords_json,
                entry.case_sensitive as i32,
                entry.content,
                entry.priority,
                entry.display_order,
                now,
            ],
        )
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to update entry: {}", e),
            )
        })?;
    } else {
        conn.execute(
            r#"
            INSERT INTO lorebook_entries (
              id, lorebook_id, title, enabled, always_active, keywords,
              case_sensitive, content, priority, display_order,
              created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            "#,
            params![
                entry.id,
                entry.lorebook_id,
                entry.title,
                entry.enabled as i32,
                entry.always_active as i32,
                keywords_json,
                entry.case_sensitive as i32,
                entry.content,
                entry.priority,
                entry.display_order,
                entry.created_at,
                now,
            ],
        )
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to insert entry: {}", e),
            )
        })?;
    }

    get_lorebook_entry(conn, &entry.id)?
        .ok_or_else(|| "Failed to retrieve entry after upsert".to_string())
}

pub fn delete_lorebook_entry(conn: &DbConnection, entry_id: &str) -> Result<(), String> {
    conn.execute(
        "DELETE FROM lorebook_entries WHERE id = ?1",
        params![entry_id],
    )
    .map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to delete entry: {}", e),
        )
    })?;
    Ok(())
}

pub fn update_entry_display_order(
    conn: &DbConnection,
    updates: Vec<(String, i32)>,
) -> Result<(), String> {
    let now = now_millis()? as i64;
    for (entry_id, display_order) in updates {
        conn.execute(
            "UPDATE lorebook_entries SET display_order = ?1, updated_at = ?2 WHERE id = ?3",
            params![display_order, now, entry_id],
        )
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to update display order for {}: {}", entry_id, e),
            )
        })?;
    }
    Ok(())
}

fn number_to_i32(value: Option<&JsonValue>) -> Option<i32> {
    value
        .and_then(|v| v.as_i64())
        .or_else(|| value.and_then(|v| v.as_u64().map(|n| n as i64)))
        .and_then(|n| i32::try_from(n).ok())
}

fn value_to_string_list(value: Option<&JsonValue>) -> Vec<String> {
    value
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str())
                .map(|item| item.trim().to_string())
                .filter(|item| !item.is_empty())
                .collect::<Vec<String>>()
        })
        .unwrap_or_default()
}

fn parse_world_info_entries(entries_value: &JsonValue) -> Vec<LorebookEntry> {
    let entries: Vec<(Option<i64>, &JsonValue)> = if let Some(map) = entries_value.as_object() {
        map.iter()
            .map(|(key, value)| (key.parse::<i64>().ok(), value))
            .collect()
    } else if let Some(list) = entries_value.as_array() {
        list.iter()
            .enumerate()
            .map(|(index, value)| (Some(index as i64), value))
            .collect()
    } else {
        Vec::new()
    };

    let mut parsed: Vec<LorebookEntry> = entries
        .into_iter()
        .enumerate()
        .filter_map(|(index, (map_index, value))| {
            let obj = value.as_object()?;
            let keys = {
                let mut primary = value_to_string_list(obj.get("keys"));
                if primary.is_empty() {
                    primary = value_to_string_list(obj.get("key"));
                }
                primary
            };
            let title = obj
                .get("name")
                .and_then(|v| v.as_str())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .or_else(|| keys.first().cloned())
                .unwrap_or_else(|| format!("Entry {}", index + 1));

            let content = obj
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            if content.trim().is_empty() {
                return None;
            }

            let enabled = obj
                .get("enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or_else(|| {
                    !obj.get("disable")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                });

            let display_order = number_to_i32(obj.get("insertion_order"))
                .or_else(|| number_to_i32(obj.get("displayIndex")).map(|n| n.saturating_sub(1)))
                .or_else(|| {
                    map_index
                        .and_then(|n| i32::try_from(n).ok())
                        .map(|n| n.saturating_sub(1))
                })
                .unwrap_or(index as i32);

            Some(LorebookEntry {
                id: Uuid::new_v4().to_string(),
                lorebook_id: String::new(),
                title,
                enabled,
                always_active: obj
                    .get("constant")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                keywords: keys,
                case_sensitive: obj
                    .get("case_sensitive")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                content,
                priority: number_to_i32(obj.get("priority"))
                    .or_else(|| number_to_i32(obj.get("order")))
                    .unwrap_or(0),
                display_order,
                created_at: 0,
                updated_at: 0,
            })
        })
        .collect();

    parsed.sort_by_key(|entry| entry.display_order);
    for (idx, entry) in parsed.iter_mut().enumerate() {
        entry.display_order = idx as i32;
    }

    parsed
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub fn lorebooks_list(app: tauri::AppHandle) -> Result<String, String> {
    let conn = crate::storage_manager::db::open_db(&app)?;
    let lorebooks = list_lorebooks(&conn)?;
    serde_json::to_string(&lorebooks).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to serialize lorebooks: {}", e),
        )
    })
}

#[tauri::command]
pub fn lorebook_upsert(app: tauri::AppHandle, lorebook_json: String) -> Result<String, String> {
    let lorebook: Lorebook = serde_json::from_str(&lorebook_json).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Invalid lorebook JSON: {}", e),
        )
    })?;
    let conn = crate::storage_manager::db::open_db(&app)?;
    let updated = upsert_lorebook(&conn, &lorebook)?;
    serde_json::to_string(&updated).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to serialize lorebook: {}", e),
        )
    })
}

#[tauri::command]
pub fn lorebook_delete(app: tauri::AppHandle, lorebook_id: String) -> Result<(), String> {
    let conn = crate::storage_manager::db::open_db(&app)?;
    delete_lorebook(&conn, &lorebook_id)
}

#[tauri::command]
pub fn character_lorebooks_list(
    app: tauri::AppHandle,
    character_id: String,
) -> Result<String, String> {
    let conn = crate::storage_manager::db::open_db(&app)?;
    let lorebooks = list_character_lorebooks(&conn, &character_id)?;
    serde_json::to_string(&lorebooks).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to serialize character lorebooks: {}", e),
        )
    })
}

#[tauri::command]
pub fn character_lorebooks_set(
    app: tauri::AppHandle,
    character_id: String,
    lorebook_ids_json: String,
) -> Result<(), String> {
    let lorebook_ids: Vec<String> = serde_json::from_str(&lorebook_ids_json).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Invalid lorebook ids JSON: {}", e),
        )
    })?;
    let mut conn = crate::storage_manager::db::open_db(&app)?;
    set_character_lorebooks(&mut conn, &character_id, &lorebook_ids)
}

#[tauri::command]
pub fn lorebook_entries_list(app: tauri::AppHandle, lorebook_id: String) -> Result<String, String> {
    let conn = crate::storage_manager::db::open_db(&app)?;
    let entries = get_lorebook_entries(&conn, &lorebook_id)?;
    serde_json::to_string(&entries).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to serialize entries: {}", e),
        )
    })
}

#[tauri::command]
pub fn lorebook_entry_get(app: tauri::AppHandle, entry_id: String) -> Result<String, String> {
    let conn = crate::storage_manager::db::open_db(&app)?;
    let entry = get_lorebook_entry(&conn, &entry_id)?;
    serde_json::to_string(&entry).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to serialize entry: {}", e),
        )
    })
}

#[tauri::command]
pub fn lorebook_entry_upsert(app: tauri::AppHandle, entry_json: String) -> Result<String, String> {
    let entry: LorebookEntry = serde_json::from_str(&entry_json).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Invalid entry JSON: {}", e),
        )
    })?;

    let conn = crate::storage_manager::db::open_db(&app)?;
    let updated_entry = upsert_lorebook_entry(&conn, &entry)?;

    serde_json::to_string(&updated_entry).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to serialize updated entry: {}", e),
        )
    })
}

#[tauri::command]
pub fn lorebook_entry_delete(app: tauri::AppHandle, entry_id: String) -> Result<(), String> {
    let conn = crate::storage_manager::db::open_db(&app)?;
    delete_lorebook_entry(&conn, &entry_id)
}

#[tauri::command]
pub fn lorebook_entry_create_blank(
    app: tauri::AppHandle,
    lorebook_id: String,
) -> Result<String, String> {
    let conn = crate::storage_manager::db::open_db(&app)?;

    let max_order: i32 = conn
        .query_row(
            "SELECT COALESCE(MAX(display_order), -1) FROM lorebook_entries WHERE lorebook_id = ?1",
            params![lorebook_id],
            |row| row.get(0),
        )
        .unwrap_or(-1);

    let now = now_millis()? as i64;
    let new_entry = LorebookEntry {
        id: Uuid::new_v4().to_string(),
        lorebook_id: lorebook_id.clone(),
        title: "".to_string(),
        enabled: true,
        always_active: false,
        keywords: vec![],
        case_sensitive: false,
        content: String::new(),
        priority: 0,
        display_order: max_order + 1,
        created_at: now,
        updated_at: now,
    };

    let created = upsert_lorebook_entry(&conn, &new_entry)?;
    serde_json::to_string(&created).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to serialize entry: {}", e),
        )
    })
}

#[tauri::command]
pub fn lorebook_entries_reorder(app: tauri::AppHandle, updates_json: String) -> Result<(), String> {
    let updates: Vec<(String, i32)> = serde_json::from_str(&updates_json).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Invalid updates JSON: {}", e),
        )
    })?;

    let conn = crate::storage_manager::db::open_db(&app)?;
    update_entry_display_order(&conn, updates)
}

#[tauri::command]
pub fn lorebook_export(app: tauri::AppHandle, lorebook_id: String) -> Result<String, String> {
    let conn = crate::storage_manager::db::open_db(&app)?;
    let lorebook = get_lorebook(&conn, &lorebook_id)?.ok_or_else(|| {
        crate::utils::err_msg(module_path!(), line!(), "Lorebook not found for export")
    })?;
    let entries = get_lorebook_entries(&conn, &lorebook_id)?;

    let mut entry_map = BTreeMap::new();
    for (index, entry) in entries.iter().enumerate() {
        let seq = (index + 1) as i64;
        entry_map.insert(
            seq.to_string(),
            WorldInfoExportEntry {
                uid: seq,
                key: entry.keywords.clone(),
                keysecondary: Vec::new(),
                comment: String::new(),
                content: entry.content.clone(),
                constant: entry.always_active,
                selective: false,
                selective_logic: 0,
                order: entry.priority,
                position: 1,
                disable: !entry.enabled,
                add_memo: true,
                exclude_recursion: true,
                probability: 100,
                display_index: index as i32 + 1,
                use_probability: true,
                secondary_keys: Vec::new(),
                keys: entry.keywords.clone(),
                id: seq,
                priority: entry.priority,
                insertion_order: entry.display_order,
                enabled: entry.enabled,
                name: entry.title.clone(),
                extensions: JsonValue::Object(JsonMap::new()),
                case_sensitive: entry.case_sensitive,
                depth: 4,
                character_filter: None,
            },
        );
    }

    let payload = WorldInfoExport {
        name: lorebook.name,
        description: String::new(),
        is_creation: false,
        scan_depth: 4,
        token_budget: 0,
        recursive_scanning: false,
        extensions: {
            let mut extensions = JsonMap::new();
            extensions.insert(
                "lettuceai".to_string(),
                serde_json::json!({
                    "keywordDetectionMode": match lorebook.keyword_detection_mode {
                        LorebookKeywordDetectionMode::RecentMessageWindow => "recentMessageWindow",
                        LorebookKeywordDetectionMode::LatestUserMessage => "latestUserMessage",
                    }
                }),
            );
            JsonValue::Object(extensions)
        },
        entries: entry_map,
    };

    serde_json::to_string_pretty(&payload).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to serialize lorebook export: {}", e),
        )
    })
}

#[tauri::command]
pub fn lorebook_export_as_usc(
    app: tauri::AppHandle,
    lorebook_id: String,
) -> Result<String, String> {
    let conn = crate::storage_manager::db::open_db(&app)?;
    let lorebook = get_lorebook(&conn, &lorebook_id)?.ok_or_else(|| {
        crate::utils::err_msg(module_path!(), line!(), "Lorebook not found for export")
    })?;
    let entries = get_lorebook_entries(&conn, &lorebook_id)?;
    let card = crate::storage_manager::system_cards::create_lorebook_usc(&lorebook, &entries);

    serde_json::to_string_pretty(&card).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to serialize USC lorebook export: {}", e),
        )
    })
}

#[tauri::command]
pub fn lorebook_import(app: tauri::AppHandle, import_json: String) -> Result<String, String> {
    let parsed: WorldInfoImport = serde_json::from_str(&import_json).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Invalid lorebook import JSON: {}", e),
        )
    })?;

    let mut parsed_entries = parse_world_info_entries(&parsed.entries);
    let now = now_millis()? as i64;
    let keyword_detection_mode = parsed
        .extensions
        .get("lettuceai")
        .and_then(|value| value.get("keywordDetectionMode"))
        .and_then(|value| value.as_str())
        .map(|value| match value {
            "latestUserMessage" => LorebookKeywordDetectionMode::LatestUserMessage,
            _ => LorebookKeywordDetectionMode::RecentMessageWindow,
        })
        .unwrap_or(LorebookKeywordDetectionMode::RecentMessageWindow);
    let lorebook = Lorebook {
        id: Uuid::new_v4().to_string(),
        name: parsed.name.trim().to_string(),
        avatar_path: None,
        keyword_detection_mode,
        created_at: now,
        updated_at: now,
    };

    if lorebook.name.is_empty() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "Lorebook name is required",
        ));
    }

    let conn = crate::storage_manager::db::open_db(&app)?;
    upsert_lorebook(&conn, &lorebook)?;
    for (index, mut entry) in parsed_entries.drain(..).enumerate() {
        entry.lorebook_id = lorebook.id.clone();
        entry.created_at = now;
        entry.updated_at = now;
        entry.display_order = index as i32;
        upsert_lorebook_entry(&conn, &entry)?;
    }

    serde_json::to_string(&lorebook).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to serialize imported lorebook: {}", e),
        )
    })
}
