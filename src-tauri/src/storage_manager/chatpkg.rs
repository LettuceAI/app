use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tauri::State;
use uuid::Uuid;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use super::db::{now_ms, open_db, SwappablePool};
use super::legacy::storage_root;

const CHATPKG_VERSION: i64 = 1;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatpkgImportOptions {
    pub target_character_id: Option<String>,
    pub participant_character_map: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatpkgInspectParticipant {
    id: Option<String>,
    character_id: Option<String>,
    character_display_name: Option<String>,
    resolved: bool,
}

struct ChatpkgSource {
    chat_json: JsonValue,
    attachments: HashMap<String, Vec<u8>>,
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

fn usage_from_tokens(
    prompt_tokens: Option<i64>,
    completion_tokens: Option<i64>,
    total_tokens: Option<i64>,
) -> Option<JsonValue> {
    if prompt_tokens.is_none() && completion_tokens.is_none() && total_tokens.is_none() {
        return None;
    }
    let mut usage = serde_json::Map::new();
    if let Some(v) = prompt_tokens {
        usage.insert("promptTokens".into(), JsonValue::from(v));
    }
    if let Some(v) = completion_tokens {
        usage.insert("completionTokens".into(), JsonValue::from(v));
    }
    if let Some(v) = total_tokens {
        usage.insert("totalTokens".into(), JsonValue::from(v));
    }
    Some(JsonValue::Object(usage))
}

fn collect_attachment_paths(messages: &[JsonValue]) -> Vec<(String, String)> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();

    for msg in messages {
        let Some(attachments) = msg.get("attachments").and_then(|v| v.as_array()) else {
            continue;
        };

        for att in attachments {
            let Some(storage_path) = att.get("storagePath").and_then(|v| v.as_str()) else {
                continue;
            };
            if storage_path.is_empty() {
                continue;
            }

            let normalized = storage_path.replace('\\', "/");
            let zip_rel = if let Some(rest) = normalized.strip_prefix("attachments/") {
                format!("attachments/{}", rest)
            } else {
                let fname = Path::new(&normalized)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("attachment.bin");
                format!("attachments/{}", fname)
            };

            if seen.insert((normalized.clone(), zip_rel.clone())) {
                out.push((normalized, zip_rel));
            }
        }
    }

    out
}

fn write_chatpkg(
    output_path: &Path,
    chat_json: &JsonValue,
    attachment_rel_paths: &[(String, String)],
    app: &tauri::AppHandle,
) -> Result<(), String> {
    let root = storage_root(app)?;
    let file = File::create(output_path)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    zip.start_file("chat.json", options)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let serialized = serde_json::to_string_pretty(chat_json)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    zip.write_all(serialized.as_bytes())
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    for (rel_src, zip_rel) in attachment_rel_paths {
        let src = if Path::new(rel_src).is_absolute() {
            PathBuf::from(rel_src)
        } else {
            root.join(rel_src)
        };

        if !src.exists() || !src.is_file() {
            continue;
        }

        let bytes =
            fs::read(&src).map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        zip.start_file(zip_rel, options)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        zip.write_all(&bytes)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    zip.finish()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    Ok(())
}

fn file_stem_or_default(path: &str, default_value: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(default_value)
        .to_string()
}

fn normalize_chat_role(raw: &str) -> Option<&'static str> {
    let role = raw.trim().to_lowercase();
    match role.as_str() {
        "user" | "human" => Some("user"),
        "assistant" | "ai" | "bot" | "model" | "character" | "char" => Some("assistant"),
        "system" => Some("system"),
        _ => None,
    }
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

fn extract_message_text(value: &JsonValue) -> Option<String> {
    if let Some(text) = value.as_str() {
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    let obj = value.as_object()?;

    for key in ["content", "text", "message", "value", "body", "mes"] {
        let Some(candidate) = obj.get(key) else {
            continue;
        };

        if let Some(text) = candidate.as_str() {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }

        if let Some(items) = candidate.as_array() {
            let joined = items
                .iter()
                .filter_map(|item| {
                    item.as_str().map(|s| s.to_string()).or_else(|| {
                        item.get("text")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                    })
                })
                .collect::<Vec<_>>()
                .join("\n");
            if !joined.trim().is_empty() {
                return Some(joined);
            }
        }
    }

    None
}

fn normalize_message_object(value: &JsonValue) -> Option<(JsonValue, bool)> {
    let is_sillytavern = value.get("mes").is_some()
        || value.get("is_user").is_some()
        || value.get("is_system").is_some();
    let role = if value
        .get("is_system")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        Some("system")
    } else {
        value
            .get("is_user")
            .and_then(|v| v.as_bool())
            .map(|is_user| if is_user { "user" } else { "assistant" })
            .or_else(|| {
                ["role", "sender", "author", "speaker", "from"]
                    .iter()
                    .find_map(|key| value.get(*key).and_then(|v| v.as_str()))
                    .and_then(normalize_chat_role)
            })
    }?;
    let content = extract_message_text(value)?;
    let created_at = ["createdAt", "timestamp", "time", "send_date"]
        .iter()
        .find_map(|key| parse_created_at(value.get(*key)))
        .unwrap_or_else(|| now_ms() as i64);

    Some((
        json!({
            "id": Uuid::new_v4().to_string(),
            "role": role,
            "content": content,
            "createdAt": created_at,
            "attachments": value.get("attachments").cloned().unwrap_or_else(|| JsonValue::Array(vec![])),
        }),
        is_sillytavern,
    ))
}

fn build_single_chat_envelope_from_messages(
    messages: Vec<JsonValue>,
    source_path: &str,
    source_format: &str,
) -> JsonValue {
    let now = now_ms() as i64;
    let source_app = if source_format == "sillytavern" {
        "sillytavern"
    } else {
        "external"
    };
    json!({
        "type": "single_chat",
        "version": CHATPKG_VERSION,
        "exportedAt": now,
        "source": {
            "app": source_app,
            "format": source_format,
        },
        "payload": {
            "session": {
                "id": Uuid::new_v4().to_string(),
                "title": file_stem_or_default(source_path, "Imported Chat"),
                "personaId": JsonValue::Null,
                "archived": false,
                "createdAt": now,
                "updatedAt": now,
                "messages": messages,
            }
        }
    })
}

fn build_external_single_chat_envelope(
    messages: Vec<JsonValue>,
    source_path: &str,
    is_sillytavern: bool,
) -> JsonValue {
    build_single_chat_envelope_from_messages(
        messages,
        source_path,
        if is_sillytavern {
            "sillytavern"
        } else {
            "jsonl"
        },
    )
}

fn parse_chat_json_lines(raw: &str, source_path: &str) -> Result<JsonValue, String> {
    if let Ok(parsed) = serde_json::from_str::<JsonValue>(raw) {
        if parsed.get("type").is_some() && parsed.get("payload").is_some() {
            return Ok(parsed);
        }

        if let Some(items) = parsed.as_array() {
            let mut is_sillytavern = false;
            let normalized = items
                .iter()
                .filter_map(|item| {
                    normalize_message_object(item).map(|(message, st)| {
                        if st {
                            is_sillytavern = true;
                        }
                        message
                    })
                })
                .collect::<Vec<_>>();
            if !normalized.is_empty() {
                return Ok(build_external_single_chat_envelope(
                    normalized,
                    source_path,
                    is_sillytavern,
                ));
            }
        }

        if let Some(messages) = parsed.get("messages").and_then(|v| v.as_array()) {
            let mut is_sillytavern = false;
            let normalized = messages
                .iter()
                .filter_map(|message| {
                    normalize_message_object(message).map(|(normalized_message, st)| {
                        if st {
                            is_sillytavern = true;
                        }
                        normalized_message
                    })
                })
                .collect::<Vec<_>>();
            if !normalized.is_empty() {
                return Ok(build_external_single_chat_envelope(
                    normalized,
                    source_path,
                    is_sillytavern,
                ));
            }
        }
    }

    let lines = raw
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    if lines.is_empty() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "CHATPKG_INVALID_JSON",
        ));
    }

    let mut parsed_lines = Vec::new();
    for line in lines {
        let value = serde_json::from_str::<JsonValue>(line)
            .map_err(|_| crate::utils::err_msg(module_path!(), line!(), "CHATPKG_INVALID_JSONL"))?;
        parsed_lines.push(value);
    }

    if parsed_lines.len() == 1 {
        let single = parsed_lines.remove(0);
        if single.get("type").is_some() && single.get("payload").is_some() {
            return Ok(single);
        }
        if let Some(messages) = single.get("messages").and_then(|v| v.as_array()) {
            let mut is_sillytavern = false;
            let normalized = messages
                .iter()
                .filter_map(|message| {
                    normalize_message_object(message).map(|(normalized_message, st)| {
                        if st {
                            is_sillytavern = true;
                        }
                        normalized_message
                    })
                })
                .collect::<Vec<_>>();
            if !normalized.is_empty() {
                return Ok(build_external_single_chat_envelope(
                    normalized,
                    source_path,
                    is_sillytavern,
                ));
            }
        }
        parsed_lines.push(single);
    }

    let mut messages = Vec::new();
    let mut saw_sillytavern = false;
    for entry in parsed_lines {
        if let Some(items) = entry.get("messages").and_then(|v| v.as_array()) {
            for item in items.iter() {
                if let Some((message, is_sillytavern)) = normalize_message_object(item) {
                    if is_sillytavern {
                        saw_sillytavern = true;
                    }
                    messages.push(message);
                }
            }
            continue;
        }

        if let Some((message, is_sillytavern)) = normalize_message_object(&entry) {
            if is_sillytavern {
                saw_sillytavern = true;
            }
            messages.push(message);
        }
    }

    if messages.is_empty() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "CHATPKG_INVALID_JSONL",
        ));
    }

    Ok(build_external_single_chat_envelope(
        messages,
        source_path,
        saw_sillytavern,
    ))
}

fn sillytavern_send_date(created_at_ms: i64) -> String {
    Utc.timestamp_millis_opt(created_at_ms)
        .single()
        .unwrap_or_else(Utc::now)
        .format("%Y-%m-%dT%H:%M:%S%.3f")
        .to_string()
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

#[tauri::command]
pub fn chatpkg_export_single_chat_sillytavern(
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
        .and_then(|v| v.get("createdAt"))
        .and_then(|v| v.as_i64())
        .unwrap_or_else(|| now_ms() as i64);

    let mut lines: Vec<String> = Vec::with_capacity(messages.len() + 1);
    let metadata = json!({
        "user_name": user_name,
        "character_name": character_name,
        "create_date": sillytavern_send_date(first_created_at),
        "chat_metadata": {},
    });
    lines.push(
        serde_json::to_string(&metadata)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
    );

    for message in messages {
        let role = message
            .get("role")
            .and_then(|v| v.as_str())
            .unwrap_or("assistant");
        let created_at = message
            .get("createdAt")
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
        let line = json!({
            "name": name,
            "is_user": is_user,
            "is_system": is_system,
            "send_date": sillytavern_send_date(created_at),
            "mes": content,
            "extra": {},
            "original_avatar": "",
        });
        lines.push(
            serde_json::to_string(&line)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
        );
    }

    let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!(
        "chat_{}_{}_sillytavern.jsonl",
        sanitize_filename(title),
        timestamp
    );
    let output_path = get_downloads_dir()?.join(filename);
    fs::write(&output_path, lines.join("\n"))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(output_path.to_string_lossy().to_string())
}

fn read_chatpkg_source(path: &str) -> Result<ChatpkgSource, String> {
    let file = File::open(path).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to open .chatpkg file: {}", e),
        )
    })?;

    if let Ok(mut archive) = ZipArchive::new(file) {
        let mut content = String::new();
        {
            let mut entry = archive.by_name("chat.json").map_err(|_| {
                crate::utils::err_msg(module_path!(), line!(), "CHATPKG_MISSING_CHAT_JSON")
            })?;
            entry
                .read_to_string(&mut content)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        }

        let chat_json = serde_json::from_str::<JsonValue>(&content)
            .map_err(|_| crate::utils::err_msg(module_path!(), line!(), "CHATPKG_INVALID_JSON"))?;

        let mut attachments = HashMap::new();
        for index in 0..archive.len() {
            let mut entry = archive
                .by_index(index)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            let name = entry.name().to_string();
            if !name.starts_with("attachments/") || name.ends_with('/') {
                continue;
            }

            let mut bytes = Vec::new();
            entry
                .read_to_end(&mut bytes)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            attachments.insert(name, bytes);
        }

        return Ok(ChatpkgSource {
            chat_json,
            attachments,
        });
    }

    let raw = fs::read_to_string(path)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let chat_json = parse_chat_json_lines(&raw, path)?;
    Ok(ChatpkgSource {
        chat_json,
        attachments: HashMap::new(),
    })
}

fn restore_attachment_paths(
    messages: &mut [JsonValue],
    attachments: &HashMap<String, Vec<u8>>,
    app: &tauri::AppHandle,
) -> Result<(), String> {
    if attachments.is_empty() {
        return Ok(());
    }

    let root = storage_root(app)?;
    for message in messages {
        let Some(items) = message
            .get_mut("attachments")
            .and_then(|v| v.as_array_mut())
        else {
            continue;
        };

        for attachment in items {
            let Some(obj) = attachment.as_object_mut() else {
                continue;
            };

            let current_path = obj
                .get("storagePath")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let normalized = current_path.replace('\\', "/");
            let zip_rel = if let Some(rest) = normalized.strip_prefix("attachments/") {
                format!("attachments/{}", rest)
            } else {
                let Some(filename) = Path::new(&normalized)
                    .file_name()
                    .and_then(|name| name.to_str())
                else {
                    continue;
                };
                format!("attachments/{}", filename)
            };

            let Some(bytes) = attachments.get(&zip_rel) else {
                continue;
            };

            let output_path = root.join(&zip_rel);
            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            }
            fs::write(&output_path, bytes)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            obj.insert("storagePath".to_string(), JsonValue::String(zip_rel));
        }
    }

    Ok(())
}

fn read_group_session_payload(
    conn: &rusqlite::Connection,
    session_id: &str,
) -> Result<Option<JsonValue>, String> {
    let row = conn
        .query_row(
            "SELECT id, name, character_ids, muted_character_ids, persona_id, created_at, updated_at, archived,
                    chat_type, starting_scene, background_image_path,
                    memories, memory_embeddings, memory_summary, memory_summary_token_count,
                    memory_tool_events, speaker_selection_method
             FROM group_sessions WHERE id = ?1",
            params![session_id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, i64>(5)?,
                    r.get::<_, i64>(6)?,
                    r.get::<_, i64>(7)?,
                    r.get::<_, String>(8)?,
                    r.get::<_, Option<String>>(9)?,
                    r.get::<_, Option<String>>(10)?,
                    r.get::<_, Option<String>>(11)?,
                    r.get::<_, Option<String>>(12)?,
                    r.get::<_, Option<String>>(13)?,
                    r.get::<_, Option<i64>>(14)?,
                    r.get::<_, Option<String>>(15)?,
                    r.get::<_, Option<String>>(16)?,
                ))
            },
        )
        .optional()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let Some((
        id,
        name,
        character_ids_json,
        muted_character_ids_json,
        persona_id,
        created_at,
        updated_at,
        archived,
        chat_type,
        starting_scene_json,
        background_image_path,
        memories_json,
        memory_embeddings_json,
        memory_summary,
        memory_summary_token_count,
        memory_tool_events_json,
        speaker_selection_method,
    )) = row
    else {
        return Ok(None);
    };

    let character_ids: JsonValue =
        serde_json::from_str(&character_ids_json).unwrap_or_else(|_| JsonValue::Array(vec![]));
    let muted_character_ids: JsonValue = serde_json::from_str(&muted_character_ids_json)
        .unwrap_or_else(|_| JsonValue::Array(vec![]));
    let memories: JsonValue = memories_json
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_else(|| JsonValue::Array(vec![]));
    let memory_embeddings: JsonValue = memory_embeddings_json
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_else(|| JsonValue::Array(vec![]));
    let starting_scene: JsonValue = starting_scene_json
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or(JsonValue::Null);
    let memory_tool_events: JsonValue = memory_tool_events_json
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_else(|| JsonValue::Array(vec![]));

    Ok(Some(json!({
        "id": id,
        "name": name,
        "characterIds": character_ids,
        "mutedCharacterIds": muted_character_ids,
        "personaId": persona_id,
        "createdAt": created_at,
        "updatedAt": updated_at,
        "archived": archived != 0,
        "chatType": chat_type,
        "startingScene": starting_scene,
        "backgroundImagePath": background_image_path,
        "memories": memories,
        "memoryEmbeddings": memory_embeddings,
        "memorySummary": memory_summary.unwrap_or_default(),
        "memorySummaryTokenCount": memory_summary_token_count.unwrap_or(0),
        "memoryToolEvents": memory_tool_events,
        "speakerSelectionMethod": speaker_selection_method.unwrap_or_else(|| "llm".to_string()),
    })))
}

fn read_group_messages_payload(
    conn: &rusqlite::Connection,
    session_id: &str,
) -> Result<Vec<JsonValue>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, role, content, speaker_character_id, turn_number, created_at,
                    prompt_tokens, completion_tokens, total_tokens, selected_variant_id,
                    is_pinned, attachments, used_lorebook_entries, reasoning, selection_reasoning, model_id
             FROM group_messages WHERE session_id = ?1 ORDER BY created_at ASC",
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let rows = stmt
        .query_map(params![session_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, i64>(5)?,
                r.get::<_, Option<i64>>(6)?,
                r.get::<_, Option<i64>>(7)?,
                r.get::<_, Option<i64>>(8)?,
                r.get::<_, Option<String>>(9)?,
                r.get::<_, i64>(10)?,
                r.get::<_, String>(11)?,
                r.get::<_, String>(12)?,
                r.get::<_, Option<String>>(13)?,
                r.get::<_, Option<String>>(14)?,
                r.get::<_, Option<String>>(15)?,
            ))
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let mut messages = Vec::new();
    for row in rows {
        let (
            message_id,
            role,
            content,
            speaker_character_id,
            turn_number,
            created_at,
            prompt_tokens,
            completion_tokens,
            total_tokens,
            selected_variant_id,
            is_pinned,
            attachments_json,
            used_lorebook_entries_json,
            reasoning,
            selection_reasoning,
            model_id,
        ) = row.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        let mut variants_stmt = conn
            .prepare(
                "SELECT id, content, speaker_character_id, created_at, prompt_tokens, completion_tokens,
                        total_tokens, reasoning, selection_reasoning, model_id
                 FROM group_message_variants WHERE message_id = ?1 ORDER BY created_at ASC",
            )
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        let variants_rows = variants_stmt
            .query_map(params![&message_id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, Option<i64>>(4)?,
                    r.get::<_, Option<i64>>(5)?,
                    r.get::<_, Option<i64>>(6)?,
                    r.get::<_, Option<String>>(7)?,
                    r.get::<_, Option<String>>(8)?,
                    r.get::<_, Option<String>>(9)?,
                ))
            })
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        let mut variants = Vec::new();
        for v in variants_rows {
            let (
                id,
                content,
                speaker_character_id,
                created_at,
                prompt_tokens,
                completion_tokens,
                total_tokens,
                reasoning,
                selection_reasoning,
                model_id,
            ) = v.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

            let mut variant_obj = json!({
                "id": id,
                "content": content,
                "speakerCharacterId": speaker_character_id,
                "createdAt": created_at,
                "reasoning": reasoning,
                "selectionReasoning": selection_reasoning,
                "modelId": model_id,
            });
            if let Some(usage) = usage_from_tokens(prompt_tokens, completion_tokens, total_tokens) {
                variant_obj["usage"] = usage;
            }
            variants.push(variant_obj);
        }

        let attachments = serde_json::from_str::<JsonValue>(&attachments_json)
            .unwrap_or_else(|_| JsonValue::Array(vec![]));

        let mut message_obj = json!({
            "id": message_id,
            "role": role,
            "content": content,
            "speakerCharacterId": speaker_character_id,
            "turnNumber": turn_number,
            "createdAt": created_at,
            "selectedVariantId": selected_variant_id,
            "isPinned": is_pinned != 0,
            "attachments": attachments,
            "usedLorebookEntries": serde_json::from_str::<Vec<String>>(&used_lorebook_entries_json)
                .unwrap_or_default(),
            "reasoning": reasoning,
            "selectionReasoning": selection_reasoning,
            "modelId": model_id,
            "variants": variants,
        });

        if let Some(usage) = usage_from_tokens(prompt_tokens, completion_tokens, total_tokens) {
            message_obj["usage"] = usage;
        }

        messages.push(message_obj);
    }

    Ok(messages)
}

fn read_group_participation_payload(
    conn: &rusqlite::Connection,
    session_id: &str,
) -> Result<Vec<JsonValue>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT gp.id, gp.character_id, gp.speak_count, gp.last_spoke_turn, gp.last_spoke_at,
                    c.name
             FROM group_participation gp
             LEFT JOIN characters c ON c.id = gp.character_id
             WHERE gp.session_id = ?1",
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let rows = stmt
        .query_map(params![session_id], |r| {
            Ok(json!({
                "id": r.get::<_, String>(0)?,
                "characterId": r.get::<_, String>(1)?,
                "characterDisplayName": r.get::<_, Option<String>>(5)?.unwrap_or_else(|| "Unknown".to_string()),
                "speakCount": r.get::<_, i64>(2)?,
                "lastSpokeTurn": r.get::<_, Option<i64>>(3)?,
                "lastSpokeAt": r.get::<_, Option<i64>>(4)?,
            }))
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

fn maybe_read_character_snapshots(
    conn: &rusqlite::Connection,
    character_ids: &[String],
) -> Result<Vec<JsonValue>, String> {
    let mut out = Vec::new();

    for character_id in character_ids {
        let row = conn
            .query_row(
                "SELECT id, name, memory_type, created_at, updated_at FROM characters WHERE id = ?1",
                params![character_id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, i64>(3)?,
                        r.get::<_, i64>(4)?,
                    ))
                },
            )
            .optional()
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        let Some((id, name, memory_type, created_at, updated_at)) = row else {
            continue;
        };

        let mut rules_stmt = conn
            .prepare("SELECT rule FROM character_rules WHERE character_id = ?1 ORDER BY idx")
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let rules = rules_stmt
            .query_map(params![&id], |r| r.get::<_, String>(0))
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        let mut scenes_stmt = conn
            .prepare(
                "SELECT id, content, direction, background_image_path, created_at, selected_variant_id
                 FROM scenes WHERE character_id = ?1 ORDER BY created_at ASC",
            )
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let scenes_rows = scenes_stmt
            .query_map(params![&id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, Option<String>>(5)?,
                ))
            })
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        let mut scenes = Vec::new();
        for scene in scenes_rows {
            let (
                scene_id,
                content,
                direction,
                background_image_path,
                scene_created_at,
                selected_variant_id,
            ) = scene.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

            let mut variants_stmt = conn
                .prepare(
                    "SELECT id, content, direction, created_at
                     FROM scene_variants WHERE scene_id = ?1 ORDER BY created_at ASC",
                )
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            let variants = variants_stmt
                .query_map(params![&scene_id], |r| {
                    Ok(json!({
                        "id": r.get::<_, String>(0)?,
                        "content": r.get::<_, String>(1)?,
                        "direction": r.get::<_, Option<String>>(2)?,
                        "createdAt": r.get::<_, i64>(3)?,
                    }))
                })
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

            scenes.push(json!({
                "id": scene_id,
                "content": content,
                "direction": direction,
                "backgroundImagePath": background_image_path,
                "createdAt": scene_created_at,
                "selectedVariantId": selected_variant_id,
                "variants": variants,
            }));
        }

        out.push(json!({
            "id": id,
            "name": name,
            "rules": rules,
            "scenes": scenes,
            "memoryType": memory_type,
            "createdAt": created_at,
            "updatedAt": updated_at,
        }));
    }

    Ok(out)
}

fn maybe_read_persona_snapshot(
    conn: &rusqlite::Connection,
    persona_id: Option<&str>,
) -> Result<Option<JsonValue>, String> {
    let Some(pid) = persona_id else {
        return Ok(None);
    };

    let row = conn
        .query_row(
            "SELECT id, title, description, is_default, created_at, updated_at
             FROM personas WHERE id = ?1",
            params![pid],
            |r| {
                Ok(json!({
                    "id": r.get::<_, String>(0)?,
                    "title": r.get::<_, String>(1)?,
                    "description": r.get::<_, String>(2)?,
                    "isDefault": r.get::<_, i64>(3)? != 0,
                    "createdAt": r.get::<_, i64>(4)?,
                    "updatedAt": r.get::<_, i64>(5)?,
                }))
            },
        )
        .optional()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    Ok(row)
}

#[tauri::command]
pub fn chatpkg_export_single_chat(
    app: tauri::AppHandle,
    session_id: String,
    include_character_id: Option<bool>,
) -> Result<String, String> {
    let include_character_id = include_character_id.unwrap_or(true);
    let session_json = super::sessions::session_get(app.clone(), session_id)?
        .ok_or_else(|| crate::utils::err_msg(module_path!(), line!(), "Session not found"))?;

    let mut session: JsonValue = serde_json::from_str(&session_json)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let persona_disabled = session
        .get("personaDisabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if persona_disabled {
        session["personaId"] = JsonValue::Null;
    }
    if let Some(obj) = session.as_object_mut() {
        obj.remove("personaDisabled");
        obj.remove("memoryStatus");
        obj.remove("memoryError");
        if !include_character_id {
            obj.remove("characterId");
        }
    }

    let title = session
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("chat");

    let messages = session
        .get("messages")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let attachments = collect_attachment_paths(&messages);

    let envelope = json!({
        "type": "single_chat",
        "version": CHATPKG_VERSION,
        "exportedAt": now_ms() as i64,
        "source": {
            "app": "lettuce",
            "format": "chatpkg",
            "appVersion": crate::utils::app_version(&app),
        },
        "payload": {
            "session": session,
        }
    });

    let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!("chat_{}_{}.chatpkg", sanitize_filename(title), timestamp);
    let output_path = get_downloads_dir()?.join(filename);

    write_chatpkg(&output_path, &envelope, &attachments, &app)?;
    Ok(output_path.to_string_lossy().to_string())
}

#[tauri::command]
pub fn chatpkg_export_group_chat(
    app: tauri::AppHandle,
    session_id: String,
    include_character_snapshots: Option<bool>,
    pool: State<'_, SwappablePool>,
) -> Result<String, String> {
    let include_character_snapshots = include_character_snapshots.unwrap_or(false);
    let conn = pool.get_connection()?;

    let group_session = read_group_session_payload(&conn, &session_id)?
        .ok_or_else(|| crate::utils::err_msg(module_path!(), line!(), "Session not found"))?;

    let messages = read_group_messages_payload(&conn, &session_id)?;
    let participation = read_group_participation_payload(&conn, &session_id)?;

    let character_ids = group_session
        .get("characterIds")
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let character_snapshots = if include_character_snapshots {
        maybe_read_character_snapshots(&conn, &character_ids)?
    } else {
        vec![]
    };

    let persona_snapshot = maybe_read_persona_snapshot(
        &conn,
        group_session.get("personaId").and_then(|v| v.as_str()),
    )?;

    let attachments = collect_attachment_paths(&messages);

    let mut payload = json!({
        "groupSession": group_session,
        "messages": messages,
        "participation": participation,
    });

    if include_character_snapshots {
        payload["characterSnapshots"] = JsonValue::Array(character_snapshots);
    }
    if let Some(persona) = persona_snapshot {
        payload["personaSnapshot"] = persona;
    }

    let title = payload
        .get("groupSession")
        .and_then(|s| s.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or("group_chat");

    let envelope = json!({
        "type": "group_chat",
        "version": CHATPKG_VERSION,
        "exportedAt": now_ms() as i64,
        "source": {
            "app": "lettuce",
            "format": "chatpkg",
            "appVersion": crate::utils::app_version(&app),
        },
        "payload": payload,
    });

    let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!(
        "group_chat_{}_{}.chatpkg",
        sanitize_filename(title),
        timestamp
    );
    let output_path = get_downloads_dir()?.join(filename);

    write_chatpkg(&output_path, &envelope, &attachments, &app)?;
    Ok(output_path.to_string_lossy().to_string())
}

#[tauri::command]
pub fn chatpkg_inspect(app: tauri::AppHandle, package_path: String) -> Result<String, String> {
    let ChatpkgSource { chat_json, .. } = read_chatpkg_source(&package_path)?;

    let pkg_type = chat_json
        .get("type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| crate::utils::err_msg(module_path!(), line!(), "CHATPKG_INVALID_TYPE"))?;

    let version = chat_json
        .get("version")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    if version != CHATPKG_VERSION {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("CHATPKG_UNSUPPORTED_VERSION:{}", version),
        ));
    }

    match pkg_type {
        "single_chat" => {
            let session = chat_json
                .get("payload")
                .and_then(|v| v.get("session"))
                .ok_or_else(|| {
                    crate::utils::err_msg(module_path!(), line!(), "CHATPKG_PAYLOAD_MISSING")
                })?;

            let title = session
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("Untitled");
            let character_id = session
                .get("characterId")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let out = json!({
                "type": "single_chat",
                "version": version,
                "title": title,
                "characterId": character_id,
                "requiresCharacterSelection": character_id.is_none(),
                "exportedAt": chat_json.get("exportedAt").cloned().unwrap_or(JsonValue::Null),
                "source": chat_json.get("source").cloned().unwrap_or(JsonValue::Null),
            });

            serde_json::to_string(&out)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
        }
        "group_chat" => {
            let conn = open_db(&app)?;
            let payload = chat_json.get("payload").ok_or_else(|| {
                crate::utils::err_msg(module_path!(), line!(), "CHATPKG_PAYLOAD_MISSING")
            })?;
            let session = payload.get("groupSession").ok_or_else(|| {
                crate::utils::err_msg(module_path!(), line!(), "CHATPKG_PAYLOAD_MISSING")
            })?;

            let participants = payload
                .get("participation")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            let mut inspected = Vec::new();
            for p in participants {
                let character_id = p
                    .get("characterId")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let resolved = if let Some(cid) = character_id.as_ref() {
                    let exists: bool = conn
                        .query_row(
                            "SELECT 1 FROM characters WHERE id = ?1",
                            params![cid],
                            |_| Ok(true),
                        )
                        .unwrap_or(false);
                    exists
                } else {
                    false
                };

                inspected.push(ChatpkgInspectParticipant {
                    id: p.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    character_id,
                    character_display_name: p
                        .get("characterDisplayName")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    resolved,
                });
            }

            let out = json!({
                "type": "group_chat",
                "version": version,
                "title": session.get("name").and_then(|v| v.as_str()).unwrap_or("Untitled Group Chat"),
                "participantCount": inspected.len(),
                "participants": inspected,
                "exportedAt": chat_json.get("exportedAt").cloned().unwrap_or(JsonValue::Null),
                "source": chat_json.get("source").cloned().unwrap_or(JsonValue::Null),
            });

            serde_json::to_string(&out)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
        }
        _ => Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "CHATPKG_INVALID_TYPE",
        )),
    }
}

fn remap_single_chat_messages(messages: &mut [JsonValue]) {
    for message in messages.iter_mut() {
        if !message.is_object() {
            continue;
        }

        message["id"] = JsonValue::String(Uuid::new_v4().to_string());

        if let Some(variants) = message.get_mut("variants").and_then(|v| v.as_array_mut()) {
            let mut variant_map: HashMap<String, String> = HashMap::new();
            for variant in variants.iter_mut() {
                if let Some(old) = variant.get("id").and_then(|v| v.as_str()) {
                    let new_id = Uuid::new_v4().to_string();
                    variant_map.insert(old.to_string(), new_id.clone());
                    variant["id"] = JsonValue::String(new_id);
                }
            }

            if let Some(selected) = message
                .get("selectedVariantId")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
            {
                if let Some(new_selected) = variant_map.get(&selected) {
                    message["selectedVariantId"] = JsonValue::String(new_selected.clone());
                }
            }
        }
    }
}

#[tauri::command]
pub fn chatpkg_import(
    app: tauri::AppHandle,
    package_path: String,
    options_json: Option<String>,
    pool: State<'_, SwappablePool>,
) -> Result<String, String> {
    let ChatpkgSource {
        chat_json,
        attachments,
    } = read_chatpkg_source(&package_path)?;
    let pkg_type = chat_json
        .get("type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| crate::utils::err_msg(module_path!(), line!(), "CHATPKG_INVALID_TYPE"))?;

    let version = chat_json
        .get("version")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    if version != CHATPKG_VERSION {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("CHATPKG_UNSUPPORTED_VERSION:{}", version),
        ));
    }

    let options: ChatpkgImportOptions = match options_json {
        Some(raw) => serde_json::from_str(&raw)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
        None => ChatpkgImportOptions {
            target_character_id: None,
            participant_character_map: None,
        },
    };

    match pkg_type {
        "single_chat" => {
            let payload = chat_json.get("payload").ok_or_else(|| {
                crate::utils::err_msg(module_path!(), line!(), "CHATPKG_PAYLOAD_MISSING")
            })?;
            let mut session = payload.get("session").cloned().ok_or_else(|| {
                crate::utils::err_msg(module_path!(), line!(), "CHATPKG_PAYLOAD_MISSING")
            })?;

            let source_character_id = session
                .get("characterId")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let target_character_id = options
                .target_character_id
                .clone()
                .or(source_character_id)
                .ok_or_else(|| {
                    crate::utils::err_msg(module_path!(), line!(), "TARGET_CHARACTER_REQUIRED")
                })?;

            let new_session_id = Uuid::new_v4().to_string();
            session["id"] = JsonValue::String(new_session_id.clone());
            session["characterId"] = JsonValue::String(target_character_id.clone());

            if session
                .get("personaDisabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                session["personaId"] = JsonValue::Null;
            }
            if let Some(obj) = session.as_object_mut() {
                obj.remove("personaDisabled");
                obj.remove("memoryStatus");
                obj.remove("memoryError");
            }

            let mut messages = session
                .get("messages")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            restore_attachment_paths(&mut messages, &attachments, &app)?;
            remap_single_chat_messages(&mut messages);
            session["messages"] = JsonValue::Array(vec![]);

            super::sessions::session_upsert_meta(
                app.clone(),
                serde_json::to_string(&session)
                    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
            )?;

            super::sessions::messages_upsert_batch(
                app,
                new_session_id.clone(),
                serde_json::to_string(&messages)
                    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
            )?;

            let out = json!({
                "type": "single_chat",
                "sessionId": new_session_id,
                "characterId": target_character_id,
            });
            serde_json::to_string(&out)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
        }
        "group_chat" => {
            let payload = chat_json.get("payload").ok_or_else(|| {
                crate::utils::err_msg(module_path!(), line!(), "CHATPKG_PAYLOAD_MISSING")
            })?;

            let mut group_session = payload.get("groupSession").cloned().ok_or_else(|| {
                crate::utils::err_msg(module_path!(), line!(), "CHATPKG_PAYLOAD_MISSING")
            })?;
            let mut messages = payload
                .get("messages")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            restore_attachment_paths(&mut messages, &attachments, &app)?;
            let participation = payload
                .get("participation")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            let map = options.participant_character_map.unwrap_or_default();
            let conn = pool.get_connection()?;
            let new_session_id = Uuid::new_v4().to_string();
            let now = now_ms() as i64;

            let mut unresolved = Vec::new();
            let mut final_participation_rows: Vec<(String, String, i64, Option<i64>, Option<i64>)> =
                Vec::new();
            let mut source_to_target_char: HashMap<String, String> = HashMap::new();

            for p in participation {
                let participant_id = p
                    .get("id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| Uuid::new_v4().to_string());
                let source_char_id = p
                    .get("characterId")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let display_name = p
                    .get("characterDisplayName")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown")
                    .to_string();

                let mapped_char_id = map
                    .get(&participant_id)
                    .cloned()
                    .or_else(|| {
                        source_char_id
                            .as_ref()
                            .and_then(|cid| map.get(cid).cloned())
                    })
                    .or(source_char_id.clone());

                let Some(final_char_id) = mapped_char_id else {
                    unresolved.push(display_name);
                    continue;
                };

                let exists: bool = conn
                    .query_row(
                        "SELECT 1 FROM characters WHERE id = ?1",
                        params![&final_char_id],
                        |_| Ok(true),
                    )
                    .unwrap_or(false);
                if !exists {
                    unresolved.push(display_name);
                    continue;
                }

                if let Some(source) = source_char_id {
                    source_to_target_char.insert(source, final_char_id.clone());
                }

                final_participation_rows.push((
                    Uuid::new_v4().to_string(),
                    final_char_id,
                    p.get("speakCount").and_then(|v| v.as_i64()).unwrap_or(0),
                    p.get("lastSpokeTurn").and_then(|v| v.as_i64()),
                    p.get("lastSpokeAt").and_then(|v| v.as_i64()),
                ));
            }

            if !unresolved.is_empty() {
                return Err(crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("UNRESOLVED_PARTICIPANTS:{}", unresolved.join(", ")),
                ));
            }

            let mut unique_character_ids = Vec::new();
            let mut seen = HashSet::new();
            for (_, cid, _, _, _) in &final_participation_rows {
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

            group_session["id"] = JsonValue::String(new_session_id.clone());
            group_session["characterIds"] = JsonValue::Array(
                unique_character_ids
                    .iter()
                    .cloned()
                    .map(JsonValue::String)
                    .collect(),
            );
            group_session["updatedAt"] = JsonValue::from(now);

            let name = group_session
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("Imported Group Chat");
            let persona_id = group_session
                .get("personaId")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let created_at = group_session
                .get("createdAt")
                .and_then(|v| v.as_i64())
                .unwrap_or(now);
            let archived = group_session
                .get("archived")
                .and_then(|v| v.as_bool())
                .unwrap_or(false) as i64;
            let chat_type = group_session
                .get("chatType")
                .and_then(|v| v.as_str())
                .unwrap_or("conversation");
            let starting_scene = group_session
                .get("startingScene")
                .filter(|v| !v.is_null())
                .and_then(|v| serde_json::to_string(v).ok());
            let background_image_path = group_session
                .get("backgroundImagePath")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let memories_json = serde_json::to_string(
                group_session
                    .get("memories")
                    .unwrap_or(&JsonValue::Array(vec![])),
            )
            .unwrap_or_else(|_| "[]".to_string());
            let memory_embeddings_json = serde_json::to_string(
                group_session
                    .get("memoryEmbeddings")
                    .unwrap_or(&JsonValue::Array(vec![])),
            )
            .unwrap_or_else(|_| "[]".to_string());
            let memory_summary = group_session
                .get("memorySummary")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let memory_summary_token_count = group_session
                .get("memorySummaryTokenCount")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let memory_tool_events_json = serde_json::to_string(
                group_session
                    .get("memoryToolEvents")
                    .unwrap_or(&JsonValue::Array(vec![])),
            )
            .unwrap_or_else(|_| "[]".to_string());
            let speaker_selection_method = group_session
                .get("speakerSelectionMethod")
                .and_then(|v| v.as_str())
                .unwrap_or("llm");
            let muted_character_ids: Vec<String> = group_session
                .get("mutedCharacterIds")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|value| value.as_str())
                        .filter_map(|old_id| source_to_target_char.get(old_id).cloned())
                        .filter(|id| unique_character_ids.contains(id))
                        .collect()
                })
                .unwrap_or_default();

            conn.execute(
                "INSERT INTO group_sessions (id, name, character_ids, muted_character_ids, persona_id, created_at, updated_at, archived,
                 chat_type, starting_scene, background_image_path, memories, memory_embeddings, memory_summary,
                 memory_summary_token_count, memory_tool_events, speaker_selection_method)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
                params![
                    &new_session_id,
                    name,
                    serde_json::to_string(&unique_character_ids)
                        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
                    serde_json::to_string(&muted_character_ids)
                        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
                    persona_id,
                    created_at,
                    now,
                    archived,
                    chat_type,
                    starting_scene,
                    background_image_path,
                    memories_json,
                    memory_embeddings_json,
                    memory_summary,
                    memory_summary_token_count,
                    memory_tool_events_json,
                    speaker_selection_method,
                ],
            )
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

            for (id, character_id, speak_count, last_spoke_turn, last_spoke_at) in
                final_participation_rows
            {
                conn.execute(
                    "INSERT INTO group_participation (id, session_id, character_id, speak_count, last_spoke_turn, last_spoke_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![id, &new_session_id, character_id, speak_count, last_spoke_turn, last_spoke_at],
                )
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            }

            for message in messages {
                let new_message_id = Uuid::new_v4().to_string();
                let source_speaker = message
                    .get("speakerCharacterId")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let mapped_speaker = source_speaker
                    .as_ref()
                    .and_then(|sid| source_to_target_char.get(sid).cloned())
                    .or(source_speaker);

                let usage = message.get("usage");
                let prompt_tokens = usage
                    .and_then(|u| u.get("promptTokens"))
                    .and_then(|v| v.as_i64());
                let completion_tokens = usage
                    .and_then(|u| u.get("completionTokens"))
                    .and_then(|v| v.as_i64());
                let total_tokens = usage
                    .and_then(|u| u.get("totalTokens"))
                    .and_then(|v| v.as_i64());

                let attachments_json = serde_json::to_string(
                    message
                        .get("attachments")
                        .unwrap_or(&JsonValue::Array(vec![])),
                )
                .unwrap_or_else(|_| "[]".to_string());

                let turn_number = message
                    .get("turnNumber")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                let created_at = message
                    .get("createdAt")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(now);

                let variants = message
                    .get("variants")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();

                let mut variant_map: HashMap<String, String> = HashMap::new();
                for variant in &variants {
                    if let Some(old) = variant.get("id").and_then(|v| v.as_str()) {
                        variant_map.insert(old.to_string(), Uuid::new_v4().to_string());
                    }
                }

                let selected_variant_id = message
                    .get("selectedVariantId")
                    .and_then(|v| v.as_str())
                    .and_then(|id| variant_map.get(id).cloned());

                conn.execute(
                    "INSERT INTO group_messages (id, session_id, role, content, speaker_character_id, turn_number,
                     created_at, prompt_tokens, completion_tokens, total_tokens, selected_variant_id, is_pinned,
                     attachments, used_lorebook_entries, reasoning, selection_reasoning, model_id)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
                    params![
                        &new_message_id,
                        &new_session_id,
                        message.get("role").and_then(|v| v.as_str()).unwrap_or("assistant"),
                        message.get("content").and_then(|v| v.as_str()).unwrap_or(""),
                        mapped_speaker,
                        turn_number,
                        created_at,
                        prompt_tokens,
                        completion_tokens,
                        total_tokens,
                        selected_variant_id,
                        message.get("isPinned").and_then(|v| v.as_bool()).unwrap_or(false) as i64,
                        attachments_json,
                        serde_json::to_string(
                            &message
                                .get("usedLorebookEntries")
                                .and_then(|v| v.as_array())
                                .cloned()
                                .unwrap_or_default(),
                        )
                        .unwrap_or_else(|_| "[]".to_string()),
                        message.get("reasoning").and_then(|v| v.as_str()),
                        message.get("selectionReasoning").and_then(|v| v.as_str()),
                        message.get("modelId").and_then(|v| v.as_str()),
                    ],
                )
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

                for variant in variants {
                    let old_variant_id = variant.get("id").and_then(|v| v.as_str()).unwrap_or("");
                    let new_variant_id = variant_map
                        .get(old_variant_id)
                        .cloned()
                        .unwrap_or_else(|| Uuid::new_v4().to_string());

                    let v_usage = variant.get("usage");
                    let v_prompt_tokens = v_usage
                        .and_then(|u| u.get("promptTokens"))
                        .and_then(|v| v.as_i64());
                    let v_completion_tokens = v_usage
                        .and_then(|u| u.get("completionTokens"))
                        .and_then(|v| v.as_i64());
                    let v_total_tokens = v_usage
                        .and_then(|u| u.get("totalTokens"))
                        .and_then(|v| v.as_i64());

                    let v_source_speaker = variant
                        .get("speakerCharacterId")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    let v_mapped_speaker = v_source_speaker
                        .as_ref()
                        .and_then(|sid| source_to_target_char.get(sid).cloned())
                        .or(v_source_speaker);

                    conn.execute(
                        "INSERT INTO group_message_variants (id, message_id, content, speaker_character_id, created_at,
                         prompt_tokens, completion_tokens, total_tokens, reasoning, selection_reasoning, model_id)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                        params![
                            new_variant_id,
                            &new_message_id,
                            variant.get("content").and_then(|v| v.as_str()).unwrap_or(""),
                            v_mapped_speaker,
                            variant.get("createdAt").and_then(|v| v.as_i64()).unwrap_or(now),
                            v_prompt_tokens,
                            v_completion_tokens,
                            v_total_tokens,
                            variant.get("reasoning").and_then(|v| v.as_str()),
                            variant.get("selectionReasoning").and_then(|v| v.as_str()),
                            variant.get("modelId").and_then(|v| v.as_str()),
                        ],
                    )
                    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
                }
            }

            let out = json!({
                "type": "group_chat",
                "sessionId": new_session_id,
            });
            serde_json::to_string(&out)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
        }
        _ => Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "CHATPKG_INVALID_TYPE",
        )),
    }
}
