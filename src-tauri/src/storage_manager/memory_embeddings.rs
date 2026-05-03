//! Storage layer for `MemoryEmbedding` records, replacing the per-session JSON
//! blob (previously stored in the `memory_embeddings` column on
//! `sessions` / `group_sessions`) with a normalised table keyed by
//! `(session_id, session_kind, memory_id)`.
//!
//! The embedding vector is stored as a raw little-endian f32 BLOB to avoid the
//! cost of JSON serialisation/parsing on the per-turn save path. Backfill from
//! the legacy JSON column is performed lazily on session load (see
//! `sessions.rs` and `group_sessions.rs`).

use rusqlite::{params, Connection};
use std::convert::TryInto;
use tauri::AppHandle;

use crate::chat_manager::types::{MemoryEmbedding, MemoryEntityAnchor};
use crate::storage_manager::db::open_db;

/// Distinguishes between rows owned by single-character `sessions` and
/// multi-character `group_sessions`. Persisted as a TEXT column.
#[derive(Clone, Copy, Debug)]
pub enum SessionKind {
    Session,
    GroupSession,
}

impl SessionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SessionKind::Session => "session",
            SessionKind::GroupSession => "group_session",
        }
    }
}

/// Narrow update used by the hot retrieval path (`mark_memories_accessed`).
/// Avoids touching the BLOB column on every turn.
#[derive(Clone, Debug)]
pub struct AccessUpdate {
    pub memory_id: String,
    pub importance_score: f32,
    pub last_accessed_at: u64,
    pub access_count: u32,
}

/// Encode an f32 slice as raw little-endian bytes suitable for a SQLite BLOB.
fn embedding_to_blob(embedding: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(embedding.len() * 4);
    for v in embedding {
        bytes.extend_from_slice(&v.to_le_bytes());
    }
    bytes
}

/// Decode raw little-endian f32 bytes back into a `Vec<f32>`. Trailing bytes
/// (which should never occur for well-formed rows) are ignored.
fn blob_to_embedding(bytes: &[u8]) -> Vec<f32> {
    let mut out = Vec::with_capacity(bytes.len() / 4);
    let mut i = 0;
    while i + 4 <= bytes.len() {
        let arr: [u8; 4] = match bytes[i..i + 4].try_into() {
            Ok(a) => a,
            Err(_) => break,
        };
        out.push(f32::from_le_bytes(arr));
        i += 4;
    }
    out
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn json_or_null<T: serde::Serialize>(value: &T) -> Option<String> {
    serde_json::to_string(value).ok()
}

fn parse_canonical_entities(raw: Option<String>) -> Vec<MemoryEntityAnchor> {
    raw.as_deref()
        .and_then(|s| serde_json::from_str::<Vec<MemoryEntityAnchor>>(s).ok())
        .unwrap_or_default()
}

fn parse_supersedes(raw: Option<String>) -> Vec<String> {
    raw.as_deref()
        .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
        .unwrap_or_default()
}

pub fn parse_legacy_json(raw: &str) -> Vec<MemoryEmbedding> {
    serde_json::from_str(raw).unwrap_or_default()
}

pub fn canonical_json_for_session(
    conn: &Connection,
    session_id: &str,
    kind: SessionKind,
    legacy_json: Option<&str>,
) -> Result<String, String> {
    let normalized = load_for_session(conn, session_id, kind)?;
    if !normalized.is_empty() {
        return serde_json::to_string(&normalized)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e));
    }

    Ok(legacy_json.unwrap_or("[]").to_string())
}

pub fn replace_all_from_json(
    conn: &mut Connection,
    session_id: &str,
    kind: SessionKind,
    raw: Option<&str>,
) -> Result<(), String> {
    let parsed = raw.map(parse_legacy_json).unwrap_or_default();
    if parsed.is_empty() {
        delete_all_for_session(conn, session_id, kind)?;
    } else {
        replace_all(conn, session_id, kind, &parsed)?;
    }
    Ok(())
}

/// Read all memory embeddings for the given session. Ordered by `created_at`
/// ascending so the in-memory `Vec<MemoryEmbedding>` keeps its historical
/// insertion order.
pub fn load_for_session(
    conn: &Connection,
    session_id: &str,
    kind: SessionKind,
) -> Result<Vec<MemoryEmbedding>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT memory_id, embedding, embedding_dim, embedding_model, text, token_count, \
             category, importance_score, persistence_importance, prompt_importance, volatility, \
             is_cold, is_pinned, access_count, fact_signature, fact_polarity, source_role, \
             source_message_id, superseded_by, superseded_at, supersedes_json, \
             canonical_entities_json, created_at, last_accessed_at \
             FROM memory_embeddings \
             WHERE session_id = ?1 AND session_kind = ?2 \
             ORDER BY created_at ASC",
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let rows = stmt
        .query_map(params![session_id, kind.as_str()], |r| {
            let memory_id: String = r.get(0)?;
            let embedding_blob: Vec<u8> = r.get(1)?;
            let _embedding_dim: i64 = r.get(2)?;
            let embedding_model: Option<String> = r.get(3)?;
            let text: String = r.get(4)?;
            let token_count: i64 = r.get(5)?;
            let category: Option<String> = r.get(6)?;
            let importance_score: f64 = r.get(7)?;
            let persistence_importance: f64 = r.get(8)?;
            let prompt_importance: f64 = r.get(9)?;
            let volatility: f64 = r.get(10)?;
            let is_cold: i64 = r.get(11)?;
            let is_pinned: i64 = r.get(12)?;
            let access_count: i64 = r.get(13)?;
            let fact_signature: Option<String> = r.get(14)?;
            let fact_polarity: Option<i64> = r.get(15)?;
            let source_role: Option<String> = r.get(16)?;
            let source_message_id: Option<String> = r.get(17)?;
            let superseded_by: Option<String> = r.get(18)?;
            let superseded_at: Option<i64> = r.get(19)?;
            let supersedes_json: Option<String> = r.get(20)?;
            let canonical_entities_json: Option<String> = r.get(21)?;
            let created_at: i64 = r.get(22)?;
            let last_accessed_at: i64 = r.get(23)?;

            let embedding = blob_to_embedding(&embedding_blob);
            let embedding_dimensions = if embedding.is_empty() {
                None
            } else {
                Some(embedding.len())
            };

            Ok(MemoryEmbedding {
                id: memory_id,
                text,
                embedding,
                created_at: created_at as u64,
                token_count: token_count as u32,
                is_cold: is_cold != 0,
                last_accessed_at: last_accessed_at as u64,
                importance_score: importance_score as f32,
                persistence_importance: persistence_importance as f32,
                prompt_importance: prompt_importance as f32,
                volatility: volatility as f32,
                is_pinned: is_pinned != 0,
                access_count: access_count as u32,
                embedding_source_version: embedding_model,
                embedding_dimensions,
                match_score: None,
                category,
                canonical_entities: parse_canonical_entities(canonical_entities_json),
                fact_signature,
                fact_polarity: fact_polarity.map(|v| v as i8),
                source_role,
                source_message_id,
                superseded_by,
                superseded_at: superseded_at.map(|v| v as u64),
                supersedes: parse_supersedes(supersedes_json),
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?);
    }
    Ok(out)
}

/// Returns true when the table holds zero rows for this session. Used by the
/// lazy backfill path to decide whether the legacy JSON column should be
/// imported.
pub fn is_empty_for_session(
    conn: &Connection,
    session_id: &str,
    kind: SessionKind,
) -> Result<bool, String> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM memory_embeddings WHERE session_id = ?1 AND session_kind = ?2",
            params![session_id, kind.as_str()],
            |r| r.get(0),
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(count == 0)
}

/// Replace every row for the session with the supplied vector. Runs in a single
/// transaction.
pub fn replace_all(
    conn: &mut Connection,
    session_id: &str,
    kind: SessionKind,
    memories: &[MemoryEmbedding],
) -> Result<(), String> {
    let tx = conn
        .transaction()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    tx.execute(
        "DELETE FROM memory_embeddings WHERE session_id = ?1 AND session_kind = ?2",
        params![session_id, kind.as_str()],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    {
        let mut stmt = tx
            .prepare(
                "INSERT INTO memory_embeddings (\
                    session_id, session_kind, memory_id, embedding, embedding_dim, \
                    embedding_model, text, token_count, category, importance_score, \
                    persistence_importance, prompt_importance, volatility, is_cold, is_pinned, \
                    access_count, fact_signature, fact_polarity, source_role, source_message_id, \
                    superseded_by, superseded_at, supersedes_json, canonical_entities_json, \
                    created_at, last_accessed_at, updated_at\
                 ) VALUES (\
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, \
                    ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27\
                 )",
            )
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        let now = now_ms() as i64;
        for m in memories {
            let blob = embedding_to_blob(&m.embedding);
            let dim = m.embedding.len() as i64;
            let supersedes_json = if m.supersedes.is_empty() {
                None
            } else {
                json_or_null(&m.supersedes)
            };
            let canonical_entities_json = if m.canonical_entities.is_empty() {
                None
            } else {
                json_or_null(&m.canonical_entities)
            };
            stmt.execute(params![
                session_id,
                kind.as_str(),
                &m.id,
                &blob,
                dim,
                &m.embedding_source_version,
                &m.text,
                m.token_count as i64,
                &m.category,
                m.importance_score as f64,
                m.persistence_importance as f64,
                m.prompt_importance as f64,
                m.volatility as f64,
                m.is_cold as i64,
                m.is_pinned as i64,
                m.access_count as i64,
                &m.fact_signature,
                m.fact_polarity.map(|v| v as i64),
                &m.source_role,
                &m.source_message_id,
                &m.superseded_by,
                m.superseded_at.map(|v| v as i64),
                &supersedes_json,
                &canonical_entities_json,
                m.created_at as i64,
                m.last_accessed_at as i64,
                now,
            ])
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        }
    }

    tx.commit()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

/// Hot-path narrow update: only `importance_score`, `last_accessed_at`, and
/// `access_count`. Single transaction.
pub fn apply_access_updates(
    conn: &mut Connection,
    session_id: &str,
    kind: SessionKind,
    updates: &[AccessUpdate],
) -> Result<(), String> {
    if updates.is_empty() {
        return Ok(());
    }
    let tx = conn
        .transaction()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    {
        let mut stmt = tx
            .prepare(
                "UPDATE memory_embeddings \
                 SET importance_score = ?1, last_accessed_at = ?2, access_count = ?3, \
                     updated_at = ?4 \
                 WHERE session_id = ?5 AND session_kind = ?6 AND memory_id = ?7",
            )
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let now = now_ms() as i64;
        for u in updates {
            stmt.execute(params![
                u.importance_score as f64,
                u.last_accessed_at as i64,
                u.access_count as i64,
                now,
                session_id,
                kind.as_str(),
                &u.memory_id,
            ])
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        }
    }
    tx.commit()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

/// Bulk-set `is_cold` for the given memory ids in a single statement.
pub fn set_cold_many(
    conn: &Connection,
    session_id: &str,
    kind: SessionKind,
    memory_ids: &[String],
    is_cold: bool,
) -> Result<(), String> {
    if memory_ids.is_empty() {
        return Ok(());
    }
    let placeholders = memory_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "UPDATE memory_embeddings SET is_cold = ?, updated_at = ? \
         WHERE session_id = ? AND session_kind = ? AND memory_id IN ({})",
        placeholders
    );
    let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::with_capacity(4 + memory_ids.len());
    params_vec.push(Box::new(is_cold as i64));
    params_vec.push(Box::new(now_ms() as i64));
    params_vec.push(Box::new(session_id.to_string()));
    params_vec.push(Box::new(kind.as_str().to_string()));
    for id in memory_ids {
        params_vec.push(Box::new(id.clone()));
    }
    let refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|b| b.as_ref()).collect();
    conn.execute(&sql, refs.as_slice())
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

/// Delete the listed memory ids. Single statement.
pub fn delete_many(
    conn: &Connection,
    session_id: &str,
    kind: SessionKind,
    memory_ids: &[String],
) -> Result<(), String> {
    if memory_ids.is_empty() {
        return Ok(());
    }
    let placeholders = memory_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "DELETE FROM memory_embeddings \
         WHERE session_id = ? AND session_kind = ? AND memory_id IN ({})",
        placeholders
    );
    let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::with_capacity(2 + memory_ids.len());
    params_vec.push(Box::new(session_id.to_string()));
    params_vec.push(Box::new(kind.as_str().to_string()));
    for id in memory_ids {
        params_vec.push(Box::new(id.clone()));
    }
    let refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|b| b.as_ref()).collect();
    conn.execute(&sql, refs.as_slice())
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

/// Delete every row for the session. Used when a session itself is deleted.
pub fn delete_all_for_session(
    conn: &Connection,
    session_id: &str,
    kind: SessionKind,
) -> Result<(), String> {
    conn.execute(
        "DELETE FROM memory_embeddings WHERE session_id = ?1 AND session_kind = ?2",
        params![session_id, kind.as_str()],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

pub fn count_for_session(
    conn: &Connection,
    session_id: &str,
    kind: SessionKind,
) -> Result<i64, String> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM memory_embeddings WHERE session_id = ?1 AND session_kind = ?2",
            params![session_id, kind.as_str()],
            |r| r.get(0),
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(count)
}

/// Idempotent backfill helper: if the new table has no rows for this session
/// and the supplied legacy vec is non-empty, insert the legacy rows. Returns
/// the count inserted (0 if backfill wasn't needed).
pub fn backfill_from_legacy_if_needed(
    conn: &mut Connection,
    session_id: &str,
    kind: SessionKind,
    legacy: &[MemoryEmbedding],
) -> Result<usize, String> {
    if legacy.is_empty() {
        return Ok(0);
    }
    if !is_empty_for_session(conn, session_id, kind)? {
        return Ok(0);
    }
    replace_all(conn, session_id, kind, legacy)?;
    Ok(legacy.len())
}

// AppHandle convenience wrappers ------------------------------------------------

pub fn load_for_session_app(
    app: &AppHandle,
    session_id: &str,
    kind: SessionKind,
) -> Result<Vec<MemoryEmbedding>, String> {
    let conn = open_db(app)?;
    load_for_session(&conn, session_id, kind)
}

pub fn replace_all_app(
    app: &AppHandle,
    session_id: &str,
    kind: SessionKind,
    memories: &[MemoryEmbedding],
) -> Result<(), String> {
    let mut conn = open_db(app)?;
    replace_all(&mut *conn, session_id, kind, memories)
}

pub fn apply_access_updates_app(
    app: &AppHandle,
    session_id: &str,
    kind: SessionKind,
    updates: &[AccessUpdate],
) -> Result<(), String> {
    let mut conn = open_db(app)?;
    apply_access_updates(&mut *conn, session_id, kind, updates)
}

pub fn set_cold_many_app(
    app: &AppHandle,
    session_id: &str,
    kind: SessionKind,
    memory_ids: &[String],
    is_cold: bool,
) -> Result<(), String> {
    let conn = open_db(app)?;
    set_cold_many(&conn, session_id, kind, memory_ids, is_cold)
}

pub fn delete_many_app(
    app: &AppHandle,
    session_id: &str,
    kind: SessionKind,
    memory_ids: &[String],
) -> Result<(), String> {
    let conn = open_db(app)?;
    delete_many(&conn, session_id, kind, memory_ids)
}

pub fn delete_all_for_session_app(
    app: &AppHandle,
    session_id: &str,
    kind: SessionKind,
) -> Result<(), String> {
    let conn = open_db(app)?;
    delete_all_for_session(&conn, session_id, kind)
}

#[allow(dead_code)]
pub fn count_for_session_app(
    app: &AppHandle,
    session_id: &str,
    kind: SessionKind,
) -> Result<i64, String> {
    let conn = open_db(app)?;
    count_for_session(&conn, session_id, kind)
}

pub fn backfill_from_legacy_if_needed_app(
    app: &AppHandle,
    session_id: &str,
    kind: SessionKind,
    legacy: &[MemoryEmbedding],
) -> Result<usize, String> {
    let mut conn = open_db(app)?;
    backfill_from_legacy_if_needed(&mut *conn, session_id, kind, legacy)
}
