use serde_json::{json, Value};
use std::collections::HashMap;
use tauri::AppHandle;

use crate::api::{api_request, ApiRequest, ApiResponse};
use crate::chat_manager::companion::{
    self, soul_category_is_changeable, SoulGrowthEntry, CORE_SOUL_CATEGORIES,
};
use crate::chat_manager::execution::{find_model_with_credential, prepare_default_sampling_request};
use crate::chat_manager::memory::flow::resolve_dynamic_memory_summarisation_model_id;
use crate::chat_manager::prompts;
use crate::chat_manager::request::{extract_error_message, extract_text, extract_usage};
use crate::chat_manager::request_builder;
use crate::chat_manager::service::{record_usage_if_available, require_api_key, ChatContext};
use crate::chat_manager::storage::{get_base_prompt_entries, PromptType};
use crate::chat_manager::tooling::{parse_tool_calls, ToolChoice, ToolConfig, ToolDefinition};
use crate::chat_manager::types::{
    Character, PromptEntryRole, ProviderCredential, Session, Settings, SystemPromptEntry,
};
use crate::usage::tracking::UsageOperationType;
use crate::utils::{log_info, log_warn, now_millis};

const CONSOLIDATION_THRESHOLD: usize = 12;

pub async fn maybe_run_consolidation(
    app: &AppHandle,
    context: &ChatContext,
    settings: &Settings,
    session: &mut Session,
    character: &Character,
) -> Result<usize, String> {
    if !companion::is_companion_mode(session, character) {
        return Ok(0);
    }

    let active = companion::active_soul_growth_entries(character, session);
    let accumulated: Vec<SoulGrowthEntry> = active
        .iter()
        .filter(|entry| soul_category_is_changeable(&entry.category) && !entry.id.trim().is_empty())
        .cloned()
        .collect();
    if accumulated.len() < CONSOLIDATION_THRESHOLD {
        return Ok(0);
    }

    let core_overlay: Vec<SoulGrowthEntry> = active
        .iter()
        .filter(|entry| {
            CORE_SOUL_CATEGORIES.contains(&entry.category.as_str()) && !entry.id.trim().is_empty()
        })
        .cloned()
        .collect();
    let authored = companion::core_soul_authored(character);

    let model_id = resolve_dynamic_memory_summarisation_model_id(app, settings, None)?;
    let (model, credential) = find_model_with_credential(settings, &model_id)
        .ok_or_else(|| "Consolidation model could not be resolved".to_string())?;
    let api_key = require_api_key(app, credential, "companion_consolidation")?;

    let (request_settings, extra_body_fields) = prepare_default_sampling_request(
        &credential.provider_id,
        session,
        model,
        settings,
        0.3,
        1.0,
        None,
        None,
        None,
    );

    let messages = render_messages(app, credential, character, &authored, &core_overlay, &accumulated);
    if messages.is_empty() {
        return Err("Consolidation template rendered no prompt content".to_string());
    }
    let tool_config = build_tool_config();

    let response = send_request(
        app,
        credential,
        model,
        &api_key,
        &messages,
        request_settings.max_tokens,
        request_settings.context_length,
        extra_body_fields,
        &tool_config,
    )
    .await?;

    let usage = extract_usage(response.data());
    record_usage_if_available(
        context,
        &usage,
        session,
        character,
        model,
        credential,
        &api_key,
        now_millis().unwrap_or(0),
        UsageOperationType::ReplyHelper,
        "companion_consolidation",
    )
    .await;

    if !response.ok {
        let status_fallback = format!("Provider returned status {}", response.status);
        return Err(extract_error_message(response.data()).unwrap_or(status_fallback));
    }

    let (core_entries, retire_ids) = parse_consolidation(app, credential, &response);
    let now = now_millis()?;
    let applied_core = companion::append_core_soul_growth(session, character, core_entries, now);
    let retired = companion::retire_soul_growth_entries(session, &retire_ids, now);

    let total = applied_core + retired;
    if total > 0 {
        log_info(
            app,
            "companion_consolidation",
            format!(
                "Consolidation applied {} core change(s) and retired {} growth entry(ies) for session {}",
                applied_core, retired, session.id
            ),
        );
    }
    Ok(total)
}

fn render_messages(
    app: &AppHandle,
    credential: &ProviderCredential,
    character: &Character,
    authored: &[(String, String)],
    core_overlay: &[SoulGrowthEntry],
    accumulated: &[SoulGrowthEntry],
) -> Vec<Value> {
    let entries = load_prompt_entries(app);
    let system_role = request_builder::system_role_for(credential);
    let authored_core = format_authored(authored);
    let current_core = format_entries(core_overlay);
    let accumulated_growth = format_entries(accumulated);

    let mut messages = Vec::new();
    for entry in entries {
        if !entry.enabled {
            continue;
        }
        let role = match entry.role {
            PromptEntryRole::System => system_role.as_ref(),
            PromptEntryRole::User => "user",
            PromptEntryRole::Assistant => "assistant",
        };
        let rendered = entry
            .content
            .replace("{{companion.name}}", character.name.trim())
            .replace("{{authored_core}}", &authored_core)
            .replace("{{current_core}}", &current_core)
            .replace("{{accumulated_growth}}", &accumulated_growth);
        let trimmed = rendered.trim();
        if trimmed.is_empty() {
            continue;
        }
        messages.push(json!({ "role": role, "content": trimmed }));
    }

    messages
}

fn load_prompt_entries(app: &AppHandle) -> Vec<SystemPromptEntry> {
    match prompts::get_template(app, prompts::APP_COMPANION_CONSOLIDATION_TEMPLATE_ID) {
        Ok(Some(template)) if !template.entries.is_empty() => template.entries,
        _ => get_base_prompt_entries(PromptType::CompanionConsolidationPrompt),
    }
}

fn format_authored(authored: &[(String, String)]) -> String {
    let mut out = String::new();
    for (category, value) in authored {
        let text = if value.trim().is_empty() {
            "(empty)"
        } else {
            value.trim()
        };
        out.push_str(&format!("- {}: {}\n", category, text));
    }
    out
}

fn format_entries(entries: &[SoulGrowthEntry]) -> String {
    let mut out = String::new();
    for entry in entries {
        out.push_str(&format!(
            "- id={} [{}]: {}\n",
            entry.id,
            entry.category,
            entry.value.trim()
        ));
    }
    if out.is_empty() {
        return "(none)".to_string();
    }
    out
}

fn build_tool_config() -> ToolConfig {
    let core: Vec<Value> = CORE_SOUL_CATEGORIES
        .iter()
        .map(|category| json!(category))
        .collect();

    let tools = vec![ToolDefinition {
        name: "consolidate_soul".to_string(),
        description: Some(
            "Fold accumulated companion growth: optionally evolve the core (essence/traits) when a sustained pattern warrants it, and retire growth entries whose meaning is now absorbed. Both arrays may be empty.".to_string(),
        ),
        parameters: json!({
            "type": "object",
            "properties": {
                "coreAdjustments": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "category": { "type": "string", "enum": core },
                            "value": { "type": "string" },
                            "supersedes": {
                                "type": "array",
                                "items": { "type": "string" }
                            }
                        },
                        "required": ["category", "value"]
                    }
                },
                "retire": {
                    "type": "array",
                    "items": { "type": "string" }
                }
            },
            "required": ["coreAdjustments", "retire"]
        }),
    }];

    ToolConfig {
        tools,
        choice: Some(ToolChoice::Required),
    }
}

fn parse_consolidation(
    app: &AppHandle,
    credential: &ProviderCredential,
    response: &ApiResponse,
) -> (Vec<SoulGrowthEntry>, Vec<String>) {
    let calls = parse_tool_calls(&credential.provider_id, response.data());
    let args = calls
        .iter()
        .find(|call| call.name == "consolidate_soul")
        .map(|call| call.arguments.clone())
        .or_else(|| fallback_args(response, &credential.provider_id));

    let args = match args {
        Some(value) => value,
        None => {
            log_warn(
                app,
                "companion_consolidation",
                "Consolidation returned no usable consolidate_soul call".to_string(),
            );
            return (Vec::new(), Vec::new());
        }
    };

    let mut core_entries = Vec::new();
    if let Some(Value::Array(items)) = args.get("coreAdjustments").cloned() {
        for item in items {
            let category = item
                .get("category")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            if !CORE_SOUL_CATEGORIES.contains(&category.as_str()) {
                continue;
            }
            let value = item
                .get("value")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_string();
            if value.is_empty() {
                continue;
            }
            let supersedes = string_array(item.get("supersedes"));
            core_entries.push(SoulGrowthEntry {
                category,
                value,
                kind: "consolidated".to_string(),
                supersedes,
                ..Default::default()
            });
        }
    }

    let retire_ids = string_array(args.get("retire"));
    (core_entries, retire_ids)
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .filter(|s| !s.trim().is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn fallback_args(response: &ApiResponse, provider_id: &str) -> Option<Value> {
    let text = extract_text(response.data(), Some(provider_id))?;
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if end <= start {
        return None;
    }
    serde_json::from_str(&text[start..=end]).ok()
}

#[allow(clippy::too_many_arguments)]
async fn send_request(
    app: &AppHandle,
    credential: &ProviderCredential,
    model: &crate::chat_manager::types::Model,
    api_key: &str,
    messages: &Vec<Value>,
    max_tokens: u32,
    context_length: Option<u32>,
    extra_body_fields: Option<HashMap<String, Value>>,
    tool_config: &ToolConfig,
) -> Result<ApiResponse, String> {
    let built = request_builder::build_chat_request(
        credential,
        api_key,
        &model.name,
        messages,
        None,
        Some(0.3),
        Some(1.0),
        max_tokens,
        context_length,
        false,
        None,
        None,
        None,
        None,
        Some(tool_config),
        false,
        None,
        None,
        false,
        extra_body_fields,
    );

    api_request(
        app.clone(),
        ApiRequest {
            url: built.url,
            method: Some("POST".into()),
            headers: Some(built.headers),
            query: None,
            body: Some(built.body),
            timeout_ms: Some(crate::transport::DEFAULT_REQUEST_TIMEOUT_MS),
            stream: Some(false),
            request_id: built.request_id,
            provider_id: Some(credential.provider_id.clone()),
        },
    )
    .await
}
