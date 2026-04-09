use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

use super::tools::{get_creation_helper_system_prompt, get_creation_helper_tools};
use super::types::*;
use crate::abort_manager::AbortRegistry;
use crate::api::{api_request, ApiRequest, ApiResponse};
use crate::chat_manager::request as chat_request;
use crate::chat_manager::request_builder::{build_chat_request, effective_streaming_enabled};
use crate::chat_manager::service::{
    apply_openrouter_cost_to_usage, insert_extended_usage_metadata,
};
use crate::chat_manager::sse::accumulate_tool_calls_from_sse;
use crate::chat_manager::tooling::{parse_tool_calls, ToolChoice, ToolConfig};
use crate::image_generator::commands::generate_image;
use crate::image_generator::types::ImageGenerationRequest;
use crate::storage_manager::characters as characters_storage;
use crate::storage_manager::db::{now_ms, open_db};
use crate::storage_manager::lorebook as lorebook_storage;
use crate::storage_manager::media::{storage_read_image_data, storage_write_image_data};
use crate::storage_manager::personas as personas_storage;
use crate::storage_manager::settings::internal_read_settings;
use crate::usage::{
    add_usage_record,
    tracking::{RequestUsage, UsageFinishReason, UsageOperationType},
};
use crate::utils::{log_error, log_info, log_warn};

lazy_static::lazy_static! {
    static ref SESSIONS: Mutex<HashMap<String, CreationSession>> = Mutex::new(HashMap::new());
    static ref UPLOADED_IMAGES: Mutex<HashMap<String, HashMap<String, UploadedImage>>> = Mutex::new(HashMap::new());
    static ref LAST_GENERATED_IMAGES: Mutex<HashMap<String, String>> = Mutex::new(HashMap::new());
}

fn serialize_creation_goal(goal: &CreationGoal) -> &'static str {
    match goal {
        CreationGoal::Character => "character",
        CreationGoal::Persona => "persona",
        CreationGoal::Lorebook => "lorebook",
    }
}

fn serialize_creation_status(status: &CreationStatus) -> &'static str {
    match status {
        CreationStatus::Active => "active",
        CreationStatus::PreviewShown => "previewShown",
        CreationStatus::Completed => "completed",
        CreationStatus::Cancelled => "cancelled",
    }
}

fn get_cached_images_map(session_id: &str) -> Result<HashMap<String, UploadedImage>, String> {
    let images = UPLOADED_IMAGES
        .lock()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(images.get(session_id).cloned().unwrap_or_default())
}

fn get_uploaded_image_record(
    session_id: &str,
    image_id: &str,
) -> Result<Option<UploadedImage>, String> {
    let images = UPLOADED_IMAGES
        .lock()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(images
        .get(session_id)
        .and_then(|session_images| session_images.get(image_id))
        .cloned())
}

fn hydrate_uploaded_image(app: &AppHandle, image: UploadedImage) -> Result<UploadedImage, String> {
    if !image.data.is_empty() || image.asset_id.is_none() {
        return Ok(image);
    }

    let asset_id = image.asset_id.as_deref().unwrap_or_default();
    let data = storage_read_image_data(app, asset_id)?;

    Ok(UploadedImage { data, ..image })
}

fn insert_uploaded_image_record(session_id: &str, image: UploadedImage) -> Result<(), String> {
    let mut images = UPLOADED_IMAGES
        .lock()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let session_images = images
        .entry(session_id.to_string())
        .or_insert_with(HashMap::new);
    session_images.insert(image.id.clone(), image);
    Ok(())
}

fn persist_session(app: &AppHandle, session: &CreationSession) -> Result<(), String> {
    let conn = open_db(app)?;
    let session_json = serde_json::to_string(session)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let images_json = serde_json::to_string(&get_cached_images_map(&session.id)?)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    conn.execute(
        "INSERT INTO creation_helper_sessions
           (id, creation_goal, status, session_json, uploaded_images_json, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(id) DO UPDATE SET
           creation_goal = excluded.creation_goal,
           status = excluded.status,
           session_json = excluded.session_json,
           uploaded_images_json = excluded.uploaded_images_json,
           updated_at = excluded.updated_at",
        rusqlite::params![
            session.id,
            serialize_creation_goal(&session.creation_goal),
            serialize_creation_status(&session.status),
            session_json,
            images_json,
            session.created_at,
            session.updated_at
        ],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(())
}

fn emit_creation_helper_update(
    app: &AppHandle,
    session_id: &str,
    session: &CreationSession,
    active_tool_calls: Option<&[CreationToolCall]>,
    active_tool_results: Option<&[CreationToolResult]>,
) {
    let active_tool_calls = active_tool_calls.map(|calls| calls.to_vec());
    let active_tool_results = active_tool_results.map(|results| results.to_vec());

    let _ = app.emit(
        "creation-helper-update",
        json!({
            "sessionId": session_id,
            "draft": session.draft,
            "status": session.status,
            "messages": session.messages,
            "activeToolCalls": active_tool_calls,
            "activeToolResults": active_tool_results,
        }),
    );
}

fn is_ollama_provider(provider_id: &str) -> bool {
    provider_id.eq_ignore_ascii_case("ollama")
}

fn creation_tool_call_payload(
    provider_id: &str,
    id: &str,
    index: usize,
    name: &str,
    arguments: &Value,
) -> Value {
    let arguments = if is_ollama_provider(provider_id) {
        arguments.clone()
    } else {
        Value::String(serde_json::to_string(arguments).unwrap_or_default())
    };

    if is_ollama_provider(provider_id) {
        json!({
            "type": "function",
            "function": {
                "index": index,
                "name": name,
                "arguments": arguments
            }
        })
    } else {
        json!({
            "id": id,
            "type": "function",
            "function": {
                "name": name,
                "arguments": arguments
            }
        })
    }
}

fn creation_tool_result_message(
    provider_id: &str,
    tool_call_id: &str,
    tool_name: Option<&str>,
    result: &Value,
) -> Value {
    let mut message = json!({
        "role": "tool",
        "content": serde_json::to_string(result).unwrap_or_default()
    });

    if let Some(obj) = message.as_object_mut() {
        if is_ollama_provider(provider_id) {
            if let Some(name) = tool_name {
                obj.insert("tool_name".to_string(), json!(name));
            }
        } else {
            obj.insert("tool_call_id".to_string(), json!(tool_call_id));
        }
    }

    message
}

fn hydrate_session_cache(
    session: &CreationSession,
    images: HashMap<String, UploadedImage>,
) -> Result<(), String> {
    let mut sessions = SESSIONS
        .lock()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    sessions.insert(session.id.clone(), session.clone());
    drop(sessions);

    let mut cached_images = UPLOADED_IMAGES
        .lock()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    cached_images.insert(session.id.clone(), images);
    Ok(())
}

fn load_persisted_session(
    app: &AppHandle,
    session_id: &str,
) -> Result<Option<(CreationSession, HashMap<String, UploadedImage>)>, String> {
    let conn = open_db(app)?;
    let row = conn.query_row(
        "SELECT session_json, uploaded_images_json
         FROM creation_helper_sessions
         WHERE id = ?1",
        [session_id],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
    );
    match row {
        Ok((session_json, images_json)) => {
            let session: CreationSession = serde_json::from_str(&session_json)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            let images: HashMap<String, UploadedImage> = serde_json::from_str(&images_json)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            Ok(Some((session, images)))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(crate::utils::err_to_string(module_path!(), line!(), e)),
    }
}

pub fn start_session(
    app: &AppHandle,
    creation_goal: CreationGoal,
    creation_mode: CreationMode,
    target_type: Option<CreationGoal>,
    target_id: Option<String>,
) -> Result<CreationSession, String> {
    let now = now_ms() as i64;
    let session_id = Uuid::new_v4().to_string();
    let resolved_target_type = target_type.or_else(|| {
        if creation_mode == CreationMode::Edit {
            Some(creation_goal.clone())
        } else {
            None
        }
    });

    let mut initial_draft = DraftCharacter::default();
    if creation_mode == CreationMode::Edit {
        let tid = target_id
            .as_deref()
            .ok_or_else(|| "Missing target_id for edit mode".to_string())?;
        let ttype = resolved_target_type
            .clone()
            .ok_or_else(|| "Missing target_type for edit mode".to_string())?;
        initial_draft = load_target_draft(app, &ttype, tid)?;
    }

    let session = CreationSession {
        id: session_id.clone(),
        messages: vec![],
        draft: initial_draft,
        draft_history: vec![],
        creation_goal,
        creation_mode,
        target_type: resolved_target_type,
        target_id,
        status: CreationStatus::Active,
        created_at: now,
        updated_at: now,
    };

    {
        let mut sessions = SESSIONS
            .lock()
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        sessions.insert(session_id.clone(), session.clone());
    }

    {
        let mut images = UPLOADED_IMAGES
            .lock()
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        images.insert(session_id, HashMap::new());
    }

    persist_session(app, &session)?;

    Ok(session)
}

pub fn get_session(app: &AppHandle, session_id: &str) -> Result<Option<CreationSession>, String> {
    {
        let sessions = SESSIONS
            .lock()
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        if let Some(session) = sessions.get(session_id) {
            return Ok(Some(session.clone()));
        }
    }

    if let Some((session, images)) = load_persisted_session(app, session_id)? {
        hydrate_session_cache(&session, images)?;
        return Ok(Some(session));
    }

    Ok(None)
}

pub fn get_latest_resumable_session(
    app: &AppHandle,
    creation_goal: Option<CreationGoal>,
) -> Result<Option<CreationSession>, String> {
    let conn = open_db(app)?;
    let row = if let Some(goal) = creation_goal {
        conn.query_row(
            "SELECT session_json, uploaded_images_json
             FROM creation_helper_sessions
             WHERE creation_goal = ?1 AND status != 'completed'
             ORDER BY updated_at DESC
             LIMIT 1",
            [serialize_creation_goal(&goal)],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )
    } else {
        conn.query_row(
            "SELECT session_json, uploaded_images_json
             FROM creation_helper_sessions
             WHERE status != 'completed'
             ORDER BY updated_at DESC
             LIMIT 1",
            [],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )
    };

    match row {
        Ok((session_json, images_json)) => {
            let session: CreationSession = serde_json::from_str(&session_json)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            let images: HashMap<String, UploadedImage> = serde_json::from_str(&images_json)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            hydrate_session_cache(&session, images)?;
            Ok(Some(session))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(crate::utils::err_to_string(module_path!(), line!(), e)),
    }
}

fn build_session_summary(session: &CreationSession) -> CreationSessionSummary {
    let title = session
        .draft
        .name
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "Untitled conversation".to_string());

    let preview = session
        .messages
        .iter()
        .rev()
        .find_map(|msg| {
            let content = msg.content.trim();
            if content.is_empty() {
                None
            } else {
                Some(content.to_string())
            }
        })
        .unwrap_or_default();

    CreationSessionSummary {
        id: session.id.clone(),
        creation_goal: session.creation_goal.clone(),
        creation_mode: session.creation_mode.clone(),
        target_type: session.target_type.clone(),
        target_id: session.target_id.clone(),
        status: session.status.clone(),
        title,
        preview,
        message_count: session.messages.len(),
        created_at: session.created_at,
        updated_at: session.updated_at,
    }
}

fn load_target_draft(
    app: &AppHandle,
    target_type: &CreationGoal,
    target_id: &str,
) -> Result<DraftCharacter, String> {
    match target_type {
        CreationGoal::Character => load_character_draft(app, target_id),
        CreationGoal::Persona => load_persona_draft(app, target_id),
        CreationGoal::Lorebook => load_lorebook_draft(app, target_id),
    }
}

fn load_character_draft(app: &AppHandle, target_id: &str) -> Result<DraftCharacter, String> {
    let characters: Vec<Value> = characters_storage::characters_list_typed(app)?;
    let target = characters
        .into_iter()
        .find(|c| c.get("id").and_then(|v| v.as_str()) == Some(target_id))
        .ok_or_else(|| "Character not found".to_string())?;

    let scenes: Vec<DraftScene> = target
        .get("scenes")
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|scene| {
                    Some(DraftScene {
                        id: scene.get("id")?.as_str()?.to_string(),
                        content: scene
                            .get("content")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        direction: scene
                            .get("direction")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string()),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(DraftCharacter {
        name: target
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        definition: target
            .get("definition")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        description: target
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        scenes,
        default_scene_id: target
            .get("defaultSceneId")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        avatar_path: target
            .get("avatarPath")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        background_image_path: target
            .get("backgroundImagePath")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        disable_avatar_gradient: target
            .get("disableAvatarGradient")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        default_model_id: target
            .get("defaultModelId")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        prompt_template_id: target
            .get("promptTemplateId")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
    })
}

fn load_persona_draft(app: &AppHandle, target_id: &str) -> Result<DraftCharacter, String> {
    let personas: Vec<Value> = personas_storage::personas_list_typed(app)?;
    let target = personas
        .into_iter()
        .find(|p| p.get("id").and_then(|v| v.as_str()) == Some(target_id))
        .ok_or_else(|| "Persona not found".to_string())?;

    Ok(DraftCharacter {
        name: target
            .get("title")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        definition: None,
        description: target
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        scenes: Vec::new(),
        default_scene_id: None,
        avatar_path: target
            .get("avatarPath")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        background_image_path: None,
        disable_avatar_gradient: false,
        default_model_id: None,
        prompt_template_id: None,
    })
}

fn load_lorebook_draft(app: &AppHandle, target_id: &str) -> Result<DraftCharacter, String> {
    let raw = lorebook_storage::lorebooks_list(app.clone())?;
    let lorebooks: Vec<Value> = serde_json::from_str(&raw)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let target = lorebooks
        .into_iter()
        .find(|lb| lb.get("id").and_then(|v| v.as_str()) == Some(target_id))
        .ok_or_else(|| "Lorebook not found".to_string())?;

    Ok(DraftCharacter {
        name: target
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        definition: None,
        description: None,
        scenes: Vec::new(),
        default_scene_id: None,
        avatar_path: None,
        background_image_path: None,
        disable_avatar_gradient: false,
        default_model_id: None,
        prompt_template_id: None,
    })
}

pub fn list_sessions(
    app: &AppHandle,
    creation_goal: Option<CreationGoal>,
) -> Result<Vec<CreationSessionSummary>, String> {
    let conn = open_db(app)?;
    let sql = if creation_goal.is_some() {
        "SELECT session_json
         FROM creation_helper_sessions
         WHERE creation_goal = ?1
         ORDER BY updated_at DESC"
    } else {
        "SELECT session_json
         FROM creation_helper_sessions
         ORDER BY updated_at DESC"
    };

    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let mut summaries = Vec::new();
    if let Some(goal) = creation_goal {
        let rows = stmt
            .query_map([serialize_creation_goal(&goal)], |row| {
                row.get::<_, String>(0)
            })
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        for row in rows {
            let session_json =
                row.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            let session: CreationSession = serde_json::from_str(&session_json)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            summaries.push(build_session_summary(&session));
        }
    } else {
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        for row in rows {
            let session_json =
                row.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            let session: CreationSession = serde_json::from_str(&session_json)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            summaries.push(build_session_summary(&session));
        }
    }

    Ok(summaries)
}

pub fn save_uploaded_image(
    app: &AppHandle,
    session_id: &str,
    image_id: String,
    data: String,
    mime_type: String,
) -> Result<(), String> {
    let stored = storage_write_image_data(app, &image_id, &data)?;
    insert_uploaded_image_record(
        session_id,
        UploadedImage {
            id: image_id.clone(),
            data: String::new(),
            mime_type: if mime_type.trim().is_empty() {
                stored.mime_type
            } else {
                mime_type
            },
            asset_id: Some(image_id.clone()),
        },
    )
}

fn save_uploaded_image_asset_ref(
    session_id: &str,
    image_id: String,
    asset_id: String,
    mime_type: String,
) -> Result<(), String> {
    insert_uploaded_image_record(
        session_id,
        UploadedImage {
            id: image_id,
            data: String::new(),
            mime_type,
            asset_id: Some(asset_id),
        },
    )
}

pub fn get_uploaded_image(
    app: &AppHandle,
    session_id: &str,
    image_id: &str,
) -> Result<Option<UploadedImage>, String> {
    get_uploaded_image_record(session_id, image_id)?
        .map(|image| hydrate_uploaded_image(app, image))
        .transpose()
}

pub fn get_uploaded_image_metadata(
    session_id: &str,
    image_id: &str,
) -> Result<Option<UploadedImage>, String> {
    get_uploaded_image_record(session_id, image_id)
}

pub fn get_all_uploaded_images_metadata(session_id: &str) -> Result<Vec<UploadedImage>, String> {
    let images = UPLOADED_IMAGES
        .lock()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(images
        .get(session_id)
        .map(|session_images| session_images.values().cloned().collect())
        .unwrap_or_default())
}

fn resolve_uploaded_image_id(session_id: &str, image_id: &str) -> Result<Option<String>, String> {
    if get_uploaded_image_record(session_id, image_id)?.is_some() {
        return Ok(Some(image_id.to_string()));
    }

    let compact: String = image_id.chars().filter(|c| *c != '-').collect();
    if compact.len() >= 8 {
        let short_id: String = compact.chars().take(8).collect();
        if get_uploaded_image_record(session_id, &short_id)?.is_some() {
            return Ok(Some(short_id));
        }
    }

    if compact.len() >= 32 {
        if let Some(last) = get_last_generated_image(session_id)? {
            if get_uploaded_image_record(session_id, &last)?.is_some() {
                return Ok(Some(last));
            }
        }
    }

    Ok(None)
}

fn set_last_generated_image(session_id: &str, image_id: &str) -> Result<(), String> {
    let mut latest = LAST_GENERATED_IMAGES
        .lock()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    latest.insert(session_id.to_string(), image_id.to_string());
    Ok(())
}

fn get_last_generated_image(session_id: &str) -> Result<Option<String>, String> {
    let latest = LAST_GENERATED_IMAGES
        .lock()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    Ok(latest.get(session_id).cloned())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CreationTurnStage {
    Discovery,
    Drafting,
    Preview,
    Finalize,
}

struct CreationTurnPlan {
    stage: CreationTurnStage,
    tool_config: ToolConfig,
    guidance: String,
}

fn has_text(value: Option<&str>) -> bool {
    value.map(|s| !s.trim().is_empty()).unwrap_or(false)
}

fn latest_user_message_text(session: &CreationSession) -> Option<&str> {
    session
        .messages
        .iter()
        .rev()
        .find(|msg| msg.role == CreationMessageRole::User)
        .map(|msg| msg.content.as_str())
}

fn has_progress(session: &CreationSession) -> bool {
    has_text(session.draft.name.as_deref())
        || has_text(session.draft.definition.as_deref())
        || has_text(session.draft.description.as_deref())
        || !session.draft.scenes.is_empty()
        || has_text(session.draft.avatar_path.as_deref())
        || has_text(session.draft.background_image_path.as_deref())
}

fn is_substantive_request(message: &str) -> bool {
    let trimmed = message.trim();
    if trimmed.len() >= 40 {
        return true;
    }
    trimmed.split_whitespace().count() >= 6
}

fn lower_contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

fn user_requested_edits(message: &str) -> bool {
    lower_contains_any(
        message,
        &[
            "change",
            "edit",
            "update",
            "rewrite",
            "rename",
            "remove",
            "delete",
            "different",
            "instead",
            "replace",
            "add ",
        ],
    )
}

fn user_requested_visuals(message: &str) -> bool {
    lower_contains_any(
        message,
        &[
            "avatar",
            "background",
            "image",
            "picture",
            "photo",
            "portrait",
            "art",
            "illustration",
        ],
    )
}

fn user_requested_model_tools(message: &str) -> bool {
    lower_contains_any(message, &[" model", "provider", "engine", "llm"])
}

fn user_requested_prompt_tools(message: &str) -> bool {
    lower_contains_any(
        message,
        &[
            "system prompt",
            "prompt template",
            "template",
            "jailbreak",
            "instructions",
        ],
    )
}

fn user_requested_scene_work(message: &str) -> bool {
    lower_contains_any(
        message,
        &["scene", "opening", "starter", "first message", "intro"],
    )
}

fn user_requested_lorebooks(message: &str) -> bool {
    lower_contains_any(
        message,
        &["lorebook", "lore book", "world info", "encyclopedia"],
    )
}

fn user_requested_persona_admin(message: &str) -> bool {
    lower_contains_any(
        message,
        &["default persona", "existing persona", "delete persona"],
    )
}

fn user_requested_lorebook_admin(message: &str) -> bool {
    lower_contains_any(
        message,
        &[
            "entry",
            "entries",
            "keyword",
            "reorder",
            "delete lorebook",
            "delete entry",
        ],
    )
}

fn draft_ready_for_preview(session: &CreationSession) -> bool {
    match session.creation_goal {
        CreationGoal::Character => {
            has_text(session.draft.name.as_deref())
                && (has_text(session.draft.definition.as_deref())
                    || has_text(session.draft.description.as_deref()))
                && !session.draft.scenes.is_empty()
        }
        CreationGoal::Persona => {
            has_text(session.draft.name.as_deref())
                && (has_text(session.draft.description.as_deref())
                    || has_text(session.draft.definition.as_deref()))
        }
        CreationGoal::Lorebook => has_text(session.draft.name.as_deref()),
    }
}

fn infer_turn_stage(session: &CreationSession, latest_user_message: &str) -> CreationTurnStage {
    let lowered = latest_user_message.to_ascii_lowercase();
    if user_requested_edits(&lowered) {
        return CreationTurnStage::Drafting;
    }

    if session.status == CreationStatus::PreviewShown {
        return CreationTurnStage::Finalize;
    }

    if draft_ready_for_preview(session) {
        return CreationTurnStage::Preview;
    }

    if !has_progress(session) && !is_substantive_request(latest_user_message) {
        return CreationTurnStage::Discovery;
    }

    CreationTurnStage::Drafting
}

fn push_tool_name(names: &mut Vec<String>, name: &str) {
    if !names.iter().any(|existing| existing == name) {
        names.push(name.to_string());
    }
}

fn infer_default_image_id(session: &CreationSession) -> Option<String> {
    if let Ok(Some(last)) = get_last_generated_image(&session.id) {
        return Some(last);
    }

    get_cached_images_map(&session.id).ok().and_then(|images| {
        if images.len() == 1 {
            images.into_iter().next().map(|(_, img)| img.id)
        } else {
            None
        }
    })
}

fn scene_id_hint(session: &CreationSession) -> Option<String> {
    session
        .draft
        .default_scene_id
        .clone()
        .or_else(|| session.draft.scenes.first().map(|scene| scene.id.clone()))
}

fn stage_label(stage: CreationTurnStage) -> &'static str {
    match stage {
        CreationTurnStage::Discovery => "discovery",
        CreationTurnStage::Drafting => "drafting",
        CreationTurnStage::Preview => "preview",
        CreationTurnStage::Finalize => "finalize",
    }
}

fn filter_tools_by_name(
    tools: Vec<crate::chat_manager::tooling::ToolDefinition>,
    names: &[String],
) -> Vec<crate::chat_manager::tooling::ToolDefinition> {
    let allowed: HashSet<&str> = names.iter().map(|name| name.as_str()).collect();
    tools
        .into_iter()
        .filter(|tool| allowed.contains(tool.name.as_str()))
        .collect()
}

fn choose_tool_choice(
    stage: CreationTurnStage,
    tool_names: &[String],
    latest_user_message: &str,
) -> Option<ToolChoice> {
    if tool_names.is_empty() {
        return None;
    }

    if tool_names.len() == 1 {
        return Some(ToolChoice::Tool {
            name: tool_names[0].clone(),
        });
    }

    if matches!(stage, CreationTurnStage::Drafting) && is_substantive_request(latest_user_message) {
        return Some(ToolChoice::Required);
    }

    None
}

fn build_turn_guidance(
    smart_tool_selection: bool,
    stage: CreationTurnStage,
    tool_names: &[String],
) -> String {
    if !smart_tool_selection {
        if tool_names.is_empty() {
            return "Manual tool selection mode is enabled and no tools are available on this turn. Ask at most two short follow-up questions.".to_string();
        }

        return format!(
            "Manual tool selection mode is enabled. Use only these tools if they are clearly needed on this turn: {}. Prefer a single tool call, otherwise respond conversationally.",
            tool_names.join(", ")
        );
    }

    if tool_names.is_empty() {
        return format!(
            "Current phase: {}. No tools are available on this turn. Ask at most two short follow-up questions, gather missing details, and do not invent IDs or state.",
            stage_label(stage)
        );
    }

    let phase_instruction = match stage {
        CreationTurnStage::Discovery => {
            "Ask focused questions first. Only move into editing once the user has given enough detail."
        }
        CreationTurnStage::Drafting => {
            "Use the available drafting tools once details are clear. Prefer one concrete tool call instead of describing actions in prose."
        }
        CreationTurnStage::Preview => {
            "Call the preview tool now instead of only describing the preview."
        }
        CreationTurnStage::Finalize => {
            "Call the confirmation tool now unless the user explicitly asked for more edits."
        }
    };

    let mut extra_guidance = String::new();
    if tool_names
        .iter()
        .any(|name| name == "set_character_definition")
    {
        extra_guidance.push_str(" When writing a character definition, use plain prose or short labeled sections. Focus on stable traits, voice, motives, background, and boundaries. Do not format it as JSON, XML, or a dialogue transcript.");
    }
    if tool_names
        .iter()
        .any(|name| name == "add_scene" || name == "update_scene")
    {
        extra_guidance.push_str(" When writing a scene, make the `content` the actual playable opening message or opening situation, not notes about the scene. Put meta instructions in `direction` when needed.");
    }

    format!(
        "Current phase: {}. Tools available on this turn: {}. {}{} Never mention or invent unavailable tools.",
        stage_label(stage),
        tool_names.join(", "),
        phase_instruction,
        extra_guidance
    )
}

fn build_creation_turn_plan(
    session: &CreationSession,
    smart_tool_selection: bool,
    enabled_tools: Option<&[String]>,
) -> CreationTurnPlan {
    let all_tools = get_creation_helper_tools(&session.creation_goal, smart_tool_selection);
    let latest_user_message = latest_user_message_text(session).unwrap_or("").trim();
    let lowered = latest_user_message.to_ascii_lowercase();
    let stage = infer_turn_stage(session, latest_user_message);

    let mut tool_names = if smart_tool_selection {
        let mut planned = Vec::new();

        match session.creation_goal {
            CreationGoal::Character => match stage {
                CreationTurnStage::Discovery => {}
                CreationTurnStage::Drafting => {
                    if !has_text(session.draft.name.as_deref()) {
                        push_tool_name(&mut planned, "set_character_name");
                    }
                    if !has_text(session.draft.definition.as_deref())
                        && !has_text(session.draft.description.as_deref())
                    {
                        push_tool_name(&mut planned, "set_character_definition");
                    }
                    if session.draft.scenes.is_empty() {
                        push_tool_name(&mut planned, "add_scene");
                    } else if user_requested_scene_work(&lowered)
                        || session.creation_mode == CreationMode::Edit
                    {
                        push_tool_name(&mut planned, "update_scene");
                    }
                }
                CreationTurnStage::Preview => push_tool_name(&mut planned, "show_preview"),
                CreationTurnStage::Finalize => push_tool_name(&mut planned, "request_confirmation"),
            },
            CreationGoal::Persona => match stage {
                CreationTurnStage::Discovery => {}
                CreationTurnStage::Drafting => push_tool_name(&mut planned, "upsert_persona"),
                CreationTurnStage::Preview => push_tool_name(&mut planned, "show_preview"),
                CreationTurnStage::Finalize => push_tool_name(&mut planned, "request_confirmation"),
            },
            CreationGoal::Lorebook => match stage {
                CreationTurnStage::Discovery => {}
                CreationTurnStage::Drafting => {
                    push_tool_name(&mut planned, "upsert_lorebook");
                    if session.target_id.is_some() || has_text(session.draft.name.as_deref()) {
                        push_tool_name(&mut planned, "upsert_lorebook_entry");
                        push_tool_name(&mut planned, "create_blank_lorebook_entry");
                    }
                }
                CreationTurnStage::Preview => push_tool_name(&mut planned, "show_preview"),
                CreationTurnStage::Finalize => push_tool_name(&mut planned, "request_confirmation"),
            },
        }

        if user_requested_visuals(&lowered) {
            push_tool_name(&mut planned, "generate_image");
            if infer_default_image_id(session).is_some() {
                match session.creation_goal {
                    CreationGoal::Character => {
                        push_tool_name(&mut planned, "use_uploaded_image_as_avatar");
                        if lowered.contains("background") {
                            push_tool_name(&mut planned, "use_uploaded_image_as_chat_background");
                        }
                    }
                    CreationGoal::Persona => {
                        push_tool_name(&mut planned, "use_uploaded_image_as_persona_avatar");
                    }
                    CreationGoal::Lorebook => {}
                }
            }
            if lowered.contains("gradient") && session.creation_goal == CreationGoal::Character {
                push_tool_name(&mut planned, "toggle_avatar_gradient");
            }
        }

        if user_requested_model_tools(&lowered) && session.creation_goal == CreationGoal::Character
        {
            push_tool_name(&mut planned, "get_model_list");
            push_tool_name(&mut planned, "set_default_model");
        }

        if user_requested_prompt_tools(&lowered) && session.creation_goal == CreationGoal::Character
        {
            push_tool_name(&mut planned, "get_system_prompt_list");
            push_tool_name(&mut planned, "set_system_prompt");
        }

        if session.creation_goal == CreationGoal::Character
            && session.target_type == Some(CreationGoal::Character)
            && session.target_id.is_some()
            && user_requested_lorebooks(&lowered)
        {
            push_tool_name(&mut planned, "list_character_lorebooks");
            push_tool_name(&mut planned, "set_character_lorebooks");
        }

        if session.creation_goal == CreationGoal::Persona && user_requested_persona_admin(&lowered)
        {
            push_tool_name(&mut planned, "list_personas");
            push_tool_name(&mut planned, "get_default_persona");
            if lowered.contains("delete") {
                push_tool_name(&mut planned, "delete_persona");
            }
        }

        if session.creation_goal == CreationGoal::Lorebook
            && user_requested_lorebook_admin(&lowered)
        {
            push_tool_name(&mut planned, "list_lorebooks");
            if session.target_id.is_some() {
                push_tool_name(&mut planned, "list_lorebook_entries");
            }
            if lowered.contains("reorder") {
                push_tool_name(&mut planned, "reorder_lorebook_entries");
            }
            if lowered.contains("delete") {
                push_tool_name(&mut planned, "delete_lorebook");
                push_tool_name(&mut planned, "delete_lorebook_entry");
            }
        }

        planned
    } else {
        all_tools.iter().map(|tool| tool.name.clone()).collect()
    };

    if let Some(enabled) = enabled_tools {
        let enabled_set: HashSet<&str> = enabled.iter().map(|tool| tool.as_str()).collect();
        tool_names.retain(|tool| enabled_set.contains(tool.as_str()));
    }

    let tools = if smart_tool_selection {
        filter_tools_by_name(all_tools, &tool_names)
    } else {
        let enabled: HashSet<&str> = tool_names.iter().map(|name| name.as_str()).collect();
        all_tools
            .into_iter()
            .filter(|tool| enabled.contains(tool.name.as_str()))
            .collect()
    };

    let choice = if smart_tool_selection {
        choose_tool_choice(stage, &tool_names, latest_user_message)
    } else {
        None
    };

    CreationTurnPlan {
        stage,
        guidance: build_turn_guidance(smart_tool_selection, stage, &tool_names),
        tool_config: ToolConfig { tools, choice },
    }
}

fn first_string_argument(
    arguments: &serde_json::Map<String, Value>,
    keys: &[&str],
) -> Option<String> {
    keys.iter().find_map(|key| {
        arguments
            .get(*key)
            .and_then(|value| value.as_str())
            .map(|value| value.to_string())
            .filter(|value| !value.trim().is_empty())
    })
}

fn insert_string_if_missing(
    arguments: &mut serde_json::Map<String, Value>,
    key: &str,
    value: Option<String>,
) {
    if arguments
        .get(key)
        .and_then(|value| value.as_str())
        .is_some()
    {
        return;
    }
    if let Some(value) = value.filter(|value| !value.trim().is_empty()) {
        arguments.insert(key.to_string(), Value::String(value));
    }
}

fn normalize_tool_arguments(
    session: &CreationSession,
    tool_name: &str,
    arguments: &Value,
) -> Value {
    let mut normalized = arguments.as_object().cloned().unwrap_or_default();
    let fallback_image_id = infer_default_image_id(session);

    match tool_name {
        "set_character_name" => {
            let inferred_name = first_string_argument(&normalized, &["character_name", "title"]);
            insert_string_if_missing(&mut normalized, "name", inferred_name);
        }
        "set_character_definition" | "set_character_description" => {
            let inferred_definition = first_string_argument(
                &normalized,
                &["description", "personality", "bio", "content", "text"],
            );
            insert_string_if_missing(&mut normalized, "definition", inferred_definition);
        }
        "add_scene" => {
            let inferred_content = first_string_argument(
                &normalized,
                &["scene", "opening", "opening_message", "message", "text"],
            );
            insert_string_if_missing(&mut normalized, "content", inferred_content);
        }
        "update_scene" => {
            let inferred_scene_id =
                first_string_argument(&normalized, &["id"]).or_else(|| scene_id_hint(session));
            insert_string_if_missing(&mut normalized, "scene_id", inferred_scene_id);

            let inferred_content = first_string_argument(
                &normalized,
                &["scene", "opening", "opening_message", "message", "text"],
            );
            insert_string_if_missing(&mut normalized, "content", inferred_content);
        }
        "set_default_model" => {
            let inferred_model_id = first_string_argument(&normalized, &["id", "model"]);
            insert_string_if_missing(&mut normalized, "model_id", inferred_model_id);
        }
        "set_system_prompt" => {
            let inferred_prompt_id = first_string_argument(
                &normalized,
                &["id", "system_prompt_id", "prompt_template_id"],
            );
            insert_string_if_missing(&mut normalized, "prompt_id", inferred_prompt_id);
        }
        "use_uploaded_image_as_avatar" | "use_uploaded_image_as_chat_background" => {
            let inferred_image_id = first_string_argument(&normalized, &["id", "image"])
                .or_else(|| fallback_image_id.clone());
            insert_string_if_missing(&mut normalized, "image_id", inferred_image_id);
        }
        "use_uploaded_image_as_persona_avatar" => {
            let inferred_persona_id = first_string_argument(&normalized, &["id"]).or_else(|| {
                if session.target_type == Some(CreationGoal::Persona) {
                    session.target_id.clone()
                } else {
                    None
                }
            });
            insert_string_if_missing(&mut normalized, "persona_id", inferred_persona_id);

            let inferred_image_id = first_string_argument(&normalized, &["image", "id"])
                .or_else(|| fallback_image_id.clone());
            insert_string_if_missing(&mut normalized, "image_id", inferred_image_id);
        }
        "upsert_persona" => {
            insert_string_if_missing(
                &mut normalized,
                "id",
                if session.target_type == Some(CreationGoal::Persona) {
                    session.target_id.clone()
                } else {
                    None
                },
            );
            let inferred_title = first_string_argument(&normalized, &["name"]);
            insert_string_if_missing(&mut normalized, "title", inferred_title);

            let inferred_description =
                first_string_argument(&normalized, &["definition", "content", "text"]);
            insert_string_if_missing(&mut normalized, "description", inferred_description);
        }
        "delete_persona" => {
            let inferred_id = first_string_argument(&normalized, &["persona_id"]).or_else(|| {
                if session.target_type == Some(CreationGoal::Persona) {
                    session.target_id.clone()
                } else {
                    None
                }
            });
            insert_string_if_missing(&mut normalized, "id", inferred_id);
        }
        "upsert_lorebook" => {
            insert_string_if_missing(
                &mut normalized,
                "id",
                if session.target_type == Some(CreationGoal::Lorebook) {
                    session.target_id.clone()
                } else {
                    None
                },
            );
            let inferred_name = first_string_argument(&normalized, &["title"]);
            insert_string_if_missing(&mut normalized, "name", inferred_name);
        }
        "delete_lorebook" | "list_lorebook_entries" | "create_blank_lorebook_entry" => {
            let inferred_lorebook_id = first_string_argument(&normalized, &["id"]).or_else(|| {
                if session.target_type == Some(CreationGoal::Lorebook) {
                    session.target_id.clone()
                } else {
                    None
                }
            });
            insert_string_if_missing(&mut normalized, "lorebook_id", inferred_lorebook_id);
        }
        "upsert_lorebook_entry" => {
            let inferred_lorebook_id = first_string_argument(&normalized, &["id"]).or_else(|| {
                if session.target_type == Some(CreationGoal::Lorebook) {
                    session.target_id.clone()
                } else {
                    None
                }
            });
            insert_string_if_missing(&mut normalized, "lorebook_id", inferred_lorebook_id);

            let inferred_content =
                first_string_argument(&normalized, &["description", "body", "text"]);
            insert_string_if_missing(&mut normalized, "content", inferred_content);
        }
        "get_lorebook_entry" | "delete_lorebook_entry" => {
            let inferred_entry_id = first_string_argument(&normalized, &["id"]);
            insert_string_if_missing(&mut normalized, "entry_id", inferred_entry_id);
        }
        "list_character_lorebooks" | "set_character_lorebooks" => {
            let inferred_character_id = first_string_argument(&normalized, &["id"]).or_else(|| {
                if session.target_type == Some(CreationGoal::Character) {
                    session.target_id.clone()
                } else {
                    None
                }
            });
            insert_string_if_missing(&mut normalized, "character_id", inferred_character_id);
        }
        _ => {}
    }

    Value::Object(normalized)
}

fn tool_choice_requires_auto(error: &str) -> bool {
    let lower = error.to_ascii_lowercase();
    lower.contains("tool choice must be auto")
        || lower.contains("tool_choice must be auto")
        || (lower.contains("tool choice") && lower.contains("auto"))
}

fn tool_config_with_auto_choice(tool_config: &ToolConfig) -> ToolConfig {
    let mut cloned = tool_config.clone();
    cloned.choice = Some(ToolChoice::Auto);
    cloned
}

async fn send_creation_api_request(
    app: &AppHandle,
    session_id: &str,
    stream_request_id: &str,
    provider_id: &str,
    cred: &crate::chat_manager::types::ProviderCredential,
    api_key: &str,
    model_name: &str,
    messages: &Vec<Value>,
    streaming_enabled: bool,
    tool_config: Option<&ToolConfig>,
) -> Result<ApiResponse, String> {
    let mut current_tool_config = tool_config.cloned();

    loop {
        let built = build_chat_request(
            cred,
            api_key,
            model_name,
            messages,
            None,
            Some(0.7),
            Some(1.0),
            20480,
            None,
            streaming_enabled,
            Some(stream_request_id.to_string()),
            None,
            None,
            None,
            current_tool_config.as_ref(),
            false,
            None,
            None,
            false,
            None,
        );

        log_info(
            app,
            "creation_helper",
            format!("Sending request to {} with model {}", built.url, model_name),
        );

        let api_request_payload = ApiRequest {
            url: built.url,
            method: Some("POST".into()),
            headers: Some(built.headers),
            query: None,
            body: Some(built.body),
            timeout_ms: Some(crate::transport::DEFAULT_REQUEST_TIMEOUT_MS),
            stream: Some(built.stream),
            request_id: built.request_id.clone(),
            provider_id: Some(provider_id.to_string()),
        };

        let mut abort_rx = {
            let registry = app.state::<AbortRegistry>();
            registry.register(session_id.to_string())
        };

        let api_response = tokio::select! {
            _ = &mut abort_rx => {
                log_warn(
                    app,
                    "creation_helper",
                    format!("[creation_helper] request aborted by user for session {}", session_id),
                );
                return Err(crate::utils::err_msg(module_path!(), line!(), "Request aborted by user"));
            }
            res = api_request(app.clone(), api_request_payload) => res?
        };

        {
            let registry = app.state::<AbortRegistry>();
            registry.unregister(session_id);
        }

        if !api_response.ok {
            let err_message = api_response
                .data()
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("API request failed")
                .to_string();

            if let Some(cfg) = current_tool_config.as_ref() {
                if !matches!(cfg.choice, None | Some(ToolChoice::Auto))
                    && tool_choice_requires_auto(&err_message)
                {
                    log_warn(
                        app,
                        "creation_helper",
                        format!(
                            "Provider rejected forced tool choice; retrying creation helper request with auto tool choice. Provider={}, model={}",
                            provider_id, model_name
                        ),
                    );
                    current_tool_config = Some(tool_config_with_auto_choice(cfg));
                    continue;
                }
            }
        }

        return Ok(api_response);
    }
}

fn record_image_generation_usage(
    app: &AppHandle,
    session_id: &str,
    model_id: &str,
    model_name: &str,
    provider_id: &str,
    provider_label: &str,
    character_name: &str,
    success: bool,
    error_message: Option<String>,
) {
    let request_id = Uuid::new_v4().to_string();
    let mut metadata = HashMap::new();
    metadata.insert("image_generation".to_string(), "true".to_string());
    metadata.insert("tool".to_string(), "generate_image".to_string());

    let usage = RequestUsage {
        id: request_id,
        timestamp: now_ms() as u64,
        session_id: session_id.to_string(),
        character_id: "creation_helper".to_string(),
        character_name: if character_name.is_empty() {
            "New Character".to_string()
        } else {
            character_name.to_string()
        },
        model_id: model_id.to_string(),
        model_name: model_name.to_string(),
        provider_id: provider_id.to_string(),
        provider_label: provider_label.to_string(),
        operation_type: UsageOperationType::AICreator,
        finish_reason: None,
        prompt_tokens: None,
        completion_tokens: None,
        total_tokens: None,
        cached_prompt_tokens: None,
        cache_write_tokens: None,
        memory_tokens: None,
        summary_tokens: None,
        reasoning_tokens: None,
        image_tokens: None,
        web_search_requests: None,
        api_cost: None,
        cost: None,
        success,
        error_message,
        metadata,
    };

    if let Err(e) = add_usage_record(app, usage) {
        log_error(
            app,
            "creation_helper",
            format!("Failed to record image generation usage: {}", e),
        );
    }
}

async fn execute_tool(
    app: &AppHandle,
    session: &mut CreationSession,
    tool_call_id: &str,
    tool_name: &str,
    arguments: &Value,
) -> CreationToolResult {
    let normalized_arguments = normalize_tool_arguments(session, tool_name, arguments);
    if normalized_arguments != *arguments {
        log_info(
            app,
            "creation_helper",
            format!(
                "Normalized tool args for {}: original={} normalized={}",
                tool_name, arguments, normalized_arguments
            ),
        );
    }
    let arguments = &normalized_arguments;

    log_info(
        app,
        "creation_helper",
        format!(
            "Executing tool: {} with id: {} args: {}",
            tool_name, tool_call_id, arguments
        ),
    );

    let result = match tool_name {
        "set_character_name" => {
            if let Some(name) = arguments.get("name").and_then(|v| v.as_str()) {
                session.draft.name = Some(name.to_string());
                json!({ "success": true, "message": format!("Name set to '{}'", name) })
            } else {
                json!({ "success": false, "error": "Missing 'name' argument" })
            }
        }
        "set_character_definition" | "set_character_description" => {
            let value = arguments
                .get("definition")
                .or_else(|| arguments.get("description"))
                .and_then(|v| v.as_str());
            if let Some(def) = value {
                session.draft.definition = Some(def.to_string());
                json!({ "success": true, "message": "Definition updated" })
            } else {
                json!({ "success": false, "error": "Missing 'definition' argument" })
            }
        }
        "add_scene" => {
            if let Some(content) = arguments.get("content").and_then(|v| v.as_str()) {
                let scene_id = Uuid::new_v4().to_string();
                let direction = arguments
                    .get("direction")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let scene = DraftScene {
                    id: scene_id.clone(),
                    content: content.to_string(),
                    direction,
                };
                session.draft.scenes.push(scene);

                if session.draft.default_scene_id.is_none() {
                    session.draft.default_scene_id = Some(scene_id.clone());
                }

                json!({ "success": true, "scene_id": scene_id, "message": "Scene added" })
            } else {
                json!({ "success": false, "error": "Missing 'content' argument" })
            }
        }
        "update_scene" => {
            let scene_id = arguments.get("scene_id").and_then(|v| v.as_str());
            let content = arguments.get("content").and_then(|v| v.as_str());

            if let (Some(scene_id), Some(content)) = (scene_id, content) {
                if let Some(scene) = session.draft.scenes.iter_mut().find(|s| s.id == scene_id) {
                    scene.content = content.to_string();
                    if let Some(dir) = arguments.get("direction").and_then(|v| v.as_str()) {
                        scene.direction = Some(dir.to_string());
                    }
                    json!({ "success": true, "message": "Scene updated" })
                } else {
                    json!({ "success": false, "error": "Scene not found" })
                }
            } else {
                json!({ "success": false, "error": "Missing required arguments" })
            }
        }
        "toggle_avatar_gradient" => {
            if let Some(enabled) = arguments.get("enabled").and_then(|v| v.as_bool()) {
                session.draft.disable_avatar_gradient = !enabled;
                json!({ "success": true, "message": format!("Avatar gradient {}", if enabled { "enabled" } else { "disabled" }) })
            } else {
                json!({ "success": false, "error": "Missing 'enabled' argument" })
            }
        }
        "set_default_model" => {
            if let Some(model_id) = arguments.get("model_id").and_then(|v| v.as_str()) {
                session.draft.default_model_id = Some(model_id.to_string());
                json!({ "success": true, "message": "Default model set" })
            } else {
                json!({ "success": false, "error": "Missing 'model_id' argument" })
            }
        }
        "set_system_prompt" => {
            if let Some(prompt_id) = arguments.get("prompt_id").and_then(|v| v.as_str()) {
                session.draft.prompt_template_id = Some(prompt_id.to_string());
                json!({ "success": true, "message": "System prompt set" })
            } else {
                json!({ "success": false, "error": "Missing 'prompt_id' argument" })
            }
        }
        "get_system_prompt_list" => match get_system_prompts(app) {
            Ok(prompts) => json!({ "success": true, "prompts": prompts }),
            Err(e) => json!({ "success": false, "error": e }),
        },
        "get_model_list" => match get_models(app) {
            Ok(models) => json!({ "success": true, "models": models }),
            Err(e) => json!({ "success": false, "error": e }),
        },
        "use_uploaded_image_as_avatar" => {
            if let Some(image_id) = arguments.get("image_id").and_then(|v| v.as_str()) {
                match resolve_uploaded_image_id(&session.id, image_id) {
                    Ok(Some(resolved_id)) => {
                        session.draft.avatar_path = Some(resolved_id);
                        json!({ "success": true, "message": "Avatar set from uploaded image" })
                    }
                    Ok(None) => json!({ "success": false, "error": "Image not found" }),
                    Err(e) => json!({ "success": false, "error": e }),
                }
            } else {
                json!({ "success": false, "error": "Missing 'image_id' argument" })
            }
        }
        "use_uploaded_image_as_chat_background" => {
            if let Some(image_id) = arguments.get("image_id").and_then(|v| v.as_str()) {
                // Verify image exists but store only the ID
                match resolve_uploaded_image_id(&session.id, image_id) {
                    Ok(Some(resolved_id)) => {
                        session.draft.background_image_path = Some(resolved_id);
                        json!({ "success": true, "message": "Background set from uploaded image" })
                    }
                    Ok(None) => json!({ "success": false, "error": "Image not found" }),
                    Err(e) => json!({ "success": false, "error": e }),
                }
            } else {
                json!({ "success": false, "error": "Missing 'image_id' argument" })
            }
        }
        "generate_image" => {
            let prompt = arguments.get("prompt").and_then(|v| v.as_str());
            if let Some(prompt) = prompt {
                match build_image_request(app, prompt, arguments) {
                    Ok((request, meta)) => match generate_image(app.clone(), request).await {
                        Ok(response) => {
                            if let Some(image) = response.images.first() {
                                let image_id = short_image_id();
                                if let Err(err) = save_uploaded_image_asset_ref(
                                    &session.id,
                                    image_id.clone(),
                                    image.asset_id.clone(),
                                    image.mime_type.clone(),
                                ) {
                                    record_image_generation_usage(
                                        app,
                                        &session.id,
                                        &meta.model_id,
                                        &meta.model_name,
                                        &meta.provider_id,
                                        &meta.provider_label,
                                        session.draft.name.as_deref().unwrap_or(""),
                                        false,
                                        Some(err.clone()),
                                    );
                                    json!({ "success": false, "error": err })
                                } else {
                                    let _ = set_last_generated_image(&session.id, &image_id);
                                    record_image_generation_usage(
                                        app,
                                        &session.id,
                                        &meta.model_id,
                                        &meta.model_name,
                                        &meta.provider_id,
                                        &meta.provider_label,
                                        session.draft.name.as_deref().unwrap_or(""),
                                        true,
                                        None,
                                    );
                                    json!({
                                        "success": true,
                                        "image_id": image_id,
                                        "message": "Image generated"
                                    })
                                }
                            } else {
                                record_image_generation_usage(
                                    app,
                                    &session.id,
                                    &meta.model_id,
                                    &meta.model_name,
                                    &meta.provider_id,
                                    &meta.provider_label,
                                    session.draft.name.as_deref().unwrap_or(""),
                                    false,
                                    Some("No image returned".to_string()),
                                );
                                json!({ "success": false, "error": "No image returned" })
                            }
                        }
                        Err(err) => {
                            record_image_generation_usage(
                                app,
                                &session.id,
                                &meta.model_id,
                                &meta.model_name,
                                &meta.provider_id,
                                &meta.provider_label,
                                session.draft.name.as_deref().unwrap_or(""),
                                false,
                                Some(err.clone()),
                            );
                            json!({ "success": false, "error": err })
                        }
                    },
                    Err(err) => json!({ "success": false, "error": err }),
                }
            } else {
                json!({ "success": false, "error": "Missing 'prompt' argument" })
            }
        }
        "show_preview" => {
            session.status = CreationStatus::PreviewShown;
            let message = arguments
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("Here's a preview of what we've built so far!");

            json!({
                "success": true,
                "action": "show_preview",
                "message": message,
                "draft": session.draft
            })
        }
        "request_confirmation" => {
            session.status = CreationStatus::PreviewShown;
            let message = arguments
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("Are you happy with this so far?");

            json!({
                "success": true,
                "action": "request_confirmation",
                "message": message,
                "draft": session.draft
            })
        }
        "list_personas" => match personas_storage::personas_list_typed::<Value>(app) {
            Ok(personas) => json!({ "success": true, "personas": personas }),
            Err(e) => json!({ "success": false, "error": e }),
        },
        "upsert_persona" => {
            let title = arguments.get("title").and_then(|v| v.as_str());
            let description = arguments.get("description").and_then(|v| v.as_str());
            if let (Some(title), Some(description)) = (title, description) {
                let now = now_ms() as i64;
                let persona_json = json!({
                    "id": arguments.get("id").and_then(|v| v.as_str()),
                    "title": title,
                    "description": description,
                    "avatarPath": arguments.get("avatar_path").and_then(|v| v.as_str()),
                    "isDefault": arguments.get("is_default").and_then(|v| v.as_bool()).unwrap_or(false),
                    "createdAt": now,
                    "updatedAt": now,
                });
                match personas_storage::persona_upsert_typed::<_, Value>(app, &persona_json) {
                    Ok(persona) => json!({ "success": true, "persona": persona }),
                    Err(e) => json!({ "success": false, "error": e }),
                }
            } else {
                json!({ "success": false, "error": "Missing required persona fields" })
            }
        }
        "delete_persona" => {
            if let Some(id) = arguments.get("id").and_then(|v| v.as_str()) {
                match personas_storage::persona_delete(app.clone(), id.to_string()) {
                    Ok(()) => json!({ "success": true }),
                    Err(e) => json!({ "success": false, "error": e }),
                }
            } else {
                json!({ "success": false, "error": "Missing 'id' argument" })
            }
        }
        "get_default_persona" => match personas_storage::persona_default_get_typed::<Value>(app) {
            Ok(Some(persona)) => json!({ "success": true, "persona": persona }),
            Ok(None) => json!({ "success": true, "persona": Value::Null }),
            Err(e) => json!({ "success": false, "error": e }),
        },
        "use_uploaded_image_as_persona_avatar" => {
            let persona_id = arguments.get("persona_id").and_then(|v| v.as_str());
            let image_id = arguments.get("image_id").and_then(|v| v.as_str());
            if let (Some(persona_id), Some(image_id)) = (persona_id, image_id) {
                match resolve_uploaded_image_id(&session.id, image_id) {
                    Ok(Some(resolved_id)) => {
                        match get_uploaded_image(app, &session.id, &resolved_id) {
                            Ok(Some(image)) => {
                                match personas_storage::personas_list_typed::<Value>(app) {
                                    Ok(personas) => {
                                        if let Some(persona) = personas.iter().find_map(|p| {
                                            let id = p.get("id")?.as_str()?;
                                            if id != persona_id {
                                                return None;
                                            }
                                            Some(p)
                                        }) {
                                            let title =
                                                persona.get("title").and_then(|v| v.as_str());
                                            let description =
                                                persona.get("description").and_then(|v| v.as_str());
                                            let is_default =
                                                persona.get("isDefault").and_then(|v| v.as_bool());
                                            if let (Some(title), Some(description)) =
                                                (title, description)
                                            {
                                                let persona_json = json!({
                                                    "id": persona_id,
                                                    "title": title,
                                                    "description": description,
                                                    "avatarPath": image.data,
                                                    "isDefault": is_default.unwrap_or(false),
                                                });
                                                match personas_storage::persona_upsert_typed::<
                                                    _,
                                                    Value,
                                                >(
                                                    app, &persona_json
                                                ) {
                                                    Ok(persona) => {
                                                        json!({ "success": true, "persona": persona })
                                                    }
                                                    Err(e) => {
                                                        json!({ "success": false, "error": e })
                                                    }
                                                }
                                            } else {
                                                json!({ "success": false, "error": "Persona missing title or description" })
                                            }
                                        } else {
                                            json!({ "success": false, "error": "Persona not found" })
                                        }
                                    }
                                    Err(e) => json!({ "success": false, "error": e }),
                                }
                            }
                            Ok(None) => json!({ "success": false, "error": "Image not found" }),
                            Err(e) => json!({ "success": false, "error": e }),
                        }
                    }
                    Ok(None) => json!({ "success": false, "error": "Image not found" }),
                    Err(e) => json!({ "success": false, "error": e }),
                }
            } else {
                json!({ "success": false, "error": "Missing 'persona_id' or 'image_id' argument" })
            }
        }
        "list_lorebooks" => match lorebook_storage::lorebooks_list(app.clone()) {
            Ok(raw) => match serde_json::from_str::<Value>(&raw) {
                Ok(lorebooks) => json!({ "success": true, "lorebooks": lorebooks }),
                Err(e) => json!({ "success": false, "error": e.to_string() }),
            },
            Err(e) => json!({ "success": false, "error": e }),
        },
        "upsert_lorebook" => {
            if let Some(name) = arguments.get("name").and_then(|v| v.as_str()) {
                let now = now_ms() as i64;
                let lorebook_id = arguments
                    .get("id")
                    .and_then(|v| v.as_str())
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| Uuid::new_v4().to_string());
                let lorebook_json = json!({
                    "id": lorebook_id,
                    "name": name,
                    "createdAt": now,
                    "updatedAt": now,
                });
                match lorebook_storage::lorebook_upsert(app.clone(), lorebook_json.to_string()) {
                    Ok(raw) => match serde_json::from_str::<Value>(&raw) {
                        Ok(lorebook) => json!({ "success": true, "lorebook": lorebook }),
                        Err(e) => json!({ "success": false, "error": e.to_string() }),
                    },
                    Err(e) => json!({ "success": false, "error": e }),
                }
            } else {
                json!({ "success": false, "error": "Missing 'name' argument" })
            }
        }
        "delete_lorebook" => {
            if let Some(id) = arguments.get("lorebook_id").and_then(|v| v.as_str()) {
                match lorebook_storage::lorebook_delete(app.clone(), id.to_string()) {
                    Ok(()) => json!({ "success": true }),
                    Err(e) => json!({ "success": false, "error": e }),
                }
            } else {
                json!({ "success": false, "error": "Missing 'lorebook_id' argument" })
            }
        }
        "list_lorebook_entries" => {
            if let Some(id) = arguments.get("lorebook_id").and_then(|v| v.as_str()) {
                match lorebook_storage::lorebook_entries_list(app.clone(), id.to_string()) {
                    Ok(raw) => match serde_json::from_str::<Value>(&raw) {
                        Ok(entries) => json!({ "success": true, "entries": entries }),
                        Err(e) => json!({ "success": false, "error": e.to_string() }),
                    },
                    Err(e) => json!({ "success": false, "error": e }),
                }
            } else {
                json!({ "success": false, "error": "Missing 'lorebook_id' argument" })
            }
        }
        "get_lorebook_entry" => {
            if let Some(id) = arguments.get("entry_id").and_then(|v| v.as_str()) {
                match lorebook_storage::lorebook_entry_get(app.clone(), id.to_string()) {
                    Ok(raw) => match serde_json::from_str::<Value>(&raw) {
                        Ok(entry) => json!({ "success": true, "entry": entry }),
                        Err(e) => json!({ "success": false, "error": e.to_string() }),
                    },
                    Err(e) => json!({ "success": false, "error": e }),
                }
            } else {
                json!({ "success": false, "error": "Missing 'entry_id' argument" })
            }
        }
        "upsert_lorebook_entry" => {
            let lorebook_id = arguments.get("lorebook_id").and_then(|v| v.as_str());
            let title = arguments.get("title").and_then(|v| v.as_str());
            let content = arguments.get("content").and_then(|v| v.as_str());
            if let (Some(lorebook_id), Some(title), Some(content)) = (lorebook_id, title, content) {
                let now = now_ms() as i64;
                let entry_id = arguments
                    .get("id")
                    .and_then(|v| v.as_str())
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| Uuid::new_v4().to_string());
                let keywords: Vec<String> = arguments
                    .get("keywords")
                    .and_then(|v| v.as_array())
                    .map(|values| {
                        values
                            .iter()
                            .filter_map(|value| value.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                let entry_json = json!({
                    "id": entry_id,
                    "lorebookId": lorebook_id,
                    "title": title,
                    "enabled": arguments.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true),
                    "alwaysActive": arguments.get("always_active").and_then(|v| v.as_bool()).unwrap_or(false),
                    "keywords": keywords,
                    "caseSensitive": arguments.get("case_sensitive").and_then(|v| v.as_bool()).unwrap_or(false),
                    "content": content,
                    "priority": arguments.get("priority").and_then(|v| v.as_i64()).unwrap_or(0),
                    "displayOrder": arguments.get("display_order").and_then(|v| v.as_i64()).unwrap_or(0),
                    "createdAt": now,
                    "updatedAt": now,
                });
                match lorebook_storage::lorebook_entry_upsert(app.clone(), entry_json.to_string()) {
                    Ok(raw) => match serde_json::from_str::<Value>(&raw) {
                        Ok(entry) => json!({ "success": true, "entry": entry }),
                        Err(e) => json!({ "success": false, "error": e.to_string() }),
                    },
                    Err(e) => json!({ "success": false, "error": e }),
                }
            } else {
                json!({ "success": false, "error": "Missing required lorebook entry fields" })
            }
        }
        "delete_lorebook_entry" => {
            if let Some(id) = arguments.get("entry_id").and_then(|v| v.as_str()) {
                match lorebook_storage::lorebook_entry_delete(app.clone(), id.to_string()) {
                    Ok(()) => json!({ "success": true }),
                    Err(e) => json!({ "success": false, "error": e }),
                }
            } else {
                json!({ "success": false, "error": "Missing 'entry_id' argument" })
            }
        }
        "create_blank_lorebook_entry" => {
            if let Some(id) = arguments.get("lorebook_id").and_then(|v| v.as_str()) {
                match lorebook_storage::lorebook_entry_create_blank(app.clone(), id.to_string()) {
                    Ok(raw) => match serde_json::from_str::<Value>(&raw) {
                        Ok(entry) => json!({ "success": true, "entry": entry }),
                        Err(e) => json!({ "success": false, "error": e.to_string() }),
                    },
                    Err(e) => json!({ "success": false, "error": e }),
                }
            } else {
                json!({ "success": false, "error": "Missing 'lorebook_id' argument" })
            }
        }
        "reorder_lorebook_entries" => {
            if let Some(updates) = arguments.get("updates").and_then(|v| v.as_array()) {
                let mapped: Vec<(String, i32)> = updates
                    .iter()
                    .filter_map(|entry| {
                        let entry_id = entry.get("entry_id").and_then(|v| v.as_str())?;
                        let display_order = entry.get("display_order").and_then(|v| v.as_i64())?;
                        Some((entry_id.to_string(), display_order as i32))
                    })
                    .collect();
                match lorebook_storage::lorebook_entries_reorder(
                    app.clone(),
                    serde_json::to_string(&mapped).unwrap_or_default(),
                ) {
                    Ok(()) => json!({ "success": true }),
                    Err(e) => json!({ "success": false, "error": e }),
                }
            } else {
                json!({ "success": false, "error": "Missing 'updates' argument" })
            }
        }
        "list_character_lorebooks" => {
            if let Some(id) = arguments.get("character_id").and_then(|v| v.as_str()) {
                match lorebook_storage::character_lorebooks_list(app.clone(), id.to_string()) {
                    Ok(raw) => match serde_json::from_str::<Value>(&raw) {
                        Ok(lorebooks) => json!({ "success": true, "lorebooks": lorebooks }),
                        Err(e) => json!({ "success": false, "error": e.to_string() }),
                    },
                    Err(e) => json!({ "success": false, "error": e }),
                }
            } else {
                json!({ "success": false, "error": "Missing 'character_id' argument" })
            }
        }
        "set_character_lorebooks" => {
            let character_id = arguments.get("character_id").and_then(|v| v.as_str());
            let lorebook_ids = arguments.get("lorebook_ids").and_then(|v| v.as_array());
            if let (Some(character_id), Some(lorebook_ids)) = (character_id, lorebook_ids) {
                let ids: Vec<String> = lorebook_ids
                    .iter()
                    .filter_map(|v| v.as_str().map(|id| id.to_string()))
                    .collect();
                match lorebook_storage::character_lorebooks_set(
                    app.clone(),
                    character_id.to_string(),
                    serde_json::to_string(&ids).unwrap_or_default(),
                ) {
                    Ok(()) => json!({ "success": true }),
                    Err(e) => json!({ "success": false, "error": e }),
                }
            } else {
                json!({ "success": false, "error": "Missing character or lorebook IDs" })
            }
        }
        _ => {
            json!({ "success": false, "error": format!("Unknown tool: {}", tool_name) })
        }
    };

    let success = result
        .get("success")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    CreationToolResult {
        tool_call_id: tool_call_id.to_string(),
        result,
        success,
    }
}

fn get_system_prompts(app: &AppHandle) -> Result<Vec<SystemPromptInfo>, String> {
    let conn = open_db(app)?;
    let mut stmt = conn
        .prepare("SELECT id, name FROM prompt_templates ORDER BY created_at DESC")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let prompt_iter = stmt
        .query_map([], |r| {
            Ok(SystemPromptInfo {
                id: r.get(0)?,
                name: r.get(1)?,
            })
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let mut prompts = Vec::new();
    for prompt in prompt_iter {
        prompts.push(prompt.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?);
    }

    Ok(prompts)
}

fn get_models(app: &AppHandle) -> Result<Vec<ModelInfo>, String> {
    let settings_json = internal_read_settings(app)?;
    if let Some(json_str) = settings_json {
        let settings: Value = serde_json::from_str(&json_str)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        if let Some(models) = settings.get("models").and_then(|v| v.as_array()) {
            let model_infos: Vec<ModelInfo> = models
                .iter()
                .filter_map(|m| {
                    Some(ModelInfo {
                        id: m.get("id")?.as_str()?.to_string(),
                        name: m.get("name")?.as_str()?.to_string(),
                        display_name: m.get("displayName")?.as_str()?.to_string(),
                    })
                })
                .collect();
            return Ok(model_infos);
        }
    }
    Ok(vec![])
}

fn short_image_id() -> String {
    let compact = Uuid::new_v4().to_string().replace('-', "");
    compact.chars().take(8).collect()
}

struct ImageGenerationMeta {
    model_id: String,
    model_name: String,
    provider_id: String,
    provider_label: String,
}

fn build_image_request(
    app: &AppHandle,
    prompt: &str,
    arguments: &Value,
) -> Result<(ImageGenerationRequest, ImageGenerationMeta), String> {
    let settings_json =
        internal_read_settings(app)?.ok_or_else(|| "No settings found".to_string())?;
    let settings: Value = serde_json::from_str(&settings_json)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let advanced = settings.get("advancedSettings");
    let image_model_id = advanced
        .and_then(|a| a.get("creationHelperImageModelId"))
        .and_then(|v| v.as_str());

    let models = settings
        .get("models")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "No models configured".to_string())?;

    let image_models: Vec<&Value> = models
        .iter()
        .filter(|model| {
            model
                .get("outputScopes")
                .and_then(|v| v.as_array())
                .map(|scopes| scopes.iter().any(|s| s.as_str() == Some("image")))
                .unwrap_or(false)
        })
        .collect();

    let model = if let Some(id) = image_model_id {
        image_models
            .iter()
            .find(|m| m.get("id").and_then(|v| v.as_str()) == Some(id))
            .copied()
    } else {
        image_models.first().copied()
    }
    .ok_or_else(|| "No image generation model configured".to_string())?;

    let model_id = model
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Image model ID missing".to_string())?;
    let model_name = model
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Image model name missing".to_string())?;
    let provider_id = model
        .get("providerId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Image model provider missing".to_string())?;
    let provider_label = model.get("providerLabel").and_then(|v| v.as_str());

    let credentials = settings
        .get("providerCredentials")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "No provider credentials configured".to_string())?;
    let credential = credentials
        .iter()
        .find(|cred| {
            let matches_label = provider_label
                .map(|label| cred.get("label").and_then(|v| v.as_str()) == Some(label))
                .unwrap_or(true);
            cred.get("providerId").and_then(|v| v.as_str()) == Some(provider_id) && matches_label
        })
        .or_else(|| {
            credentials
                .iter()
                .find(|cred| cred.get("providerId").and_then(|v| v.as_str()) == Some(provider_id))
        })
        .ok_or_else(|| "No credentials found for image model provider".to_string())?;

    let credential_id = credential
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Credential ID missing".to_string())?;

    let provider_label_value = provider_label.unwrap_or(provider_id);
    Ok((
        ImageGenerationRequest {
            prompt: prompt.to_string(),
            model: model_name.to_string(),
            provider_id: provider_id.to_string(),
            credential_id: credential_id.to_string(),
            advanced_model_settings: None,
            input_images: None,
            size: arguments
                .get("size")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .or_else(|| Some("1024x1024".to_string())),
            quality: arguments
                .get("quality")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            style: arguments
                .get("style")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            n: Some(1),
        },
        ImageGenerationMeta {
            model_id: model_id.to_string(),
            model_name: model_name.to_string(),
            provider_id: provider_id.to_string(),
            provider_label: provider_label_value.to_string(),
        },
    ))
}

fn should_force_post_tool_summary(initial_content: &str, final_content: &str) -> bool {
    let final_trim = final_content.trim();
    if final_trim.is_empty() {
        return true;
    }
    if final_trim.len() < 80 {
        return true;
    }
    let initial_trim = initial_content.trim();
    if !initial_trim.is_empty() && final_trim == initial_trim {
        return true;
    }
    false
}

fn extract_provider_semantic_error(provider_id: &str, data: &Value) -> Option<String> {
    if data.get("error").is_some() {
        return chat_request::extract_error_message(data);
    }

    let is_gemini = provider_id.eq_ignore_ascii_case("gemini") || provider_id.starts_with("google");
    if !is_gemini {
        return None;
    }

    if data
        .get("promptFeedback")
        .and_then(|v| v.get("blockReason"))
        .and_then(|v| v.as_str())
        .is_some()
    {
        return chat_request::extract_error_message(data);
    }

    let has_bad_finish_reason = data
        .get("candidates")
        .and_then(|v| v.as_array())
        .map(|candidates| {
            candidates.iter().any(|candidate| {
                matches!(
                    candidate.get("finishReason").and_then(|v| v.as_str()),
                    Some(reason)
                        if !matches!(
                            reason,
                            "STOP" | "MAX_TOKENS" | "FINISH_REASON_UNSPECIFIED"
                        )
                )
            })
        })
        .unwrap_or(false);

    if has_bad_finish_reason {
        return chat_request::extract_error_message(data)
            .or_else(|| Some("Gemini returned a blocked/invalid finish reason".to_string()));
    }

    None
}

fn extract_latest_gemini_function_call_content(data: &Value) -> Option<Value> {
    let mut latest: Option<Value> = None;

    let mut inspect_payload = |payload: &Value| {
        let Some(candidates) = payload.get("candidates").and_then(|v| v.as_array()) else {
            return;
        };
        for candidate in candidates {
            let Some(content) = candidate.get("content").and_then(|v| v.as_object()) else {
                continue;
            };
            let Some(parts) = content.get("parts").and_then(|v| v.as_array()) else {
                continue;
            };
            let has_function_call = parts.iter().any(|part| {
                part.get("functionCall").is_some() || part.get("function_call").is_some()
            });
            if has_function_call {
                latest = Some(json!({
                    "role": content
                        .get("role")
                        .and_then(|v| v.as_str())
                        .unwrap_or("model"),
                    "parts": parts
                }));
            }
        }
    };

    if let Some(raw) = data.as_str() {
        for line in raw.lines() {
            let trimmed = line.trim();
            if !trimmed.starts_with("data:") {
                continue;
            }
            let payload = trimmed[5..].trim();
            if payload.is_empty() || payload == "[DONE]" {
                continue;
            }
            let Ok(v) = serde_json::from_str::<Value>(payload) else {
                continue;
            };
            inspect_payload(&v);
        }
        return latest;
    }

    inspect_payload(data);
    latest
}

pub async fn send_message(
    app: AppHandle,
    session_id: String,
    user_message: String,
    uploaded_images: Option<Vec<(String, String, String)>>, // (id, data, mime_type)
    request_id: Option<String>,
) -> Result<CreationSession, String> {
    let now = now_ms() as i64;

    let mut session =
        get_session(&app, &session_id)?.ok_or_else(|| "Session not found".to_string())?;

    if let Some(images) = uploaded_images {
        for (id, data, mime_type) in images {
            save_uploaded_image(&app, &session_id, id, data, mime_type)?;
        }
    }

    let user_msg = CreationMessage {
        id: Uuid::new_v4().to_string(),
        role: CreationMessageRole::User,
        content: user_message.clone(),
        tool_calls: vec![],
        tool_results: vec![],
        created_at: now,
    };
    session.messages.push(user_msg);

    // Save state before assistant turn
    session.draft_history.push(session.draft.clone());
    if session.draft_history.len() > 20 {
        session.draft_history.remove(0);
    }

    process_assistant_turn(app, session_id, session, request_id).await
}

pub async fn regenerate_response(
    app: AppHandle,
    session_id: String,
    request_id: Option<String>,
) -> Result<CreationSession, String> {
    let mut session =
        get_session(&app, &session_id)?.ok_or_else(|| "Session not found".to_string())?;

    // Find last assistant message
    let last_assistant_idx = session
        .messages
        .iter()
        .rposition(|m| m.role == CreationMessageRole::Assistant);

    if let Some(idx) = last_assistant_idx {
        // Remove it
        session.messages.remove(idx);

        // Restore draft state if available
        if let Some(prev_draft) = session.draft_history.pop() {
            session.draft = prev_draft;
        }

        // Save state again before we start the new turn
        session.draft_history.push(session.draft.clone());
        if session.draft_history.len() > 20 {
            session.draft_history.remove(0);
        }

        process_assistant_turn(app, session_id, session, request_id).await
    } else {
        Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "No assistant message to regenerate",
        ))
    }
}

async fn process_assistant_turn(
    app: AppHandle,
    session_id: String,
    mut session: CreationSession,
    request_id: Option<String>,
) -> Result<CreationSession, String> {
    let settings_json =
        internal_read_settings(&app)?.ok_or_else(|| "No settings found".to_string())?;
    let settings: Value = serde_json::from_str(&settings_json)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let advanced_settings = settings.get("advancedSettings");

    let model_id = advanced_settings
        .and_then(|a| a.get("creationHelperModelId"))
        .and_then(|v| v.as_str())
        .or_else(|| settings.get("defaultModelId").and_then(|v| v.as_str()))
        .ok_or_else(|| "No model configured".to_string())?;

    let streaming_enabled = advanced_settings
        .and_then(|a| a.get("creationHelperStreaming"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let stream_request_id =
        request_id.unwrap_or_else(|| format!("creation-helper-{}-{}", session_id, Uuid::new_v4()));

    let models = settings.get("models").and_then(|v| v.as_array());
    let model = models
        .and_then(|m| {
            m.iter()
                .find(|model| model.get("id").and_then(|v| v.as_str()) == Some(model_id))
        })
        .ok_or_else(|| "Model not found".to_string())?;

    let provider_id = model
        .get("providerId")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let model_name = model.get("name").and_then(|v| v.as_str()).unwrap_or("");

    let credentials = settings
        .get("providerCredentials")
        .and_then(|v| v.as_array());
    let credential = credentials
        .and_then(|c| {
            c.iter()
                .find(|cred| cred.get("providerId").and_then(|v| v.as_str()) == Some(provider_id))
        })
        .ok_or_else(|| "No credentials found for provider".to_string())?;

    let api_key = credential
        .get("apiKey")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let base_url = credential.get("baseUrl").and_then(|v| v.as_str());

    let provider_label = credential
        .get("label")
        .and_then(|v| v.as_str())
        .unwrap_or(provider_id);

    let smart_tool_selection = advanced_settings
        .and_then(|a| a.get("creationHelperSmartToolSelection"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let enabled_tools: Option<Vec<String>> = advanced_settings
        .and_then(|a| a.get("creationHelperEnabledTools"))
        .and_then(|v| v.as_array())
        .map(|values| {
            values
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        });

    let mut api_messages = vec![json!({
        "role": "system",
        "content": get_creation_helper_system_prompt(
            &session.creation_goal,
            &session.creation_mode,
            session.target_type.as_ref(),
            session.target_id.as_deref(),
            smart_tool_selection
        )
    })];

    for msg in &session.messages {
        let role = match msg.role {
            CreationMessageRole::User => "user",
            CreationMessageRole::Assistant => "assistant",
            CreationMessageRole::System => "system",
        };

        if msg.role == CreationMessageRole::Assistant && !msg.tool_calls.is_empty() {
            let tool_calls_json: Vec<Value> = msg
                .tool_calls
                .iter()
                .enumerate()
                .map(|(index, tc)| {
                    creation_tool_call_payload(provider_id, &tc.id, index, &tc.name, &tc.arguments)
                })
                .collect();

            api_messages.push(json!({
                "role": role,
                "content": if msg.content.is_empty() { Value::Null } else { json!(msg.content) },
                "tool_calls": tool_calls_json
            }));

            for result in &msg.tool_results {
                let tool_name = msg
                    .tool_calls
                    .iter()
                    .find(|call| call.id == result.tool_call_id)
                    .map(|call| call.name.as_str());
                api_messages.push(creation_tool_result_message(
                    provider_id,
                    &result.tool_call_id,
                    tool_name,
                    &result.result,
                ));
            }
        } else {
            api_messages.push(json!({
                "role": role,
                "content": msg.content
            }));
        }
    }

    let initial_turn_plan =
        build_creation_turn_plan(&session, smart_tool_selection, enabled_tools.as_deref());

    let cred = crate::chat_manager::types::ProviderCredential {
        id: credential
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        provider_id: provider_id.to_string(),
        label: credential
            .get("label")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        api_key: Some(api_key.to_string()),
        base_url: base_url.map(|s| s.to_string()),
        default_model: None,
        headers: credential
            .get("headers")
            .cloned()
            .and_then(|value| serde_json::from_value(value).ok()),
        config: credential.get("config").cloned(),
    };

    let streaming_enabled = effective_streaming_enabled(&cred, streaming_enabled);

    log_info(
        &app,
        "creation_helper",
        format!("Streaming enabled: {}", streaming_enabled),
    );

    log_info(
        &app,
        "creation_helper",
        format!(
            "Initial turn plan: stage={:?}, tools={}, choice={:?}",
            initial_turn_plan.stage,
            initial_turn_plan.tool_config.tools.len(),
            initial_turn_plan.tool_config.choice
        ),
    );

    let mut request_messages = api_messages.clone();
    request_messages.push(json!({
        "role": "system",
        "content": initial_turn_plan.guidance.clone()
    }));

    let api_response = send_creation_api_request(
        &app,
        &session_id,
        &stream_request_id,
        provider_id,
        &cred,
        api_key,
        model_name,
        &request_messages,
        streaming_enabled,
        Some(&initial_turn_plan.tool_config),
    )
    .await?;

    if !api_response.ok {
        let full_error = serde_json::to_string_pretty(api_response.data()).unwrap_or_default();
        log_error(
            &app,
            "creation_helper",
            format!("API error response: {}", full_error),
        );
        let err = api_response
            .data()
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
            .unwrap_or("API request failed");
        record_creation_usage(
            &app,
            api_response.data(),
            &session_id,
            model_id,
            model_name,
            provider_id,
            provider_label,
            api_key,
            session.draft.name.as_deref().unwrap_or(""),
            false,
            Some(err.to_string()),
        )
        .await;
        log_error(&app, "creation_helper", format!("API error: {}", err));
        return Err(err.to_string());
    }

    let response_data = api_response.data();
    let initial_content =
        chat_request::extract_text(response_data, Some(provider_id)).unwrap_or_default();
    let mut tool_calls = if response_data.is_string() {
        accumulate_tool_calls_from_sse(response_data.as_str().unwrap(), provider_id)
    } else {
        parse_tool_calls(provider_id, response_data)
    };
    let mut latest_gemini_function_call_content =
        if provider_id.eq_ignore_ascii_case("gemini") || provider_id.starts_with("google") {
            extract_latest_gemini_function_call_content(response_data)
        } else {
            None
        };
    let initial_provider_error = extract_provider_semantic_error(provider_id, response_data);
    let initial_has_meaningful = (!initial_content.trim().is_empty() || !tool_calls.is_empty())
        && initial_provider_error.is_none();
    record_creation_usage(
        &app,
        response_data,
        &session_id,
        model_id,
        model_name,
        provider_id,
        provider_label,
        api_key,
        session.draft.name.as_deref().unwrap_or(""),
        initial_has_meaningful,
        if let Some(err) = &initial_provider_error {
            Some(err.clone())
        } else if initial_has_meaningful {
            None
        } else {
            Some("Model returned empty response".to_string())
        },
    )
    .await;
    log_info(
        &app,
        "creation_helper",
        format!(
            "Parsed initial response: text_len={}, tool_calls={}",
            initial_content.len(),
            tool_calls.len()
        ),
    );
    if !initial_has_meaningful {
        return Err(
            initial_provider_error.unwrap_or_else(|| "Model returned empty response".to_string())
        );
    }
    let initial_content_for_summary = initial_content.clone();
    let mut current_step_content = initial_content;
    let mut final_content = if tool_calls.is_empty() {
        current_step_content.clone()
    } else {
        String::new()
    };

    let mut all_tool_calls = Vec::new();
    let mut all_tool_results = Vec::new();

    let mut iteration = 0;
    const MAX_TOOL_ITERATIONS: i32 = 5;

    while !tool_calls.is_empty() && iteration < MAX_TOOL_ITERATIONS {
        iteration += 1;
        log_info(
            &app,
            "creation_helper",
            format!(
                "Processing {} tool calls (iteration {})",
                tool_calls.len(),
                iteration
            ),
        );

        let current_batch_calls: Vec<CreationToolCall> = tool_calls
            .iter()
            .map(|tc| CreationToolCall {
                id: tc.id.clone(),
                name: tc.name.clone(),
                arguments: tc.arguments.clone(),
            })
            .collect();
        all_tool_calls.extend(current_batch_calls);
        emit_creation_helper_update(
            &app,
            &session_id,
            &session,
            Some(&all_tool_calls),
            Some(&all_tool_results),
        );

        for tc in &tool_calls {
            let result = execute_tool(&app, &mut session, &tc.id, &tc.name, &tc.arguments).await;
            all_tool_results.push(result);
            emit_creation_helper_update(
                &app,
                &session_id,
                &session,
                Some(&all_tool_calls),
                Some(&all_tool_results),
            );
        }

        let tool_calls_json: Vec<Value> = tool_calls
            .iter()
            .enumerate()
            .map(|(index, tc)| {
                creation_tool_call_payload(provider_id, &tc.id, index, &tc.name, &tc.arguments)
            })
            .collect();

        if (provider_id.eq_ignore_ascii_case("gemini") || provider_id.starts_with("google"))
            && latest_gemini_function_call_content.is_some()
        {
            api_messages.push(json!({
                "role": "assistant",
                "content": if current_step_content.is_empty() { Value::Null } else { json!(current_step_content) },
                "gemini_content": latest_gemini_function_call_content.clone().unwrap_or(Value::Null)
            }));
        } else {
            api_messages.push(json!({
                "role": "assistant",
                "content": if current_step_content.is_empty() { Value::Null } else { json!(current_step_content) },
                "tool_calls": tool_calls_json
            }));
        }

        for result in &all_tool_results[all_tool_results.len() - tool_calls.len()..] {
            let tool_name = tool_calls
                .iter()
                .find(|call| call.id == result.tool_call_id)
                .map(|call| call.name.as_str());
            api_messages.push(creation_tool_result_message(
                provider_id,
                &result.tool_call_id,
                tool_name,
                &result.result,
            ));
        }

        log_info(
            &app,
            "creation_helper",
            format!(
                "Sending follow-up request after tool execution (iteration {})",
                iteration
            ),
        );

        let followup_turn_plan =
            build_creation_turn_plan(&session, smart_tool_selection, enabled_tools.as_deref());
        log_info(
            &app,
            "creation_helper",
            format!(
                "Follow-up turn plan: stage={:?}, tools={}, choice={:?}",
                followup_turn_plan.stage,
                followup_turn_plan.tool_config.tools.len(),
                followup_turn_plan.tool_config.choice
            ),
        );

        let mut followup_messages = api_messages.clone();
        followup_messages.push(json!({
            "role": "system",
            "content": followup_turn_plan.guidance.clone()
        }));

        let followup_response = send_creation_api_request(
            &app,
            &session_id,
            &stream_request_id,
            provider_id,
            &cred,
            api_key,
            model_name,
            &followup_messages,
            streaming_enabled,
            Some(&followup_turn_plan.tool_config),
        )
        .await?;

        if !followup_response.ok {
            let full_error =
                serde_json::to_string_pretty(followup_response.data()).unwrap_or_default();
            log_error(
                &app,
                "creation_helper",
                format!("Follow-up API error: {}", full_error),
            );
            let err = followup_response
                .data()
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("API request failed");
            record_creation_usage(
                &app,
                followup_response.data(),
                &session_id,
                model_id,
                model_name,
                provider_id,
                provider_label,
                api_key,
                session.draft.name.as_deref().unwrap_or(""),
                false,
                Some(err.to_string()),
            )
            .await;
            return Err(err.to_string());
        }

        let followup_data = followup_response.data();
        current_step_content =
            chat_request::extract_text(followup_data, Some(provider_id)).unwrap_or_default();
        let parsed_followup_tool_calls = if followup_data.is_string() {
            accumulate_tool_calls_from_sse(followup_data.as_str().unwrap(), provider_id)
        } else {
            parse_tool_calls(provider_id, followup_data)
        };
        let followup_provider_error = extract_provider_semantic_error(provider_id, followup_data);
        latest_gemini_function_call_content =
            if provider_id.eq_ignore_ascii_case("gemini") || provider_id.starts_with("google") {
                extract_latest_gemini_function_call_content(followup_data)
            } else {
                None
            };
        let followup_has_meaningful = (!current_step_content.trim().is_empty()
            || !parsed_followup_tool_calls.is_empty())
            && followup_provider_error.is_none();
        record_creation_usage(
            &app,
            followup_data,
            &session_id,
            model_id,
            model_name,
            provider_id,
            provider_label,
            api_key,
            session.draft.name.as_deref().unwrap_or(""),
            followup_has_meaningful,
            if let Some(err) = &followup_provider_error {
                Some(err.clone())
            } else if followup_has_meaningful {
                None
            } else {
                Some("Model returned empty follow-up response".to_string())
            },
        )
        .await;
        log_info(
            &app,
            "creation_helper",
            format!(
                "Parsed follow-up response (iteration {}): text_len={}, tool_calls={}",
                iteration,
                current_step_content.len(),
                parsed_followup_tool_calls.len()
            ),
        );
        if let Some(err) = followup_provider_error {
            return Err(err);
        }
        if !current_step_content.is_empty() {
            if !final_content.is_empty() {
                if streaming_enabled {
                    crate::transport::emit_normalized(
                        &app,
                        &stream_request_id,
                        crate::chat_manager::types::NormalizedEvent::Delta {
                            text: "\n\n".to_string(),
                        },
                    );
                }
                final_content.push_str("\n\n");
            }
            final_content.push_str(&current_step_content);
        }
        tool_calls = parsed_followup_tool_calls;
    }

    if iteration >= MAX_TOOL_ITERATIONS {
        log_info(
            &app,
            "creation_helper",
            "Max tool iterations reached".to_string(),
        );
    }

    if !all_tool_calls.is_empty()
        && should_force_post_tool_summary(&initial_content_for_summary, &final_content)
    {
        log_info(
            &app,
            "creation_helper",
            "Running finalization pass without tools to get user-facing response".to_string(),
        );

        let mut finalize_messages = api_messages.clone();
        finalize_messages.push(json!({
            "role": "user",
            "content": "Using only the tool outputs above, answer the user's latest request directly with concrete details. Do not call tools."
        }));

        let finalize_built = build_chat_request(
            &cred,
            api_key,
            model_name,
            &finalize_messages,
            None,
            Some(0.7),
            Some(1.0),
            20480,
            None,
            streaming_enabled,
            Some(stream_request_id.clone()),
            None,
            None,
            None,
            None,
            false,
            None,
            None,
            false,
            None,
        );

        let finalize_request = ApiRequest {
            url: finalize_built.url,
            method: Some("POST".into()),
            headers: Some(finalize_built.headers),
            query: None,
            body: Some(finalize_built.body),
            timeout_ms: Some(crate::transport::DEFAULT_REQUEST_TIMEOUT_MS),
            stream: Some(finalize_built.stream),
            request_id: finalize_built.request_id.clone(),
            provider_id: Some(provider_id.to_string()),
        };

        let mut abort_rx = {
            let registry = app.state::<AbortRegistry>();
            registry.register(session_id.clone())
        };

        let finalize_response = tokio::select! {
            _ = &mut abort_rx => {
                log_warn(
                    &app,
                    "creation_helper",
                    format!("[creation_helper] finalization request aborted by user for session {}", session_id),
                );
                return Err(crate::utils::err_msg(module_path!(), line!(), "Request aborted by user"));
            }
            res = api_request(app.clone(), finalize_request) => res?
        };

        {
            let registry = app.state::<AbortRegistry>();
            registry.unregister(&session_id);
        }

        if !finalize_response.ok {
            let full_error =
                serde_json::to_string_pretty(finalize_response.data()).unwrap_or_default();
            log_error(
                &app,
                "creation_helper",
                format!("Finalization API error: {}", full_error),
            );
        } else {
            let finalize_data = finalize_response.data();
            let finalize_provider_error =
                extract_provider_semantic_error(provider_id, finalize_data);
            record_creation_usage(
                &app,
                finalize_data,
                &session_id,
                model_id,
                model_name,
                provider_id,
                provider_label,
                api_key,
                session.draft.name.as_deref().unwrap_or(""),
                finalize_provider_error.is_none(),
                finalize_provider_error.clone(),
            )
            .await;
            let finalize_content =
                chat_request::extract_text(finalize_data, Some(provider_id)).unwrap_or_default();
            let finalize_tool_calls = if finalize_data.is_string() {
                accumulate_tool_calls_from_sse(finalize_data.as_str().unwrap(), provider_id)
            } else {
                parse_tool_calls(provider_id, finalize_data)
            };
            log_info(
                &app,
                "creation_helper",
                format!(
                    "Parsed finalization response: text_len={}, tool_calls={}",
                    finalize_content.len(),
                    finalize_tool_calls.len()
                ),
            );
            if let Some(err) = finalize_provider_error {
                log_warn(
                    &app,
                    "creation_helper",
                    format!("Finalization returned provider error: {}", err),
                );
            }
            if !finalize_content.trim().is_empty() {
                final_content = finalize_content;
            }
        }
    }

    if final_content.trim().is_empty() {
        return Err("Model returned empty response".to_string());
    }

    let assistant_msg = CreationMessage {
        id: Uuid::new_v4().to_string(),
        role: CreationMessageRole::Assistant,
        content: final_content,
        tool_calls: all_tool_calls,
        tool_results: all_tool_results,
        created_at: now_ms() as i64,
    };
    session.messages.push(assistant_msg);
    session.updated_at = now_ms() as i64;

    {
        let mut sessions = SESSIONS
            .lock()
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        sessions.insert(session_id.clone(), session.clone());
    }

    persist_session(&app, &session)?;

    emit_creation_helper_update(&app, &session_id, &session, None, None);

    Ok(session)
}

pub fn cancel_session(app: &AppHandle, session_id: &str) -> Result<(), String> {
    // First, abort any ongoing request via the AbortRegistry
    let registry = app.state::<AbortRegistry>();
    match registry.abort(session_id) {
        Ok(_) => log_info(
            app,
            "creation_helper",
            format!("Aborted request for session {}", session_id),
        ),
        Err(e) => log_warn(
            app,
            "creation_helper",
            format!(
                "No active request to abort for session {}: {}",
                session_id, e
            ),
        ),
    }

    // Keep session resumable; cancel only aborts in-flight generation.
    if let Some(mut session) = get_session(app, session_id)? {
        session.updated_at = now_ms() as i64;
        {
            let mut sessions = SESSIONS
                .lock()
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            sessions.insert(session_id.to_string(), session.clone());
        }
        persist_session(app, &session)?;
    }
    Ok(())
}

pub fn get_draft(app: &AppHandle, session_id: &str) -> Result<Option<DraftCharacter>, String> {
    let session = get_session(app, session_id)?;
    Ok(session.map(|s| s.draft))
}

pub fn complete_session(app: &AppHandle, session_id: &str) -> Result<DraftCharacter, String> {
    let mut session = get_session(app, session_id)?
        .ok_or_else(|| crate::utils::err_msg(module_path!(), line!(), "Session not found"))?;

    session.status = CreationStatus::Completed;
    session.updated_at = now_ms() as i64;

    {
        let mut sessions = SESSIONS
            .lock()
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        sessions.insert(session_id.to_string(), session.clone());
    }
    persist_session(app, &session)?;

    let mut draft = session.draft.clone();

    // Resolve avatar path if it's an ID (not data URI)
    if let Some(ref path) = draft.avatar_path {
        if !path.starts_with("data:") {
            if let Ok(Some(img)) = get_uploaded_image(app, session_id, path) {
                draft.avatar_path = Some(img.data);
            }
        }
    }

    // Resolve background path if it's an ID (not data URI)
    if let Some(ref path) = draft.background_image_path {
        if !path.starts_with("data:") {
            if let Ok(Some(img)) = get_uploaded_image(app, session_id, path) {
                draft.background_image_path = Some(img.data);
            }
        }
    }

    if session.creation_mode == CreationMode::Edit
        && session.target_type == Some(CreationGoal::Character)
        && session.target_id.is_some()
    {
        let target_id = session.target_id.as_deref().unwrap_or_default();
        apply_character_edit(app, target_id, &draft)?;
    } else if session.creation_mode == CreationMode::Edit
        && session.target_type == Some(CreationGoal::Persona)
        && session.target_id.is_some()
    {
        let target_id = session.target_id.as_deref().unwrap_or_default();
        apply_persona_edit(app, target_id, &draft)?;
    } else if session.creation_mode == CreationMode::Edit
        && session.target_type == Some(CreationGoal::Lorebook)
        && session.target_id.is_some()
    {
        let target_id = session.target_id.as_deref().unwrap_or_default();
        apply_lorebook_edit(app, target_id, &draft)?;
    }

    Ok(draft)
}

fn apply_character_edit(
    app: &AppHandle,
    target_id: &str,
    draft: &DraftCharacter,
) -> Result<(), String> {
    let mut characters: Vec<Value> = characters_storage::characters_list_typed(app)?;
    let mut target = characters
        .drain(..)
        .find(|c| c.get("id").and_then(|v| v.as_str()) == Some(target_id))
        .ok_or_else(|| "Character not found".to_string())?;

    target["id"] = json!(target_id);
    target["name"] = json!(draft
        .name
        .clone()
        .or_else(|| {
            target
                .get("name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "Unnamed Character".to_string()));

    if draft.definition.is_some() {
        target["definition"] = json!(draft.definition);
    }
    if draft.description.is_some() {
        target["description"] = json!(draft.description);
    }

    target["scenes"] = json!(draft
        .scenes
        .iter()
        .map(|scene| {
            json!({
                "id": scene.id,
                "content": scene.content,
                "direction": scene.direction
            })
        })
        .collect::<Vec<Value>>());
    target["defaultSceneId"] = json!(draft.default_scene_id);
    target["avatarPath"] = json!(draft.avatar_path);
    target["backgroundImagePath"] = json!(draft.background_image_path);
    target["disableAvatarGradient"] = json!(draft.disable_avatar_gradient);
    target["defaultModelId"] = json!(draft.default_model_id);
    target["promptTemplateId"] = json!(draft.prompt_template_id);

    let _: Value = characters_storage::character_upsert_typed(app, &target)?;
    Ok(())
}

fn apply_persona_edit(
    app: &AppHandle,
    target_id: &str,
    draft: &DraftCharacter,
) -> Result<(), String> {
    let personas: Vec<Value> = personas_storage::personas_list_typed(app)?;
    let target = personas
        .into_iter()
        .find(|p| p.get("id").and_then(|v| v.as_str()) == Some(target_id))
        .ok_or_else(|| "Persona not found".to_string())?;

    let existing_title = target
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Untitled Persona");
    let existing_desc = target
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let is_default = target
        .get("isDefault")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let payload = json!({
        "id": target_id,
        "title": draft.name.as_deref().unwrap_or(existing_title),
        "description": draft
            .description
            .as_deref()
            .or(draft.definition.as_deref())
            .unwrap_or(existing_desc),
        "avatarPath": draft.avatar_path,
        "isDefault": is_default,
    });

    let _: Value = personas_storage::persona_upsert_typed(app, &payload)?;
    Ok(())
}

fn apply_lorebook_edit(
    app: &AppHandle,
    target_id: &str,
    draft: &DraftCharacter,
) -> Result<(), String> {
    let raw = lorebook_storage::lorebooks_list(app.clone())?;
    let lorebooks: Vec<Value> = serde_json::from_str(&raw)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let target = lorebooks
        .into_iter()
        .find(|lb| lb.get("id").and_then(|v| v.as_str()) == Some(target_id))
        .ok_or_else(|| "Lorebook not found".to_string())?;

    let existing_name = target
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("Untitled Lorebook");

    let payload = json!({
        "id": target_id,
        "name": draft.name.as_deref().unwrap_or(existing_name),
    });

    let _ = lorebook_storage::lorebook_upsert(
        app.clone(),
        serde_json::to_string(&payload)
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?,
    )?;
    Ok(())
}

#[allow(dead_code)]
pub fn cleanup_old_sessions(max_age_ms: i64) -> Result<usize, String> {
    let now = now_ms() as i64;
    let mut sessions = SESSIONS
        .lock()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let mut images = UPLOADED_IMAGES
        .lock()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let old_ids: Vec<String> = sessions
        .iter()
        .filter(|(_, s)| now - s.updated_at > max_age_ms)
        .map(|(id, _)| id.clone())
        .collect();

    let count = old_ids.len();
    for id in old_ids {
        sessions.remove(&id);
        images.remove(&id);
    }

    Ok(count)
}

async fn record_creation_usage(
    app: &AppHandle,
    response_data: &Value,
    session_id: &str,
    model_id: &str,
    model_name: &str,
    provider_id: &str,
    provider_label: &str,
    api_key: &str,
    character_name: &str,
    success: bool,
    error_message: Option<String>,
) {
    let usage_summary = chat_request::extract_usage(response_data);
    let request_id = Uuid::new_v4().to_string();

    let mut usage = RequestUsage {
        id: request_id,
        timestamp: now_ms() as u64,
        session_id: session_id.to_string(),
        character_id: "creation_helper".to_string(),
        character_name: if character_name.is_empty() {
            "New Character".to_string()
        } else {
            character_name.to_string()
        },
        model_id: model_id.to_string(),
        model_name: model_name.to_string(),
        provider_id: provider_id.to_string(),
        provider_label: provider_label.to_string(),
        operation_type: UsageOperationType::AICreator,
        finish_reason: usage_summary.as_ref().and_then(|u| {
            u.finish_reason
                .as_ref()
                .and_then(|s| UsageFinishReason::from_str(s))
        }),
        prompt_tokens: usage_summary.as_ref().and_then(|u| u.prompt_tokens),
        completion_tokens: usage_summary.as_ref().and_then(|u| u.completion_tokens),
        total_tokens: usage_summary.as_ref().and_then(|u| u.total_tokens),
        cached_prompt_tokens: usage_summary.as_ref().and_then(|u| u.cached_prompt_tokens),
        cache_write_tokens: usage_summary.as_ref().and_then(|u| u.cache_write_tokens),
        memory_tokens: None,
        summary_tokens: None,
        reasoning_tokens: usage_summary.as_ref().and_then(|u| u.reasoning_tokens),
        image_tokens: usage_summary.as_ref().and_then(|u| u.image_tokens),
        web_search_requests: usage_summary.as_ref().and_then(|u| u.web_search_requests),
        api_cost: usage_summary.as_ref().and_then(|u| u.api_cost),
        cost: None,
        success,
        error_message,
        metadata: HashMap::new(),
    };

    if let Some(summary) = usage_summary.as_ref() {
        if provider_id.eq_ignore_ascii_case("openrouter") {
            apply_openrouter_cost_to_usage(
                app,
                &mut usage,
                summary,
                model_name,
                api_key,
                "creation_helper",
            )
            .await;
        } else if summary.cached_prompt_tokens.is_some()
            || summary.cache_write_tokens.is_some()
            || summary.web_search_requests.is_some()
            || summary.api_cost.is_some()
        {
            insert_extended_usage_metadata(&mut usage.metadata, summary);
        }
    }

    if let Err(e) = add_usage_record(app, usage) {
        log_error(
            app,
            "creation_helper",
            format!("Failed to record usage: {}", e),
        );
    }
}
