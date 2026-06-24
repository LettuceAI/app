use serde_json::{json, Value};
use std::collections::HashMap;
use tauri::AppHandle;

use crate::api::{api_request, ApiRequest, ApiResponse};
use crate::chat_manager::companion::{
    self, soul_category_is_changeable, soul_category_label, SoulGrowthEntry,
    CHANGEABLE_SOUL_CATEGORIES,
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
    Character, MemoryEmbedding, Model, PromptEntryRole, ProviderCredential, Session, Settings,
    SystemPromptEntry,
};
use crate::usage::tracking::UsageOperationType;
use crate::utils::{log_info, log_warn, now_millis};

const MAX_NEW_MEMORIES: usize = 16;

pub async fn run_growthcycle(
    app: &AppHandle,
    context: &ChatContext,
    settings: &Settings,
    session: &mut Session,
    character: &Character,
    new_memories: &[MemoryEmbedding],
) -> Result<usize, String> {
    if !companion::is_companion_mode(session, character) {
        return Ok(0);
    }

    let fresh: Vec<&MemoryEmbedding> = new_memories
        .iter()
        .filter(|memory| !memory.text.trim().is_empty())
        .take(MAX_NEW_MEMORIES)
        .collect();
    if fresh.is_empty() {
        return Ok(0);
    }

    let snapshot = companion::changeable_soul_snapshot(character, session);
    let existing_growth = companion::active_soul_growth_entries(character, session);

    let model_id = resolve_dynamic_memory_summarisation_model_id(app, settings, None)?;
    let (model, credential) = find_model_with_credential(settings, &model_id)
        .ok_or_else(|| "Growthcycle model could not be resolved".to_string())?;
    let api_key = require_api_key(app, credential, "companion_growth")?;

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

    let messages = render_messages(app, credential, character, &snapshot, &existing_growth, &fresh);
    if messages.is_empty() {
        return Err("Growthcycle template rendered no prompt content".to_string());
    }
    let tool_config = build_tool_config();

    let response = send_growth_request(
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
        "companion_growth",
    )
    .await;

    if !response.ok {
        let status_fallback = format!("Provider returned status {}", response.status);
        return Err(extract_error_message(response.data()).unwrap_or(status_fallback));
    }

    let memory_ids: Vec<String> = fresh.iter().map(|memory| memory.id.clone()).collect();
    let entries = parse_growth_entries(app, credential, &response, &memory_ids);
    if entries.is_empty() {
        return Ok(0);
    }

    let now = now_millis()?;
    let applied = companion::append_soul_growth(session, character, entries, now);
    if applied > 0 {
        log_info(
            app,
            "companion_growth",
            format!(
                "Growthcycle recorded {} soul adjustment(s) for session {}",
                applied, session.id
            ),
        );
    }
    Ok(applied)
}

fn render_messages(
    app: &AppHandle,
    credential: &ProviderCredential,
    character: &Character,
    snapshot: &[(String, String)],
    existing_growth: &[SoulGrowthEntry],
    fresh: &[&MemoryEmbedding],
) -> Vec<Value> {
    let entries = load_prompt_entries(app);
    let system_role = request_builder::system_role_for(credential);
    let categories = format_categories(snapshot);
    let current_growth = format_current_growth(existing_growth);
    let memories = format_memories(fresh);

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
        let rendered =
            render_growth_content(&entry.content, character, &categories, &current_growth, &memories);
        let trimmed = rendered.trim();
        if trimmed.is_empty() {
            continue;
        }
        messages.push(json!({ "role": role, "content": trimmed }));
    }

    messages
}

fn load_prompt_entries(app: &AppHandle) -> Vec<SystemPromptEntry> {
    match prompts::get_template(app, prompts::APP_COMPANION_GROWTHCYCLE_TEMPLATE_ID) {
        Ok(Some(template)) if !template.entries.is_empty() => template.entries,
        _ => get_base_prompt_entries(PromptType::CompanionGrowthcyclePrompt),
    }
}

fn render_growth_content(
    content: &str,
    character: &Character,
    categories: &str,
    current_growth: &str,
    memories: &str,
) -> String {
    content
        .replace("{{companion.name}}", character.name.trim())
        .replace("{{char.name}}", character.name.trim())
        .replace("{{changeable_categories}}", categories)
        .replace("{{current_growth}}", current_growth)
        .replace("{{new_memories}}", memories)
}

fn format_current_growth(entries: &[SoulGrowthEntry]) -> String {
    let mut out = String::new();
    for entry in entries {
        if entry.id.trim().is_empty() {
            continue;
        }
        out.push_str(&format!(
            "- id={} [{}]: {}\n",
            entry.id,
            entry.category,
            entry.value.trim()
        ));
    }
    if out.is_empty() {
        return "(none yet)".to_string();
    }
    out
}

fn format_categories(snapshot: &[(String, String)]) -> String {
    let mut out = String::new();
    for (category, value) in snapshot {
        let current = if value.trim().is_empty() {
            "(empty)"
        } else {
            value.trim()
        };
        out.push_str(&format!(
            "- {} [{}]: {}\n",
            soul_category_label(category),
            category,
            current
        ));
    }
    out
}

fn format_memories(fresh: &[&MemoryEmbedding]) -> String {
    let mut out = String::new();
    for (index, memory) in fresh.iter().enumerate() {
        out.push_str(&format!("{}. {}\n", index, memory.text.trim()));
    }
    out
}

fn build_tool_config() -> ToolConfig {
    let categories: Vec<Value> = CHANGEABLE_SOUL_CATEGORIES
        .iter()
        .map(|category| json!(category))
        .collect();

    let tools = vec![ToolDefinition {
        name: "record_growth".to_string(),
        description: Some(
            "Record how the new memories change the companion's changeable personality categories. To revise or replace an existing growth entry, set kind to adjust and list its id in supersedes. Pass an empty adjustments array when nothing changed.".to_string(),
        ),
        parameters: json!({
            "type": "object",
            "properties": {
                "adjustments": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "category": { "type": "string", "enum": categories },
                            "kind": { "type": "string", "enum": ["add", "adjust"] },
                            "value": { "type": "string" },
                            "sourceIndices": {
                                "type": "array",
                                "items": { "type": "integer" }
                            },
                            "supersedes": {
                                "type": "array",
                                "items": { "type": "string" }
                            }
                        },
                        "required": ["category", "value"]
                    }
                }
            },
            "required": ["adjustments"]
        }),
    }];

    ToolConfig {
        tools,
        choice: Some(ToolChoice::Required),
    }
}

fn parse_growth_entries(
    app: &AppHandle,
    credential: &ProviderCredential,
    response: &ApiResponse,
    memory_ids: &[String],
) -> Vec<SoulGrowthEntry> {
    let calls = parse_tool_calls(&credential.provider_id, response.data());
    let adjustments = calls
        .iter()
        .find(|call| call.name == "record_growth")
        .and_then(|call| call.arguments.get("adjustments").cloned())
        .or_else(|| fallback_adjustments(response, &credential.provider_id));

    let adjustments = match adjustments {
        Some(Value::Array(items)) => items,
        _ => {
            log_warn(
                app,
                "companion_growth",
                "Growthcycle returned no usable record_growth call".to_string(),
            );
            return Vec::new();
        }
    };

    let mut entries = Vec::new();
    for item in adjustments {
        let category = item
            .get("category")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if !soul_category_is_changeable(&category) {
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
        let kind = match item.get("kind").and_then(Value::as_str) {
            Some("adjust") => "adjust",
            _ => "add",
        }
        .to_string();
        let source_memory_ids = resolve_sources(item.get("sourceIndices"), memory_ids);
        let supersedes = item
            .get("supersedes")
            .and_then(Value::as_array)
            .map(|ids| {
                ids.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        entries.push(SoulGrowthEntry {
            category,
            value,
            kind,
            source_memory_ids,
            supersedes,
            ..Default::default()
        });
    }
    entries
}

fn resolve_sources(indices: Option<&Value>, memory_ids: &[String]) -> Vec<String> {
    let mapped: Vec<String> = indices
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_u64)
                .filter_map(|index| memory_ids.get(index as usize).cloned())
                .collect()
        })
        .unwrap_or_default();
    if mapped.is_empty() {
        memory_ids.to_vec()
    } else {
        mapped
    }
}

fn fallback_adjustments(response: &ApiResponse, provider_id: &str) -> Option<Value> {
    let text = extract_text(response.data(), Some(provider_id))?;
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if end <= start {
        return None;
    }
    let parsed: Value = serde_json::from_str(&text[start..=end]).ok()?;
    parsed.get("adjustments").cloned()
}

#[allow(clippy::too_many_arguments)]
async fn send_growth_request(
    app: &AppHandle,
    credential: &ProviderCredential,
    model: &Model,
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
