use serde_json::{json, Value};
use std::collections::HashMap;
use tauri::AppHandle;

use crate::api::{api_request, ApiRequest};
use crate::chat_manager::execution::find_model_with_credential;
use crate::chat_manager::prompting::entry_conditions::{
    entry_is_active, PromptEntryConditionContext,
};
use crate::chat_manager::prompting::prompts::{
    get_template, APP_LOREBOOK_GENERATOR_COHERENCE_TEMPLATE_ID,
    APP_LOREBOOK_GENERATOR_PLANNER_TEMPLATE_ID, APP_LOREBOOK_GENERATOR_REFINE_TEMPLATE_ID,
    APP_LOREBOOK_GENERATOR_WRITER_TEMPLATE_ID,
};
use crate::chat_manager::prompting::request::{extract_error_message, extract_text};
use crate::chat_manager::prompting::request_builder::{build_chat_request, system_role_for};
use crate::chat_manager::tooling::{
    parse_tool_calls, parse_tool_calls_from_text, ToolCall, ToolChoice, ToolConfig, ToolDefinition,
};
use crate::chat_manager::types::{
    Model, PromptEntryChatMode, PromptEntryInfoSource, PromptEntryPosition, PromptEntryRole,
    ProviderCredential, Settings, SystemPromptEntry,
};
use crate::storage_manager::settings::read_settings_typed;

use super::state::{
    CoherenceChange, EntryDraft, EntryPlan, JobState, PromptOverrides, SourceExcerpt, WorldDigest,
};

const DEFAULT_TARGET_COUNT: u32 = 12;
const MIN_TARGET_COUNT: u32 = 5;
const MAX_TARGET_COUNT: u32 = 50;
const DEFAULT_MAX_TOKENS: u32 = 4096;
const MIN_MAX_TOKENS: u32 = 256;
const MAX_MAX_TOKENS: u32 = 32768;

fn resolve_max_tokens(settings: &Settings) -> u32 {
    settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| advanced.lorebook_generator_max_tokens)
        .map(|v| v.clamp(MIN_MAX_TOKENS, MAX_MAX_TOKENS))
        .unwrap_or(DEFAULT_MAX_TOKENS)
}

pub fn clamp_target_count(value: u32) -> u32 {
    value.clamp(MIN_TARGET_COUNT, MAX_TARGET_COUNT)
}

pub fn default_target_count_from_settings(settings: &Settings) -> u32 {
    settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| advanced.lorebook_generator_default_target_count)
        .map(clamp_target_count)
        .unwrap_or(DEFAULT_TARGET_COUNT)
}

pub fn load_settings(app: &AppHandle) -> Result<Settings, String> {
    read_settings_typed::<Settings>(app)?
        .ok_or_else(|| "Settings have not been initialized".to_string())
}

fn resolve_pipeline_model<'a>(
    settings: &'a Settings,
    overrides: &PromptOverrides,
) -> Result<(&'a Model, &'a ProviderCredential), String> {
    let preferred = overrides
        .model_id
        .as_deref()
        .filter(|v| !v.trim().is_empty())
        .or_else(|| {
            settings
                .advanced_settings
                .as_ref()
                .and_then(|a| a.lorebook_generator_model_id.as_deref())
                .filter(|v| !v.trim().is_empty())
        })
        .or(settings.default_model_id.as_deref());

    let model_id = preferred.ok_or_else(|| {
        "No model is configured for the lorebook generator. Set one in Advanced → Lorebook Generator."
            .to_string()
    })?;

    find_model_with_credential(settings, model_id).ok_or_else(|| {
        "Configured lorebook generator model could not be resolved (model or credential missing)."
            .to_string()
    })
}

fn template_id_for_planner(settings: &Settings, overrides: &PromptOverrides) -> String {
    overrides
        .planner_prompt_template_id
        .clone()
        .or_else(|| {
            settings
                .advanced_settings
                .as_ref()
                .and_then(|a| a.lorebook_generator_planner_prompt_template_id.clone())
        })
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| APP_LOREBOOK_GENERATOR_PLANNER_TEMPLATE_ID.to_string())
}

fn template_id_for_writer(settings: &Settings, overrides: &PromptOverrides) -> String {
    overrides
        .writer_prompt_template_id
        .clone()
        .or_else(|| {
            settings
                .advanced_settings
                .as_ref()
                .and_then(|a| a.lorebook_generator_writer_prompt_template_id.clone())
        })
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| APP_LOREBOOK_GENERATOR_WRITER_TEMPLATE_ID.to_string())
}

fn template_id_for_refine(settings: &Settings, overrides: &PromptOverrides) -> String {
    overrides
        .refine_prompt_template_id
        .clone()
        .or_else(|| {
            settings
                .advanced_settings
                .as_ref()
                .and_then(|a| a.lorebook_generator_refine_prompt_template_id.clone())
        })
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| APP_LOREBOOK_GENERATOR_REFINE_TEMPLATE_ID.to_string())
}

fn template_id_for_coherence(settings: &Settings, overrides: &PromptOverrides) -> String {
    overrides
        .coherence_prompt_template_id
        .clone()
        .or_else(|| {
            settings
                .advanced_settings
                .as_ref()
                .and_then(|a| a.lorebook_generator_coherence_prompt_template_id.clone())
        })
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| APP_LOREBOOK_GENERATOR_COHERENCE_TEMPLATE_ID.to_string())
}

fn load_template_entries(
    app: &AppHandle,
    template_id: &str,
    fallback_label: &str,
    fallback_content: &str,
) -> Vec<SystemPromptEntry> {
    if let Ok(Some(template)) = get_template(app, template_id) {
        if !template.entries.is_empty() {
            return template.entries;
        }
        if !template.content.trim().is_empty() {
            return vec![SystemPromptEntry {
                id: format!("{}_single", fallback_label),
                name: fallback_label.to_string(),
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
            }];
        }
    }
    vec![SystemPromptEntry {
        id: format!("{}_default", fallback_label),
        name: fallback_label.to_string(),
        role: PromptEntryRole::System,
        content: fallback_content.to_string(),
        enabled: true,
        injection_position: PromptEntryPosition::Relative,
        injection_depth: 0,
        conditional_min_messages: None,
        interval_turns: None,
        system_prompt: true,
        conditions: None,
        prompt_entry_payload: None,
    }]
}

fn render_entries(
    entries: &[SystemPromptEntry],
    replacements: &[(&str, String)],
    model: &Model,
) -> Vec<SystemPromptEntry> {
    let condition_context = PromptEntryConditionContext {
        chat_mode: PromptEntryChatMode::Direct,
        info_source: PromptEntryInfoSource::Messages,
        scene_generation_enabled: false,
        avatar_generation_enabled: false,
        has_scene: false,
        has_scene_direction: false,
        has_persona: false,
        message_count: 0,
        participant_count: 1,
        recent_text: "",
        dynamic_memory_enabled: false,
        has_memory_summary: false,
        has_key_memories: false,
        has_lorebook_content: false,
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

    let mut out = Vec::new();
    for entry in entries {
        if !entry_is_active(entry, &condition_context) {
            continue;
        }
        let mut content = entry.content.clone();
        for (key, value) in replacements {
            content = content.replace(key, value);
        }
        if content.trim().is_empty() && entry.prompt_entry_payload.is_none() {
            continue;
        }
        let mut next = entry.clone();
        next.content = content;
        out.push(next);
    }
    out
}

fn entries_to_messages(
    credential: &ProviderCredential,
    entries: &[SystemPromptEntry],
    final_user_instruction: &str,
) -> Vec<Value> {
    let system_role = system_role_for(credential);
    let mut messages = Vec::new();
    for entry in entries {
        let trimmed = entry.content.trim();
        if trimmed.is_empty() {
            continue;
        }
        let role = match entry.role {
            PromptEntryRole::System => system_role.as_ref(),
            PromptEntryRole::User => "user",
            PromptEntryRole::Assistant => "assistant",
        };
        messages.push(json!({ "role": role, "content": trimmed }));
    }
    messages.push(json!({ "role": "user", "content": final_user_instruction }));
    messages
}

async fn invoke_tool(
    app: &AppHandle,
    credential: &ProviderCredential,
    model: &Model,
    api_key: &str,
    messages: &Vec<Value>,
    tool_config: &ToolConfig,
    max_tokens: u32,
) -> Result<Vec<ToolCall>, String> {
    let extra_body_fields: Option<HashMap<String, Value>> = None;
    let built = build_chat_request(
        credential,
        api_key,
        &model.name,
        messages,
        None,
        Some(0.3),
        Some(1.0),
        max_tokens,
        None,
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

    let response = api_request(
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
    .await?;

    if !response.ok {
        let fallback = format!("Provider returned status {}", response.status);
        let err = extract_error_message(response.data()).unwrap_or(fallback);
        return Err(err);
    }

    let calls = parse_tool_calls(&credential.provider_id, response.data());
    if !calls.is_empty() {
        return Ok(calls);
    }

    if let Some(text) = extract_text(response.data(), Some(credential.provider_id.as_str())) {
        let inferred = parse_tool_calls_from_text(&text);
        if !inferred.is_empty() {
            return Ok(inferred);
        }
        return Err(format!(
            "Model did not return a tool call. Raw output: {}",
            truncate_for_error(&text)
        ));
    }

    Err("Model returned no usable tool call or text".to_string())
}

fn truncate_for_error(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= 400 {
        trimmed.to_string()
    } else {
        let mut out: String = trimmed.chars().take(400).collect();
        out.push_str("…");
        out
    }
}

fn format_excerpts(digest: &WorldDigest) -> String {
    if digest.excerpts.is_empty() {
        return "(none)".to_string();
    }
    digest
        .excerpts
        .iter()
        .map(|e| format!("[{}] {}\n{}", e.id, e.label, e.content))
        .collect::<Vec<_>>()
        .join("\n\n---\n\n")
}

fn relevant_excerpts_for_plan(plan: &EntryPlan, digest: &WorldDigest) -> String {
    if plan.source_refs.is_empty() {
        return format_excerpts(digest);
    }
    let mut out = Vec::new();
    for excerpt in &digest.excerpts {
        if plan.source_refs.iter().any(|r| r == &excerpt.id) {
            out.push(format!(
                "[{}] {}\n{}",
                excerpt.id, excerpt.label, excerpt.content
            ));
        }
    }
    if out.is_empty() {
        return format_excerpts(digest);
    }
    out.join("\n\n---\n\n")
}

fn format_outline_for_context(outline: &[EntryPlan]) -> String {
    if outline.is_empty() {
        return "(empty)".to_string();
    }
    outline
        .iter()
        .map(|p| {
            format!(
                "{}. {} [{}] keys: {}",
                p.idx + 1,
                p.title,
                p.category,
                if p.proposed_keys.is_empty() {
                    "(none)".to_string()
                } else {
                    p.proposed_keys.join(", ")
                },
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_drafted_entries(drafts: &[EntryDraft]) -> String {
    if drafts.is_empty() {
        return "(none)".to_string();
    }
    drafts
        .iter()
        .enumerate()
        .map(|(i, d)| {
            format!(
                "Entry {} (idx {}): \"{}\"\nKeys: {}\nAlwaysActive: {}\nContent: {}",
                i + 1,
                i,
                d.title,
                if d.keywords.is_empty() {
                    "(none)".to_string()
                } else {
                    d.keywords.join(", ")
                },
                d.always_active,
                d.content,
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n---\n\n")
}

fn planner_tool_config() -> ToolConfig {
    ToolConfig {
        tools: vec![ToolDefinition {
            name: "propose_lorebook_outline".to_string(),
            description: Some("Propose the full outline of lorebook entries to draft.".to_string()),
            parameters: json!({
                "type": "object",
                "properties": {
                    "entries": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "title": { "type": "string" },
                                "category": { "type": "string" },
                                "proposedKeys": {
                                    "type": "array",
                                    "items": { "type": "string" }
                                },
                                "rationale": { "type": "string" },
                                "sourceRefs": {
                                    "type": "array",
                                    "items": { "type": "string" }
                                }
                            },
                            "required": ["title", "category", "rationale"]
                        }
                    }
                },
                "required": ["entries"]
            }),
        }],
        choice: Some(ToolChoice::Required),
    }
}

fn parse_planner_result(calls: &[ToolCall]) -> Result<Vec<EntryPlan>, String> {
    let call = calls
        .iter()
        .find(|c| c.name == "propose_lorebook_outline")
        .ok_or_else(|| "Model did not call propose_lorebook_outline".to_string())?;
    let entries_raw = call
        .arguments
        .get("entries")
        .and_then(Value::as_array)
        .ok_or_else(|| "propose_lorebook_outline missing 'entries' array".to_string())?;

    let mut out = Vec::with_capacity(entries_raw.len());
    for (idx, val) in entries_raw.iter().enumerate() {
        let obj = val
            .as_object()
            .ok_or_else(|| format!("entry {} is not an object", idx))?;
        let title = obj
            .get("title")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .ok_or_else(|| format!("entry {} missing title", idx))?
            .to_string();
        let category = obj
            .get("category")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .unwrap_or("other")
            .to_string();
        let rationale = obj
            .get("rationale")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("")
            .to_string();
        let proposed_keys = obj
            .get("proposedKeys")
            .or_else(|| obj.get("proposed_keys"))
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let source_refs = obj
            .get("sourceRefs")
            .or_else(|| obj.get("source_refs"))
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        out.push(EntryPlan {
            idx,
            title,
            category,
            proposed_keys,
            rationale,
            source_refs,
        });
    }
    Ok(out)
}

pub async fn run_planner(app: &AppHandle, job: &JobState) -> Result<Vec<EntryPlan>, String> {
    let settings = load_settings(app)?;
    let (model, credential) = resolve_pipeline_model(&settings, &job.overrides)?;
    let api_key = require_api_key(credential)?;

    let template_id = template_id_for_planner(&settings, &job.overrides);
    let entries = load_template_entries(
        app,
        &template_id,
        "Lorebook Generator: Planner",
        crate::chat_manager::prompt_engine::default_lorebook_generator_planner_prompt().as_str(),
    );
    let replacements: Vec<(&str, String)> = vec![
        ("{{brief}}", job.brief.clone()),
        ("{{target_count}}", job.target_count.to_string()),
        ("{{source_excerpts}}", format_excerpts(&job.digest)),
    ];
    let rendered = render_entries(&entries, &replacements, model);
    let messages = entries_to_messages(
        credential,
        &rendered,
        "Call propose_lorebook_outline now with exactly the requested number of entries.",
    );
    let tool_config = planner_tool_config();
    let calls = invoke_tool(
        app,
        credential,
        model,
        &api_key,
        &messages,
        &tool_config,
        resolve_max_tokens(&settings),
    )
    .await?;
    let mut plan = parse_planner_result(&calls)?;
    if plan.is_empty() {
        return Err("Planner returned an empty outline".to_string());
    }
    for (i, p) in plan.iter_mut().enumerate() {
        p.idx = i;
    }
    Ok(plan)
}

fn writer_tool_config() -> ToolConfig {
    ToolConfig {
        tools: vec![ToolDefinition {
            name: "write_lorebook_entry".to_string(),
            description: Some("Write the body of a single lorebook entry.".to_string()),
            parameters: json!({
                "type": "object",
                "properties": {
                    "title": { "type": "string" },
                    "keywords": {
                        "type": "array",
                        "items": { "type": "string" }
                    },
                    "content": { "type": "string" },
                    "alwaysActive": { "type": "boolean" }
                },
                "required": ["title", "content"]
            }),
        }],
        choice: Some(ToolChoice::Required),
    }
}

fn parse_writer_result(plan_idx: usize, calls: &[ToolCall]) -> Result<EntryDraft, String> {
    let call = calls
        .iter()
        .find(|c| c.name == "write_lorebook_entry")
        .ok_or_else(|| "Model did not call write_lorebook_entry".to_string())?;
    let obj = call
        .arguments
        .as_object()
        .ok_or_else(|| "write_lorebook_entry arguments must be an object".to_string())?;
    let title = obj
        .get("title")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| "write_lorebook_entry missing title".to_string())?
        .to_string();
    let content = obj
        .get("content")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| "write_lorebook_entry missing content".to_string())?
        .to_string();
    let always_active = obj
        .get("alwaysActive")
        .or_else(|| obj.get("always_active"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let keywords = obj
        .get("keywords")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(EntryDraft {
        plan_idx,
        title,
        keywords,
        content,
        always_active,
        status: super::state::DraftStatus::Drafted,
        revisions: Vec::new(),
    })
}

pub async fn run_writer(
    app: &AppHandle,
    job: &JobState,
    plan: &EntryPlan,
) -> Result<EntryDraft, String> {
    let settings = load_settings(app)?;
    let (model, credential) = resolve_pipeline_model(&settings, &job.overrides)?;
    let api_key = require_api_key(credential)?;

    let template_id = template_id_for_writer(&settings, &job.overrides);
    let entries = load_template_entries(
        app,
        &template_id,
        "Lorebook Generator: Writer",
        crate::chat_manager::prompt_engine::default_lorebook_generator_writer_prompt().as_str(),
    );
    let replacements: Vec<(&str, String)> = vec![
        ("{{brief}}", job.brief.clone()),
        ("{{outline}}", format_outline_for_context(&job.outline)),
        ("{{entry_title}}", plan.title.clone()),
        ("{{entry_category}}", plan.category.clone()),
        (
            "{{entry_proposed_keys}}",
            if plan.proposed_keys.is_empty() {
                "(none)".to_string()
            } else {
                plan.proposed_keys.join(", ")
            },
        ),
        ("{{entry_rationale}}", plan.rationale.clone()),
        (
            "{{relevant_excerpts}}",
            relevant_excerpts_for_plan(plan, &job.digest),
        ),
    ];
    let rendered = render_entries(&entries, &replacements, model);
    let messages = entries_to_messages(
        credential,
        &rendered,
        "Call write_lorebook_entry now with the final entry.",
    );
    let tool_config = writer_tool_config();
    let calls = invoke_tool(
        app,
        credential,
        model,
        &api_key,
        &messages,
        &tool_config,
        resolve_max_tokens(&settings),
    )
    .await?;
    parse_writer_result(plan.idx, &calls)
}

pub async fn run_refine(
    app: &AppHandle,
    job: &JobState,
    draft: &EntryDraft,
    feedback: &str,
) -> Result<EntryDraft, String> {
    let settings = load_settings(app)?;
    let (model, credential) = resolve_pipeline_model(&settings, &job.overrides)?;
    let api_key = require_api_key(credential)?;

    let template_id = template_id_for_refine(&settings, &job.overrides);
    let entries = load_template_entries(
        app,
        &template_id,
        "Lorebook Generator: Refine",
        crate::chat_manager::prompt_engine::default_lorebook_generator_refine_prompt().as_str(),
    );
    let plan_excerpts = job
        .outline
        .iter()
        .find(|p| p.idx == draft.plan_idx)
        .map(|p| relevant_excerpts_for_plan(p, &job.digest))
        .unwrap_or_else(|| format_excerpts(&job.digest));

    let replacements: Vec<(&str, String)> = vec![
        ("{{brief}}", job.brief.clone()),
        ("{{outline}}", format_outline_for_context(&job.outline)),
        ("{{entry_title}}", draft.title.clone()),
        (
            "{{entry_keywords}}",
            if draft.keywords.is_empty() {
                "(none)".to_string()
            } else {
                draft.keywords.join(", ")
            },
        ),
        ("{{entry_always_active}}", draft.always_active.to_string()),
        ("{{entry_content}}", draft.content.clone()),
        ("{{user_feedback}}", feedback.to_string()),
        ("{{relevant_excerpts}}", plan_excerpts),
    ];
    let rendered = render_entries(&entries, &replacements, model);
    let messages = entries_to_messages(
        credential,
        &rendered,
        "Call write_lorebook_entry now with the revised entry.",
    );
    let tool_config = writer_tool_config();
    let calls = invoke_tool(
        app,
        credential,
        model,
        &api_key,
        &messages,
        &tool_config,
        resolve_max_tokens(&settings),
    )
    .await?;
    let mut revised = parse_writer_result(draft.plan_idx, &calls)?;
    revised.revisions = draft.revisions.clone();
    revised.revisions.push(super::state::DraftRevision {
        feedback: feedback.to_string(),
        content: revised.content.clone(),
        timestamp_ms: now_millis(),
    });
    Ok(revised)
}

fn coherence_tool_config() -> ToolConfig {
    ToolConfig {
        tools: vec![ToolDefinition {
            name: "propose_coherence_changes".to_string(),
            description: Some(
                "Propose surgical coherence fixes across the drafted entries.".to_string(),
            ),
            parameters: json!({
                "type": "object",
                "properties": {
                    "changes": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "kind": {
                                    "type": "string",
                                    "enum": [
                                        "mergeKeys",
                                        "renameTerm",
                                        "flagContradiction",
                                        "toggleAlwaysActive"
                                    ]
                                },
                                "entryIdx": { "type": "integer" },
                                "removeKeys": {
                                    "type": "array",
                                    "items": { "type": "string" }
                                },
                                "oldTerm": { "type": "string" },
                                "newTerm": { "type": "string" },
                                "affectedEntryIdxs": {
                                    "type": "array",
                                    "items": { "type": "integer" }
                                },
                                "entryIdxs": {
                                    "type": "array",
                                    "items": { "type": "integer" }
                                },
                                "description": { "type": "string" },
                                "newValue": { "type": "boolean" },
                                "reason": { "type": "string" }
                            },
                            "required": ["kind"]
                        }
                    }
                },
                "required": ["changes"]
            }),
        }],
        choice: Some(ToolChoice::Required),
    }
}

fn parse_coherence_result(calls: &[ToolCall]) -> Result<Vec<CoherenceChange>, String> {
    let call = calls
        .iter()
        .find(|c| c.name == "propose_coherence_changes")
        .ok_or_else(|| "Model did not call propose_coherence_changes".to_string())?;
    let arr = call
        .arguments
        .get("changes")
        .and_then(Value::as_array)
        .ok_or_else(|| "propose_coherence_changes missing 'changes' array".to_string())?;
    let mut out = Vec::new();
    for (i, val) in arr.iter().enumerate() {
        let obj = match val.as_object() {
            Some(o) => o,
            None => continue,
        };
        let kind = obj
            .get("kind")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("");
        let id = format!("change_{}", i);
        let reason = obj
            .get("reason")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        match kind {
            "mergeKeys" => {
                let entry_idx = obj
                    .get("entryIdx")
                    .and_then(Value::as_u64)
                    .unwrap_or_default() as usize;
                let remove_keys = obj
                    .get("removeKeys")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str().map(str::to_string))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                if remove_keys.is_empty() {
                    continue;
                }
                out.push(CoherenceChange::MergeKeys {
                    id,
                    entry_idx,
                    remove_keys,
                    reason,
                });
            }
            "renameTerm" => {
                let old_term = obj
                    .get("oldTerm")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let new_term = obj
                    .get("newTerm")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                if old_term.is_empty() || new_term.is_empty() || old_term == new_term {
                    continue;
                }
                let affected_entry_idxs = obj
                    .get("affectedEntryIdxs")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_u64())
                            .map(|n| n as usize)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                out.push(CoherenceChange::RenameTerm {
                    id,
                    old_term,
                    new_term,
                    affected_entry_idxs,
                    reason,
                });
            }
            "flagContradiction" => {
                let entry_idxs = obj
                    .get("entryIdxs")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_u64())
                            .map(|n| n as usize)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let description = obj
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                if entry_idxs.is_empty() || description.is_empty() {
                    continue;
                }
                out.push(CoherenceChange::FlagContradiction {
                    id,
                    entry_idxs,
                    description,
                });
            }
            "toggleAlwaysActive" => {
                let entry_idx = obj
                    .get("entryIdx")
                    .and_then(Value::as_u64)
                    .unwrap_or_default() as usize;
                let new_value = obj
                    .get("newValue")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                out.push(CoherenceChange::ToggleAlwaysActive {
                    id,
                    entry_idx,
                    new_value,
                    reason,
                });
            }
            _ => continue,
        }
    }
    Ok(out)
}

pub async fn run_coherence(
    app: &AppHandle,
    job: &JobState,
) -> Result<Vec<CoherenceChange>, String> {
    let settings = load_settings(app)?;
    let (model, credential) = resolve_pipeline_model(&settings, &job.overrides)?;
    let api_key = require_api_key(credential)?;

    let template_id = template_id_for_coherence(&settings, &job.overrides);
    let entries = load_template_entries(
        app,
        &template_id,
        "Lorebook Generator: Coherence",
        crate::chat_manager::prompt_engine::default_lorebook_generator_coherence_prompt().as_str(),
    );
    let replacements: Vec<(&str, String)> =
        vec![("{{drafted_entries}}", format_drafted_entries(&job.drafts))];
    let rendered = render_entries(&entries, &replacements, model);
    let messages = entries_to_messages(
        credential,
        &rendered,
        "Call propose_coherence_changes now with the list of changes.",
    );
    let tool_config = coherence_tool_config();
    let calls = invoke_tool(
        app,
        credential,
        model,
        &api_key,
        &messages,
        &tool_config,
        resolve_max_tokens(&settings),
    )
    .await?;
    parse_coherence_result(&calls)
}

fn require_api_key(credential: &ProviderCredential) -> Result<String, String> {
    if credential.provider_id == "llamacpp" || credential.provider_id == "ollama" {
        return Ok(credential.api_key.clone().unwrap_or_default());
    }
    credential
        .api_key
        .clone()
        .filter(|k| !k.is_empty())
        .ok_or_else(|| "Provider credential is missing an API key".to_string())
}

fn now_millis() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn apply_coherence_changes(drafts: &mut [EntryDraft], changes: &[CoherenceChange]) {
    for change in changes {
        match change {
            CoherenceChange::MergeKeys {
                entry_idx,
                remove_keys,
                ..
            } => {
                if let Some(d) = drafts.get_mut(*entry_idx) {
                    let lowered: Vec<String> =
                        remove_keys.iter().map(|k| k.to_ascii_lowercase()).collect();
                    d.keywords
                        .retain(|k| !lowered.contains(&k.to_ascii_lowercase()));
                }
            }
            CoherenceChange::RenameTerm {
                old_term,
                new_term,
                affected_entry_idxs,
                ..
            } => {
                let targets: Vec<usize> = if affected_entry_idxs.is_empty() {
                    (0..drafts.len()).collect()
                } else {
                    affected_entry_idxs.clone()
                };
                for idx in targets {
                    if let Some(d) = drafts.get_mut(idx) {
                        d.title = d.title.replace(old_term, new_term);
                        d.content = d.content.replace(old_term, new_term);
                        for k in d.keywords.iter_mut() {
                            *k = k.replace(old_term, new_term);
                        }
                    }
                }
            }
            CoherenceChange::FlagContradiction { .. } => {}
            CoherenceChange::ToggleAlwaysActive {
                entry_idx,
                new_value,
                ..
            } => {
                if let Some(d) = drafts.get_mut(*entry_idx) {
                    d.always_active = *new_value;
                }
            }
        }
    }
}

#[allow(dead_code)]
fn _unused_warning_silencer(_: &SourceExcerpt) {}
