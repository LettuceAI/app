use chrono::{DateTime, NaiveDateTime, SecondsFormat, TimeZone, Utc};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use tauri::State;
use uuid::Uuid;

use super::db::{now_ms, open_db, SwappablePool};
#[cfg(target_os = "android")]
use std::io::Read;
#[cfg(target_os = "android")]
use tauri_plugin_android_fs::{AndroidFs, AndroidFsExt};
#[cfg(target_os = "android")]
use tauri_plugin_fs::FilePath;
#[cfg(target_os = "android")]
use url::Url;

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct JsonlImportOptions {
    pub target_character_id: Option<String>,
    pub participant_character_map: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonlInspectParticipant {
    name: String,
    message_count: i64,
}

fn get_downloads_dir() -> Result<PathBuf, String> {
    #[cfg(target_os = "android")]
    {
        Ok(PathBuf::from("/storage/emulated/0/Download"))
    }

    #[cfg(not(target_os = "android"))]
    {
        dirs::download_dir().ok_or_else(|| "Could not find Downloads directory".to_string())
    }
}

fn sanitize_filename(input: &str) -> String {
    let s = input
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    let s = s.trim_matches('_').to_lowercase();
    if s.is_empty() {
        "chat".to_string()
    } else {
        s
    }
}

fn send_date(created_at_ms: i64) -> String {
    Utc.timestamp_millis_opt(created_at_ms)
        .single()
        .unwrap_or_else(Utc::now)
        .to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn parse_created_at(value: Option<&JsonValue>) -> Option<i64> {
    let raw = value?;
    if let Some(v) = raw.as_i64() {
        return Some(if v < 10_000_000_000 { v * 1000 } else { v });
    }
    if let Some(s) = raw.as_str() {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return None;
        }
        if let Ok(v) = trimmed.parse::<i64>() {
            return Some(if v < 10_000_000_000 { v * 1000 } else { v });
        }
        if let Ok(dt) = DateTime::parse_from_rfc3339(trimmed) {
            return Some(dt.timestamp_millis());
        }
        if let Ok(dt) = NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%dT%H:%M:%S%.f") {
            return Some(dt.and_utc().timestamp_millis());
        }
        if let Ok(dt) = NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M:%S%.f") {
            return Some(dt.and_utc().timestamp_millis());
        }
    }
    None
}

fn pick_message_content(message: &JsonValue) -> String {
    if let Some(content) = message.get("content").and_then(|v| v.as_str()) {
        return content.to_string();
    }
    if let Some(variants) = message.get("variants").and_then(|v| v.as_array()) {
        if let Some(selected_id) = message.get("selectedVariantId").and_then(|v| v.as_str()) {
            if let Some(selected) = variants
                .iter()
                .find(|variant| variant.get("id").and_then(|v| v.as_str()) == Some(selected_id))
            {
                if let Some(content) = selected.get("content").and_then(|v| v.as_str()) {
                    return content.to_string();
                }
            }
        }
        if let Some(first) = variants.first() {
            if let Some(content) = first.get("content").and_then(|v| v.as_str()) {
                return content.to_string();
            }
        }
    }
    String::new()
}

/// Converts Lettuce message variants to SillyTavern's swipe representation.
/// Returns `None` when the message has no usable alternatives.
fn message_swipes(message: &JsonValue) -> Option<(Vec<String>, usize)> {
    let variants = message.get("variants")?.as_array()?;
    let mut swipe_variants: Vec<(&str, Option<&str>)> = variants
        .iter()
        .filter_map(|variant| {
            variant
                .get("content")
                .and_then(JsonValue::as_str)
                .map(|content| {
                    (
                        content,
                        variant.get("id").and_then(JsonValue::as_str),
                    )
                })
        })
        .collect();
    let current = pick_message_content(message);
    if swipe_variants.is_empty() {
        return None;
    }

    let selected_id = message.get("selectedVariantId").and_then(JsonValue::as_str);
    let selected = selected_id
        .and_then(|id| swipe_variants.iter().position(|(_, variant_id)| *variant_id == Some(id)))
        .or_else(|| {
            swipe_variants
                .iter()
                .position(|(content, _)| *content == current)
        })
        .unwrap_or_else(|| {
            swipe_variants.insert(0, (&current, None));
            0
        });
    let swipes = swipe_variants
        .into_iter()
        .map(|(content, _)| content.to_owned())
        .collect();
    Some((swipes, selected))
}

fn sillytavern_message(
    name: &str,
    is_user: bool,
    is_system: bool,
    created_at: i64,
    content: String,
    swipes: Option<(Vec<String>, usize)>,
) -> JsonValue {
    let mut line = json!({
        "name": name,
        "is_user": is_user,
        "is_system": is_system,
        "send_date": send_date(created_at),
        "mes": content,
        "extra": {},
        "original_avatar": "",
    });
    if let Some((swipes, selected)) = swipes {
        line["swipe_id"] = json!(selected);
        line["swipes"] = json!(swipes);
    }
    line
}

// ------------------- Export: single chat -------------------

#[tauri::command]
pub fn jsonl_export_single_chat(
    app: tauri::AppHandle,
    session_id: String,
) -> Result<String, String> {
    let session_json = super::sessions::session_get(app.clone(), session_id)?
        .ok_or_else(|| crate::utils::err_msg(module_path!(), line!(), "Session not found"))?;
    let session: JsonValue = serde_json::from_str(&session_json)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let title = session
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("chat");
    let character_id = session.get("characterId").and_then(|v| v.as_str());
    let persona_id = session.get("personaId").and_then(|v| v.as_str());
    let messages = session
        .get("messages")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let conn = open_db(&app)?;
    let character_name = match character_id {
        Some(cid) => conn
            .query_row(
                "SELECT name FROM characters WHERE id = ?1",
                params![cid],
                |r| r.get::<_, String>(0),
            )
            .optional()
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
            .unwrap_or_else(|| "Character".to_string()),
        None => "Character".to_string(),
    };
    let user_name = match persona_id {
        Some(pid) => conn
            .query_row(
                "SELECT title FROM personas WHERE id = ?1",
                params![pid],
                |r| r.get::<_, String>(0),
            )
            .optional()
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
            .unwrap_or_else(|| "User".to_string()),
        None => "User".to_string(),
    };

    let first_created_at = messages
        .first()
        .and_then(|v| v.get("effectiveAt").or_else(|| v.get("createdAt")))
        .and_then(|v| v.as_i64())
        .unwrap_or_else(|| now_ms() as i64);

    let mut lines: Vec<String> = Vec::with_capacity(messages.len() + 1);
    let metadata = json!({
        "user_name": user_name,
        "character_name": character_name,
        "create_date": send_date(first_created_at),
        "chat_metadata": {},
    });
    lines.push(serde_json::to_string(&metadata).unwrap());

    for message in messages {
        let role = message
            .get("role")
            .and_then(|v| v.as_str())
            .unwrap_or("assistant");
        let created_at = message
            .get("effectiveAt")
            .or_else(|| message.get("createdAt"))
            .and_then(|v| v.as_i64())
            .unwrap_or_else(|| now_ms() as i64);
        let content = pick_message_content(&message);
        if content.trim().is_empty() {
            continue;
        }

        let (name, is_user, is_system) = match role {
            "user" => (user_name.as_str(), true, false),
            "system" => ("System", false, true),
            _ => (character_name.as_str(), false, false),
        };
        let swipes = message_swipes(&message);
        let line = sillytavern_message(name, is_user, is_system, created_at, content, swipes);
        lines.push(serde_json::to_string(&line).unwrap());
    }

    let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!("chat_{}_{}.jsonl", sanitize_filename(title), timestamp);
    let output_path = get_downloads_dir()?.join(filename);
    fs::write(&output_path, lines.join("\n"))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(output_path.to_string_lossy().to_string())
}

// ------------------- Export: group chat -------------------

#[tauri::command]
pub fn jsonl_export_group_chat(
    session_id: String,
    pool: State<'_, SwappablePool>,
) -> Result<String, String> {
    let conn = pool.get_connection()?;

    let (title, persona_id) = conn
        .query_row(
            "SELECT name, persona_id FROM group_sessions WHERE id = ?1",
            params![&session_id],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?)),
        )
        .optional()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
        .ok_or_else(|| crate::utils::err_msg(module_path!(), line!(), "Session not found"))?;

    let user_name = match persona_id.as_deref() {
        Some(pid) => conn
            .query_row(
                "SELECT title FROM personas WHERE id = ?1",
                params![pid],
                |r| r.get::<_, String>(0),
            )
            .optional()
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
            .unwrap_or_else(|| "User".to_string()),
        None => "User".to_string(),
    };

    // Preload character_id -> display name so each line carries the speaker's name.
    let mut char_name_stmt = conn
        .prepare("SELECT id, name FROM characters")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let char_name_rows = char_name_stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let mut char_names: HashMap<String, String> = HashMap::new();
    for row in char_name_rows {
        let (id, name) =
            row.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        char_names.insert(id, name);
    }

    let mut msg_stmt = conn
        .prepare(
            "SELECT id, role, content, speaker_character_id, created_at, selected_variant_id
             FROM group_messages WHERE session_id = ?1 ORDER BY created_at ASC",
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let msg_rows = msg_stmt
        .query_map(params![&session_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, Option<String>>(5)?,
            ))
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let mut messages: Vec<(String, String, String, Option<String>, i64, Option<String>)> =
        Vec::new();
    for row in msg_rows {
        messages.push(row.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?);
    }

    let first_created_at = messages
        .first()
        .map(|m| m.4)
        .unwrap_or_else(|| now_ms() as i64);

    let mut lines: Vec<String> = Vec::with_capacity(messages.len() + 1);
    let metadata = json!({
        "user_name": user_name,
        "character_name": title,
        "create_date": send_date(first_created_at),
        "chat_metadata": { "group": true },
    });
    lines.push(serde_json::to_string(&metadata).unwrap());

    for (message_id, role, content_db, speaker_character_id, created_at, selected_variant_id) in
        messages
    {
        let mut variant_stmt = conn
            .prepare(
                "SELECT id, content FROM group_message_variants
                 WHERE message_id = ?1 ORDER BY created_at ASC",
            )
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let variant_rows = variant_stmt
            .query_map(params![&message_id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let variants: Vec<(String, String)> = variant_rows
            .collect::<Result<_, _>>()
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let selected = selected_variant_id
            .as_deref()
            .and_then(|id| variants.iter().position(|(variant_id, _)| variant_id == id))
            .unwrap_or(0);
        let content = variants
            .get(selected)
            .map(|(_, content)| content.clone())
            .unwrap_or(content_db);
        let swipes = (!variants.is_empty()).then(|| {
            (
                variants.into_iter().map(|(_, content)| content).collect(),
                selected,
            )
        });

        if content.trim().is_empty() {
            continue;
        }

        let (name, is_user, is_system) = match role.as_str() {
            "user" => (user_name.clone(), true, false),
            "system" => ("System".to_string(), false, true),
            _ => {
                let speaker = speaker_character_id
                    .as_deref()
                    .and_then(|id| char_names.get(id).cloned())
                    .unwrap_or_else(|| "Character".to_string());
                (speaker, false, false)
            }
        };

        let line = sillytavern_message(&name, is_user, is_system, created_at, content, swipes);
        lines.push(serde_json::to_string(&line).unwrap());
    }

    let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!(
        "group_chat_{}_{}.jsonl",
        sanitize_filename(&title),
        timestamp
    );
    let output_path = get_downloads_dir()?.join(filename);
    fs::write(&output_path, lines.join("\n"))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(output_path.to_string_lossy().to_string())
}

// ------------------- Read / parse -------------------

#[derive(Debug)]
struct ParsedJsonl {
    metadata: Option<JsonValue>,
    messages: Vec<JsonValue>,
}

fn parse_jsonl(raw: &str) -> Result<ParsedJsonl, String> {
    let mut entries: Vec<JsonValue> = Vec::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parsed: JsonValue = serde_json::from_str(trimmed)
            .map_err(|_| crate::utils::err_msg(module_path!(), line!(), "JSONL_INVALID_LINE"))?;
        if !parsed.is_object() {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                "JSONL_INVALID_ENTRY",
            ));
        }
        entries.push(parsed);
    }
    if entries.is_empty() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "JSONL_EMPTY",
        ));
    }

    // SillyTavern chat headers are identified by chat_metadata. Keep the older
    // name markers as a compatibility fallback for third-party ST exporters.
    let first_is_metadata = {
        let first = &entries[0];
        first.get("mes").is_none()
            && (first.get("chat_metadata").is_some()
                || first.get("user_name").is_some()
                || first.get("character_name").is_some())
    };
    let metadata = first_is_metadata.then(|| entries.remove(0));
    Ok(ParsedJsonl {
        metadata,
        messages: entries,
    })
}

fn read_jsonl_file(_app: &tauri::AppHandle, path: &str) -> Result<String, String> {
    #[cfg(target_os = "android")]
    {
        if path.starts_with("content://") {
            let api = _app.android_fs();
            let url = Url::parse(path).map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Invalid URI '{}': {}", path, e),
                )
            })?;
            let file_path = FilePath::Url(url);
            let mut file = api.open_file(&file_path).map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to open Android file: {}", e),
                )
            })?;
            let mut raw = String::new();
            file.read_to_string(&mut raw)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            return Ok(raw);
        }
    }

    fs::read_to_string(path).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

fn read_jsonl(app: &tauri::AppHandle, path: &str) -> Result<ParsedJsonl, String> {
    let raw = read_jsonl_file(app, path)?;
    parse_jsonl(&raw)
}

fn imported_variants(entry: &JsonValue, created_at: i64) -> (Vec<JsonValue>, Option<String>) {
    let Some(swipes) = entry.get("swipes").and_then(JsonValue::as_array) else {
        return (Vec::new(), None);
    };
    let variants: Vec<JsonValue> = swipes
        .iter()
        .filter_map(JsonValue::as_str)
        .map(|content| {
            json!({
                "id": Uuid::new_v4().to_string(),
                "content": content,
                "createdAt": created_at,
            })
        })
        .collect();
    let selected = entry
        .get("swipe_id")
        .and_then(JsonValue::as_u64)
        .and_then(|index| variants.get(index as usize))
        .and_then(|variant| variant.get("id"))
        .and_then(JsonValue::as_str)
        .map(str::to_owned)
        .or_else(|| {
            variants
                .first()
                .and_then(|variant| variant.get("id"))
                .and_then(JsonValue::as_str)
                .map(str::to_owned)
        });
    (variants, selected)
}

fn message_role(entry: &JsonValue) -> &'static str {
    if entry
        .get("is_system")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        "system"
    } else if entry
        .get("is_user")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        "user"
    } else {
        "assistant"
    }
}

fn message_text(entry: &JsonValue) -> Option<String> {
    for key in ["mes", "content", "text", "message"] {
        if let Some(value) = entry.get(key).and_then(|v| v.as_str()) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

fn message_created_at(entry: &JsonValue) -> i64 {
    for key in ["send_date", "createdAt", "timestamp", "time"] {
        if let Some(v) = parse_created_at(entry.get(key)) {
            return v;
        }
    }
    now_ms() as i64
}

// ------------------- Inspect -------------------

#[tauri::command]
pub fn jsonl_inspect(app: tauri::AppHandle, path: String) -> Result<String, String> {
    let parsed = read_jsonl(&app, &path)?;

    let title = parsed
        .metadata
        .as_ref()
        .and_then(|m| m.get("character_name"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            std::path::Path::new(&path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Imported Chat")
                .to_string()
        });

    // Group inspect: count messages per non-user, non-system `name`.
    let mut counts: BTreeMap<String, i64> = BTreeMap::new();
    for entry in &parsed.messages {
        let role = message_role(entry);
        if role != "assistant" {
            continue;
        }
        let name = entry
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Character")
            .to_string();
        *counts.entry(name).or_insert(0) += 1;
    }

    let participants: Vec<JsonlInspectParticipant> = counts
        .into_iter()
        .map(|(name, message_count)| JsonlInspectParticipant {
            name,
            message_count,
        })
        .collect();

    let is_group = participants.len() > 1;
    let out = json!({
        "type": if is_group { "group_chat" } else { "single_chat" },
        "title": title,
        "messageCount": parsed.messages.len(),
        "participants": participants,
        "requiresCharacterSelection": !is_group,
        "requiresParticipantMapping": is_group,
    });
    serde_json::to_string(&out).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

// ------------------- Import -------------------

#[tauri::command]
pub fn jsonl_import(
    app: tauri::AppHandle,
    path: String,
    options_json: Option<String>,
    pool: State<'_, SwappablePool>,
) -> Result<String, String> {
    let options: JsonlImportOptions = match options_json {
        Some(raw) => serde_json::from_str(&raw)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
        None => JsonlImportOptions::default(),
    };

    let parsed = read_jsonl(&app, &path)?;

    // Determine single vs group from distinct assistant speakers.
    let mut speakers: HashSet<String> = HashSet::new();
    for entry in &parsed.messages {
        if message_role(entry) == "assistant" {
            if let Some(name) = entry.get("name").and_then(|v| v.as_str()) {
                speakers.insert(name.to_string());
            }
        }
    }
    let is_group = speakers.len() > 1;

    if is_group {
        import_group(&parsed, &options, &pool, &speakers)
    } else {
        import_single(&app, &parsed, &options, &path)
    }
}

fn import_single(
    app: &tauri::AppHandle,
    parsed: &ParsedJsonl,
    options: &JsonlImportOptions,
    source_path: &str,
) -> Result<String, String> {
    let target_character_id = options.target_character_id.clone().ok_or_else(|| {
        crate::utils::err_msg(module_path!(), line!(), "TARGET_CHARACTER_REQUIRED")
    })?;

    let new_session_id = Uuid::new_v4().to_string();
    let now = now_ms() as i64;

    let title = parsed
        .metadata
        .as_ref()
        .and_then(|m| m.get("character_name"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            std::path::Path::new(source_path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Imported Chat")
                .to_string()
        });

    let session = json!({
        "id": new_session_id,
        "characterId": target_character_id,
        "title": title,
        "personaId": JsonValue::Null,
        "archived": false,
        "createdAt": now,
        "updatedAt": now,
        "messages": [],
    });

    super::sessions::session_upsert_meta(
        app.clone(),
        serde_json::to_string(&session)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
    )?;

    let mut messages: Vec<JsonValue> = Vec::with_capacity(parsed.messages.len());
    for entry in &parsed.messages {
        let Some(content) = message_text(entry) else {
            continue;
        };
        let role = message_role(entry);
        let created_at = message_created_at(entry);
        let (variants, selected_variant_id) = imported_variants(entry, created_at);
        let mut message = json!({
            "id": Uuid::new_v4().to_string(),
            "role": role,
            "content": content,
            "createdAt": created_at,
            "attachments": [],
        });
        if matches!(role, "user" | "assistant") {
            message["effectiveAt"] = json!(created_at);
        }
        if !variants.is_empty() {
            message["variants"] = json!(variants);
            message["selectedVariantId"] = json!(selected_variant_id);
        }
        messages.push(message);
    }

    super::sessions::messages_upsert_batch(
        app.clone(),
        new_session_id.clone(),
        serde_json::to_string(&messages)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
    )?;

    let out = json!({
        "type": "single_chat",
        "sessionId": new_session_id,
        "characterId": target_character_id,
    });
    serde_json::to_string(&out).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

fn import_group(
    parsed: &ParsedJsonl,
    options: &JsonlImportOptions,
    pool: &State<'_, SwappablePool>,
    speakers: &HashSet<String>,
) -> Result<String, String> {
    let map = options
        .participant_character_map
        .clone()
        .unwrap_or_default();

    let conn = pool.get_connection()?;
    let now = now_ms() as i64;

    let mut unresolved: Vec<String> = Vec::new();
    let mut speaker_to_char: HashMap<String, String> = HashMap::new();
    for speaker in speakers {
        let Some(char_id) = map.get(speaker) else {
            unresolved.push(speaker.clone());
            continue;
        };
        let exists: bool = conn
            .query_row(
                "SELECT 1 FROM characters WHERE id = ?1",
                params![char_id],
                |_| Ok(true),
            )
            .unwrap_or(false);
        if !exists {
            unresolved.push(speaker.clone());
            continue;
        }
        speaker_to_char.insert(speaker.clone(), char_id.clone());
    }

    if !unresolved.is_empty() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("UNRESOLVED_PARTICIPANTS:{}", unresolved.join(", ")),
        ));
    }

    let mut unique_character_ids: Vec<String> = Vec::new();
    let mut seen = HashSet::new();
    for cid in speaker_to_char.values() {
        if seen.insert(cid.clone()) {
            unique_character_ids.push(cid.clone());
        }
    }

    if unique_character_ids.is_empty() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "GROUP_CHAT_IMPORT_REQUIRES_CHARACTER_MAPPING",
        ));
    }

    let new_session_id = Uuid::new_v4().to_string();
    let title = parsed
        .metadata
        .as_ref()
        .and_then(|m| m.get("character_name"))
        .and_then(|v| v.as_str())
        .unwrap_or("Imported Group Chat");
    let group_id = Uuid::new_v4().to_string();
    let character_ids_json = serde_json::to_string(&unique_character_ids)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    conn.execute(
        "INSERT INTO group_characters (id, name, character_ids, muted_character_ids, persona_id, created_at, updated_at, archived, chat_type, lorebook_ids, disable_character_lorebooks, speaker_selection_method, memory_type) VALUES (?1, ?2, ?3, '[]', NULL, ?4, ?4, 0, 'conversation', '[]', 0, 'llm', 'manual')",
        params![group_id, title, character_ids_json, now],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let none_str: Option<String> = None;
    conn.execute(
        "INSERT INTO group_sessions (id, group_character_id, name, character_ids, muted_character_ids, persona_id, created_at, updated_at, archived,
         chat_type, starting_scene, background_image_path, memories, memory_embeddings, memory_summary,
         memory_summary_token_count, memory_tool_events, speaker_selection_method)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
        params![
            &new_session_id,
            &group_id,
            title,
            character_ids_json,
            "[]",
            &none_str,
            now,
            now,
            0_i64,
            "conversation",
            &none_str,
            &none_str,
            "[]",
            "[]",
            "",
            0_i64,
            "[]",
            "llm",
        ],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    for cid in &unique_character_ids {
        conn.execute(
            "INSERT INTO group_participation (id, session_id, character_id, speak_count, last_spoke_turn, last_spoke_at)
             VALUES (?1, ?2, ?3, 0, NULL, NULL)",
            params![Uuid::new_v4().to_string(), &new_session_id, cid],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    let mut turn_number: i64 = 0;
    for entry in &parsed.messages {
        let Some(content) = message_text(entry) else {
            continue;
        };
        let role = message_role(entry);
        let created_at = message_created_at(entry);
        let speaker_char_id = if role == "assistant" {
            entry
                .get("name")
                .and_then(|v| v.as_str())
                .and_then(|name| speaker_to_char.get(name).cloned())
        } else {
            None
        };
        let message_id = Uuid::new_v4().to_string();
        let (variants, selected_variant_id) = imported_variants(entry, created_at);

        turn_number += 1;
        conn.execute(
            "INSERT INTO group_messages (id, session_id, role, content, speaker_character_id, turn_number,
             created_at, prompt_tokens, completion_tokens, total_tokens, selected_variant_id, is_pinned,
             attachments, used_lorebook_entries, reasoning, selection_reasoning, model_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, NULL, NULL, ?8, 0, '[]', '[]', NULL, NULL, NULL)",
            params![
                &message_id,
                &new_session_id,
                role,
                content,
                &speaker_char_id,
                turn_number,
                created_at,
                &selected_variant_id,
            ],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        for variant in variants {
            conn.execute(
                "INSERT INTO group_message_variants
                 (id, message_id, content, speaker_character_id, created_at,
                  prompt_tokens, completion_tokens, total_tokens, reasoning, selection_reasoning,
                  first_token_ms, tokens_per_second, mtp_stats)
                 VALUES (?1, ?2, ?3, ?4, ?5, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL)",
                params![
                    variant.get("id").and_then(JsonValue::as_str),
                    &message_id,
                    variant.get("content").and_then(JsonValue::as_str),
                    &speaker_char_id,
                    created_at,
                ],
            )
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        }
    }

    let out = json!({
        "type": "group_chat",
        "sessionId": new_session_id,
    });
    serde_json::to_string(&out).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_matches_sillytavern_iso_utc_format() {
        assert_eq!(send_date(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(parse_created_at(Some(&json!(send_date(0)))), Some(0));
    }

    #[test]
    fn parses_sillytavern_header_and_messages() {
        let raw = concat!(
            "{\"user_name\":\"unused\",\"character_name\":\"unused\",\"chat_metadata\":{}}\n",
            "{\"name\":\"User\",\"is_user\":true,\"is_system\":false,\"send_date\":\"2026-01-02T03:04:05.000Z\",\"mes\":\"Hi\",\"extra\":{}}\n"
        );
        let parsed = parse_jsonl(raw).unwrap();
        assert!(parsed.metadata.is_some());
        assert_eq!(parsed.messages.len(), 1);
        assert_eq!(message_role(&parsed.messages[0]), "user");
        assert_eq!(message_text(&parsed.messages[0]).as_deref(), Some("Hi"));
    }

    #[test]
    fn round_trips_variants_as_sillytavern_swipes() {
        let message = json!({
            "selectedVariantId": "b",
            "variants": [
                {"id": "a", "content": "First"},
                {"id": "b", "content": "Second"}
            ]
        });
        let swipes = message_swipes(&message).unwrap();
        assert_eq!(swipes, (vec!["First".to_string(), "Second".to_string()], 1));

        let entry =
            sillytavern_message("Character", false, false, 0, "Second".into(), Some(swipes));
        let (variants, selected) = imported_variants(&entry, 0);
        assert_eq!(variants.len(), 2);
        assert_eq!(selected, variants[1]["id"].as_str().map(str::to_owned));
    }

    #[test]
    fn rejects_non_object_jsonl_entries() {
        assert!(parse_jsonl("[]\n")
            .unwrap_err()
            .contains("JSONL_INVALID_ENTRY"));
    }
}
