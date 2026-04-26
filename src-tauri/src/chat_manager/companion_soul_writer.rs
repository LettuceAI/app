use quick_xml::escape::{resolve_xml_entity, unescape};
use quick_xml::events::{BytesRef, Event};
use quick_xml::Reader;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use tauri::AppHandle;

use crate::api::{api_request, ApiRequest, ApiResponse};
use crate::chat_manager::execution::{
    find_model_with_credential, prepare_default_sampling_request,
};
use crate::chat_manager::prompts;
use crate::chat_manager::request::{extract_error_message, extract_text, extract_usage};
use crate::chat_manager::service::{record_usage_if_available, require_api_key, ChatContext};
use crate::chat_manager::storage::{
    get_base_prompt_entries, resolve_credential_for_model, PromptType,
};
use crate::chat_manager::tooling::{
    parse_tool_calls, parse_tool_calls_from_text, ToolCall, ToolChoice, ToolConfig, ToolDefinition,
};
use crate::chat_manager::types::{
    ChatGenerateCompanionSoulArgs, DynamicMemoryStructuredFallbackFormat, Model,
    ProviderCredential, Session, Settings, SystemPromptEntry,
};
use crate::usage::tracking::UsageOperationType;
use crate::utils::{log_info, log_warn, now_millis};

const MAX_LOOP_ITERATIONS: usize = 8;

const COMPANION_SOUL_JSON_FALLBACK_PROMPT: &str = r#"Return only JSON. Format: {"operations":[{"name":"set_identity","arguments":{"essence":"...","voice":"...","relationalStyle":"...","vulnerabilities":"...","habits":"...","boundaries":"..."}},{"name":"set_baseline_affect","arguments":{"warmth":0.5,"trust":0.5,"calm":0.5,"vulnerability":0.5,"longing":0.5,"hurt":0.5,"tension":0.5,"irritation":0.5,"affectionIntensity":0.5,"reassuranceNeed":0.5}},{"name":"set_regulation_style","arguments":{"suppression":0.5,"volatility":0.5,"recoverySpeed":0.5,"conflictAvoidance":0.5,"reassuranceSeeking":0.5,"protestBehavior":0.5,"emotionalTransparency":0.5,"attachmentActivation":0.5,"pride":0.5}},{"name":"set_relationship_defaults","arguments":{"closeness":0.2,"trust":0.3,"affection":0.2,"tension":0.05}},{"name":"done","arguments":{"notes":"optional"}}]}. End with done. Numeric fields are optional and clamped to [0,1]. Do not use markdown."#;

const COMPANION_SOUL_XML_FALLBACK_PROMPT: &str = r#"Return only XML. Format: <soul_ops><set_identity><essence>...</essence><voice>...</voice><relationalStyle>...</relationalStyle><vulnerabilities>...</vulnerabilities><habits>...</habits><boundaries>...</boundaries></set_identity><set_baseline_affect warmth="0.5" trust="0.5" calm="0.5" vulnerability="0.5" longing="0.5" hurt="0.5" tension="0.5" irritation="0.5" affectionIntensity="0.5" reassuranceNeed="0.5" /><set_regulation_style suppression="0.5" volatility="0.5" recoverySpeed="0.5" conflictAvoidance="0.5" reassuranceSeeking="0.5" protestBehavior="0.5" emotionalTransparency="0.5" attachmentActivation="0.5" pride="0.5" /><set_relationship_defaults closeness="0.2" trust="0.3" affection="0.2" tension="0.05" /><done summary="optional" /></soul_ops>. End with <done />. Numeric fields are optional and clamped to [0,1]. Do not use markdown."#;

const TEXT_FIELDS: &[&str] = &[
    "essence",
    "voice",
    "relationalStyle",
    "vulnerabilities",
    "habits",
    "boundaries",
];

const BASELINE_AFFECT_FIELDS: &[&str] = &[
    "warmth",
    "trust",
    "calm",
    "vulnerability",
    "longing",
    "hurt",
    "tension",
    "irritation",
    "affectionIntensity",
    "reassuranceNeed",
];

const REGULATION_STYLE_FIELDS: &[&str] = &[
    "suppression",
    "volatility",
    "recoverySpeed",
    "conflictAvoidance",
    "reassuranceSeeking",
    "protestBehavior",
    "emotionalTransparency",
    "attachmentActivation",
    "pride",
];

const RELATIONSHIP_DEFAULTS_FIELDS: &[&str] = &["closeness", "trust", "affection", "tension"];

const SOUL_OP_ROOT_TAGS: &[&str] = &["soul_ops", "operations"];
const SOUL_OP_TAGS: &[&str] = &[
    "set_identity",
    "set_baseline_affect",
    "set_regulation_style",
    "set_relationship_defaults",
    "done",
];

fn supports_text_generation_model(model: &Model) -> bool {
    model
        .input_scopes
        .iter()
        .any(|scope| scope.eq_ignore_ascii_case("text"))
        && model
            .output_scopes
            .iter()
            .any(|scope| scope.eq_ignore_ascii_case("text"))
}

fn resolve_companion_soul_writer_target<'a>(
    settings: &'a Settings,
    preferred_model_id: Option<&str>,
) -> Result<(&'a Model, &'a ProviderCredential), String> {
    if let Some(model_id) = preferred_model_id.filter(|id| !id.trim().is_empty()) {
        let (model, credential) = find_model_with_credential(settings, model_id)
            .ok_or_else(|| "Selected Soul writer model could not be resolved".to_string())?;
        if !supports_text_generation_model(model) {
            return Err(
                "Selected Soul writer model must support text input and text output".to_string(),
            );
        }
        return Ok((model, credential));
    }

    let configured = settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| advanced.companion_soul_writer_model_id.as_deref())
        .filter(|value| !value.trim().is_empty())
        .and_then(|model_id| find_model_with_credential(settings, model_id))
        .filter(|(model, _)| supports_text_generation_model(model));
    if let Some(target) = configured {
        return Ok(target);
    }

    let default_model = settings
        .default_model_id
        .as_deref()
        .and_then(|model_id| find_model_with_credential(settings, model_id))
        .filter(|(model, _)| supports_text_generation_model(model));

    default_model
        .or_else(|| {
            settings.models.iter().find_map(|model| {
                if !supports_text_generation_model(model) {
                    return None;
                }
                let credential = resolve_credential_for_model(settings, model)?;
                Some((model, credential))
            })
        })
        .ok_or_else(|| "No text generation model is configured".to_string())
}

fn resolve_companion_soul_writer_fallback_target<'a>(
    settings: &'a Settings,
) -> Option<(&'a Model, &'a ProviderCredential)> {
    let model_id = settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| advanced.companion_soul_writer_fallback_model_id.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let (model, credential) = find_model_with_credential(settings, model_id)?;
    if !supports_text_generation_model(model) {
        return None;
    }
    Some((model, credential))
}

fn fallback_format(settings: &Settings) -> DynamicMemoryStructuredFallbackFormat {
    settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| advanced.companion_soul_writer_structured_fallback_format)
        .unwrap_or(DynamicMemoryStructuredFallbackFormat::Json)
}

fn fallback_format_label(format: DynamicMemoryStructuredFallbackFormat) -> &'static str {
    match format {
        DynamicMemoryStructuredFallbackFormat::Json => "json",
        DynamicMemoryStructuredFallbackFormat::Xml => "xml",
    }
}

fn fallback_prompt(format: DynamicMemoryStructuredFallbackFormat) -> &'static str {
    match format {
        DynamicMemoryStructuredFallbackFormat::Json => COMPANION_SOUL_JSON_FALLBACK_PROMPT,
        DynamicMemoryStructuredFallbackFormat::Xml => COMPANION_SOUL_XML_FALLBACK_PROMPT,
    }
}

fn selected_prompt_template_id(settings: &Settings) -> &str {
    settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| advanced.companion_soul_writer_prompt_template_id.as_deref())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(prompts::APP_COMPANION_SOUL_WRITER_TEMPLATE_ID)
}

fn render_companion_soul_prompt_content(
    content: &str,
    args: &ChatGenerateCompanionSoulArgs,
) -> String {
    let current_soul = args
        .current_soul
        .as_ref()
        .map(|value| serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".to_string()))
        .unwrap_or_else(|| "{}".to_string());
    let definition = args
        .character_definition
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("Not provided.");
    let description = args
        .character_description
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("Not provided.");
    let opening_context = args
        .opening_context
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("Not provided.");
    let user_notes = args
        .user_notes
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("No special direction.");

    content
        .replace("{{char.name}}", args.character_name.trim())
        .replace("{{char.definition}}", definition)
        .replace("{{char.description}}", description)
        .replace("{{char.desc}}", definition)
        .replace("{{opening_context}}", opening_context)
        .replace("{{current_soul}}", &current_soul)
        .replace("{{user_notes}}", user_notes)
}

fn load_prompt_entries(app: &AppHandle, template_id: &str) -> Vec<SystemPromptEntry> {
    match prompts::get_template(app, template_id) {
        Ok(Some(template)) => {
            if !template.entries.is_empty() {
                template.entries
            } else if !template.content.trim().is_empty() {
                vec![SystemPromptEntry {
                    id: "companion_soul_single_entry".to_string(),
                    name: "Companion Soul Writer".to_string(),
                    role: crate::chat_manager::types::PromptEntryRole::System,
                    content: template.content,
                    enabled: true,
                    injection_position: crate::chat_manager::types::PromptEntryPosition::Relative,
                    injection_depth: 0,
                    conditional_min_messages: None,
                    interval_turns: None,
                    system_prompt: true,
                    conditions: None,
                    prompt_entry_payload: None,
                }]
            } else {
                get_base_prompt_entries(PromptType::CompanionSoulWriterPrompt)
            }
        }
        _ => get_base_prompt_entries(PromptType::CompanionSoulWriterPrompt),
    }
}

fn render_messages(
    app: &AppHandle,
    settings: &Settings,
    credential: &ProviderCredential,
    args: &ChatGenerateCompanionSoulArgs,
) -> Vec<Value> {
    let template_entries = load_prompt_entries(app, selected_prompt_template_id(settings));
    let system_role = crate::chat_manager::request_builder::system_role_for(credential);

    let mut messages = Vec::new();
    for entry in template_entries {
        if !entry.enabled {
            continue;
        }
        let role = match entry.role {
            crate::chat_manager::types::PromptEntryRole::System => system_role.as_ref(),
            crate::chat_manager::types::PromptEntryRole::User => "user",
            crate::chat_manager::types::PromptEntryRole::Assistant => "assistant",
        };
        let rendered = render_companion_soul_prompt_content(&entry.content, args);
        let trimmed = rendered.trim();
        if trimmed.is_empty() {
            continue;
        }
        messages.push(json!({
            "role": role,
            "content": trimmed,
        }));
    }

    messages.push(json!({
        "role": "user",
        "content": "Author the Companion Soul now. Issue tool calls (set_identity, set_baseline_affect, set_regulation_style, set_relationship_defaults) across one or more turns, then call done to finish.",
    }));

    messages
}

fn build_tool_config() -> ToolConfig {
    let identity_props = json!({
        "essence": { "type": "string" },
        "voice": { "type": "string" },
        "relationalStyle": { "type": "string" },
        "vulnerabilities": { "type": "string" },
        "habits": { "type": "string" },
        "boundaries": { "type": "string" }
    });
    let baseline_affect_props = numeric_props(BASELINE_AFFECT_FIELDS);
    let regulation_props = numeric_props(REGULATION_STYLE_FIELDS);
    let relationship_props = numeric_props(RELATIONSHIP_DEFAULTS_FIELDS);

    let tools = vec![
        ToolDefinition {
            name: "set_identity".to_string(),
            description: Some(
                "Set or refine the durable identity text fields. All fields optional; later calls overwrite earlier values for the same field.".to_string(),
            ),
            parameters: json!({
                "type": "object",
                "properties": identity_props,
            }),
        },
        ToolDefinition {
            name: "set_baseline_affect".to_string(),
            description: Some(
                "Set or refine baseline affect floats in [0,1]. All fields optional; values are clamped.".to_string(),
            ),
            parameters: json!({
                "type": "object",
                "properties": baseline_affect_props,
            }),
        },
        ToolDefinition {
            name: "set_regulation_style".to_string(),
            description: Some(
                "Set or refine regulation style floats in [0,1]. All fields optional; values are clamped.".to_string(),
            ),
            parameters: json!({
                "type": "object",
                "properties": regulation_props,
            }),
        },
        ToolDefinition {
            name: "set_relationship_defaults".to_string(),
            description: Some(
                "Set the starting relationship-with-user defaults (closeness/trust/affection/tension), each in [0,1]. All fields optional.".to_string(),
            ),
            parameters: json!({
                "type": "object",
                "properties": relationship_props,
            }),
        },
        ToolDefinition {
            name: "done".to_string(),
            description: Some(
                "Call once the Companion Soul is finalized. Terminal — no more setters after this.".to_string(),
            ),
            parameters: json!({
                "type": "object",
                "properties": {
                    "notes": { "type": "string" }
                }
            }),
        },
    ];

    ToolConfig {
        tools,
        choice: Some(ToolChoice::Required),
    }
}

fn numeric_props(keys: &[&str]) -> Value {
    let mut map = Map::new();
    for key in keys {
        map.insert(
            (*key).to_string(),
            json!({
                "type": "number",
                "minimum": 0,
                "maximum": 1,
            }),
        );
    }
    Value::Object(map)
}

fn default_soul_value() -> Value {
    let mut soul = Map::new();
    for key in TEXT_FIELDS {
        soul.insert((*key).to_string(), Value::String(String::new()));
    }
    let mut baseline = Map::new();
    for key in BASELINE_AFFECT_FIELDS {
        baseline.insert((*key).to_string(), json!(0.0));
    }
    let mut regulation = Map::new();
    for key in REGULATION_STYLE_FIELDS {
        regulation.insert((*key).to_string(), json!(0.0));
    }
    soul.insert("baselineAffect".to_string(), Value::Object(baseline));
    soul.insert("regulationStyle".to_string(), Value::Object(regulation));

    let mut relationship = Map::new();
    for key in RELATIONSHIP_DEFAULTS_FIELDS {
        relationship.insert((*key).to_string(), json!(0.0));
    }

    let mut root = Map::new();
    root.insert("soul".to_string(), Value::Object(soul));
    root.insert(
        "relationshipDefaults".to_string(),
        Value::Object(relationship),
    );
    Value::Object(root)
}

fn normalize_working_soul(current: Option<&Value>) -> Value {
    let mut working = default_soul_value();

    let Some(current) = current else {
        return working;
    };
    let Some(current_obj) = current.as_object() else {
        return working;
    };

    if let Some(soul_in) = current_obj.get("soul").and_then(Value::as_object) {
        if let Some(soul_out) = working.get_mut("soul").and_then(Value::as_object_mut) {
            for key in TEXT_FIELDS {
                if let Some(Value::String(value)) = soul_in.get(*key) {
                    soul_out.insert((*key).to_string(), Value::String(value.clone()));
                }
            }
            if let Some(in_baseline) = soul_in.get("baselineAffect").and_then(Value::as_object) {
                if let Some(out_baseline) = soul_out
                    .get_mut("baselineAffect")
                    .and_then(Value::as_object_mut)
                {
                    for key in BASELINE_AFFECT_FIELDS {
                        if let Some(num) = in_baseline.get(*key).and_then(Value::as_f64) {
                            insert_clamped(out_baseline, key, num);
                        }
                    }
                }
            }
            if let Some(in_reg) = soul_in.get("regulationStyle").and_then(Value::as_object) {
                if let Some(out_reg) = soul_out
                    .get_mut("regulationStyle")
                    .and_then(Value::as_object_mut)
                {
                    for key in REGULATION_STYLE_FIELDS {
                        if let Some(num) = in_reg.get(*key).and_then(Value::as_f64) {
                            insert_clamped(out_reg, key, num);
                        }
                    }
                }
            }
        }
    }

    if let Some(in_rel) = current_obj
        .get("relationshipDefaults")
        .and_then(Value::as_object)
    {
        if let Some(out_rel) = working
            .get_mut("relationshipDefaults")
            .and_then(Value::as_object_mut)
        {
            for key in RELATIONSHIP_DEFAULTS_FIELDS {
                if let Some(num) = in_rel.get(*key).and_then(Value::as_f64) {
                    insert_clamped(out_rel, key, num);
                }
            }
        }
    }

    working
}

fn insert_clamped(map: &mut Map<String, Value>, key: &str, value: f64) {
    let clamped = value.clamp(0.0, 1.0);
    if let Some(num) = serde_json::Number::from_f64(clamped) {
        map.insert(key.to_string(), Value::Number(num));
    }
}

fn apply_set_identity(working: &mut Value, args: &Value) -> Vec<String> {
    let mut applied = Vec::new();
    let Some(args_obj) = args.as_object() else {
        return applied;
    };
    let Some(soul_obj) = working.get_mut("soul").and_then(Value::as_object_mut) else {
        return applied;
    };
    for key in TEXT_FIELDS {
        if let Some(value) = args_obj.get(*key) {
            if let Some(text) = value.as_str() {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    soul_obj.insert((*key).to_string(), Value::String(trimmed.to_string()));
                    applied.push((*key).to_string());
                }
            }
        }
    }
    applied
}

fn apply_set_numeric_section(
    working: &mut Value,
    section_path: &[&str],
    fields: &[&str],
    args: &Value,
) -> Vec<String> {
    let mut applied = Vec::new();
    let Some(args_obj) = args.as_object() else {
        return applied;
    };

    let mut node: &mut Value = working;
    for segment in section_path {
        let Some(map) = node.as_object_mut() else {
            return applied;
        };
        if !map.contains_key(*segment) {
            return applied;
        }
        node = map.get_mut(*segment).expect("checked above");
    }
    let Some(target) = node.as_object_mut() else {
        return applied;
    };

    for key in fields {
        if let Some(value) = args_obj.get(*key) {
            if let Some(num) = value.as_f64() {
                insert_clamped(target, key, num);
                applied.push((*key).to_string());
            }
        }
    }
    applied
}

#[allow(clippy::too_many_arguments)]
async fn send_request(
    app: &AppHandle,
    credential: &ProviderCredential,
    model: &Model,
    api_key: &str,
    messages: &Vec<Value>,
    max_tokens: u32,
    context_length: Option<u32>,
    extra_body_fields: Option<HashMap<String, Value>>,
    tool_config: Option<&ToolConfig>,
    request_id: Option<String>,
) -> Result<ApiResponse, String> {
    let built = crate::chat_manager::request_builder::build_chat_request(
        credential,
        api_key,
        &model.name,
        messages,
        None,
        Some(0.4),
        Some(1.0),
        max_tokens,
        context_length,
        false,
        request_id.clone(),
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
            provider_id: Some(credential.provider_id.clone()),
        },
    )
    .await?;

    if !first_response.ok {
        let status_fallback = format!("Provider returned status {}", first_response.status);
        let err_message = extract_error_message(first_response.data()).unwrap_or(status_fallback);

        if let Some(cfg) = tool_config {
            if credential.provider_id == "llamacpp"
                && parallel_tool_calls_requires_disable(&err_message)
            {
                let mut extra = extra_body_fields.clone().unwrap_or_default();
                extra.insert("parallel_tool_calls".to_string(), json!(false));
                let built = crate::chat_manager::request_builder::build_chat_request(
                    credential,
                    api_key,
                    &model.name,
                    messages,
                    None,
                    Some(0.4),
                    Some(1.0),
                    max_tokens,
                    context_length,
                    false,
                    request_id.clone(),
                    None,
                    None,
                    None,
                    tool_config,
                    false,
                    None,
                    None,
                    false,
                    Some(extra),
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
                        provider_id: Some(credential.provider_id.clone()),
                    },
                )
                .await;
            }

            if !matches!(cfg.choice, Some(ToolChoice::Auto))
                && tool_choice_requires_auto(&err_message)
            {
                let mut auto_cfg = cfg.clone();
                auto_cfg.choice = Some(ToolChoice::Auto);
                let built = crate::chat_manager::request_builder::build_chat_request(
                    credential,
                    api_key,
                    &model.name,
                    messages,
                    None,
                    Some(0.4),
                    Some(1.0),
                    max_tokens,
                    context_length,
                    false,
                    request_id,
                    None,
                    None,
                    None,
                    Some(&auto_cfg),
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
                        provider_id: Some(credential.provider_id.clone()),
                    },
                )
                .await;
            }
        }
    }

    Ok(first_response)
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

fn tool_call_payload(provider_id: &str, call: &ToolCall, index: usize) -> Value {
    let is_ollama = crate::ollama::is_ollama_provider(Some(provider_id));
    let arguments = if is_ollama {
        call.arguments.clone()
    } else {
        Value::String(
            call.raw_arguments
                .clone()
                .unwrap_or_else(|| serde_json::to_string(&call.arguments).unwrap_or_default()),
        )
    };

    if is_ollama {
        json!({
            "type": "function",
            "function": {
                "index": index,
                "name": call.name,
                "arguments": arguments,
            }
        })
    } else {
        json!({
            "id": call.id,
            "type": "function",
            "function": {
                "name": call.name,
                "arguments": arguments,
            }
        })
    }
}

fn tool_result_message(
    provider_id: &str,
    tool_call_id: &str,
    tool_name: Option<&str>,
    result: &Value,
) -> Value {
    let mut message = json!({
        "role": "tool",
        "content": serde_json::to_string(result).unwrap_or_default(),
    });

    if let Some(obj) = message.as_object_mut() {
        if crate::ollama::is_ollama_provider(Some(provider_id)) {
            if let Some(name) = tool_name {
                obj.insert("tool_name".to_string(), json!(name));
            }
        } else {
            obj.insert("tool_call_id".to_string(), json!(tool_call_id));
        }
    }

    message
}

fn apply_call(working_soul: &mut Value, call: &ToolCall) -> (bool, Value) {
    match call.name.as_str() {
        "set_identity" => {
            let applied = apply_set_identity(working_soul, &call.arguments);
            (false, json!({ "ok": true, "applied": applied }))
        }
        "set_baseline_affect" => {
            let applied = apply_set_numeric_section(
                working_soul,
                &["soul", "baselineAffect"],
                BASELINE_AFFECT_FIELDS,
                &call.arguments,
            );
            (false, json!({ "ok": true, "applied": applied }))
        }
        "set_regulation_style" => {
            let applied = apply_set_numeric_section(
                working_soul,
                &["soul", "regulationStyle"],
                REGULATION_STYLE_FIELDS,
                &call.arguments,
            );
            (false, json!({ "ok": true, "applied": applied }))
        }
        "set_relationship_defaults" => {
            let applied = apply_set_numeric_section(
                working_soul,
                &["relationshipDefaults"],
                RELATIONSHIP_DEFAULTS_FIELDS,
                &call.arguments,
            );
            (false, json!({ "ok": true, "applied": applied }))
        }
        "done" => (true, json!({ "ok": true, "done": true })),
        _ => (
            false,
            json!({ "ok": false, "error": "unknown_tool", "name": call.name }),
        ),
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_with_target(
    app: &AppHandle,
    context: &ChatContext,
    settings: &Settings,
    args: &ChatGenerateCompanionSoulArgs,
    model: &Model,
    credential: &ProviderCredential,
    api_key: &str,
    operation_label: &str,
) -> Result<Value, String> {
    let mut working_soul = normalize_working_soul(args.current_soul.as_ref());

    let preview_session = preview_session();
    let (request_settings, extra_body_fields) = prepare_default_sampling_request(
        &credential.provider_id,
        &preview_session,
        model,
        settings,
        0.35,
        1.0,
        None,
        None,
        None,
    );

    let mut messages_for_api = render_messages(app, settings, credential, args);
    if messages_for_api.is_empty() {
        return Err("Companion Soul writer template rendered no prompt content".to_string());
    }

    let tool_config = build_tool_config();
    let format = fallback_format(settings);
    let format_label = fallback_format_label(format);

    let mut saw_done = false;
    let mut got_any_tool_calls = false;
    let mut last_failure_reason: Option<String> = None;

    for iteration in 0..MAX_LOOP_ITERATIONS {
        let iteration_request_id = args
            .request_id
            .as_deref()
            .map(|id| format!("{}:loop-{}", id, iteration + 1));

        let send_result = send_request(
            app,
            credential,
            model,
            api_key,
            &messages_for_api,
            request_settings.max_tokens,
            request_settings.context_length,
            extra_body_fields.clone(),
            Some(&tool_config),
            iteration_request_id.clone(),
        )
        .await;

        let api_response = match send_result {
            Ok(response) => response,
            Err(err) => {
                last_failure_reason = Some(err);
                break;
            }
        };

        let usage = extract_usage(api_response.data());
        record_usage_if_available(
            context,
            &usage,
            &preview_session,
            &dummy_character(),
            model,
            credential,
            api_key,
            now_millis().unwrap_or(0),
            UsageOperationType::ReplyHelper,
            operation_label,
        )
        .await;

        if !api_response.ok {
            let status_fallback = format!("Provider returned status {}", api_response.status);
            let err_message = extract_error_message(api_response.data()).unwrap_or(status_fallback);
            last_failure_reason = Some(err_message);
            break;
        }

        let mut calls = parse_tool_calls(&credential.provider_id, api_response.data());
        if calls.is_empty() {
            if let Some(text) = extract_text(api_response.data(), Some(&credential.provider_id)) {
                calls = parse_tool_calls_from_text(&text);
            }
        }

        if calls.is_empty() {
            log_warn(
                app,
                "companion_soul_writer",
                format!(
                    "iteration {} returned no tool calls (got_any={})",
                    iteration + 1,
                    got_any_tool_calls
                ),
            );
            if !got_any_tool_calls {
                last_failure_reason = Some("model returned no usable tool calls".to_string());
            }
            break;
        }

        got_any_tool_calls = true;

        let tool_calls_json: Vec<Value> = calls
            .iter()
            .enumerate()
            .map(|(idx, call)| tool_call_payload(&credential.provider_id, call, idx))
            .collect();
        let mut tool_results: Vec<Value> = Vec::new();

        for call in &calls {
            let (is_done, result) = apply_call(&mut working_soul, call);
            tool_results.push(result);
            if is_done {
                saw_done = true;
                break;
            }
        }

        log_info(
            app,
            "companion_soul_writer",
            format!(
                "iteration {}/{} applied calls={} saw_done={}",
                iteration + 1,
                MAX_LOOP_ITERATIONS,
                calls.len(),
                saw_done
            ),
        );

        if saw_done {
            break;
        }

        if iteration + 1 == MAX_LOOP_ITERATIONS {
            log_warn(
                app,
                "companion_soul_writer",
                format!(
                    "reached hard cap of {} iterations without done",
                    MAX_LOOP_ITERATIONS
                ),
            );
            break;
        }

        messages_for_api.push(json!({
            "role": "assistant",
            "content": Value::Null,
            "tool_calls": tool_calls_json,
        }));
        for (call_payload, result) in tool_calls_json.iter().zip(tool_results.iter()) {
            let tool_call_id = call_payload
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let tool_name = call_payload
                .get("function")
                .and_then(|value| value.get("name"))
                .and_then(Value::as_str);
            messages_for_api.push(tool_result_message(
                &credential.provider_id,
                tool_call_id,
                tool_name,
                result,
            ));
        }
    }

    if got_any_tool_calls {
        return Ok(working_soul);
    }

    let failure_reason =
        last_failure_reason.unwrap_or_else(|| "tool attempt produced no usable output".to_string());
    log_warn(
        app,
        "companion_soul_writer",
        format!(
            "tool loop yielded nothing usable; falling back to {}: {}",
            format_label, failure_reason
        ),
    );

    let mut fallback_messages = render_messages(app, settings, credential, args);
    fallback_messages.push(json!({
        "role": "user",
        "content": fallback_prompt(format),
    }));

    let api_response = send_request(
        app,
        credential,
        model,
        api_key,
        &fallback_messages,
        request_settings.max_tokens,
        request_settings.context_length,
        extra_body_fields.clone(),
        None,
        args.request_id.clone(),
    )
    .await?;

    let usage = extract_usage(api_response.data());
    record_usage_if_available(
        context,
        &usage,
        &preview_session,
        &dummy_character(),
        model,
        credential,
        api_key,
        now_millis().unwrap_or(0),
        UsageOperationType::ReplyHelper,
        &format!("{}_fallback", operation_label),
    )
    .await;

    if !api_response.ok {
        let status_fallback = format!("Provider returned status {}", api_response.status);
        let err_message = extract_error_message(api_response.data()).unwrap_or(status_fallback);
        return Err(format!(
            "companion soul {} fallback failed after tool attempt '{}': {}",
            format_label, failure_reason, err_message
        ));
    }

    let text = extract_text(api_response.data(), Some(&credential.provider_id))
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            format!(
                "companion soul {} fallback returned no text after tool attempt '{}'",
                format_label, failure_reason
            )
        })?;

    let calls = parse_fallback_calls(&text, format).map_err(|err| {
        format!(
            "companion soul {} fallback parse failed after tool attempt '{}': {}",
            format_label, failure_reason, err
        )
    })?;

    if calls.is_empty() {
        return Err(format!(
            "companion soul {} fallback returned no operations after tool attempt '{}'",
            format_label, failure_reason
        ));
    }

    for call in &calls {
        let _ = apply_call(&mut working_soul, call);
    }

    Ok(working_soul)
}

fn preview_session() -> Session {
    Session {
        id: "companion-soul-writer".to_string(),
        character_id: String::new(),
        title: "Companion Soul Writer".to_string(),
        background_image_path: None,
        system_prompt: None,
        mode: "companion".to_string(),
        selected_scene_id: None,
        prompt_template_id: None,
        lorebook_ids_override: None,
        author_note: None,
        persona_id: None,
        persona_disabled: false,
        voice_autoplay: None,
        advanced_model_settings: None,
        companion_state: None,
        memories: Vec::new(),
        memory_embeddings: Vec::new(),
        memory_summary: None,
        memory_summary_token_count: 0,
        memory_tool_events: Vec::new(),
        memory_status: None,
        memory_error: None,
        memory_progress_step: None,
        messages: Vec::new(),
        archived: false,
        created_at: 0,
        updated_at: 0,
    }
}

fn dummy_character() -> crate::chat_manager::types::Character {
    crate::chat_manager::types::Character {
        id: String::new(),
        name: String::new(),
        avatar_path: None,
        design_description: None,
        design_reference_image_ids: Vec::new(),
        background_image_path: None,
        definition: None,
        description: None,
        rules: Vec::new(),
        scenes: Vec::new(),
        default_scene_id: None,
        default_model_id: None,
        fallback_model_id: None,
        mode: "roleplay".to_string(),
        companion: None,
        memory_type: "manual".to_string(),
        active_lorebook_ids: Vec::new(),
        prompt_template_id: None,
        group_chat_prompt_template_id: None,
        group_chat_roleplay_prompt_template_id: None,
        system_prompt: None,
        created_at: 0,
        updated_at: 0,
    }
}

pub async fn chat_generate_companion_soul(
    app: AppHandle,
    args: ChatGenerateCompanionSoulArgs,
) -> Result<Value, String> {
    let context = ChatContext::initialize(app.clone())?;
    let settings = context.settings.clone();

    let primary_target = resolve_companion_soul_writer_target(&settings, args.model_id.as_deref())?;
    let (model, credential) = primary_target;
    let api_key = require_api_key(&app, credential, "companion_soul_writer")?;

    let primary_result = run_with_target(
        &app,
        &context,
        &settings,
        &args,
        model,
        credential,
        &api_key,
        "companion_soul_writer",
    )
    .await;

    if let Ok(value) = primary_result {
        return Ok(value);
    }

    let primary_err = primary_result.err().unwrap();

    if let Some((fb_model, fb_credential)) =
        resolve_companion_soul_writer_fallback_target(&settings)
    {
        if fb_model.id == model.id {
            return Err(primary_err);
        }
        log_warn(
            &app,
            "companion_soul_writer",
            format!(
                "primary model failed ({}); retrying with fallback model {}",
                primary_err, fb_model.name
            ),
        );
        let fb_api_key = require_api_key(&app, fb_credential, "companion_soul_writer")?;
        return run_with_target(
            &app,
            &context,
            &settings,
            &args,
            fb_model,
            fb_credential,
            &fb_api_key,
            "companion_soul_writer_fallback_model",
        )
        .await
        .map_err(|err| {
            format!(
                "companion soul writer failed on primary ({}) and fallback ({})",
                primary_err, err
            )
        });
    }

    Err(primary_err)
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

fn parse_fallback_calls(
    raw: &str,
    format: DynamicMemoryStructuredFallbackFormat,
) -> Result<Vec<ToolCall>, String> {
    match format {
        DynamicMemoryStructuredFallbackFormat::Json => parse_fallback_json(raw),
        DynamicMemoryStructuredFallbackFormat::Xml => parse_fallback_xml(raw),
    }
}

fn parse_fallback_json(raw: &str) -> Result<Vec<ToolCall>, String> {
    let normalized = normalize_structured_fallback_text(raw);
    let snippet = extract_json_snippet(&normalized).unwrap_or(normalized.as_str());
    let value: Value = serde_json::from_str(snippet)
        .map_err(|err| format!("fallback JSON parse error: {}", err))?;

    let operations = match &value {
        Value::Array(items) => items.clone(),
        Value::Object(map) => map
            .get("operations")
            .or_else(|| map.get("ops"))
            .and_then(Value::as_array)
            .cloned()
            .ok_or_else(|| "fallback JSON missing operations array".to_string())?,
        _ => {
            return Err("fallback JSON must be object or array".to_string());
        }
    };

    let mut calls = Vec::new();
    for (index, item) in operations.iter().enumerate() {
        let Some(obj) = item.as_object() else {
            continue;
        };
        let name = obj
            .get("name")
            .or_else(|| obj.get("tool"))
            .or_else(|| obj.get("op"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| format!("fallback JSON operation {} is missing a name", index + 1))?;

        if !SOUL_OP_TAGS.contains(&name) {
            continue;
        }

        let arguments = match obj.get("arguments") {
            Some(Value::Object(args)) => Value::Object(args.clone()),
            Some(other) => other.clone(),
            None => {
                let mut args = Map::new();
                for (key, value) in obj {
                    if matches!(key.as_str(), "name" | "tool" | "op") {
                        continue;
                    }
                    args.insert(key.clone(), value.clone());
                }
                Value::Object(args)
            }
        };

        calls.push(ToolCall {
            id: format!("json_op_{}", index + 1),
            name: name.to_string(),
            arguments,
            raw_arguments: None,
        });
    }

    Ok(calls)
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

fn parse_fallback_xml(raw: &str) -> Result<Vec<ToolCall>, String> {
    let normalized = normalize_structured_fallback_text(raw);
    let mut reader = Reader::from_str(&normalized);
    reader.config_mut().trim_text(false);

    let mut buf = Vec::new();
    let mut root_seen = false;
    let mut current_op: Option<String> = None;
    let mut current_args: Map<String, Value> = Map::new();
    let mut current_field: Option<String> = None;
    let mut calls = Vec::new();
    let mut op_index = 0usize;

    fn ingest_attrs(args: &mut Map<String, Value>, event: &quick_xml::events::BytesStart<'_>) {
        for attr in event.attributes().flatten() {
            let key = String::from_utf8_lossy(attr.key.as_ref()).into_owned();
            let raw = match attr.unescape_value() {
                Ok(cow) => cow.into_owned(),
                Err(_) => continue,
            };
            if let Ok(num) = raw.trim().parse::<f64>() {
                if let Some(n) = serde_json::Number::from_f64(num) {
                    args.insert(key, Value::Number(n));
                    continue;
                }
            }
            args.insert(key, Value::String(raw));
        }
    }

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(event)) => {
                let tag = String::from_utf8_lossy(event.name().as_ref()).into_owned();
                if !root_seen && SOUL_OP_ROOT_TAGS.contains(&tag.as_str()) {
                    root_seen = true;
                } else if root_seen && current_op.is_none() && SOUL_OP_TAGS.contains(&tag.as_str())
                {
                    current_op = Some(tag);
                    current_args = Map::new();
                    ingest_attrs(&mut current_args, &event);
                } else if current_op.is_some() {
                    current_field = Some(tag);
                }
            }
            Ok(Event::Empty(event)) => {
                let tag = String::from_utf8_lossy(event.name().as_ref()).into_owned();
                if !root_seen && SOUL_OP_ROOT_TAGS.contains(&tag.as_str()) {
                    root_seen = true;
                } else if root_seen && current_op.is_none() && SOUL_OP_TAGS.contains(&tag.as_str())
                {
                    let mut args = Map::new();
                    ingest_attrs(&mut args, &event);
                    op_index += 1;
                    calls.push(ToolCall {
                        id: format!("xml_op_{}", op_index),
                        name: tag,
                        arguments: Value::Object(args),
                        raw_arguments: None,
                    });
                }
            }
            Ok(Event::Text(event)) => {
                if let (Some(field), Some(_)) = (current_field.as_deref(), current_op.as_ref()) {
                    let text = decode_xml_text(event.as_ref())?;
                    append_text_field(&mut current_args, field, &text);
                }
            }
            Ok(Event::CData(event)) => {
                if let (Some(field), Some(_)) = (current_field.as_deref(), current_op.as_ref()) {
                    let text = String::from_utf8_lossy(event.as_ref());
                    append_text_field(&mut current_args, field, &text);
                }
            }
            Ok(Event::GeneralRef(event)) => {
                if let (Some(field), Some(_)) = (current_field.as_deref(), current_op.as_ref()) {
                    let text = decode_xml_general_ref(event)?;
                    append_text_field(&mut current_args, field, &text);
                }
            }
            Ok(Event::End(event)) => {
                let tag = String::from_utf8_lossy(event.name().as_ref()).into_owned();
                if current_field.as_deref() == Some(tag.as_str()) {
                    current_field = None;
                } else if current_op.as_deref() == Some(tag.as_str()) {
                    coerce_numeric_strings(&mut current_args);
                    op_index += 1;
                    calls.push(ToolCall {
                        id: format!("xml_op_{}", op_index),
                        name: current_op.take().unwrap_or_default(),
                        arguments: Value::Object(std::mem::take(&mut current_args)),
                        raw_arguments: None,
                    });
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(format!("fallback XML parse error: {}", err)),
            _ => {}
        }
        buf.clear();
    }

    if !root_seen {
        return Err("fallback response did not contain a soul_ops root".to_string());
    }

    Ok(calls)
}

fn append_text_field(args: &mut Map<String, Value>, key: &str, fragment: &str) {
    if fragment.is_empty() {
        return;
    }
    match args.get_mut(key) {
        Some(Value::String(existing)) => existing.push_str(fragment),
        _ => {
            args.insert(key.to_string(), Value::String(fragment.to_string()));
        }
    }
}

fn coerce_numeric_strings(args: &mut Map<String, Value>) {
    let numeric_keys: std::collections::HashSet<&'static str> = BASELINE_AFFECT_FIELDS
        .iter()
        .copied()
        .chain(REGULATION_STYLE_FIELDS.iter().copied())
        .chain(RELATIONSHIP_DEFAULTS_FIELDS.iter().copied())
        .collect();
    for (key, value) in args.iter_mut() {
        if !numeric_keys.contains(key.as_str()) {
            if let Value::String(text) = value {
                *text = text.trim().to_string();
            }
            continue;
        }
        if let Value::String(text) = value {
            if let Ok(num) = text.trim().parse::<f64>() {
                if let Some(n) = serde_json::Number::from_f64(num) {
                    *value = Value::Number(n);
                }
            }
        }
    }
    args.retain(|_, value| match value {
        Value::String(text) => !text.is_empty(),
        _ => true,
    });
}
