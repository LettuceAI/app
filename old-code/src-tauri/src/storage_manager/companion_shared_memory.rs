use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value as JsonValue;
use std::collections::{HashMap, HashSet};
use tauri::AppHandle;

use super::db::open_db;
use crate::storage_manager::memory_embeddings::SessionKind;
use crate::chat_manager::companion::SoulGrowthEntry;

#[derive(Clone, Debug)]
pub struct EffectiveMemoryOwner {
    pub owner_id: String,
    pub kind: SessionKind,
    pub shared: bool,
}

#[derive(Clone, Debug)]
pub struct SharedMemoryState {
    pub memories_json: String,
    pub memory_summary: Option<String>,
    pub memory_summary_token_count: i64,
    pub memory_tool_events_json: String,
    pub memory_status: Option<String>,
    pub memory_error: Option<String>,
    pub memory_progress_step: Option<i64>,
    pub soul_growth_json: String,
    pub relationship_states_json: String,
}

impl Default for SharedMemoryState {
    fn default() -> Self {
        Self {
            memories_json: "[]".to_string(),
            memory_summary: None,
            memory_summary_token_count: 0,
            memory_tool_events_json: "[]".to_string(),
            memory_status: None,
            memory_error: None,
            memory_progress_step: None,
            soul_growth_json: "[]".to_string(),
            relationship_states_json: "{}".to_string(),
        }
    }
}

pub fn character_uses_companion_mode(
    conn: &Connection,
    character_id: &str,
    mode: &str,
) -> Result<bool, String> {
    let row: Option<(Option<String>, Option<String>)> = conn
        .query_row(
            "SELECT companion, mode FROM characters WHERE id = ?1",
            params![character_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let (_, character_mode) = match row {
        Some(value) => value,
        None => return Ok(false),
    };

    let is_companion = mode.eq_ignore_ascii_case("companion")
        || character_mode
            .as_deref()
            .map(|m| m.eq_ignore_ascii_case("companion"))
            .unwrap_or(false);
    Ok(is_companion)
}

fn companion_shared_memory_enabled_for_character(
    conn: &Connection,
    character_id: &str,
    mode: &str,
) -> Result<bool, String> {
    if !character_uses_companion_mode(conn, character_id, mode)? {
        return Ok(false);
    }

    let companion_json = conn
        .query_row(
            "SELECT companion FROM characters WHERE id = ?1",
            params![character_id],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .flatten();

    Ok(companion_json
        .as_deref()
        .map(
            crate::chat_manager::companion::shared_memory_across_sessions_enabled_from_companion_json,
        )
        .unwrap_or(false))
}

pub fn resolve_effective_memory_owner(
    conn: &Connection,
    session_id: &str,
    character_id: &str,
    mode: &str,
) -> Result<EffectiveMemoryOwner, String> {
    let shared = companion_shared_memory_enabled_for_character(conn, character_id, mode)?;
    Ok(if shared {
        EffectiveMemoryOwner {
            owner_id: character_id.to_string(),
            kind: SessionKind::CompanionShared,
            shared: true,
        }
    } else {
        EffectiveMemoryOwner {
            owner_id: session_id.to_string(),
            kind: SessionKind::Session,
            shared: false,
        }
    })
}

pub fn resolve_effective_memory_owner_for_session(
    conn: &Connection,
    session_id: &str,
) -> Result<EffectiveMemoryOwner, String> {
    let (character_id, mode): (String, String) = conn
        .query_row(
            "SELECT character_id, mode FROM sessions WHERE id = ?1",
            params![session_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    resolve_effective_memory_owner(conn, session_id, &character_id, &mode)
}

pub fn resolve_effective_memory_owner_for_session_app(
    app: &AppHandle,
    session_id: &str,
) -> Result<EffectiveMemoryOwner, String> {
    let conn = open_db(app)?;
    resolve_effective_memory_owner_for_session(&conn, session_id)
}

pub fn load_state(conn: &Connection, character_id: &str) -> Result<SharedMemoryState, String> {
    conn.query_row(
        "SELECT memories, memory_summary, memory_summary_token_count, memory_tool_events, memory_status, memory_error, memory_progress_step, soul_growth, relationship_states
         FROM companion_shared_memory_state WHERE character_id = ?1",
        params![character_id],
        |row| {
            Ok(SharedMemoryState {
                memories_json: row.get::<_, String>(0)?,
                memory_summary: row.get::<_, Option<String>>(1)?,
                memory_summary_token_count: row.get::<_, i64>(2)?,
                memory_tool_events_json: row.get::<_, String>(3)?,
                memory_status: row.get::<_, Option<String>>(4)?,
                memory_error: row.get::<_, Option<String>>(5)?,
                memory_progress_step: row.get::<_, Option<i64>>(6)?,
                soul_growth_json: row.get::<_, String>(7)?,
                relationship_states_json: row.get::<_, String>(8)?,
            })
        },
    )
    .optional()
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
    .map(Ok)
    .unwrap_or_else(|| Ok(SharedMemoryState::default()))
}

fn normalized_soul_facts_table_exists(conn: &Connection) -> bool {
    conn.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM sqlite_master
           WHERE type = 'table' AND name = 'companion_soul_facts'
         )",
        [],
        |row| row.get::<_, bool>(0),
    )
    .unwrap_or(false)
}

pub fn load_normalized_soul_facts(
    conn: &Connection,
    character_id: &str,
) -> Result<Option<Vec<SoulGrowthEntry>>, String> {
    if !normalized_soul_facts_table_exists(conn) {
        return Ok(None);
    }
    let mut statement = conn
        .prepare(
            "SELECT fact_id, category, value, kind, policy, slot, confidence,
                    evidence_count, weight, valid_from, valid_until, locked,
                    source_memory_ids, created_at, supersedes, superseded_by, superseded_at
             FROM companion_soul_facts
             WHERE character_id = ?1
             ORDER BY created_at ASC, fact_id ASC",
        )
        .map_err(|error| crate::utils::err_to_string(module_path!(), line!(), error))?;
    let facts = statement
        .query_map(params![character_id], |row| {
            let source_memory_ids = row.get::<_, String>(12)?;
            let supersedes = row.get::<_, String>(14)?;
            Ok(SoulGrowthEntry {
                id: row.get(0)?,
                category: row.get(1)?,
                value: row.get(2)?,
                kind: row.get(3)?,
                policy: row.get(4)?,
                slot: row.get(5)?,
                confidence: row.get(6)?,
                evidence_count: row.get::<_, i64>(7)?.max(0) as u32,
                weight: row.get(8)?,
                valid_from: row.get::<_, i64>(9)?.max(0) as u64,
                valid_until: row.get::<_, Option<i64>>(10)?.map(|value| value.max(0) as u64),
                locked: row.get::<_, i64>(11)? != 0,
                source_memory_ids: serde_json::from_str(&source_memory_ids).unwrap_or_default(),
                created_at: row.get::<_, i64>(13)?.max(0) as u64,
                supersedes: serde_json::from_str(&supersedes).unwrap_or_default(),
                superseded_by: row.get(15)?,
                superseded_at: row.get::<_, Option<i64>>(16)?.map(|value| value.max(0) as u64),
            })
        })
        .map_err(|error| crate::utils::err_to_string(module_path!(), line!(), error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| crate::utils::err_to_string(module_path!(), line!(), error))?;
    Ok((!facts.is_empty()).then_some(facts))
}

pub fn sync_normalized_soul_facts(
    conn: &Connection,
    character_id: &str,
    growth_json: &str,
) -> Result<String, String> {
    let now = super::db::now_ms();
    let mut facts = serde_json::from_str::<Vec<SoulGrowthEntry>>(growth_json).unwrap_or_default();
    for fact in &mut facts {
        fact.normalize_for_storage(now);
    }
    let normalized_json = serde_json::to_string(&facts).unwrap_or_else(|_| "[]".to_string());
    if !normalized_soul_facts_table_exists(conn) {
        return Ok(normalized_json);
    }

    let existing = load_normalized_soul_facts(conn, character_id)?.unwrap_or_default();
    let existing_by_id = existing
        .iter()
        .map(|fact| (fact.id.as_str(), fact))
        .collect::<HashMap<_, _>>();
    let desired_ids = facts
        .iter()
        .map(|fact| fact.id.as_str())
        .collect::<HashSet<_>>();

    for fact in &facts {
        if existing_by_id.get(fact.id.as_str()).is_some_and(|current| *current == fact) {
            continue;
        }
        conn.execute(
            "INSERT INTO companion_soul_facts (
               fact_id, character_id, category, value, kind, policy, slot, confidence,
               evidence_count, weight, valid_from, valid_until, locked, source_memory_ids,
               created_at, supersedes, superseded_by, superseded_at, updated_at
             ) VALUES (
               ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
               ?15, ?16, ?17, ?18, ?19
             )
             ON CONFLICT(character_id, fact_id) DO UPDATE SET
               category = excluded.category,
               value = excluded.value,
               kind = excluded.kind,
               policy = excluded.policy,
               slot = excluded.slot,
               confidence = excluded.confidence,
               evidence_count = excluded.evidence_count,
               weight = excluded.weight,
               valid_from = excluded.valid_from,
               valid_until = excluded.valid_until,
               locked = excluded.locked,
               source_memory_ids = excluded.source_memory_ids,
               created_at = excluded.created_at,
               supersedes = excluded.supersedes,
               superseded_by = excluded.superseded_by,
               superseded_at = excluded.superseded_at,
               updated_at = excluded.updated_at",
            params![
                &fact.id,
                character_id,
                &fact.category,
                &fact.value,
                &fact.kind,
                &fact.policy,
                &fact.slot,
                fact.confidence,
                fact.evidence_count as i64,
                fact.weight,
                fact.valid_from as i64,
                fact.valid_until.map(|value| value as i64),
                i64::from(fact.locked),
                serde_json::to_string(&fact.source_memory_ids).unwrap_or_else(|_| "[]".to_string()),
                fact.created_at as i64,
                serde_json::to_string(&fact.supersedes).unwrap_or_else(|_| "[]".to_string()),
                fact.superseded_by.as_deref(),
                fact.superseded_at.map(|value| value as i64),
                now as i64,
            ],
        )
        .map_err(|error| crate::utils::err_to_string(module_path!(), line!(), error))?;
    }

    for fact in existing {
        if desired_ids.contains(fact.id.as_str()) {
            continue;
        }
        conn.execute(
            "DELETE FROM companion_soul_facts WHERE character_id = ?1 AND fact_id = ?2",
            params![character_id, fact.id],
        )
        .map_err(|error| crate::utils::err_to_string(module_path!(), line!(), error))?;
    }

    Ok(normalized_json)
}

pub fn upsert_state(
    conn: &Connection,
    character_id: &str,
    state: &SharedMemoryState,
) -> Result<(), String> {
    let now = super::db::now_ms() as i64;
    conn.execute(
        r#"
        INSERT INTO companion_shared_memory_state (
            character_id, memories, memory_summary, memory_summary_token_count,
            memory_tool_events, memory_status, memory_error, memory_progress_step,
            soul_growth, relationship_states, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)
        ON CONFLICT(character_id) DO UPDATE SET
            memories = excluded.memories,
            memory_summary = excluded.memory_summary,
            memory_summary_token_count = excluded.memory_summary_token_count,
            memory_tool_events = excluded.memory_tool_events,
            memory_status = excluded.memory_status,
            memory_error = excluded.memory_error,
            memory_progress_step = excluded.memory_progress_step,
            soul_growth = excluded.soul_growth,
            relationship_states = excluded.relationship_states,
            updated_at = excluded.updated_at
        "#,
        params![
            character_id,
            &state.memories_json,
            state.memory_summary.as_deref(),
            state.memory_summary_token_count,
            &state.memory_tool_events_json,
            state.memory_status.as_deref(),
            state.memory_error.as_deref(),
            state.memory_progress_step,
            &state.soul_growth_json,
            &state.relationship_states_json,
            now,
        ],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

pub fn export_all(app: &AppHandle) -> Result<Vec<JsonValue>, String> {
    let conn = open_db(app)?;
    let mut stmt = conn
        .prepare(
            r#"
            SELECT character_id, memories, memory_summary, memory_summary_token_count,
                   memory_tool_events, memory_status, memory_error, memory_progress_step,
                   soul_growth, relationship_states, created_at, updated_at
            FROM companion_shared_memory_state
            ORDER BY character_id ASC
            "#,
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(serde_json::json!({
                "character_id": row.get::<_, String>(0)?,
                "memories": row.get::<_, String>(1)?,
                "memory_summary": row.get::<_, Option<String>>(2)?,
                "memory_summary_token_count": row.get::<_, i64>(3)?,
                "memory_tool_events": row.get::<_, String>(4)?,
                "memory_status": row.get::<_, Option<String>>(5)?,
                "memory_error": row.get::<_, Option<String>>(6)?,
                "memory_progress_step": row.get::<_, Option<i64>>(7)?,
                "soul_growth": row.get::<_, String>(8)?,
                "relationship_states": row.get::<_, String>(9)?,
                "created_at": row.get::<_, i64>(10)?,
                "updated_at": row.get::<_, i64>(11)?,
            }))
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let mut exported = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    drop(stmt);
    if companion_episodes_table_exists(&conn) {
        for item in &mut exported {
            let Some(character_id) = item.get("character_id").and_then(JsonValue::as_str) else {
                continue;
            };
            let mut episode_statement = conn
                .prepare(
                    "SELECT session_id, persona_key, episode_index, previous_session_id,
                            started_at, ended_at, updated_at
                     FROM companion_episodes
                     WHERE character_id = ?1
                     ORDER BY persona_key ASC, episode_index ASC",
                )
                .map_err(|error| crate::utils::err_to_string(module_path!(), line!(), error))?;
            let episodes = episode_statement
                .query_map(params![character_id], |row| {
                    Ok(serde_json::json!({
                        "session_id": row.get::<_, String>(0)?,
                        "persona_key": row.get::<_, String>(1)?,
                        "episode_index": row.get::<_, i64>(2)?,
                        "previous_session_id": row.get::<_, Option<String>>(3)?,
                        "started_at": row.get::<_, i64>(4)?,
                        "ended_at": row.get::<_, Option<i64>>(5)?,
                        "updated_at": row.get::<_, i64>(6)?,
                    }))
                })
                .map_err(|error| crate::utils::err_to_string(module_path!(), line!(), error))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| crate::utils::err_to_string(module_path!(), line!(), error))?;
            if let Some(object) = item.as_object_mut() {
                object.insert("episodes".to_string(), JsonValue::Array(episodes));
            }
        }
    }
    Ok(exported)
}

fn relationship_key(persona_id: Option<&str>) -> &str {
    persona_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("__default__")
}

fn companion_episodes_table_exists(conn: &Connection) -> bool {
    conn.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM sqlite_master
           WHERE type = 'table' AND name = 'companion_episodes'
         )",
        [],
        |row| row.get::<_, bool>(0),
    )
    .unwrap_or(false)
}

fn ensure_episode(
    conn: &Connection,
    session_id: &str,
    character_id: &str,
    persona_id: Option<&str>,
) -> Result<(), String> {
    if session_id.trim().is_empty() || !companion_episodes_table_exists(conn) {
        return Ok(());
    }
    let exists = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM companion_episodes WHERE session_id = ?1)",
            params![session_id],
            |row| row.get::<_, bool>(0),
        )
        .map_err(|error| crate::utils::err_to_string(module_path!(), line!(), error))?;
    if exists {
        return Ok(());
    }

    let persona_key = relationship_key(persona_id);
    let previous = conn
        .query_row(
            "SELECT session_id, episode_index
             FROM companion_episodes
             WHERE character_id = ?1 AND persona_key = ?2
             ORDER BY episode_index DESC, started_at DESC
             LIMIT 1",
            params![character_id, persona_key],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()
        .map_err(|error| crate::utils::err_to_string(module_path!(), line!(), error))?;
    let now = super::db::now_ms() as i64;
    if let Some((previous_session_id, _)) = &previous {
        conn.execute(
            "UPDATE companion_episodes
             SET ended_at = COALESCE(ended_at, ?1), updated_at = ?1
             WHERE session_id = ?2",
            params![now, previous_session_id],
        )
        .map_err(|error| crate::utils::err_to_string(module_path!(), line!(), error))?;
    }
    conn.execute(
        "INSERT INTO companion_episodes (
           session_id, character_id, persona_key, episode_index,
           previous_session_id, started_at, ended_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?6)",
        params![
            session_id,
            character_id,
            persona_key,
            previous.as_ref().map(|(_, index)| index + 1).unwrap_or(1),
            previous.as_ref().map(|(id, _)| id.as_str()),
            now,
        ],
    )
    .map_err(|error| crate::utils::err_to_string(module_path!(), line!(), error))?;
    Ok(())
}

fn load_episode_state(
    conn: &Connection,
    session_id: &str,
) -> Result<Option<JsonValue>, String> {
    if session_id.trim().is_empty() || !companion_episodes_table_exists(conn) {
        return Ok(None);
    }
    conn.query_row(
        "SELECT episode_index, previous_session_id, started_at
         FROM companion_episodes WHERE session_id = ?1",
        params![session_id],
        |row| {
            Ok(serde_json::json!({
                "episodeId": session_id,
                "episodeIndex": row.get::<_, i64>(0)?.max(0),
                "previousEpisodeId": row.get::<_, Option<String>>(1)?,
                "startedAt": row.get::<_, i64>(2)?.max(0),
            }))
        },
    )
    .optional()
    .map_err(|error| crate::utils::err_to_string(module_path!(), line!(), error))
}

pub fn merge_continuity_into_state(
    conn: &Connection,
    session_id: Option<&str>,
    character_id: &str,
    persona_id: Option<&str>,
    state: Option<JsonValue>,
    pull_relationship: bool,
) -> Result<Option<JsonValue>, String> {
    let Some(mut state) = state else {
        return Ok(None);
    };
    let Some(state_object) = state.as_object_mut() else {
        return Ok(Some(state));
    };

    let continuity = load_state(conn, character_id)?;
    let soul_growth = load_normalized_soul_facts(conn, character_id)?
        .and_then(|facts| serde_json::to_value(facts).ok())
        .unwrap_or_else(|| {
            serde_json::from_str::<JsonValue>(&continuity.soul_growth_json)
                .unwrap_or_else(|_| JsonValue::Array(Vec::new()))
        });
    if soul_growth.as_array().is_some_and(|items| !items.is_empty()) {
        let mut merged = state_object
            .get("soulGrowth")
            .and_then(JsonValue::as_array)
            .cloned()
            .unwrap_or_default();
        for fact in soul_growth.as_array().into_iter().flatten() {
            let fact_id = fact.get("id").and_then(JsonValue::as_str).unwrap_or("");
            let existing_index = (!fact_id.is_empty()).then(|| {
                merged.iter().position(|candidate| {
                    candidate.get("id").and_then(JsonValue::as_str) == Some(fact_id)
                })
            }).flatten();
            if let Some(index) = existing_index {
                merged[index] = fact.clone();
            } else {
                merged.push(fact.clone());
            }
        }
        state_object.insert("soulGrowth".to_string(), JsonValue::Array(merged));
    }

    // Only pull the shared relationship state in when the caller wants it (i.e.
    // on load, or when a fresh session should inherit the shared bond). On save
    // of a session that already carries its own — possibly freshly evolved —
    // relationship state, pulling here would overwrite it with the stale stored
    // value before it can be persisted, permanently freezing the bond.
    if pull_relationship {
        let relationship_states =
            serde_json::from_str::<JsonValue>(&continuity.relationship_states_json)
                .unwrap_or_else(|_| JsonValue::Object(Default::default()));
        if let Some(relationship) = relationship_states
            .get(relationship_key(persona_id))
            .filter(|value| value.is_object())
        {
            state_object.insert("relationshipState".to_string(), relationship.clone());
        }
    }
    if let Some(episode) = session_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| load_episode_state(conn, value))
        .transpose()?
        .flatten()
    {
        state_object.insert("continuity".to_string(), episode);
    }

    Ok(Some(state))
}

pub fn persist_continuity_from_state(
    conn: &Connection,
    session_id: Option<&str>,
    character_id: &str,
    persona_id: Option<&str>,
    state: &JsonValue,
) -> Result<(), String> {
    let Some(state_object) = state.as_object() else {
        return Ok(());
    };
    let mut continuity = load_state(conn, character_id)?;
    if let Some(session_id) = session_id {
        ensure_episode(conn, session_id, character_id, persona_id)?;
    }

    if let Some(soul_growth) = state_object.get("soulGrowth").filter(|value| value.is_array()) {
        let growth_json =
            serde_json::to_string(soul_growth).unwrap_or_else(|_| "[]".to_string());
        continuity.soul_growth_json =
            sync_normalized_soul_facts(conn, character_id, &growth_json)?;
    }

    if let Some(relationship) = state_object
        .get("relationshipState")
        .filter(|value| value.is_object())
    {
        let mut relationships =
            serde_json::from_str::<JsonValue>(&continuity.relationship_states_json)
                .ok()
                .and_then(|value| value.as_object().cloned())
                .unwrap_or_default();
        relationships.insert(
            relationship_key(persona_id).to_string(),
            relationship.clone(),
        );
        continuity.relationship_states_json =
            serde_json::to_string(&relationships).unwrap_or_else(|_| "{}".to_string());
    }

    upsert_state(conn, character_id, &continuity)
}

#[cfg(test)]
mod tests {
    use super::{merge_continuity_into_state, persist_continuity_from_state};
    use rusqlite::Connection;
    use serde_json::json;

    fn connection() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE companion_shared_memory_state (
               character_id TEXT PRIMARY KEY,
               memories TEXT NOT NULL DEFAULT '[]',
               memory_summary TEXT,
               memory_summary_token_count INTEGER NOT NULL DEFAULT 0,
               memory_tool_events TEXT NOT NULL DEFAULT '[]',
               memory_status TEXT,
               memory_error TEXT,
               memory_progress_step INTEGER,
               soul_growth TEXT NOT NULL DEFAULT '[]',
               relationship_states TEXT NOT NULL DEFAULT '{}',
               created_at INTEGER NOT NULL,
               updated_at INTEGER NOT NULL
             );
             CREATE TABLE companion_soul_facts (
               fact_id TEXT NOT NULL,
               character_id TEXT NOT NULL,
               category TEXT NOT NULL,
               value TEXT NOT NULL,
               kind TEXT NOT NULL DEFAULT 'add',
               policy TEXT NOT NULL DEFAULT 'adaptive',
               slot TEXT NOT NULL DEFAULT '',
               confidence REAL NOT NULL DEFAULT 1.0,
               evidence_count INTEGER NOT NULL DEFAULT 0,
               weight REAL NOT NULL DEFAULT 1.0,
               valid_from INTEGER NOT NULL DEFAULT 0,
               valid_until INTEGER,
               locked INTEGER NOT NULL DEFAULT 0,
               source_memory_ids TEXT NOT NULL DEFAULT '[]',
               created_at INTEGER NOT NULL,
               supersedes TEXT NOT NULL DEFAULT '[]',
               superseded_by TEXT,
               superseded_at INTEGER,
               updated_at INTEGER NOT NULL,
               PRIMARY KEY(character_id, fact_id)
             );
             CREATE TABLE companion_episodes (
               session_id TEXT PRIMARY KEY,
               character_id TEXT NOT NULL,
               persona_key TEXT NOT NULL DEFAULT '__default__',
               episode_index INTEGER NOT NULL,
               previous_session_id TEXT,
               started_at INTEGER NOT NULL,
               ended_at INTEGER,
               updated_at INTEGER NOT NULL
             );",
        )
        .unwrap();
        conn
    }

    #[test]
    fn continuity_carries_soul_and_persona_relationship_between_sessions() {
        let conn = connection();
        let first = json!({
            "soulGrowth": [{"id": "growth-1", "category": "goals", "value": "Protect the garden"}],
            "relationshipState": {"trust": 0.82, "interactionCount": 14},
            "emotionalState": {"felt": {"calm": 0.1}}
        });
        persist_continuity_from_state(&conn, None, "character-1", Some("persona-1"), &first)
            .unwrap();

        let fresh_session = json!({
            "soulGrowth": [],
            "relationshipState": {"trust": 0.2, "interactionCount": 0},
            "emotionalState": {"felt": {"calm": 0.9}}
        });
        let hydrated = merge_continuity_into_state(
            &conn,
            None,
            "character-1",
            Some("persona-1"),
            Some(fresh_session),
            true,
        )
        .unwrap()
        .unwrap();

        assert_eq!(hydrated["soulGrowth"][0]["id"], "growth-1");
        assert_eq!(hydrated["relationshipState"]["trust"], 0.82);
        assert_eq!(hydrated["relationshipState"]["interactionCount"], 14);
        assert_eq!(hydrated["emotionalState"]["felt"]["calm"], 0.9);
    }

    #[test]
    fn saving_keeps_the_sessions_evolved_relationship_over_stored_continuity() {
        let conn = connection();
        // Stored continuity holds an older/default bond.
        persist_continuity_from_state(
            &conn,
            None,
            "character-1",
            Some("persona-1"),
            &json!({"relationshipState": {"trust": 0.25, "interactionCount": 0}}),
        )
        .unwrap();

        // A session that evolved its own bond this turn.
        let evolved = json!({
            "relationshipState": {"trust": 0.6, "interactionCount": 42},
            "emotionalState": {"felt": {"calm": 0.5}}
        });

        // The save-side merge (pull_relationship = false) must NOT overwrite the
        // evolved bond with the stale stored value — that was the freeze bug.
        let merged = merge_continuity_into_state(
            &conn,
            None,
            "character-1",
            Some("persona-1"),
            Some(evolved),
            false,
        )
        .unwrap()
        .unwrap();
        assert_eq!(merged["relationshipState"]["trust"], 0.6);
        assert_eq!(merged["relationshipState"]["interactionCount"], 42);

        // Persisting it promotes the evolved bond to the shared store, so a fresh
        // load then sees it.
        persist_continuity_from_state(&conn, None, "character-1", Some("persona-1"), &merged)
            .unwrap();
        let reloaded = merge_continuity_into_state(
            &conn,
            None,
            "character-1",
            Some("persona-1"),
            Some(json!({"relationshipState": {"trust": 0.0, "interactionCount": 0}})),
            true,
        )
        .unwrap()
        .unwrap();
        assert_eq!(reloaded["relationshipState"]["trust"], 0.6);
        assert_eq!(reloaded["relationshipState"]["interactionCount"], 42);
    }

    #[test]
    fn relationships_remain_isolated_per_persona() {
        let conn = connection();
        persist_continuity_from_state(
            &conn,
            None,
            "character-1",
            Some("persona-1"),
            &json!({"relationshipState": {"trust": 0.9}}),
        )
        .unwrap();
        persist_continuity_from_state(
            &conn,
            None,
            "character-1",
            Some("persona-2"),
            &json!({"relationshipState": {"trust": -0.4}}),
        )
        .unwrap();

        let first = merge_continuity_into_state(
            &conn,
            None,
            "character-1",
            Some("persona-1"),
            Some(json!({"relationshipState": {"trust": 0.0}})),
            true,
        )
        .unwrap()
        .unwrap();
        let second = merge_continuity_into_state(
            &conn,
            None,
            "character-1",
            Some("persona-2"),
            Some(json!({"relationshipState": {"trust": 0.0}})),
            true,
        )
        .unwrap()
        .unwrap();

        assert_eq!(first["relationshipState"]["trust"], 0.9);
        assert_eq!(second["relationshipState"]["trust"], -0.4);
    }

    #[test]
    fn soul_facts_round_trip_through_normalized_rows() {
        let conn = connection();
        persist_continuity_from_state(
            &conn,
            None,
            "character-1",
            None,
            &json!({
                "soulGrowth": [{
                    "id": "fact-1",
                    "category": "fears",
                    "value": "Being forgotten",
                    "policy": "adaptive",
                    "slot": "abandonment",
                    "confidence": 0.86,
                    "evidenceCount": 3,
                    "weight": 0.72,
                    "validFrom": 100,
                    "locked": true,
                    "sourceMemoryIds": ["memory-1", "memory-2", "memory-3"]
                }]
            }),
        )
        .unwrap();

        let (policy, slot, locked, evidence_count): (String, String, i64, i64) = conn
            .query_row(
                "SELECT policy, slot, locked, evidence_count
                 FROM companion_soul_facts WHERE fact_id = 'fact-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(policy, "adaptive");
        assert_eq!(slot, "abandonment");
        assert_eq!(locked, 1);
        assert_eq!(evidence_count, 3);

        let hydrated = merge_continuity_into_state(
            &conn,
            None,
            "character-1",
            None,
            Some(json!({"soulGrowth": []})),
            true,
        )
        .unwrap()
        .unwrap();
        assert_eq!(hydrated["soulGrowth"][0]["id"], "fact-1");
        assert_eq!(hydrated["soulGrowth"][0]["locked"], true);
    }

    #[test]
    fn new_sessions_roll_the_continuous_relationship_into_an_episode() {
        let conn = connection();
        let state = json!({"relationshipState": {"trust": 0.7}});
        persist_continuity_from_state(
            &conn,
            Some("episode-1"),
            "character-1",
            Some("persona-1"),
            &state,
        )
        .unwrap();
        persist_continuity_from_state(
            &conn,
            Some("episode-2"),
            "character-1",
            Some("persona-1"),
            &state,
        )
        .unwrap();

        let first_ended: Option<i64> = conn
            .query_row(
                "SELECT ended_at FROM companion_episodes WHERE session_id = 'episode-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(first_ended.is_some());

        let hydrated = merge_continuity_into_state(
            &conn,
            Some("episode-2"),
            "character-1",
            Some("persona-1"),
            Some(json!({"relationshipState": {"trust": 0.0}})),
            true,
        )
        .unwrap()
        .unwrap();
        assert_eq!(hydrated["continuity"]["episodeIndex"], 2);
        assert_eq!(hydrated["continuity"]["previousEpisodeId"], "episode-1");
        assert_eq!(hydrated["relationshipState"]["trust"], 0.7);
    }
}
