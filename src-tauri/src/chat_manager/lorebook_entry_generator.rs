use quick_xml::escape::{resolve_xml_entity, unescape};
use quick_xml::events::{BytesRef, BytesStart, Event};
use quick_xml::Reader;
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use tauri::AppHandle;

use crate::api::{api_request, ApiRequest, ApiResponse};
use crate::chat_manager::execution::{
    find_model_with_credential, prepare_default_sampling_request,
};
use crate::chat_manager::prompting::entry_conditions::{
    entry_is_active, PromptEntryConditionContext,
};
use crate::chat_manager::prompting::request::{extract_error_message, extract_text, extract_usage};
use crate::chat_manager::service::{record_usage_if_available, require_api_key, ChatContext};
use crate::chat_manager::storage::{
    get_base_prompt_entries, resolve_credential_for_model, PromptType,
};
use crate::chat_manager::tooling::{
    parse_tool_calls, parse_tool_calls_from_text, ToolCall, ToolChoice, ToolConfig, ToolDefinition,
};
use crate::chat_manager::turn_builder::should_insert_in_chat_prompt_entry;
use crate::chat_manager::types::{
    Character, ChatGenerateLorebookEntryDraftArgs, ChatGenerateLorebookKeywordDraftArgs,
    LorebookEntryDraft, LorebookEntryDraftResult, LorebookKeywordDraftResult, Model, Persona,
    PromptEntryChatMode, PromptEntryPosition, PromptEntryRole, ProviderCredential, Session,
    Settings, StoredMessage, SystemPromptEntry,
};
use crate::storage_manager::db::open_db;
use crate::storage_manager::lorebook::{get_lorebook, get_lorebook_entries};
use crate::storage_manager::sessions::session_get_meta_internal;
use crate::usage::tracking::UsageOperationType;
use crate::utils::{log_warn, now_millis};

const LOREBOOK_RESULT_XML_ROOT_TAGS: &[&str] = &["lorebook_result", "result", "response"];
const LOREBOOK_JSON_FALLBACK_PROMPT: &str = r#"Return only JSON. Format: {"result":{"name":"write_lorebook_entry","arguments":{"title":"...","keywords":["..."],"content":"...","alwaysActive":false}}}. If no durable entry should be created, return {"result":{"name":"no_entry","arguments":{"reason":"..."}}}. Do not use markdown."#;
const LOREBOOK_XML_FALLBACK_PROMPT: &str = r#"Return only XML. Format: <lorebook_result><write_lorebook_entry alwaysActive="false"><title>...</title><keywords><keyword>...</keyword></keywords><content>...</content></write_lorebook_entry></lorebook_result>. If no durable entry should be created, return <lorebook_result><no_entry><reason>...</reason></no_entry></lorebook_result>. Do not use markdown."#;
const LOREBOOK_JSON_FALLBACK_PROMPT_FORCE: &str = r#"Return only JSON. Format: {"result":{"name":"write_lorebook_entry","arguments":{"title":"...","keywords":["..."],"content":"...","alwaysActive":false}}}. You MUST return write_lorebook_entry. The no_entry option is disabled. Do not use markdown."#;
const LOREBOOK_XML_FALLBACK_PROMPT_FORCE: &str = r#"Return only XML. Format: <lorebook_result><write_lorebook_entry alwaysActive="false"><title>...</title><keywords><keyword>...</keyword></keywords><content>...</content></write_lorebook_entry></lorebook_result>. You MUST return write_lorebook_entry. The no_entry option is disabled. Do not use markdown."#;
const LOREBOOK_KEYWORDS_JSON_FALLBACK_PROMPT: &str = r#"Return only JSON. Format: {"result":{"name":"write_lorebook_keywords","arguments":{"keywords":["..."]}}}. You MUST return write_lorebook_keywords. Do not use markdown."#;
const LOREBOOK_KEYWORDS_XML_FALLBACK_PROMPT: &str = r#"Return only XML. Format: <lorebook_result><write_lorebook_keywords><keywords><keyword>...</keyword></keywords></write_lorebook_keywords></lorebook_result>. You MUST return write_lorebook_keywords. Do not use markdown."#;
const MAX_GENERATED_KEYWORDS: usize = 24;

fn supports_lorebook_entry_writer_model(model: &Model) -> bool {
    model
        .input_scopes
        .iter()
        .any(|scope| scope.eq_ignore_ascii_case("text"))
        && model
            .output_scopes
            .iter()
            .any(|scope| scope.eq_ignore_ascii_case("text"))
}

fn resolve_lorebook_entry_writer_target<'a>(
    settings: &'a Settings,
    preferred_model_id: Option<&str>,
) -> Result<(&'a Model, &'a ProviderCredential), String> {
    if let Some(model_id) = preferred_model_id.filter(|id| !id.trim().is_empty()) {
        let (model, credential) =
            find_model_with_credential(settings, model_id).ok_or_else(|| {
                "Configured lorebook entry generator model could not be resolved".to_string()
            })?;
        if !supports_lorebook_entry_writer_model(model) {
            return Err(
                "Configured lorebook entry generator model must support text input and text output"
                    .to_string(),
            );
        }
        return Ok((model, credential));
    }

    settings
        .models
        .iter()
        .find_map(|model| {
            if !supports_lorebook_entry_writer_model(model) {
                return None;
            }
            let credential = resolve_credential_for_model(settings, model)?;
            Some((model, credential))
        })
        .ok_or_else(|| "No compatible lorebook entry generator model is configured".to_string())
}

fn lorebook_entry_structured_fallback_format(
    settings: &Settings,
) -> crate::chat_manager::types::DynamicMemoryStructuredFallbackFormat {
    settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| advanced.lorebook_entry_generator_structured_fallback_format)
        .unwrap_or(crate::chat_manager::types::DynamicMemoryStructuredFallbackFormat::Json)
}

fn fallback_format_label(
    format: crate::chat_manager::types::DynamicMemoryStructuredFallbackFormat,
) -> &'static str {
    match format {
        crate::chat_manager::types::DynamicMemoryStructuredFallbackFormat::Json => "json",
        crate::chat_manager::types::DynamicMemoryStructuredFallbackFormat::Xml => "xml",
    }
}

fn fallback_prompt(
    format: crate::chat_manager::types::DynamicMemoryStructuredFallbackFormat,
    force: bool,
) -> &'static str {
    match (format, force) {
        (crate::chat_manager::types::DynamicMemoryStructuredFallbackFormat::Json, false) => {
            LOREBOOK_JSON_FALLBACK_PROMPT
        }
        (crate::chat_manager::types::DynamicMemoryStructuredFallbackFormat::Xml, false) => {
            LOREBOOK_XML_FALLBACK_PROMPT
        }
        (crate::chat_manager::types::DynamicMemoryStructuredFallbackFormat::Json, true) => {
            LOREBOOK_JSON_FALLBACK_PROMPT_FORCE
        }
        (crate::chat_manager::types::DynamicMemoryStructuredFallbackFormat::Xml, true) => {
            LOREBOOK_XML_FALLBACK_PROMPT_FORCE
        }
    }
}

fn selected_prompt_template_id(settings: &Settings) -> &str {
    settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| {
            advanced
                .lorebook_entry_generator_prompt_template_id
                .as_deref()
        })
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(crate::chat_manager::prompts::APP_LOREBOOK_ENTRY_WRITER_TEMPLATE_ID)
}

fn selected_keyword_prompt_template_id(settings: &Settings) -> &str {
    settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| {
            advanced
                .lorebook_keyword_generator_prompt_template_id
                .as_deref()
        })
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(crate::chat_manager::prompts::APP_LOREBOOK_KEYWORD_GENERATOR_TEMPLATE_ID)
}

fn load_lorebook_entry_prompt_entries(
    app: &AppHandle,
    template_id: &str,
) -> (Vec<SystemPromptEntry>, bool) {
    match crate::chat_manager::prompts::get_template(app, template_id) {
        Ok(Some(template)) => {
            if !template.entries.is_empty() {
                (template.entries, template.condense_prompt_entries)
            } else if !template.content.trim().is_empty() {
                (
                    vec![SystemPromptEntry {
                        id: "lorebook_entry_single_entry".to_string(),
                        name: "Lorebook Entry Writer".to_string(),
                        role: PromptEntryRole::System,
                        content: template.content,
                        enabled: true,
                        injection_position: PromptEntryPosition::Relative,
                        injection_depth: 0,
                        conditional_min_messages: None,
                        interval_turns: None,
                        system_prompt: true,
                        conditions: None,
                        prompt_entry_payload: None,
                    }],
                    template.condense_prompt_entries,
                )
            } else {
                (
                    get_base_prompt_entries(PromptType::LorebookEntryWriterPrompt),
                    false,
                )
            }
        }
        _ => (
            get_base_prompt_entries(PromptType::LorebookEntryWriterPrompt),
            false,
        ),
    }
}

fn load_lorebook_keyword_prompt_entries(
    app: &AppHandle,
    template_id: &str,
) -> (Vec<SystemPromptEntry>, bool) {
    match crate::chat_manager::prompts::get_template(app, template_id) {
        Ok(Some(template)) => {
            if !template.entries.is_empty() {
                (template.entries, template.condense_prompt_entries)
            } else if !template.content.trim().is_empty() {
                (
                    vec![SystemPromptEntry {
                        id: "lorebook_keyword_single_entry".to_string(),
                        name: "Lorebook Keyword Generator".to_string(),
                        role: PromptEntryRole::System,
                        content: template.content,
                        enabled: true,
                        injection_position: PromptEntryPosition::Relative,
                        injection_depth: 0,
                        conditional_min_messages: None,
                        interval_turns: None,
                        system_prompt: true,
                        conditions: None,
                        prompt_entry_payload: None,
                    }],
                    template.condense_prompt_entries,
                )
            } else {
                (
                    get_base_prompt_entries(PromptType::LorebookKeywordGeneratorPrompt),
                    false,
                )
            }
        }
        _ => (
            get_base_prompt_entries(PromptType::LorebookKeywordGeneratorPrompt),
            false,
        ),
    }
}

fn condense_prompt_entries(entries: Vec<SystemPromptEntry>) -> Vec<SystemPromptEntry> {
    let mut condensed: Vec<SystemPromptEntry> = Vec::new();

    for entry in entries {
        let trimmed = entry.content.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Some(last) = condensed.last_mut() {
            let same_role = last.role == entry.role;
            let same_position = last.injection_position == entry.injection_position;
            let same_depth = last.injection_depth == entry.injection_depth;
            let same_payload = last.prompt_entry_payload == entry.prompt_entry_payload;
            let same_conditions = last.conditions == entry.conditions;

            if same_role && same_position && same_depth && same_payload && same_conditions {
                if !last.content.trim().is_empty() {
                    last.content.push_str("\n\n");
                }
                last.content.push_str(trimmed);
                continue;
            }
        }

        let mut next = entry.clone();
        next.content = trimmed.to_string();
        condensed.push(next);
    }

    condensed
}

fn replace_custom_placeholder(content: &str, key: &str, value: &str) -> String {
    content.replace(key, value)
}

fn render_simple_prompt_content(content: &str, replacements: &[(&str, &str)]) -> String {
    replacements
        .iter()
        .fold(content.to_string(), |next, (key, value)| {
            replace_custom_placeholder(&next, key, value)
        })
}

fn render_lorebook_entry_prompt_content(
    app: &AppHandle,
    settings: &Settings,
    character: &Character,
    persona: Option<&Persona>,
    session: &Session,
    content: &str,
    lorebook_name: &str,
    existing_entries: &str,
    direction_prompt: &str,
    selected_messages: &str,
    memory_summary: &str,
    selected_memories: &str,
) -> String {
    let rendered = crate::chat_manager::prompt_engine::render_with_context(
        app, content, character, persona, session, settings,
    );
    let rendered = replace_custom_placeholder(&rendered, "{{lorebook_name}}", lorebook_name);
    let rendered = replace_custom_placeholder(&rendered, "{{character_name}}", &character.name);
    let rendered = replace_custom_placeholder(&rendered, "{{session_title}}", &session.title);
    let rendered = replace_custom_placeholder(&rendered, "{{existing_entries}}", existing_entries);
    let rendered = replace_custom_placeholder(&rendered, "{{direction_prompt}}", direction_prompt);
    let rendered =
        replace_custom_placeholder(&rendered, "{{selected_messages}}", selected_messages);
    let rendered = replace_custom_placeholder(&rendered, "{{memory_summary}}", memory_summary);
    replace_custom_placeholder(&rendered, "{{selected_memories}}", selected_memories)
}

fn render_lorebook_entry_prompt_entries(
    app: &AppHandle,
    settings: &Settings,
    model: &Model,
    character: &Character,
    persona: Option<&Persona>,
    session: &Session,
    lorebook_name: &str,
    existing_entries: &str,
    direction_prompt: &str,
    selected_messages: &str,
    memory_summary: &str,
    selected_memories: &str,
    info_source: crate::chat_manager::types::PromptEntryInfoSource,
) -> Vec<SystemPromptEntry> {
    let (template_entries, should_condense) =
        load_lorebook_entry_prompt_entries(app, selected_prompt_template_id(settings));
    let recent_text = [
        direction_prompt,
        selected_messages,
        memory_summary,
        selected_memories,
    ]
    .join("\n");
    let condition_context = PromptEntryConditionContext {
        chat_mode: PromptEntryChatMode::Direct,
        info_source,
        scene_generation_enabled: false,
        avatar_generation_enabled: false,
        has_scene: session.selected_scene_id.is_some(),
        has_scene_direction: false,
        has_persona: persona.is_some(),
        message_count: session.messages.len(),
        participant_count: 2,
        recent_text: &recent_text,
        dynamic_memory_enabled: false,
        has_memory_summary: !memory_summary.trim().is_empty() && memory_summary.trim() != "(none)",
        has_key_memories: !selected_memories.trim().is_empty()
            && selected_memories.trim() != "(none)",
        has_lorebook_content: !existing_entries.trim().is_empty(),
        does_author_note_exists: session
            .author_note
            .as_deref()
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false),
        has_subject_description: false,
        has_current_description: false,
        has_character_reference_images: false,
        has_chat_background: false,
        has_persona_reference_images: false,
        has_character_reference_text: false,
        has_persona_reference_text: false,
        input_scopes: &model.input_scopes,
        output_scopes: &model.output_scopes,
        provider_id: Some(model.provider_id.as_str()),
        reasoning_enabled: model
            .advanced_model_settings
            .as_ref()
            .and_then(|cfg| cfg.reasoning_enabled)
            .unwrap_or(false),
        vision_enabled: false,
    };

    let mut rendered_entries = Vec::new();
    for entry in template_entries {
        if !entry_is_active(&entry, &condition_context) {
            continue;
        }

        let rendered = render_lorebook_entry_prompt_content(
            app,
            settings,
            character,
            persona,
            session,
            &entry.content,
            lorebook_name,
            existing_entries,
            direction_prompt,
            selected_messages,
            memory_summary,
            selected_memories,
        );
        if rendered.trim().is_empty() && entry.prompt_entry_payload.is_none() {
            continue;
        }
        let mut next_entry = entry.clone();
        next_entry.content = rendered;
        rendered_entries.push(next_entry);
    }

    if should_condense {
        condense_prompt_entries(rendered_entries)
    } else {
        rendered_entries
    }
}

fn render_lorebook_keyword_prompt_entries(
    app: &AppHandle,
    settings: &Settings,
    model: &Model,
    entry_title: &str,
    entry_content: &str,
    existing_keywords: &str,
    direction_prompt: &str,
) -> Vec<SystemPromptEntry> {
    let (template_entries, should_condense) =
        load_lorebook_keyword_prompt_entries(app, selected_keyword_prompt_template_id(settings));
    let recent_text = [
        entry_title,
        entry_content,
        existing_keywords,
        direction_prompt,
    ]
    .join("\n");
    let condition_context = PromptEntryConditionContext {
        chat_mode: PromptEntryChatMode::Direct,
        info_source: crate::chat_manager::types::PromptEntryInfoSource::Messages,
        scene_generation_enabled: false,
        avatar_generation_enabled: false,
        has_scene: false,
        has_scene_direction: false,
        has_persona: false,
        message_count: 0,
        participant_count: 1,
        recent_text: &recent_text,
        dynamic_memory_enabled: false,
        has_memory_summary: false,
        has_key_memories: false,
        has_lorebook_content: !entry_content.trim().is_empty(),
        does_author_note_exists: false,
        has_subject_description: false,
        has_current_description: false,
        has_character_reference_images: false,
        has_chat_background: false,
        has_persona_reference_images: false,
        has_character_reference_text: false,
        has_persona_reference_text: false,
        input_scopes: &model.input_scopes,
        output_scopes: &model.output_scopes,
        provider_id: Some(model.provider_id.as_str()),
        reasoning_enabled: model
            .advanced_model_settings
            .as_ref()
            .and_then(|cfg| cfg.reasoning_enabled)
            .unwrap_or(false),
        vision_enabled: false,
    };
    let replacements = [
        ("{{entry_title}}", entry_title),
        ("{{entry_content}}", entry_content),
        ("{{existing_keywords}}", existing_keywords),
        ("{{direction_prompt}}", direction_prompt),
    ];

    let mut rendered_entries = Vec::new();
    for entry in template_entries {
        if !entry_is_active(&entry, &condition_context) {
            continue;
        }

        let rendered = render_simple_prompt_content(&entry.content, &replacements);
        if rendered.trim().is_empty() && entry.prompt_entry_payload.is_none() {
            continue;
        }
        let mut next_entry = entry.clone();
        next_entry.content = rendered;
        rendered_entries.push(next_entry);
    }

    if should_condense {
        condense_prompt_entries(rendered_entries)
    } else {
        rendered_entries
    }
}

fn prompt_entries_to_messages_with_instruction(
    provider_cred: &ProviderCredential,
    entries: &[SystemPromptEntry],
    final_instruction: &str,
) -> Vec<Value> {
    let system_role = crate::chat_manager::request_builder::system_role_for(provider_cred);
    let mut messages = Vec::new();

    let relative_entries: Vec<&SystemPromptEntry> = entries
        .iter()
        .filter(|entry| entry.injection_position != PromptEntryPosition::InChat)
        .collect();
    for entry in relative_entries {
        let role = match entry.role {
            PromptEntryRole::System => system_role.as_ref(),
            PromptEntryRole::User => "user",
            PromptEntryRole::Assistant => "assistant",
        };
        let trimmed = entry.content.trim();
        if trimmed.is_empty() {
            continue;
        }
        messages.push(json!({
            "role": role,
            "content": trimmed,
        }));
    }

    let in_chat_entries: Vec<&SystemPromptEntry> = entries
        .iter()
        .filter(|entry| entry.injection_position == PromptEntryPosition::InChat)
        .collect();
    if !in_chat_entries.is_empty() {
        let base_len = messages.len();
        let turn_count = base_len;
        let mut inserts: Vec<(usize, usize, &SystemPromptEntry)> = in_chat_entries
            .iter()
            .enumerate()
            .filter_map(|(idx, entry)| {
                if !should_insert_in_chat_prompt_entry(entry, turn_count) {
                    return None;
                }
                let depth = entry.injection_depth as usize;
                let pos = base_len.saturating_sub(depth);
                Some((pos, idx, *entry))
            })
            .collect();
        inserts.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        for (offset, (pos, _, entry)) in inserts.into_iter().enumerate() {
            let role = match entry.role {
                PromptEntryRole::System => system_role.as_ref(),
                PromptEntryRole::User => "user",
                PromptEntryRole::Assistant => "assistant",
            };
            let trimmed = entry.content.trim();
            if trimmed.is_empty() {
                continue;
            }
            messages.insert(
                (pos + offset).min(messages.len()),
                json!({
                    "role": role,
                    "content": trimmed,
                }),
            );
        }
    }

    messages.push(json!({
        "role": "user",
        "content": final_instruction,
    }));

    messages
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LorebookEntrySource {
    Messages,
    Memory,
    Mixed,
}

fn prompt_entries_to_messages(
    provider_cred: &ProviderCredential,
    entries: &[SystemPromptEntry],
    force: bool,
    source: LorebookEntrySource,
) -> Vec<Value> {
    let final_instruction = match (source, force) {
        (LorebookEntrySource::Messages, false) => {
            "Analyze the selected transcript and return exactly one result now. Use the write_lorebook_entry tool when there is a durable lorebook entry to create. Use no_entry when there is not."
        }
        (LorebookEntrySource::Messages, true) => {
            "Analyze the selected transcript and return exactly one result now. You MUST call write_lorebook_entry. The no_entry option is disabled — produce the best possible durable lorebook entry even if facts seem weak or already covered."
        }
        (LorebookEntrySource::Memory, false) => {
            "Analyze the dynamic memory context summary and the selected memories, then return exactly one result now. Use the write_lorebook_entry tool when there is a durable lorebook entry to create. Use no_entry when there is not."
        }
        (LorebookEntrySource::Memory, true) => {
            "Analyze the dynamic memory context summary and the selected memories, then return exactly one result now. You MUST call write_lorebook_entry. The no_entry option is disabled — produce the best possible durable lorebook entry even if the memories seem weak or already covered."
        }
        (LorebookEntrySource::Mixed, false) => {
            "Analyze every provided input section that is not marked (none) — selected messages, dynamic memory context summary, and selected memories — and return exactly one result now. Use the write_lorebook_entry tool when there is a durable lorebook entry to create. Use no_entry when there is not."
        }
        (LorebookEntrySource::Mixed, true) => {
            "Analyze every provided input section that is not marked (none) — selected messages, dynamic memory context summary, and selected memories — and return exactly one result now. You MUST call write_lorebook_entry. The no_entry option is disabled — produce the best possible durable lorebook entry even if facts seem weak or already covered."
        }
    };
    prompt_entries_to_messages_with_instruction(provider_cred, entries, final_instruction)
}

fn format_existing_entries(entries: &[crate::storage_manager::lorebook::LorebookEntry]) -> String {
    if entries.is_empty() {
        return "(none)".to_string();
    }

    entries
        .iter()
        .map(|entry| {
            let title = if entry.title.trim().is_empty() {
                entry
                    .keywords
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "Untitled entry".to_string())
            } else {
                entry.title.trim().to_string()
            };
            let keywords = if entry.always_active {
                "always active".to_string()
            } else if entry.keywords.is_empty() {
                "no keywords".to_string()
            } else {
                format!("keywords: {}", entry.keywords.join(", "))
            };
            let content = entry.content.trim();
            if content.is_empty() {
                format!("- {} ({})", title, keywords)
            } else {
                format!("- {} ({}): {}", title, keywords, content)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_selected_memories(
    memory_ids: &[String],
    embeddings: &[crate::chat_manager::types::MemoryEmbedding],
    legacy: &[String],
) -> String {
    if memory_ids.is_empty() {
        return "(none)".to_string();
    }
    let id_set: HashSet<&str> = memory_ids.iter().map(String::as_str).collect();
    let mut lines: Vec<String> = Vec::new();
    let mut index = 1usize;
    for embedding in embeddings {
        if id_set.contains(embedding.id.as_str()) {
            let trimmed = embedding.text.trim();
            if !trimmed.is_empty() {
                lines.push(format!("{}. {}", index, trimmed));
                index += 1;
            }
        }
    }
    for (legacy_index, text) in legacy.iter().enumerate() {
        let synthetic_id = format!("legacy:{}", legacy_index);
        if id_set.contains(synthetic_id.as_str()) {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                lines.push(format!("{}. {}", index, trimmed));
                index += 1;
            }
        }
    }
    if lines.is_empty() {
        "(none)".to_string()
    } else {
        lines.join("\n")
    }
}

fn format_selected_messages(messages: &[StoredMessage]) -> String {
    messages
        .iter()
        .enumerate()
        .map(|(index, message)| {
            let content = if message.content.trim().is_empty() {
                "[empty message]".to_string()
            } else {
                message.content.trim().to_string()
            };
            format!("{}. {}: {}", index + 1, message.role, content)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn build_lorebook_entry_tool_config(force: bool) -> ToolConfig {
    let mut tools = vec![ToolDefinition {
        name: "write_lorebook_entry".to_string(),
        description: Some(
            "Create one lorebook entry draft from the selected transcript.".to_string(),
        ),
        parameters: json!({
            "type": "object",
            "properties": {
                "title": { "type": "string", "description": "Short entry title" },
                "keywords": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Trigger keywords for this entry"
                },
                "content": { "type": "string", "description": "Final lorebook entry content" },
                "alwaysActive": { "type": "boolean", "description": "If true, entry should not require keywords" }
            },
            "required": ["title", "content"]
        }),
    }];

    if !force {
        tools.push(ToolDefinition {
            name: "no_entry".to_string(),
            description: Some(
                "Use this when the selected messages do not justify a durable lorebook entry."
                    .to_string(),
            ),
            parameters: json!({
                "type": "object",
                "properties": {
                    "reason": { "type": "string", "description": "Short explanation for why no entry should be created" }
                },
                "required": ["reason"]
            }),
        });
    }

    ToolConfig {
        tools,
        choice: Some(ToolChoice::Required),
    }
}

fn build_lorebook_keyword_tool_config() -> ToolConfig {
    ToolConfig {
        tools: vec![ToolDefinition {
            name: "write_lorebook_keywords".to_string(),
            description: Some(
                "Generate one deduplicated keyword list for the lorebook entry draft.".to_string(),
            ),
            parameters: json!({
                "type": "object",
                "properties": {
                    "keywords": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Trigger keywords, aliases, names, locations, and other durable lookup terms"
                    }
                },
                "required": ["keywords"]
            }),
        }],
        choice: Some(ToolChoice::Required),
    }
}

fn normalize_keywords(value: Option<&Value>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut result = match value {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .filter_map(|value| {
                let normalized = value.to_string();
                let key = normalized.to_ascii_lowercase();
                if seen.insert(key) {
                    Some(normalized)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>(),
        Some(Value::String(value)) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                Vec::new()
            } else {
                vec![trimmed.to_string()]
            }
        }
        _ => Vec::new(),
    };
    result.truncate(MAX_GENERATED_KEYWORDS);
    result
}

fn normalize_entry_draft(arguments: &Value) -> Result<LorebookEntryDraft, String> {
    let Some(obj) = arguments.as_object() else {
        return Err("write_lorebook_entry arguments must be an object".to_string());
    };

    let title = obj
        .get("title")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "write_lorebook_entry is missing a non-empty title".to_string())?
        .to_string();

    let content = obj
        .get("content")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "write_lorebook_entry is missing non-empty content".to_string())?
        .to_string();

    let always_active = obj
        .get("alwaysActive")
        .or_else(|| obj.get("always_active"))
        .and_then(Value::as_bool)
        .unwrap_or(false);

    Ok(LorebookEntryDraft {
        title,
        keywords: normalize_keywords(obj.get("keywords")),
        content,
        always_active,
    })
}

fn result_from_tool_calls(calls: &[ToolCall]) -> Result<Option<LorebookEntryDraftResult>, String> {
    let mut pending_no_entry: Option<LorebookEntryDraftResult> = None;

    for call in calls {
        match call.name.as_str() {
            "write_lorebook_entry" => {
                let draft = normalize_entry_draft(&call.arguments)?;
                return Ok(Some(LorebookEntryDraftResult {
                    kind: "entry".to_string(),
                    draft: Some(draft),
                    reason: None,
                }));
            }
            "no_entry" => {
                let reason = call
                    .arguments
                    .get("reason")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .unwrap_or("The selected messages do not establish durable lore.")
                    .to_string();
                pending_no_entry = Some(LorebookEntryDraftResult {
                    kind: "none".to_string(),
                    draft: None,
                    reason: Some(reason),
                });
            }
            _ => {}
        }
    }

    Ok(pending_no_entry)
}

fn normalize_structured_fallback_text(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.starts_with("```") {
        let mut lines = trimmed.lines();
        let _ = lines.next();
        let mut body: Vec<&str> = lines.collect();
        if body
            .last()
            .map(|line| line.trim() == "```")
            .unwrap_or(false)
        {
            body.pop();
        }
        return body.join("\n").trim().to_string();
    }
    trimmed.to_string()
}

fn extract_json_snippet(raw: &str) -> Option<&str> {
    let mut start = None;
    let mut stack: Vec<char> = Vec::new();
    let mut in_string = false;
    let mut escape = false;

    for (idx, ch) in raw.char_indices() {
        if in_string {
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '{' | '[' => {
                if start.is_none() {
                    start = Some(idx);
                }
                stack.push(ch);
            }
            '}' => {
                if stack.pop() != Some('{') {
                    return None;
                }
                if stack.is_empty() {
                    return start.map(|begin| &raw[begin..=idx]);
                }
            }
            ']' => {
                if stack.pop() != Some('[') {
                    return None;
                }
                if stack.is_empty() {
                    return start.map(|begin| &raw[begin..=idx]);
                }
            }
            _ => {}
        }
    }

    None
}

fn parse_fallback_json_result(raw: &str) -> Result<LorebookEntryDraftResult, String> {
    let normalized = normalize_structured_fallback_text(raw);
    let snippet = extract_json_snippet(&normalized).unwrap_or(normalized.as_str());
    let value: Value = serde_json::from_str(snippet)
        .map_err(|err| format!("fallback JSON parse error: {}", err))?;

    let node = value
        .get("result")
        .or_else(|| value.get("response"))
        .unwrap_or(&value);
    let Some(obj) = node.as_object() else {
        return Err("fallback JSON result must be an object".to_string());
    };

    let name = obj
        .get("name")
        .or_else(|| obj.get("tool"))
        .or_else(|| obj.get("action"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "fallback JSON result is missing a name".to_string())?;

    let arguments = obj
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| Value::Object(Map::new()));

    match name {
        "write_lorebook_entry" => Ok(LorebookEntryDraftResult {
            kind: "entry".to_string(),
            draft: Some(normalize_entry_draft(&arguments)?),
            reason: None,
        }),
        "no_entry" => Ok(LorebookEntryDraftResult {
            kind: "none".to_string(),
            draft: None,
            reason: Some(
                arguments
                    .get("reason")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .unwrap_or("The selected messages do not establish durable lore.")
                    .to_string(),
            ),
        }),
        other => Err(format!(
            "fallback JSON returned unsupported result '{}'",
            other
        )),
    }
}

fn parse_keyword_fallback_json_result(raw: &str) -> Result<LorebookKeywordDraftResult, String> {
    let normalized = normalize_structured_fallback_text(raw);
    let snippet = extract_json_snippet(&normalized).unwrap_or(normalized.as_str());
    let value: Value = serde_json::from_str(snippet)
        .map_err(|err| format!("fallback JSON parse error: {}", err))?;

    let node = value
        .get("result")
        .or_else(|| value.get("response"))
        .unwrap_or(&value);
    let Some(obj) = node.as_object() else {
        return Err("fallback JSON result must be an object".to_string());
    };

    let name = obj
        .get("name")
        .or_else(|| obj.get("tool"))
        .or_else(|| obj.get("action"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "fallback JSON result is missing a name".to_string())?;

    if name != "write_lorebook_keywords" {
        return Err(format!(
            "fallback JSON returned unsupported result '{}'",
            name
        ));
    }

    let arguments = obj
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| Value::Object(Map::new()));
    Ok(LorebookKeywordDraftResult {
        keywords: normalize_keywords(arguments.get("keywords")),
    })
}

fn attr_value(element: &BytesStart<'_>, key: &[u8]) -> Option<String> {
    for attr in element.attributes().flatten() {
        if attr.key.as_ref() == key {
            return attr.unescape_value().ok().map(|value| value.into_owned());
        }
    }
    None
}

fn decode_xml_text(raw: &[u8]) -> Result<String, String> {
    let text = String::from_utf8_lossy(raw);
    unescape(&text)
        .map(|cow| cow.into_owned())
        .map_err(|err| format!("fallback XML text decode error: {}", err))
}

fn decode_xml_general_ref(raw: BytesRef<'_>) -> Result<String, String> {
    if let Ok(Some(ch)) = raw.resolve_char_ref() {
        return Ok(ch.to_string());
    }

    let content = raw
        .xml_content()
        .map_err(|err| format!("fallback XML reference decode error: {}", err))?;
    if let Some(entity) = resolve_xml_entity(&content) {
        Ok(entity.to_string())
    } else {
        Ok(format!("&{};", content))
    }
}

fn parse_fallback_xml_result(raw: &str) -> Result<LorebookEntryDraftResult, String> {
    let normalized = normalize_structured_fallback_text(raw);
    let mut reader = Reader::from_str(&normalized);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut current_operation: Option<String> = None;
    let mut current_field: Option<String> = None;
    let mut title = String::new();
    let mut content = String::new();
    let mut reason = String::new();
    let mut current_keyword = String::new();
    let mut keywords: Vec<String> = Vec::new();
    let mut always_active = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(event)) => {
                let tag = String::from_utf8_lossy(event.name().as_ref()).to_string();
                if current_operation.is_none()
                    && matches!(tag.as_str(), "write_lorebook_entry" | "no_entry")
                {
                    if tag == "write_lorebook_entry" {
                        always_active = attr_value(&event, b"alwaysActive")
                            .or_else(|| attr_value(&event, b"always_active"))
                            .map(|value| {
                                matches!(
                                    value.trim().to_ascii_lowercase().as_str(),
                                    "true" | "1" | "yes"
                                )
                            })
                            .unwrap_or(false);
                    }
                    current_operation = Some(tag);
                } else if current_operation.is_some() {
                    current_field = Some(tag);
                }
            }
            Ok(Event::Empty(event)) => {
                let tag = String::from_utf8_lossy(event.name().as_ref()).to_string();
                if current_operation.is_none() && tag == "write_lorebook_entry" {
                    always_active = attr_value(&event, b"alwaysActive")
                        .or_else(|| attr_value(&event, b"always_active"))
                        .map(|value| {
                            matches!(
                                value.trim().to_ascii_lowercase().as_str(),
                                "true" | "1" | "yes"
                            )
                        })
                        .unwrap_or(false);
                    current_operation = Some(tag);
                } else if current_operation.is_none() && tag == "no_entry" {
                    reason = attr_value(&event, b"reason").unwrap_or_default();
                    current_operation = Some(tag);
                }
            }
            Ok(Event::Text(event)) => {
                let text = decode_xml_text(event.as_ref())?;
                match current_field.as_deref() {
                    Some("title") => title.push_str(&text),
                    Some("content") => content.push_str(&text),
                    Some("reason") => reason.push_str(&text),
                    Some("keyword") => current_keyword.push_str(&text),
                    _ => {}
                }
            }
            Ok(Event::GeneralRef(reference)) => {
                let text = decode_xml_general_ref(reference)?;
                match current_field.as_deref() {
                    Some("title") => title.push_str(&text),
                    Some("content") => content.push_str(&text),
                    Some("reason") => reason.push_str(&text),
                    Some("keyword") => current_keyword.push_str(&text),
                    _ => {}
                }
            }
            Ok(Event::End(event)) => {
                let tag = String::from_utf8_lossy(event.name().as_ref()).to_string();
                if tag == "keyword" {
                    let trimmed = current_keyword.trim();
                    if !trimmed.is_empty() {
                        keywords.push(trimmed.to_string());
                    }
                    current_keyword.clear();
                    current_field = None;
                } else if matches!(tag.as_str(), "title" | "content" | "reason" | "keywords") {
                    current_field = None;
                } else if current_operation.as_deref() == Some(tag.as_str()) {
                    break;
                } else if LOREBOOK_RESULT_XML_ROOT_TAGS.contains(&tag.as_str()) {
                    current_field = None;
                } else {
                    current_field = None;
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => {
                return Err(format!("fallback XML parse error: {}", err));
            }
            _ => {}
        }
        buf.clear();
    }

    match current_operation.as_deref() {
        Some("write_lorebook_entry") => Ok(LorebookEntryDraftResult {
            kind: "entry".to_string(),
            draft: Some(normalize_entry_draft(&json!({
                "title": title.trim(),
                "keywords": keywords,
                "content": content.trim(),
                "alwaysActive": always_active,
            }))?),
            reason: None,
        }),
        Some("no_entry") => Ok(LorebookEntryDraftResult {
            kind: "none".to_string(),
            draft: None,
            reason: Some(
                {
                    let trimmed = reason.trim();
                    if trimmed.is_empty() {
                        "The selected messages do not establish durable lore."
                    } else {
                        trimmed
                    }
                }
                .to_string(),
            ),
        }),
        _ => Err("fallback XML response did not contain a lorebook result".to_string()),
    }
}

fn parse_keyword_fallback_xml_result(raw: &str) -> Result<LorebookKeywordDraftResult, String> {
    let normalized = normalize_structured_fallback_text(raw);
    let mut reader = Reader::from_str(&normalized);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut current_operation: Option<String> = None;
    let mut current_field: Option<String> = None;
    let mut current_keyword = String::new();
    let mut keywords: Vec<String> = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(event)) => {
                let tag = String::from_utf8_lossy(event.name().as_ref()).to_string();
                if current_operation.is_none() && tag == "write_lorebook_keywords" {
                    current_operation = Some(tag);
                } else if current_operation.is_some() {
                    current_field = Some(tag);
                }
            }
            Ok(Event::Text(event)) => {
                let text = decode_xml_text(event.as_ref())?;
                if current_field.as_deref() == Some("keyword") {
                    current_keyword.push_str(&text);
                }
            }
            Ok(Event::GeneralRef(reference)) => {
                let text = decode_xml_general_ref(reference)?;
                if current_field.as_deref() == Some("keyword") {
                    current_keyword.push_str(&text);
                }
            }
            Ok(Event::End(event)) => {
                let tag = String::from_utf8_lossy(event.name().as_ref()).to_string();
                if tag == "keyword" {
                    let trimmed = current_keyword.trim();
                    if !trimmed.is_empty() {
                        keywords.push(trimmed.to_string());
                    }
                    current_keyword.clear();
                    current_field = None;
                } else if matches!(tag.as_str(), "keywords" | "write_lorebook_keywords") {
                    current_field = None;
                    if tag == "write_lorebook_keywords" {
                        break;
                    }
                } else if LOREBOOK_RESULT_XML_ROOT_TAGS.contains(&tag.as_str()) {
                    current_field = None;
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(format!("fallback XML parse error: {}", err)),
            _ => {}
        }
        buf.clear();
    }

    if current_operation.as_deref() != Some("write_lorebook_keywords") {
        return Err("fallback XML response did not contain lorebook keywords".to_string());
    }

    Ok(LorebookKeywordDraftResult {
        keywords: normalize_keywords(Some(&json!(keywords))),
    })
}

fn parse_fallback_result(
    raw: &str,
    format: crate::chat_manager::types::DynamicMemoryStructuredFallbackFormat,
) -> Result<LorebookEntryDraftResult, String> {
    match format {
        crate::chat_manager::types::DynamicMemoryStructuredFallbackFormat::Json => {
            parse_fallback_json_result(raw)
        }
        crate::chat_manager::types::DynamicMemoryStructuredFallbackFormat::Xml => {
            parse_fallback_xml_result(raw)
        }
    }
}

fn parse_keyword_fallback_result(
    raw: &str,
    format: crate::chat_manager::types::DynamicMemoryStructuredFallbackFormat,
) -> Result<LorebookKeywordDraftResult, String> {
    match format {
        crate::chat_manager::types::DynamicMemoryStructuredFallbackFormat::Json => {
            parse_keyword_fallback_json_result(raw)
        }
        crate::chat_manager::types::DynamicMemoryStructuredFallbackFormat::Xml => {
            parse_keyword_fallback_xml_result(raw)
        }
    }
}

fn lorebook_keyword_fallback_prompt(
    format: crate::chat_manager::types::DynamicMemoryStructuredFallbackFormat,
) -> &'static str {
    match format {
        crate::chat_manager::types::DynamicMemoryStructuredFallbackFormat::Json => {
            LOREBOOK_KEYWORDS_JSON_FALLBACK_PROMPT
        }
        crate::chat_manager::types::DynamicMemoryStructuredFallbackFormat::Xml => {
            LOREBOOK_KEYWORDS_XML_FALLBACK_PROMPT
        }
    }
}

fn tool_choice_requires_auto(error: &str) -> bool {
    let lower = error.to_ascii_lowercase();
    lower.contains("tool choice must be auto")
        || lower.contains("tool_choice must be auto")
        || (lower.contains("tool choice") && lower.contains("auto"))
}

fn parallel_tool_calls_requires_disable(error: &str) -> bool {
    let lower = error.to_ascii_lowercase();
    (lower.contains("parallel_tool_calls") || lower.contains("parallel tool calls"))
        && (lower.contains("unsupported")
            || lower.contains("unknown")
            || lower.contains("invalid")
            || lower.contains("unexpected")
            || lower.contains("must be"))
}

fn tool_extra_fields_with_parallel_disabled(
    extra_body_fields: Option<HashMap<String, Value>>,
) -> Option<HashMap<String, Value>> {
    let mut extra = extra_body_fields.unwrap_or_default();
    extra.insert("parallel_tool_calls".to_string(), json!(false));
    if extra.is_empty() {
        None
    } else {
        Some(extra)
    }
}

fn tool_config_with_auto_choice(tool_config: &ToolConfig) -> ToolConfig {
    let mut cloned = tool_config.clone();
    cloned.choice = Some(ToolChoice::Auto);
    cloned
}

async fn send_lorebook_entry_request(
    app: &AppHandle,
    provider_cred: &ProviderCredential,
    model: &Model,
    api_key: &str,
    messages_for_api: &Vec<Value>,
    max_tokens: u32,
    context_length: Option<u32>,
    extra_body_fields: Option<HashMap<String, Value>>,
    tool_config: Option<&ToolConfig>,
) -> Result<ApiResponse, String> {
    let built = crate::chat_manager::request_builder::build_chat_request(
        provider_cred,
        api_key,
        &model.name,
        messages_for_api,
        None,
        Some(0.2),
        Some(1.0),
        max_tokens,
        context_length,
        false,
        None,
        None,
        None,
        None,
        tool_config,
        false,
        None,
        None,
        false,
        extra_body_fields.clone(),
    );

    let first_response = api_request(
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
            provider_id: Some(provider_cred.provider_id.clone()),
        },
    )
    .await?;

    if !first_response.ok {
        let fallback = format!("Provider returned status {}", first_response.status);
        let err_message = extract_error_message(first_response.data()).unwrap_or(fallback);

        if let Some(cfg) = tool_config {
            if provider_cred.provider_id == "llamacpp"
                && parallel_tool_calls_requires_disable(&err_message)
            {
                let built = crate::chat_manager::request_builder::build_chat_request(
                    provider_cred,
                    api_key,
                    &model.name,
                    messages_for_api,
                    None,
                    Some(0.2),
                    Some(1.0),
                    max_tokens,
                    context_length,
                    false,
                    None,
                    None,
                    None,
                    None,
                    tool_config,
                    false,
                    None,
                    None,
                    false,
                    tool_extra_fields_with_parallel_disabled(extra_body_fields.clone()),
                );

                return api_request(
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
                        provider_id: Some(provider_cred.provider_id.clone()),
                    },
                )
                .await;
            }

            if !matches!(cfg.choice, Some(ToolChoice::Auto))
                && tool_choice_requires_auto(&err_message)
            {
                let built = crate::chat_manager::request_builder::build_chat_request(
                    provider_cred,
                    api_key,
                    &model.name,
                    messages_for_api,
                    None,
                    Some(0.2),
                    Some(1.0),
                    max_tokens,
                    context_length,
                    false,
                    None,
                    None,
                    None,
                    None,
                    Some(&tool_config_with_auto_choice(cfg)),
                    false,
                    None,
                    None,
                    false,
                    extra_body_fields,
                );

                return api_request(
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
                        provider_id: Some(provider_cred.provider_id.clone()),
                    },
                )
                .await;
            }
        }
    }

    Ok(first_response)
}

fn load_selected_messages(
    app: &AppHandle,
    session_id: &str,
    message_ids: &[String],
) -> Result<Vec<StoredMessage>, String> {
    if message_ids.is_empty() {
        return Ok(Vec::new());
    }

    let conn = open_db(app)?;
    let placeholders = message_ids
        .iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT id, role, content, created_at, scene_edited, prompt_tokens, completion_tokens, total_tokens, selected_variant_id, is_pinned, memory_refs, used_lorebook_entries, attachments, reasoning FROM messages WHERE session_id = ?1 AND id IN ({}) ORDER BY created_at ASC, id ASC",
        placeholders
    );

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|err| crate::utils::err_to_string(module_path!(), line!(), err))?;

    let mut params: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(message_ids.len() + 1);
    params.push(&session_id);
    for message_id in message_ids {
        params.push(message_id);
    }

    let rows = stmt
        .query_map(rusqlite::params_from_iter(params), |row| {
            Ok(StoredMessage {
                id: row.get::<_, String>(0)?,
                role: row.get::<_, String>(1)?,
                content: row.get::<_, String>(2)?,
                created_at: row.get::<_, i64>(3)? as u64,
                scene_edited: row.get::<_, i64>(4)? != 0,
                usage: None,
                variants: Vec::new(),
                selected_variant_id: row.get::<_, Option<String>>(8)?,
                is_pinned: row.get::<_, i64>(9)? != 0,
                memory_refs: serde_json::from_str::<Vec<String>>(
                    row.get::<_, Option<String>>(10)?.as_deref().unwrap_or("[]"),
                )
                .unwrap_or_default(),
                used_lorebook_entries: serde_json::from_str::<Vec<String>>(
                    row.get::<_, Option<String>>(11)?.as_deref().unwrap_or("[]"),
                )
                .unwrap_or_default(),
                attachments:
                    serde_json::from_str::<Vec<crate::chat_manager::types::ImageAttachment>>(
                        row.get::<_, Option<String>>(12)?.as_deref().unwrap_or("[]"),
                    )
                    .unwrap_or_default(),
                reasoning: row.get::<_, Option<String>>(13)?,
                model_id: None,
                fallback_from_model_id: None,
            })
        })
        .map_err(|err| crate::utils::err_to_string(module_path!(), line!(), err))?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|err| crate::utils::err_to_string(module_path!(), line!(), err))?);
    }
    Ok(result)
}

fn format_existing_keywords(existing_keywords: &[String]) -> String {
    let keywords = existing_keywords
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    if keywords.is_empty() {
        "(none)".to_string()
    } else {
        keywords.join(", ")
    }
}

fn result_keywords_from_tool_calls(
    calls: &[ToolCall],
) -> Result<Option<LorebookKeywordDraftResult>, String> {
    for call in calls {
        if call.name == "write_lorebook_keywords" {
            return Ok(Some(LorebookKeywordDraftResult {
                keywords: normalize_keywords(call.arguments.get("keywords")),
            }));
        }
    }
    Ok(None)
}

pub async fn chat_generate_lorebook_entry_draft(
    app: AppHandle,
    args: ChatGenerateLorebookEntryDraftArgs,
) -> Result<LorebookEntryDraftResult, String> {
    let ChatGenerateLorebookEntryDraftArgs {
        lorebook_id,
        session_id,
        message_ids,
        memory_ids,
        source,
        include_memory_summary,
        direction_prompt,
        force,
    } = args;
    let include_memory_summary = include_memory_summary.unwrap_or(true);

    if lorebook_id.trim().is_empty() {
        return Err("lorebookId cannot be empty".to_string());
    }
    if session_id.trim().is_empty() {
        return Err("sessionId cannot be empty".to_string());
    }

    let source_mode = match source.as_deref().map(str::trim).unwrap_or("messages") {
        "memory" | "memories" | "dynamic_memory" => LorebookEntrySource::Memory,
        "mixed" | "both" | "all" => LorebookEntrySource::Mixed,
        _ => LorebookEntrySource::Messages,
    };

    let messages_enabled = matches!(
        source_mode,
        LorebookEntrySource::Messages | LorebookEntrySource::Mixed
    );
    let memory_enabled = matches!(
        source_mode,
        LorebookEntrySource::Memory | LorebookEntrySource::Mixed
    );

    if source_mode == LorebookEntrySource::Messages && message_ids.is_empty() {
        return Err("At least one message must be selected".to_string());
    }

    let context = ChatContext::initialize(app.clone())?;
    let settings = &context.settings;
    let mut session = session_get_meta_internal(&app, &session_id)?
        .ok_or_else(|| "Session not found".to_string())?;
    let selected_messages = if messages_enabled && !message_ids.is_empty() {
        let loaded = load_selected_messages(&app, &session_id, &message_ids)?;
        if loaded.is_empty() && source_mode == LorebookEntrySource::Messages {
            return Err("No selected messages were found for this session".to_string());
        }
        session.messages = loaded.clone();
        loaded
    } else {
        session.messages = Vec::new();
        Vec::new()
    };

    let memory_summary_text = if memory_enabled && include_memory_summary {
        session
            .memory_summary
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("(none)")
            .to_string()
    } else {
        "(none)".to_string()
    };
    let selected_memories_text = if memory_enabled {
        format_selected_memories(&memory_ids, &session.memory_embeddings, &session.memories)
    } else {
        "(none)".to_string()
    };

    if source_mode == LorebookEntrySource::Memory
        && selected_memories_text == "(none)"
        && memory_summary_text == "(none)"
    {
        return Err(
            "Select at least one memory or ensure a context summary is available".to_string(),
        );
    }
    if source_mode == LorebookEntrySource::Mixed
        && selected_messages.is_empty()
        && selected_memories_text == "(none)"
        && memory_summary_text == "(none)"
    {
        return Err(
            "Select at least one message or memory, or include the context summary".to_string(),
        );
    }

    let character = context.find_character(&session.character_id)?;
    let effective_persona_id = if session.persona_disabled {
        Some("")
    } else {
        session.persona_id.as_deref()
    };
    let persona = context.choose_persona(effective_persona_id);

    let conn = open_db(&app)?;
    let lorebook =
        get_lorebook(&conn, &lorebook_id)?.ok_or_else(|| "Lorebook not found".to_string())?;
    let existing_entries = get_lorebook_entries(&conn, &lorebook_id)?;

    let direction_prompt_text = direction_prompt
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("(none)");
    let selected_messages_text = if !selected_messages.is_empty() {
        format_selected_messages(&selected_messages)
    } else {
        "(none)".to_string()
    };
    let existing_entries_text = format_existing_entries(&existing_entries);

    let (model, credential) = resolve_lorebook_entry_writer_target(
        settings,
        settings
            .advanced_settings
            .as_ref()
            .and_then(|advanced| advanced.lorebook_entry_generator_model_id.as_deref()),
    )?;
    let api_key = require_api_key(&app, credential, "lorebook_entry_generator")?;

    let info_source = match source_mode {
        LorebookEntrySource::Messages => crate::chat_manager::types::PromptEntryInfoSource::Messages,
        LorebookEntrySource::Memory => crate::chat_manager::types::PromptEntryInfoSource::Memory,
        LorebookEntrySource::Mixed => crate::chat_manager::types::PromptEntryInfoSource::Mixed,
    };
    let prompt_entries = render_lorebook_entry_prompt_entries(
        &app,
        settings,
        model,
        &character,
        persona,
        &session,
        &lorebook.name,
        &existing_entries_text,
        direction_prompt_text,
        &selected_messages_text,
        &memory_summary_text,
        &selected_memories_text,
        info_source,
    );
    if prompt_entries.is_empty() {
        return Err(
            "Lorebook entry generator prompt template rendered no usable entries".to_string(),
        );
    }

    let messages_for_api =
        prompt_entries_to_messages(credential, &prompt_entries, force, source_mode);
    let tool_config = build_lorebook_entry_tool_config(force);
    let (request_settings, extra_body_fields) = prepare_default_sampling_request(
        &credential.provider_id,
        &session,
        model,
        settings,
        0.2,
        1.0,
        None,
        None,
        None,
    );
    let fallback_format = lorebook_entry_structured_fallback_format(settings);
    let fallback_label = fallback_format_label(fallback_format);

    let tool_attempt = send_lorebook_entry_request(
        &app,
        credential,
        model,
        &api_key,
        &messages_for_api,
        request_settings.max_tokens,
        request_settings.context_length,
        extra_body_fields.clone(),
        Some(&tool_config),
    )
    .await;

    let tool_failure_reason = match tool_attempt {
        Ok(api_response) => {
            let usage = extract_usage(api_response.data());
            record_usage_if_available(
                &context,
                &usage,
                &session,
                &character,
                model,
                credential,
                &api_key,
                now_millis().unwrap_or(0),
                UsageOperationType::ReplyHelper,
                "lorebook_entry_generator",
            )
            .await;

            if api_response.ok {
                let mut calls = parse_tool_calls(&credential.provider_id, api_response.data());
                if calls.is_empty() {
                    if let Some(text) =
                        extract_text(api_response.data(), Some(&credential.provider_id))
                    {
                        calls = parse_tool_calls_from_text(&text);
                    }
                }

                if let Some(result) = result_from_tool_calls(&calls)? {
                    return Ok(result);
                }

                let response_text =
                    extract_text(api_response.data(), Some(&credential.provider_id))
                        .unwrap_or_default();
                if let Some(result) =
                    result_from_tool_calls(&parse_tool_calls_from_text(&response_text))?
                {
                    return Ok(result);
                }

                if calls.is_empty() {
                    "model returned no usable tool calls".to_string()
                } else {
                    let tool_names = calls
                        .iter()
                        .map(|call| call.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("model returned unsupported tool calls: {}", tool_names)
                }
            } else {
                let fallback = format!("Provider returned status {}", api_response.status);
                extract_error_message(api_response.data()).unwrap_or(fallback)
            }
        }
        Err(err) => err,
    };

    log_warn(
        &app,
        "lorebook_entry_generator",
        format!(
            "tool request failed or was invalid; retrying with {} fallback: {}",
            fallback_label, tool_failure_reason
        ),
    );

    let mut fallback_messages = messages_for_api.clone();
    fallback_messages.push(json!({
        "role": "user",
        "content": fallback_prompt(fallback_format, force),
    }));

    let api_response = send_lorebook_entry_request(
        &app,
        credential,
        model,
        &api_key,
        &fallback_messages,
        request_settings.max_tokens,
        request_settings.context_length,
        extra_body_fields,
        None,
    )
    .await?;

    let usage = extract_usage(api_response.data());
    record_usage_if_available(
        &context,
        &usage,
        &session,
        &character,
        model,
        credential,
        &api_key,
        now_millis().unwrap_or(0),
        UsageOperationType::ReplyHelper,
        "lorebook_entry_generator_fallback",
    )
    .await;

    if !api_response.ok {
        let fallback = format!("Provider returned status {}", api_response.status);
        let err_message = extract_error_message(api_response.data()).unwrap_or(fallback);
        return Err(format!(
            "lorebook entry generation {} fallback failed after tool attempt '{}': {}",
            fallback_label, tool_failure_reason, err_message
        ));
    }

    let fallback_text = extract_text(api_response.data(), Some(&credential.provider_id))
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "Lorebook entry fallback returned no text".to_string())?;

    parse_fallback_result(&fallback_text, fallback_format).map_err(|err| {
        format!(
            "lorebook entry {} fallback parse failed after tool attempt '{}': {}",
            fallback_label, tool_failure_reason, err
        )
    })
}

pub async fn chat_generate_lorebook_keyword_draft(
    app: AppHandle,
    args: ChatGenerateLorebookKeywordDraftArgs,
) -> Result<LorebookKeywordDraftResult, String> {
    let ChatGenerateLorebookKeywordDraftArgs {
        title,
        content,
        direction_prompt,
        existing_keywords,
    } = args;

    let entry_content = content.trim();
    if entry_content.is_empty() {
        return Err("content cannot be empty".to_string());
    }

    let context = ChatContext::initialize(app.clone())?;
    let settings = &context.settings;
    let (model, credential) = resolve_lorebook_entry_writer_target(
        settings,
        settings
            .advanced_settings
            .as_ref()
            .and_then(|advanced| advanced.lorebook_entry_generator_model_id.as_deref()),
    )?;
    let api_key = require_api_key(&app, credential, "lorebook_keyword_generator")?;

    let entry_title = title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("(untitled)");
    let direction_prompt_text = direction_prompt
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("(none)");
    let existing_keywords_text = format_existing_keywords(&existing_keywords);

    let prompt_entries = render_lorebook_keyword_prompt_entries(
        &app,
        settings,
        model,
        entry_title,
        entry_content,
        &existing_keywords_text,
        direction_prompt_text,
    );
    if prompt_entries.is_empty() {
        return Err(
            "Lorebook keyword generator prompt template rendered no usable entries".to_string(),
        );
    }

    let messages_for_api = prompt_entries_to_messages_with_instruction(
        credential,
        &prompt_entries,
        "Analyze the lorebook entry content and return exactly one result now. You MUST call write_lorebook_keywords with a concise, deduplicated keyword list.",
    );
    let tool_config = build_lorebook_keyword_tool_config();
    let fallback_format = lorebook_entry_structured_fallback_format(settings);
    let fallback_label = fallback_format_label(fallback_format);
    let temp_session = Session {
        id: "__lorebook_keyword_generator__".to_string(),
        character_id: String::new(),
        title: "Lorebook Keyword Generator".to_string(),
        background_image_path: None,
        system_prompt: None,
        persona_id: None,
        persona_disabled: true,
        mode: "roleplay".to_string(),
        author_note: None,
        selected_scene_id: None,
        prompt_template_id: None,
        lorebook_ids_override: None,
        voice_autoplay: None,
        advanced_model_settings: None,
        companion_state: None,
        memory_summary: None,
        memories: Vec::new(),
        memory_embeddings: Vec::new(),
        memory_summary_token_count: 0,
        memory_tool_events: Vec::new(),
        memory_status: None,
        memory_error: None,
        memory_progress_step: None,
        messages: Vec::new(),
        archived: false,
        created_at: 0,
        updated_at: 0,
    };
    let (request_settings, extra_body_fields) = prepare_default_sampling_request(
        &credential.provider_id,
        &temp_session,
        model,
        settings,
        0.2,
        1.0,
        None,
        None,
        None,
    );

    let tool_attempt = send_lorebook_entry_request(
        &app,
        credential,
        model,
        &api_key,
        &messages_for_api,
        request_settings.max_tokens,
        request_settings.context_length,
        extra_body_fields.clone(),
        Some(&tool_config),
    )
    .await;

    let tool_failure_reason = match tool_attempt {
        Ok(api_response) => {
            if api_response.ok {
                let mut calls = parse_tool_calls(&credential.provider_id, api_response.data());
                if calls.is_empty() {
                    if let Some(text) =
                        extract_text(api_response.data(), Some(&credential.provider_id))
                    {
                        calls = parse_tool_calls_from_text(&text);
                    }
                }

                if let Some(result) = result_keywords_from_tool_calls(&calls)? {
                    return Ok(result);
                }

                let response_text =
                    extract_text(api_response.data(), Some(&credential.provider_id))
                        .unwrap_or_default();
                if let Some(result) =
                    result_keywords_from_tool_calls(&parse_tool_calls_from_text(&response_text))?
                {
                    return Ok(result);
                }

                if calls.is_empty() {
                    "model returned no usable tool calls".to_string()
                } else {
                    let tool_names = calls
                        .iter()
                        .map(|call| call.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("model returned unsupported tool calls: {}", tool_names)
                }
            } else {
                let fallback = format!("Provider returned status {}", api_response.status);
                extract_error_message(api_response.data()).unwrap_or(fallback)
            }
        }
        Err(err) => err,
    };

    log_warn(
        &app,
        "lorebook_keyword_generator",
        format!(
            "tool request failed or was invalid; retrying with {} fallback: {}",
            fallback_label, tool_failure_reason
        ),
    );

    let mut fallback_messages = messages_for_api.clone();
    fallback_messages.push(json!({
        "role": "user",
        "content": lorebook_keyword_fallback_prompt(fallback_format),
    }));

    let api_response = send_lorebook_entry_request(
        &app,
        credential,
        model,
        &api_key,
        &fallback_messages,
        request_settings.max_tokens,
        request_settings.context_length,
        extra_body_fields,
        None,
    )
    .await?;

    if !api_response.ok {
        let fallback = format!("Provider returned status {}", api_response.status);
        let err_message = extract_error_message(api_response.data()).unwrap_or(fallback);
        return Err(format!(
            "lorebook keyword generation {} fallback failed after tool attempt '{}': {}",
            fallback_label, tool_failure_reason, err_message
        ));
    }

    let fallback_text = extract_text(api_response.data(), Some(&credential.provider_id))
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "Lorebook keyword fallback returned no text".to_string())?;

    parse_keyword_fallback_result(&fallback_text, fallback_format).map_err(|err| {
        format!(
            "lorebook keyword {} fallback parse failed after tool attempt '{}': {}",
            fallback_label, tool_failure_reason, err
        )
    })
}
