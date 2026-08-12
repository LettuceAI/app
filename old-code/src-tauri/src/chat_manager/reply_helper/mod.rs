use tauri::AppHandle;

use crate::api::{api_request, ApiRequest};
use crate::chat_manager::entries::in_chat_user_entry;
use crate::chat_manager::execution::prepare_feature_request;
use crate::chat_manager::feature_generation::{
    feature_model_overrides, LlmFeature, HELP_ME_REPLY_DEFAULTS,
};
use crate::chat_manager::prompts;
use crate::chat_manager::request::extract_text;
use crate::chat_manager::service::{require_api_key, ChatContext};
use crate::chat_manager::storage::{recent_messages, resolve_credential_for_model};
use crate::chat_manager::turn_builder::{
    assemble_prompt_messages, role_swap_enabled, swap_role_for_api, swapped_prompt_entities,
};
use crate::chat_manager::types::{Character, Persona, Session};
use crate::usage::tracking::UsageOperationType;
use crate::utils::{log_info, now_millis};

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

pub fn help_me_reply_participant_names<'a>(
    prompt_character: &'a Character,
    prompt_persona: Option<&'a Persona>,
) -> (&'a str, &'a str) {
    let effective_user_name = prompt_persona.map(|p| p.title.as_str()).unwrap_or("user");
    let effective_assistant_name = prompt_character.name.as_str();
    (effective_user_name, effective_assistant_name)
}

pub async fn chat_generate_user_reply(
    app: AppHandle,
    session_id: String,
    current_draft: Option<String>,
    request_id: Option<String>,
    swap_places: Option<bool>,
) -> Result<String, String> {
    let swap_places = role_swap_enabled(swap_places);
    log_info(
        &app,
        "help_me_reply",
        format!(
            "Generating user reply for session={}, has_draft={}, swap_places={}",
            &session_id,
            current_draft.is_some(),
            swap_places
        ),
    );
    let context = ChatContext::initialize(app.clone())?;
    let settings = &context.settings;

    // Check if help me reply is enabled
    if let Some(advanced) = &settings.advanced_settings {
        if advanced.help_me_reply_enabled == Some(false) {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                "Help Me Reply is disabled in settings",
            ));
        }
    }

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

    let character = context.find_character(&session.character_id)?;
    let persona = context.choose_persona(resolve_persona_id(&session, None));
    let (prompt_character, prompt_persona) = if swap_places {
        swapped_prompt_entities(&character, persona)
    } else {
        (character.clone(), persona.cloned())
    };

    let history_count = settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| advanced.help_me_reply_history_count)
        .filter(|count| *count > 0)
        .unwrap_or(10) as usize;
    let recent_msgs = recent_messages(&session, history_count);

    if recent_msgs.is_empty() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "No conversation history to base reply on",
        ));
    }

    // Use help me reply model if configured, otherwise fall back to default
    let model_id = settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| advanced.help_me_reply_model_id.as_ref())
        .or(settings.default_model_id.as_ref())
        .ok_or_else(|| "No model configured for Help Me Reply".to_string())?;

    let model = settings
        .models
        .iter()
        .find(|m| &m.id == model_id)
        .ok_or_else(|| "Help Me Reply model not found".to_string())?;

    let credential = resolve_credential_for_model(settings, model)
        .ok_or_else(|| "Provider credential not found".to_string())?;

    let api_key = require_api_key(&app, credential, "help_me_reply")?;

    // Get reply style from settings (default to roleplay)
    let reply_style = settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| advanced.help_me_reply_style.as_ref())
        .map(|s| s.as_str())
        .unwrap_or("roleplay");

    let help_me_reply_prompt_template_id = settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| {
            if reply_style == "conversational" {
                advanced
                    .help_me_reply_conversational_prompt_template_id
                    .as_deref()
            } else {
                advanced
                    .help_me_reply_roleplay_prompt_template_id
                    .as_deref()
            }
        })
        .filter(|id| !id.trim().is_empty());

    let mut prompt_entries =
        prompts::get_help_me_reply_entries(&app, reply_style, help_me_reply_prompt_template_id);

    // Get max tokens from settings (default to 150)
    let max_tokens = settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| advanced.help_me_reply_max_tokens)
        .unwrap_or(150);

    // Get streaming setting (default to true)
    let streaming_enabled = settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| advanced.help_me_reply_streaming)
        .unwrap_or(true);

    let char_name = &prompt_character.name;
    let char_desc = prompt_character
        .definition
        .as_deref()
        .or(prompt_character.description.as_deref())
        .unwrap_or("");
    let persona_name = prompt_persona
        .as_ref()
        .map(|p| p.title.as_str())
        .unwrap_or("user");
    let persona_desc = prompt_persona
        .as_ref()
        .map(|p| p.description.as_str())
        .unwrap_or("");

    let draft_str = current_draft.as_deref().unwrap_or("");
    for entry in &mut prompt_entries {
        let content = &mut entry.content;
        *content = content
            .replace("{{char.name}}", char_name)
            .replace("{{char.desc}}", char_desc)
            .replace("{{persona.name}}", persona_name)
            .replace("{{persona.desc}}", persona_desc)
            .replace("{{user.name}}", persona_name)
            .replace("{{user.desc}}", persona_desc)
            .replace("{{current_draft}}", draft_str)
            .replace("{{char}}", char_name)
            .replace("{{persona}}", persona_name)
            .replace("{{user}}", persona_name);

        if let Some(ref draft) = current_draft {
            if !draft.trim().is_empty() {
                *content = content.replace("{{#if current_draft}}", "");
                *content = content.replace("{{current_draft}}", draft);
                if let Some(else_start) = content.find("{{else}}") {
                    if let Some(endif_start) = content[else_start..].find("{{/if}}") {
                        content.replace_range(else_start..(else_start + endif_start + 7), "");
                    }
                }
                *content = content.replace("{{/if}}", "");
            } else {
                remove_if_block(content);
            }
        } else {
            remove_if_block(content);
        }
    }

    let (effective_user_name, effective_assistant_name) =
        help_me_reply_participant_names(&prompt_character, prompt_persona.as_ref());

    let conversation_context = recent_msgs
        .iter()
        .map(|msg| {
            let effective_role = if swap_places {
                swap_role_for_api(msg.role.as_str())
            } else {
                msg.role.as_str()
            };
            let role_label = if effective_role == "user" {
                effective_user_name
            } else {
                effective_assistant_name
            };
            format!("{}: {}", role_label, msg.content)
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    let user_prompt = format!(
        "Here is the recent conversation:\n\n{}\n\nGenerate a reply for {} to say next.",
        conversation_context, effective_user_name
    );

    let system_role = super::request_builder::system_role_for(credential);
    prompt_entries.push(in_chat_user_entry(
        "runtime_reply_helper_input",
        "Reply Helper Input",
        user_prompt,
        0,
    ));
    let messages_for_api = assemble_prompt_messages(prompt_entries, Vec::new(), &system_role);

    let mut overrides = feature_model_overrides(
        model,
        LlmFeature::HelpMeReply,
        HELP_ME_REPLY_DEFAULTS,
    );
    if overrides.max_output_tokens.is_none() {
        overrides.max_output_tokens = Some(max_tokens);
    }
    let mut request_session = session.clone();
    request_session.advanced_model_settings = Some(overrides);
    let (request_settings, extra_body_fields) = prepare_feature_request(
        &credential.provider_id,
        &request_session,
        model,
        settings,
    );
    let built = super::request_builder::build_chat_request(
        credential,
        &api_key,
        &model.name,
        &messages_for_api,
        None,
        request_settings.temperature,
        request_settings.top_p,
        request_settings.max_tokens,
        request_settings.context_length,
        streaming_enabled,
        request_id.clone(),
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

    log_info(
        &app,
        "help_me_reply",
        format!("Sending request to {}", built.url),
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
        provider_id: Some(credential.provider_id.clone()),
        cache_key: None,
    };

    let api_response = api_request(app.clone(), api_request_payload).await?;

    if !api_response.ok {
        return Err(format!(
            "API request failed with status {}",
            api_response.status
        ));
    }

    let generated_text = extract_text(&api_response.data, Some(&credential.provider_id))
        .ok_or_else(|| "Failed to extract text from response".to_string())?;

    let cleaned = generated_text
        .trim()
        .trim_matches('"')
        .trim_start_matches(&format!("{}:", effective_user_name))
        .trim()
        .to_string();

    log_info(
        &app,
        "help_me_reply",
        format!("Generated reply: {} chars", cleaned.len()),
    );

    let usage = super::sse::usage_from_value(&api_response.data);
    super::service::record_usage_if_available(
        &context,
        &usage,
        &session,
        &prompt_character,
        model,
        credential,
        &api_key,
        now_millis().unwrap_or(0),
        UsageOperationType::ReplyHelper,
        "help_me_reply",
    )
    .await;

    Ok(cleaned)
}

/// Helper to remove {{#if current_draft}}...{{else}}...{{/if}} and keep else content
fn remove_if_block(prompt: &mut String) {
    if let Some(if_start) = prompt.find("{{#if current_draft}}") {
        if let Some(else_pos) = prompt.find("{{else}}") {
            prompt.replace_range(if_start..(else_pos + 8), "");
        }
    }
    *prompt = prompt.replace("{{/if}}", "");
}
