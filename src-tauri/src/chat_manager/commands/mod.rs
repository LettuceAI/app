use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::chat_manager::attachments::load_attachment_data;
use crate::chat_manager::execution::{build_provider_extra_fields, RequestSettings};
use crate::chat_manager::messages::{
    push_prompt_entry_message, push_user_or_assistant_message_with_context,
    sanitize_placeholders_in_api_messages,
};
use crate::chat_manager::storage::{
    get_base_prompt, recent_messages, resolve_credential_for_model, PromptType,
};
use crate::chat_manager::turn_builder::{
    append_image_directive_instructions, conversation_window_with_pinned,
    insert_in_chat_prompt_entries, manual_window_size, maybe_swap_message_for_api,
    partition_prompt_entries,
};
use crate::utils::now_millis;

use super::attachments::persist_attachments;
use super::prompt_engine;
use super::prompts;
use super::service::ChatContext;

use super::storage::default_character_rules;
use super::types::{
    ChatAddMessageAttachmentArgs, ChatCompletionArgs, ChatContinueArgs,
    ChatGenerateDesignReferenceDescriptionArgs, ChatGenerateLorebookEntryDraftArgs,
    ChatGenerateSceneImageArgs, ChatGenerateScenePromptArgs, ChatRegenerateArgs, ChatTurnResult,
    ContinueResult, ImageAttachment, LorebookEntryDraftResult, PromptTemplateType,
    RegenerateResult, Session, Settings, StoredMessage, SystemPromptEntry, SystemPromptTemplate,
};
use crate::storage_manager::sessions::{messages_upsert_batch_typed, session_upsert_meta_typed};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessageDebugSnapshotArgs {
    pub session_id: String,
    pub message_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessageDebugSnapshot {
    pub source: String,
    pub session_id: String,
    pub message_id: String,
    pub role: String,
    pub operation: String,
    pub provider_id: String,
    pub credential_id: String,
    pub model_id: String,
    pub model: String,
    pub model_display_name: String,
    pub endpoint: String,
    pub stream: bool,
    pub request_settings: Value,
    pub prompt_template_source: String,
    pub prompt_template_id: Option<String>,
    pub prompt_template_name: Option<String>,
    pub prompt_entries: Vec<SystemPromptEntry>,
    pub relative_prompt_entries: Vec<SystemPromptEntry>,
    pub in_chat_prompt_entries: Vec<SystemPromptEntry>,
    pub request_messages: Vec<Value>,
    pub request_body: Value,
    pub notes: Vec<String>,
}

#[derive(Clone, Copy)]
enum DebugMessageOperation {
    Completion,
    Regenerate,
    Continue,
}

impl DebugMessageOperation {
    fn as_str(self) -> &'static str {
        match self {
            Self::Completion => "completion",
            Self::Regenerate => "regenerate",
            Self::Continue => "continue",
        }
    }
}

fn infer_debug_operation(session: &Session, message_index: usize) -> DebugMessageOperation {
    let message = &session.messages[message_index];
    if !message.variants.is_empty() || message.selected_variant_id.is_some() {
        return DebugMessageOperation::Regenerate;
    }

    let previous_non_scene = session.messages[..message_index]
        .iter()
        .rev()
        .find(|item| item.role == "user" || item.role == "assistant");

    if previous_non_scene
        .map(|item| item.role.eq_ignore_ascii_case("assistant"))
        .unwrap_or(false)
    {
        DebugMessageOperation::Continue
    } else {
        DebugMessageOperation::Completion
    }
}

fn request_settings_json(request_settings: &RequestSettings) -> Value {
    json!({
        "temperature": request_settings.temperature,
        "topP": request_settings.top_p,
        "maxTokens": request_settings.max_tokens,
        "contextLength": request_settings.context_length,
        "frequencyPenalty": request_settings.frequency_penalty,
        "presencePenalty": request_settings.presence_penalty,
        "topK": request_settings.top_k,
        "reasoningEnabled": request_settings.reasoning_enabled,
        "reasoningEffort": request_settings.reasoning_effort,
        "reasoningBudget": request_settings.reasoning_budget,
    })
}

fn resolve_debug_prompt_template(
    app: &AppHandle,
    session: &Session,
    character: &super::types::Character,
    settings: &Settings,
) -> (String, Option<String>, Option<String>) {
    if let Some(session_template_id) = &session.prompt_template_id {
        if let Ok(Some(template)) = prompts::get_template(app, session_template_id) {
            return (
                "session_template".to_string(),
                Some(template.id),
                Some(template.name),
            );
        }
        if let Some(character_template_id) = &character.prompt_template_id {
            if let Ok(Some(template)) = prompts::get_template(app, character_template_id) {
                return (
                    "character_template".to_string(),
                    Some(template.id),
                    Some(template.name),
                );
            }
        }
    } else if let Some(character_template_id) = &character.prompt_template_id {
        if let Ok(Some(template)) = prompts::get_template(app, character_template_id) {
            return (
                "character_template".to_string(),
                Some(template.id),
                Some(template.name),
            );
        }
    }

    if let Some(app_template_id) = &settings.prompt_template_id {
        if let Ok(Some(template)) = prompts::get_template(app, app_template_id) {
            return (
                "app_wide_template".to_string(),
                Some(template.id),
                Some(template.name),
            );
        }
    }

    if let Ok(Some(template)) = prompts::get_template(app, prompts::APP_DEFAULT_TEMPLATE_ID) {
        return (
            "app_default_template".to_string(),
            Some(template.id),
            Some(template.name),
        );
    }

    (
        "emergency_hardcoded_fallback".to_string(),
        None,
        Some("Hardcoded Fallback".to_string()),
    )
}

fn build_debug_completion_messages(
    app: &AppHandle,
    session: &Session,
    settings: &Settings,
    character_name: &str,
    persona_name: &str,
    allow_image_input: bool,
    system_role: &str,
    relative_entries: &[SystemPromptEntry],
    in_chat_entries: &[SystemPromptEntry],
) -> Vec<Value> {
    let mut out = Vec::new();
    for entry in relative_entries {
        push_prompt_entry_message(&mut out, system_role, entry);
    }

    let dynamic_memory_enabled = settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| advanced.dynamic_memory.as_ref())
        .map(|dynamic| dynamic.enabled)
        .unwrap_or(false);

    let (pinned_msgs, recent_msgs) = if dynamic_memory_enabled {
        conversation_window_with_pinned(
            &session.messages,
            crate::chat_manager::memory::dynamic::dynamic_window_size(settings),
        )
    } else {
        (
            Vec::new(),
            recent_messages(session, manual_window_size(settings)),
        )
    };

    let mut chat_messages = Vec::new();
    for msg in &pinned_msgs {
        let msg_with_data = load_attachment_data(app, msg);
        let msg_with_data = maybe_swap_message_for_api(&msg_with_data, false);
        push_user_or_assistant_message_with_context(
            &mut chat_messages,
            &msg_with_data,
            character_name,
            persona_name,
            allow_image_input,
        );
    }

    for msg in &recent_msgs {
        let msg_with_data = load_attachment_data(app, msg);
        let msg_with_data = maybe_swap_message_for_api(&msg_with_data, false);
        push_user_or_assistant_message_with_context(
            &mut chat_messages,
            &msg_with_data,
            character_name,
            persona_name,
            allow_image_input,
        );
    }

    insert_in_chat_prompt_entries(&mut chat_messages, system_role, in_chat_entries);
    out.extend(chat_messages);
    sanitize_placeholders_in_api_messages(&mut out, character_name, persona_name);
    out
}

fn build_debug_regenerate_messages(
    app: &AppHandle,
    session: &Session,
    target_index: usize,
    settings: &Settings,
    character_name: &str,
    persona_name: &str,
    allow_image_input: bool,
    system_role: &str,
    relative_entries: &[SystemPromptEntry],
    in_chat_entries: &[SystemPromptEntry],
) -> Vec<Value> {
    let mut out = Vec::new();
    for entry in relative_entries {
        push_prompt_entry_message(&mut out, system_role, entry);
    }

    let dynamic_memory_enabled = settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| advanced.dynamic_memory.as_ref())
        .map(|dynamic| dynamic.enabled)
        .unwrap_or(false);

    let messages_before_target: Vec<StoredMessage> = session
        .messages
        .iter()
        .enumerate()
        .filter(|(idx, _)| *idx < target_index)
        .map(|(_, message)| message.clone())
        .collect();

    let mut chat_messages = Vec::new();
    if dynamic_memory_enabled {
        let (pinned_msgs, recent_msgs) = conversation_window_with_pinned(
            &messages_before_target,
            crate::chat_manager::memory::dynamic::dynamic_window_size(settings),
        );
        for msg in &pinned_msgs {
            let msg_with_data = load_attachment_data(app, msg);
            let msg_with_data = maybe_swap_message_for_api(&msg_with_data, false);
            push_user_or_assistant_message_with_context(
                &mut chat_messages,
                &msg_with_data,
                character_name,
                persona_name,
                allow_image_input,
            );
        }
        for msg in &recent_msgs {
            let msg_with_data = load_attachment_data(app, msg);
            let msg_with_data = maybe_swap_message_for_api(&msg_with_data, false);
            push_user_or_assistant_message_with_context(
                &mut chat_messages,
                &msg_with_data,
                character_name,
                persona_name,
                allow_image_input,
            );
        }
    } else {
        let start_index = target_index.saturating_sub(manual_window_size(settings));
        for (idx, msg) in session.messages.iter().enumerate() {
            if idx < start_index {
                continue;
            }
            if idx >= target_index {
                break;
            }
            let msg_with_data = load_attachment_data(app, msg);
            let msg_with_data = maybe_swap_message_for_api(&msg_with_data, false);
            push_user_or_assistant_message_with_context(
                &mut chat_messages,
                &msg_with_data,
                character_name,
                persona_name,
                allow_image_input,
            );
        }
    }

    insert_in_chat_prompt_entries(&mut chat_messages, system_role, in_chat_entries);
    out.extend(chat_messages);
    sanitize_placeholders_in_api_messages(&mut out, character_name, persona_name);
    out
}

fn build_debug_continue_messages(
    app: &AppHandle,
    session: &Session,
    settings: &Settings,
    character_name: &str,
    persona_name: &str,
    allow_image_input: bool,
    system_role: &str,
    relative_entries: &[SystemPromptEntry],
    in_chat_entries: &[SystemPromptEntry],
) -> Vec<Value> {
    let mut out = build_debug_completion_messages(
        app,
        session,
        settings,
        character_name,
        persona_name,
        allow_image_input,
        system_role,
        relative_entries,
        in_chat_entries,
    );
    out.push(json!({
        "role": "user",
        "content": "[CONTINUE] You were in the middle of a response. Continue writing from exactly where you left off. Do NOT restart, regenerate, or rewrite what you already said. Simply pick up the narrative thread and continue the scene forward with new content."
    }));
    out
}

fn resolve_persona_id<'a>(session: &'a Session, explicit: Option<&'a str>) -> Option<&'a str> {
    if explicit.is_some() {
        return explicit;
    }
    if session.persona_disabled {
        Some("")
    } else {
        session.persona_id.as_deref()
    }
}

#[allow(dead_code)]
fn has_image_generation_model(settings: &Settings) -> bool {
    settings.models.iter().any(|m| {
        m.output_scopes
            .iter()
            .any(|s| s.eq_ignore_ascii_case("image"))
    })
}

pub(crate) fn take_aborted_request(app: &AppHandle, request_id: Option<&str>) -> bool {
    let Some(request_id) = request_id else {
        return false;
    };

    let registry = app.state::<crate::abort_manager::AbortRegistry>();
    registry.take_aborted(request_id)
}

#[tauri::command]
pub async fn chat_completion(
    app: AppHandle,
    args: ChatCompletionArgs,
) -> Result<ChatTurnResult, String> {
    super::flows::completion::CompletionFlow::new(app)
        .execute(args)
        .await
}

#[tauri::command]
pub async fn chat_regenerate(
    app: AppHandle,
    args: ChatRegenerateArgs,
) -> Result<RegenerateResult, String> {
    super::flows::regenerate::RegenerateFlow::new(app)
        .execute(args)
        .await
}

#[tauri::command]
pub async fn chat_continue(
    app: AppHandle,
    args: ChatContinueArgs,
) -> Result<ContinueResult, String> {
    super::flows::continuation::ContinueFlow::new(app)
        .execute(args)
        .await
}

#[tauri::command]
pub fn chat_message_debug_snapshot(
    app: AppHandle,
    args: ChatMessageDebugSnapshotArgs,
) -> Result<ChatMessageDebugSnapshot, String> {
    let context = ChatContext::initialize(app.clone())?;
    let session = context
        .load_session(&args.session_id)?
        .ok_or_else(|| "Session not found".to_string())?;

    let target_index = session
        .messages
        .iter()
        .position(|message| message.id == args.message_id)
        .ok_or_else(|| "Message not found".to_string())?;
    let target_message = session
        .messages
        .get(target_index)
        .cloned()
        .ok_or_else(|| "Message not found".to_string())?;

    if target_message.role != "assistant" {
        return Err("Debug snapshot currently supports assistant messages only".to_string());
    }

    let character = context.find_character(&session.character_id)?;
    let persona = context
        .choose_persona(resolve_persona_id(&session, None))
        .cloned();

    let (model, credential) = if let Some(model_id) = target_message.model_id.as_ref() {
        let model = context
            .settings
            .models
            .iter()
            .find(|candidate| candidate.id == *model_id)
            .ok_or_else(|| "Stored model not found".to_string())?;
        let credential = resolve_credential_for_model(&context.settings, model)
            .ok_or_else(|| "Provider credential not found".to_string())?;
        (model.clone(), credential.clone())
    } else {
        let (model, credential) = context.select_model_with_credential(&character)?;
        (model.clone(), credential.clone())
    };

    let operation = infer_debug_operation(&session, target_index);
    let prompt_session = match operation {
        DebugMessageOperation::Regenerate => session.clone(),
        DebugMessageOperation::Completion | DebugMessageOperation::Continue => {
            let mut cloned = session.clone();
            cloned.messages.truncate(target_index);
            cloned
        }
    };

    let prompt_entries = append_image_directive_instructions(
        context.build_system_prompt(&character, &model, persona.as_ref(), &prompt_session),
        &context.settings,
    );
    let (prompt_template_source, prompt_template_id, prompt_template_name) =
        resolve_debug_prompt_template(&app, &prompt_session, &character, &context.settings);
    let (relative_entries, in_chat_entries) = partition_prompt_entries(prompt_entries.clone());

    let system_role = crate::chat_manager::request_builder::system_role_for(&credential);
    let character_name = character.name.as_str();
    let persona_name = persona
        .as_ref()
        .map(|item| item.title.as_str())
        .unwrap_or("");
    let allow_image_input = model
        .input_scopes
        .iter()
        .any(|scope| scope.eq_ignore_ascii_case("image"));

    let request_messages = match operation {
        DebugMessageOperation::Completion => build_debug_completion_messages(
            &app,
            &prompt_session,
            &context.settings,
            character_name,
            persona_name,
            allow_image_input,
            &system_role,
            &relative_entries,
            &in_chat_entries,
        ),
        DebugMessageOperation::Regenerate => build_debug_regenerate_messages(
            &app,
            &session,
            target_index,
            &context.settings,
            character_name,
            persona_name,
            allow_image_input,
            &system_role,
            &relative_entries,
            &in_chat_entries,
        ),
        DebugMessageOperation::Continue => build_debug_continue_messages(
            &app,
            &prompt_session,
            &context.settings,
            character_name,
            persona_name,
            allow_image_input,
            &system_role,
            &relative_entries,
            &in_chat_entries,
        ),
    };

    let request_settings = RequestSettings::resolve(&prompt_session, &model, &context.settings);
    let extra_body_fields = build_provider_extra_fields(
        &credential.provider_id,
        &prompt_session,
        &model,
        &context.settings,
        &request_settings,
    );
    let built = crate::chat_manager::request_builder::build_chat_request(
        &credential,
        "",
        &model.name,
        &request_messages,
        None,
        request_settings.temperature,
        request_settings.top_p,
        request_settings.max_tokens,
        request_settings.context_length,
        true,
        None,
        request_settings.frequency_penalty,
        request_settings.presence_penalty,
        request_settings.top_k,
        None,
        request_settings.reasoning_enabled,
        request_settings.reasoning_effort.clone(),
        request_settings.reasoning_budget,
        request_settings.prompt_caching_enabled.unwrap_or(false),
        extra_body_fields,
    );

    let mut notes = vec!["Reconstructed from current session state; live retry timing and exact provider response still come from the in-memory trace.".to_string()];
    if target_message.model_id.is_none() {
        notes.push("This message had no stored model id, so the current character/default model configuration was used for reconstruction.".to_string());
    }
    match operation {
        DebugMessageOperation::Regenerate => notes.push(
            "Regenerate reconstruction uses the current session for prompt rendering and conversation up to the target message for the request body.".to_string(),
        ),
        DebugMessageOperation::Continue => notes.push(
            "Continue reconstruction appends the synthetic [CONTINUE] instruction used by the app.".to_string(),
        ),
        DebugMessageOperation::Completion => {}
    }

    Ok(ChatMessageDebugSnapshot {
        source: "reconstructed".to_string(),
        session_id: args.session_id,
        message_id: args.message_id,
        role: target_message.role,
        operation: operation.as_str().to_string(),
        provider_id: credential.provider_id,
        credential_id: credential.id,
        model_id: model.id,
        model: model.name,
        model_display_name: model.display_name,
        endpoint: built.url,
        stream: built.stream,
        request_settings: request_settings_json(&request_settings),
        prompt_template_source,
        prompt_template_id,
        prompt_template_name,
        prompt_entries,
        relative_prompt_entries: relative_entries,
        in_chat_prompt_entries: in_chat_entries,
        request_messages,
        request_body: built.body,
        notes,
    })
}

#[tauri::command]
pub fn get_default_character_rules(pure_mode_level: String) -> Vec<String> {
    default_character_rules(&pure_mode_level)
}

#[tauri::command]
pub fn get_default_system_prompt_template() -> String {
    get_base_prompt(PromptType::SystemPrompt)
}

// ==================== Prompt Template Commands ====================

#[tauri::command]
pub fn list_prompt_templates(app: AppHandle) -> Result<Vec<SystemPromptTemplate>, String> {
    prompts::load_templates(&app)
}

#[tauri::command]
pub fn create_prompt_template(
    app: AppHandle,
    name: String,
    prompt_type: PromptTemplateType,
    content: String,
    entries: Option<Vec<SystemPromptEntry>>,
    condense_prompt_entries: Option<bool>,
) -> Result<SystemPromptTemplate, String> {
    prompts::create_template(
        &app,
        name,
        prompt_type,
        content,
        entries,
        condense_prompt_entries,
    )
}

#[tauri::command]
pub fn update_prompt_template(
    app: AppHandle,
    id: String,
    name: Option<String>,
    prompt_type: Option<PromptTemplateType>,
    content: Option<String>,
    entries: Option<Vec<SystemPromptEntry>>,
    condense_prompt_entries: Option<bool>,
) -> Result<SystemPromptTemplate, String> {
    prompts::update_template(
        &app,
        id,
        name,
        prompt_type,
        content,
        entries,
        condense_prompt_entries,
    )
}

#[tauri::command]
pub fn delete_prompt_template(app: AppHandle, id: String) -> Result<(), String> {
    prompts::delete_template(&app, id)
}

#[tauri::command]
pub fn get_prompt_template(
    app: AppHandle,
    id: String,
) -> Result<Option<SystemPromptTemplate>, String> {
    prompts::get_template(&app, &id)
}

#[tauri::command]
pub fn export_prompt_template_as_usc(app: AppHandle, id: String) -> Result<String, String> {
    let template =
        prompts::get_template(&app, &id)?.ok_or_else(|| format!("Template not found: {}", id))?;
    let card = crate::storage_manager::system_cards::create_system_prompt_template_usc(&template);
    serde_json::to_string_pretty(&card).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to serialize USC prompt template export: {}", e),
        )
    })
}

#[tauri::command]
pub fn chat_template_export_as_usc(template_json: String) -> Result<String, String> {
    let value: Value = serde_json::from_str(&template_json).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Invalid chat template JSON for export: {}", e),
        )
    })?;

    let id = value
        .get("id")
        .and_then(|item| item.as_str())
        .ok_or_else(|| {
            crate::utils::err_msg(module_path!(), line!(), "Chat template id is required")
        })?
        .to_string();
    let name = value
        .get("name")
        .and_then(|item| item.as_str())
        .ok_or_else(|| {
            crate::utils::err_msg(module_path!(), line!(), "Chat template name is required")
        })?
        .to_string();
    let scene_id = value
        .get("sceneId")
        .and_then(|item| item.as_str())
        .map(|item| item.to_string());
    let prompt_template_id = value
        .get("promptTemplateId")
        .and_then(|item| item.as_str())
        .map(|item| item.to_string());
    let lorebook_ids_override = value
        .get("lorebookIdsOverride")
        .filter(|item| item.is_array())
        .map(|item| item.to_string());
    let created_at = value
        .get("createdAt")
        .and_then(|item| item.as_i64())
        .unwrap_or_else(|| now_millis().unwrap_or(0) as i64);

    let template = crate::sync::models::ChatTemplate {
        id: id.clone(),
        character_id: String::new(),
        name,
        scene_id,
        prompt_template_id,
        lorebook_ids_override,
        created_at,
    };

    let messages = value
        .get("messages")
        .and_then(|item| item.as_array())
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .enumerate()
        .map(|(idx, message)| crate::sync::models::ChatTemplateMessage {
            id: message
                .get("id")
                .and_then(|item| item.as_str())
                .unwrap_or_default()
                .to_string(),
            template_id: id.clone(),
            idx: idx as i64,
            role: message
                .get("role")
                .and_then(|item| item.as_str())
                .unwrap_or("assistant")
                .to_string(),
            content: message
                .get("content")
                .and_then(|item| item.as_str())
                .unwrap_or_default()
                .to_string(),
        })
        .collect::<Vec<_>>();

    let card = crate::storage_manager::system_cards::create_chat_template_usc(&template, &messages);
    serde_json::to_string_pretty(&card).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to serialize USC chat template export: {}", e),
        )
    })
}

#[tauri::command]
pub fn get_app_default_template_id() -> String {
    prompts::APP_DEFAULT_TEMPLATE_ID.to_string()
}

#[tauri::command]
pub fn is_app_default_template(id: String) -> bool {
    prompts::is_app_default_template(&id)
}

#[tauri::command]
pub fn reset_app_default_template(app: AppHandle) -> Result<SystemPromptTemplate, String> {
    prompts::reset_app_default_template(&app)
}

#[tauri::command]
pub fn reset_local_roleplay_template(app: AppHandle) -> Result<SystemPromptTemplate, String> {
    prompts::reset_local_roleplay_template(&app)
}

#[tauri::command]
pub fn reset_dynamic_summary_template(app: AppHandle) -> Result<SystemPromptTemplate, String> {
    prompts::reset_dynamic_summary_template(&app)
}

#[tauri::command]
pub fn reset_dynamic_memory_template(app: AppHandle) -> Result<SystemPromptTemplate, String> {
    prompts::reset_dynamic_memory_template(&app)
}

#[tauri::command]
pub fn reset_dynamic_memory_local_template(app: AppHandle) -> Result<SystemPromptTemplate, String> {
    prompts::reset_dynamic_memory_local_template(&app)
}

#[tauri::command]
pub fn reset_group_chat_template(app: AppHandle) -> Result<SystemPromptTemplate, String> {
    prompts::reset_group_chat_template(&app)
}

#[tauri::command]
pub fn reset_group_chat_roleplay_template(app: AppHandle) -> Result<SystemPromptTemplate, String> {
    prompts::reset_group_chat_roleplay_template(&app)
}

#[tauri::command]
pub fn reset_help_me_reply_template(app: AppHandle) -> Result<SystemPromptTemplate, String> {
    prompts::reset_help_me_reply_template(&app)
}

#[tauri::command]
pub fn reset_help_me_reply_conversational_template(
    app: AppHandle,
) -> Result<SystemPromptTemplate, String> {
    prompts::reset_help_me_reply_conversational_template(&app)
}

#[tauri::command]
pub fn reset_lorebook_entry_writer_template(
    app: AppHandle,
) -> Result<SystemPromptTemplate, String> {
    prompts::reset_lorebook_entry_writer_template(&app)
}

#[tauri::command]
pub fn reset_avatar_generation_template(app: AppHandle) -> Result<SystemPromptTemplate, String> {
    prompts::reset_avatar_generation_template(&app)
}

#[tauri::command]
pub fn reset_avatar_edit_template(app: AppHandle) -> Result<SystemPromptTemplate, String> {
    prompts::reset_avatar_edit_template(&app)
}

#[tauri::command]
pub fn reset_scene_generation_template(app: AppHandle) -> Result<SystemPromptTemplate, String> {
    prompts::reset_scene_generation_template(&app)
}

#[tauri::command]
pub fn reset_scene_prompt_writer_template(app: AppHandle) -> Result<SystemPromptTemplate, String> {
    prompts::reset_scene_prompt_writer_template(&app)
}

#[tauri::command]
pub fn reset_design_reference_template(app: AppHandle) -> Result<SystemPromptTemplate, String> {
    prompts::reset_design_reference_template(&app)
}

#[tauri::command]
pub fn get_required_template_variables(app: AppHandle, template_id: String) -> Vec<String> {
    prompts::get_template(&app, &template_id)
        .ok()
        .flatten()
        .map(|template| {
            crate::chat_manager::prompting::parameter_engine::required_variables_for_prompt_type(
                template.prompt_type,
            )
        })
        .unwrap_or_else(|| prompts::get_required_variables(&template_id))
}

#[tauri::command]
pub fn validate_template_variables(
    app: AppHandle,
    template_id: String,
    content: String,
    entries: Option<Vec<SystemPromptEntry>>,
) -> Result<(), String> {
    let validation_text = if let Some(entries) = entries {
        if entries.is_empty() {
            content
        } else {
            entries
                .iter()
                .map(|entry| entry.content.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        }
    } else {
        content
    };
    let prompt_type = prompts::get_template(&app, &template_id)?
        .map(|template| template.prompt_type)
        .unwrap_or_else(|| prompts::template_prompt_type_from_id(&template_id));
    prompts::validate_required_variables(prompt_type, &validation_text)
        .map_err(|missing| format!("Missing required variables: {}", missing.join(", ")))
}

// Deprecated: get_applicable_prompts_for_* commands removed in favor of global list on client

// ==================== Prompt Preview Command ====================

#[tauri::command]
pub fn render_prompt_preview(
    app: AppHandle,
    content: String,
    character_id: String,
    session_id: Option<String>,
    persona_id: Option<String>,
) -> Result<String, String> {
    let context = super::service::ChatContext::initialize(app.clone())?;
    let settings = &context.settings;

    let character = context.find_character(&character_id)?;

    // Load session if provided, otherwise synthesize a minimal one
    let session: Session = if let Some(sid) = session_id.as_ref() {
        context
            .load_session(sid)
            .and_then(|opt| opt.ok_or_else(|| "Session not found".to_string()))?
    } else {
        // Minimal ephemeral session for preview
        let now = now_millis()?;
        Session {
            id: "preview".to_string(),
            character_id: character.id.clone(),
            title: "Preview".to_string(),
            background_image_path: None,
            system_prompt: None,
            selected_scene_id: None,
            prompt_template_id: None,
            lorebook_ids_override: None,
            author_note: None,
            persona_id: None,
            persona_disabled: false,
            voice_autoplay: None,
            advanced_model_settings: None,
            messages: vec![],
            archived: false,
            created_at: now,
            updated_at: now,
            memory_status: None,
            memory_error: None,
            memory_progress_step: None,
            memories: vec![
                "Memory 1 (Preview): The user prefers direct communication.".to_string(),
                "Memory 2 (Preview): We met in the tavern last night.".to_string(),
            ],
            memory_embeddings: vec![],
            memory_summary: Some("This is a placeholder for the context summary that will be generated by the AI based on your conversation history.".to_string()),
            memory_summary_token_count: 0,
            memory_tool_events: vec![],
        }
    };

    let effective_persona_id = resolve_persona_id(&session, persona_id.as_deref());
    let persona = context.choose_persona(effective_persona_id);

    let rendered =
        prompt_engine::render_with_context(&app, &content, &character, persona, &session, settings);
    Ok(rendered)
}

#[tauri::command]
pub async fn retry_dynamic_memory(
    app: AppHandle,
    session_id: String,
    model_id: Option<String>,
    update_default: Option<bool>,
) -> Result<(), String> {
    super::memory::flow::retry_dynamic_memory(app, session_id, model_id, update_default).await
}

#[tauri::command]
pub async fn trigger_dynamic_memory(app: AppHandle, session_id: String) -> Result<(), String> {
    super::memory::flow::trigger_dynamic_memory(app, session_id).await
}

#[tauri::command]
pub fn abort_dynamic_memory(app: AppHandle, session_id: String) -> Result<(), String> {
    super::memory::flow::abort_dynamic_memory(app, session_id)
}

#[tauri::command]
pub async fn chat_add_message_attachment(
    app: AppHandle,
    args: ChatAddMessageAttachmentArgs,
) -> Result<StoredMessage, String> {
    let ChatAddMessageAttachmentArgs {
        session_id,
        character_id,
        message_id,
        role,
        attachment_id,
        base64_data,
        mime_type,
        filename,
        width,
        height,
    } = args;

    if base64_data.trim().is_empty() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "base64Data cannot be empty",
        ));
    }

    let mut session = super::storage::load_session(&app, &session_id)?
        .ok_or_else(|| "Session not found".to_string())?;

    let target_index = session
        .messages
        .iter()
        .position(|m| m.id == message_id)
        .ok_or_else(|| "Message not found in loaded session window".to_string())?;

    let new_attachment = persist_attachments(
        &app,
        &character_id,
        &session_id,
        &message_id,
        &role,
        vec![ImageAttachment {
            id: attachment_id,
            data: base64_data,
            mime_type,
            filename,
            width,
            height,
            storage_path: None,
        }],
    )?
    .into_iter()
    .next()
    .ok_or_else(|| "Failed to persist attachment".to_string())?;

    let updated_message = {
        let target = &mut session.messages[target_index];
        if let Some(existing) = target
            .attachments
            .iter_mut()
            .find(|att| att.id == new_attachment.id)
        {
            *existing = new_attachment;
        } else {
            target.attachments.push(new_attachment);
        }
        target.clone()
    };

    session.updated_at = now_millis()?;

    // Persist meta + the updated message (even if it's not the last message).
    let mut meta = session.clone();
    meta.messages = Vec::new();
    session_upsert_meta_typed(&app, &meta)?;
    messages_upsert_batch_typed(&app, &session_id, std::slice::from_ref(&updated_message))?;

    Ok(updated_message)
}

#[tauri::command]
pub async fn chat_generate_scene_image(
    app: AppHandle,
    args: ChatGenerateSceneImageArgs,
) -> Result<StoredMessage, String> {
    super::scene::chat_generate_scene_image(app, args).await
}

#[tauri::command]
pub async fn chat_generate_scene_prompt(
    app: AppHandle,
    args: ChatGenerateScenePromptArgs,
) -> Result<String, String> {
    super::scene::chat_generate_scene_prompt(app, args).await
}

#[tauri::command]
pub async fn chat_generate_design_reference_description(
    app: AppHandle,
    args: ChatGenerateDesignReferenceDescriptionArgs,
) -> Result<String, String> {
    super::scene::chat_generate_design_reference_description(app, args).await
}

#[tauri::command]
pub async fn chat_generate_lorebook_entry_draft(
    app: AppHandle,
    args: ChatGenerateLorebookEntryDraftArgs,
) -> Result<LorebookEntryDraftResult, String> {
    super::lorebook_entry_generator::chat_generate_lorebook_entry_draft(app, args).await
}

#[tauri::command]
pub async fn search_messages(
    app: AppHandle,
    session_id: String,
    query: String,
) -> Result<Vec<super::types::MessageSearchResult>, String> {
    let context = ChatContext::initialize(app.clone())?;

    let session = match context.load_session(&session_id)? {
        Some(s) => s,
        None => {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                "Session not found",
            ))
        }
    };

    let query_lower = query.to_lowercase();
    let results: Vec<super::types::MessageSearchResult> = session
        .messages
        .iter()
        .filter(|msg| {
            msg.content.to_lowercase().contains(&query_lower)
                && (msg.role == "user" || msg.role == "assistant")
        })
        .map(|msg| super::types::MessageSearchResult {
            message_id: msg.id.clone(),
            content: msg.content.clone(),
            created_at: msg.created_at,
            role: msg.role.clone(),
        })
        .collect();

    Ok(results)
}

#[tauri::command]
pub async fn chat_generate_user_reply(
    app: AppHandle,
    session_id: String,
    current_draft: Option<String>,
    request_id: Option<String>,
    swap_places: Option<bool>,
) -> Result<String, String> {
    super::reply_helper::chat_generate_user_reply(
        app,
        session_id,
        current_draft,
        request_id,
        swap_places,
    )
    .await
}
#[tauri::command]
pub fn get_prompt_parameter_engine(
) -> crate::chat_manager::prompting::parameter_engine::PromptParameterEngine {
    crate::chat_manager::prompting::parameter_engine::build_parameter_engine()
}
