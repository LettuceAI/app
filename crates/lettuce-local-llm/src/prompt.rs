//! Prompt building carried over from the legacy runtime: the chat template
//! resolves from an explicit override, then the GGUF's embedded template,
//! then a named preset; tool requests go through the OpenAI-compatible
//! template path, plain chats try it before the legacy template call, and
//! the raw `role: content` transcript is the opt-in last resort. BOS is only
//! added to raw completions, following the tokenizer's metadata.

use llama_cpp_2::TokenToStringError;
use llama_cpp_2::model::{
    AddBos, ChatTemplateResult, LlamaChatMessage, LlamaChatTemplate, LlamaModel,
};
use llama_cpp_2::openai::OpenAIChatTemplateParams;
use llama_cpp_2::token::LlamaToken;
use serde_json::Value;

pub const TOKENIZER_ADD_BOS_METADATA_KEY: &str = "tokenizer.ggml.add_bos_token";

const TOOL_TEMPLATE_MARKERS: &[&str] = &[
    "<tool_call>",
    "<tool_calls>",
    "</tool_calls>",
    "<tool_response>",
    "<tools>",
    "</tools>",
    "<available_tools>",
    "<function=",
    "<parameters>",
    "<parameter=",
    "<arg_key>",
    "<arg_value>",
    "<|tool_call>",
    "<tool_call|>",
    "<|tool_response>",
    "<tool_response|>",
    "<|tool_call_start|>",
    "<|tool_calls_section_begin|>",
    "<|tool_list_start|>",
    "<|tools_prefix|>",
    "<｜tool▁calls▁begin｜>",
    "tool_declare",
    "# Tools",
];

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PromptError {
    #[error("Failed to decode token bytes: {0}")]
    TokenDecode(String),
    #[error("Invalid chat message: {0}")]
    InvalidMessage(String),
    #[error("No usable chat messages for llama.cpp")]
    NoUsableMessages,
    #[error("Invalid explicit llama chat template override: {0}")]
    InvalidOverride(String),
    #[error("Invalid llama chat template preset '{preset}': {error}")]
    InvalidPreset { preset: String, error: String },
    #[error(
        "No llama chat template resolved. Provide an explicit override, use a GGUF with an embedded template, or select a known preset."
    )]
    NoTemplate,
    #[error("llama.cpp tool calling requires a resolved native chat template: {0}")]
    ToolsNeedTemplate(Box<PromptError>),
    #[error("Unsupported llama.cpp tool_choice '{0}'")]
    UnsupportedToolChoice(String),
    #[error("Unsupported llama.cpp named tool choice payload")]
    UnsupportedNamedToolChoice,
    #[error("Requested llama.cpp tool '{0}' was not found in the tools array")]
    ToolNotFound(String),
    #[error("Unsupported llama.cpp tool_choice payload: {0}")]
    UnsupportedToolChoicePayload(String),
    #[error("Failed to serialize llama.cpp messages for tool calling: {0}")]
    SerializeToolMessages(String),
    #[error("Failed to serialize llama.cpp tools for tool calling: {0}")]
    SerializeTools(String),
    #[error("Failed to serialize llama.cpp messages for chat templating: {0}")]
    SerializeMessages(String),
    #[error("Failed to apply llama.cpp OpenAI-compatible chat template: {error}{diagnostics}")]
    OpenAICompatTemplate { error: String, diagnostics: String },
    #[error(
        "Failed to apply llama chat template from {source_label} via oaicompat ({oaicompat}) and legacy ({legacy})"
    )]
    Template {
        source_label: String,
        oaicompat: String,
        legacy: String,
    },
}

#[derive(Debug)]
pub struct ResolvedChatTemplate {
    pub template: LlamaChatTemplate,
    pub source_label: String,
    pub template_text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromptMode {
    TemplatedChat,
    OpenAICompatChat,
    RawCompletion,
}

#[derive(Debug)]
pub struct BuiltPrompt {
    pub prompt: String,
    pub attempted_template_source: Option<String>,
    pub attempted_template_text: Option<String>,
    pub applied_template_source: Option<String>,
    pub applied_template_text: Option<String>,
    pub resolved_tool_choice: Option<String>,
    pub used_raw_completion_fallback: bool,
    pub raw_completion_fallback_reason: Option<String>,
    pub prompt_mode: PromptMode,
    pub chat_template_result: Option<ChatTemplateResult>,
    pub native_tool_parse_supported: bool,
    pub additional_stop_sequences: Vec<String>,
    pub tool_template_diagnostics: Option<String>,
}

/// The reply prefill that opens Gemma4's thought channel.
pub const GEMMA4_REASONING_PREFILL: &str = "<|channel>thought\n";

/// The marker that closes Gemma4's thought channel.
pub const GEMMA4_REASONING_CLOSE: &str = "<channel|>";

/// The messages with the think marker at the start of the first system
/// message, which is added when there is none: Gemma templates accept only
/// one leading system message.
#[must_use]
pub fn prepend_reasoning_system_prefix(messages: &[Value]) -> Vec<Value> {
    const THINK_OPENER: &str = "<|think|>\n";
    let mut out = messages.to_vec();
    if let Some(system) = out
        .iter_mut()
        .find(|message| message.get("role").and_then(Value::as_str) == Some("system"))
    {
        match system.get("content").cloned() {
            Some(Value::String(text)) => {
                system["content"] = Value::String(format!("{THINK_OPENER}{text}"));
            }
            Some(Value::Array(mut parts)) => {
                parts.insert(
                    0,
                    serde_json::json!({ "type": "text", "text": THINK_OPENER }),
                );
                system["content"] = Value::Array(parts);
            }
            _ => system["content"] = Value::String(THINK_OPENER.to_string()),
        }
    } else {
        out.insert(
            0,
            serde_json::json!({ "role": "system", "content": THINK_OPENER }),
        );
    }
    out
}

#[derive(Clone, Debug, Default)]
pub struct OpenAICompatPromptOptions {
    pub reasoning_format: Option<String>,
    pub chat_template_kwargs: Option<String>,
    pub parallel_tool_calls: bool,
    pub enable_thinking: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct PromptRequest<'a> {
    pub messages: &'a [Value],
    pub chat_template_override: Option<&'a str>,
    pub chat_template_preset: Option<&'a str>,
    pub allow_raw_completion_fallback: bool,
    pub tools: Option<&'a Value>,
    pub tool_choice: Option<&'a Value>,
    pub options: &'a OpenAICompatPromptOptions,
}

fn normalize_role(role: &str) -> &'static str {
    match role {
        "system" | "developer" => "system",
        "assistant" => "assistant",
        _ => "user",
    }
}

fn sanitize_text(value: &str) -> String {
    value.replace('\0', "")
}

pub fn token_piece_bytes(model: &LlamaModel, token: LlamaToken) -> Result<Vec<u8>, PromptError> {
    fn decode_with_special_mode(
        model: &LlamaModel,
        token: LlamaToken,
        special: bool,
    ) -> Result<Vec<u8>, TokenToStringError> {
        match model.token_to_piece_bytes(token, 8, special, None) {
            Ok(bytes) => Ok(bytes),
            Err(TokenToStringError::InsufficientBufferSpace(needed)) => {
                let required = usize::try_from(-needed)
                    .map_err(|_| TokenToStringError::InsufficientBufferSpace(needed))?;
                model.token_to_piece_bytes(token, required, special, None)
            }
            Err(error) => Err(error),
        }
    }

    match decode_with_special_mode(model, token, false) {
        Ok(bytes) => Ok(bytes),
        Err(TokenToStringError::UnknownTokenType) => decode_with_special_mode(model, token, true)
            .map_err(|error| PromptError::TokenDecode(error.to_string())),
        Err(error) => Err(PromptError::TokenDecode(error.to_string())),
    }
}

fn extract_text_content(message: &Value) -> String {
    match message.get("content") {
        Some(Value::String(text)) => sanitize_text(text),
        Some(Value::Array(parts)) => parts
            .iter()
            .filter(|part| part.get("type").and_then(Value::as_str) == Some("text"))
            .filter_map(|part| part.get("text").and_then(Value::as_str))
            .map(sanitize_text)
            .filter(|cleaned| !cleaned.is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

fn message_role(message: &Value) -> &'static str {
    message
        .get("role")
        .and_then(Value::as_str)
        .map(normalize_role)
        .unwrap_or("user")
}

fn build_fallback_prompt(messages: &[Value]) -> String {
    let mut prompt = String::new();
    for message in messages {
        let content = extract_text_content(message);
        if content.is_empty() {
            continue;
        }
        prompt.push_str(message_role(message));
        prompt.push_str(": ");
        prompt.push_str(&content);
        prompt.push('\n');
    }
    prompt.push_str("assistant: ");
    prompt
}

pub fn inject_media_markers(messages: &[Value]) -> Vec<Value> {
    messages
        .iter()
        .map(|message| {
            let Some(parts) = message.get("content").and_then(Value::as_array) else {
                return message.clone();
            };

            let mut text_parts = Vec::new();
            for part in parts {
                match part.get("type").and_then(Value::as_str) {
                    Some("text") => {
                        if let Some(text) = part.get("text").and_then(Value::as_str) {
                            let cleaned = sanitize_text(text);
                            if !cleaned.is_empty() {
                                text_parts.push(cleaned);
                            }
                        }
                    }
                    Some("image_url") | Some("input_audio") => {
                        text_parts.push(llama_cpp_2::mtmd::mtmd_default_marker().to_string())
                    }
                    _ => {}
                }
            }

            let mut cloned = message.clone();
            if let Some(object) = cloned.as_object_mut() {
                object.insert("content".to_string(), Value::String(text_parts.join("\n")));
            }
            cloned
        })
        .collect()
}

fn message_requires_openai_compat(message: &Value) -> bool {
    message.get("role").and_then(Value::as_str) == Some("tool")
        || message.get("tool_calls").is_some()
        || message.get("tool_call_id").is_some()
}

fn template_appears_tool_aware(template: &str) -> bool {
    TOOL_TEMPLATE_MARKERS
        .iter()
        .any(|marker| template.contains(marker))
}

fn summarize_tool_template_detection(template: &str) -> String {
    let matched = TOOL_TEMPLATE_MARKERS
        .iter()
        .copied()
        .filter(|marker| template.contains(marker))
        .collect::<Vec<_>>();
    let preview = template
        .chars()
        .take(220)
        .collect::<String>()
        .replace('\n', "\\n");

    if matched.is_empty() {
        format!("tool-template heuristic found no known markers; template_preview=\"{preview}\"")
    } else {
        format!(
            "tool-template heuristic matched markers=[{}] template_preview=\"{preview}\"",
            matched.join(", ")
        )
    }
}

fn oaicompat_result_supports_native_tool_calls(result: &ChatTemplateResult) -> bool {
    result.parse_tool_calls || result.parser.is_some() || result.grammar.is_some()
}

fn normalize_tool_choice_for_llama(
    tools: &mut Vec<Value>,
    tool_choice: Option<&Value>,
) -> Result<Option<String>, PromptError> {
    match tool_choice {
        None => Ok(Some("auto".to_string())),
        Some(Value::String(choice)) => match choice.as_str() {
            "auto" | "none" | "required" => Ok(Some(choice.clone())),
            other => Err(PromptError::UnsupportedToolChoice(other.to_string())),
        },
        Some(Value::Object(object)) => {
            let Some(name) = object
                .get("function")
                .and_then(|value| value.get("name"))
                .and_then(Value::as_str)
            else {
                return Err(PromptError::UnsupportedNamedToolChoice);
            };

            tools.retain(|tool| {
                tool.get("function")
                    .and_then(|value| value.get("name"))
                    .and_then(Value::as_str)
                    == Some(name)
            });

            if tools.is_empty() {
                return Err(PromptError::ToolNotFound(name.to_string()));
            }

            Ok(Some("required".to_string()))
        }
        Some(other) => Err(PromptError::UnsupportedToolChoicePayload(other.to_string())),
    }
}

fn templated_prompt(
    prompt: String,
    resolved_template: &ResolvedChatTemplate,
    prompt_mode: PromptMode,
) -> BuiltPrompt {
    BuiltPrompt {
        prompt,
        attempted_template_source: Some(resolved_template.source_label.clone()),
        attempted_template_text: Some(resolved_template.template_text.clone()),
        applied_template_source: Some(resolved_template.source_label.clone()),
        applied_template_text: Some(resolved_template.template_text.clone()),
        resolved_tool_choice: None,
        used_raw_completion_fallback: false,
        raw_completion_fallback_reason: None,
        prompt_mode,
        chat_template_result: None,
        native_tool_parse_supported: false,
        additional_stop_sequences: Vec::new(),
        tool_template_diagnostics: None,
    }
}

fn raw_completion_prompt(
    messages: &[Value],
    attempted: Option<&ResolvedChatTemplate>,
    reason: String,
) -> BuiltPrompt {
    BuiltPrompt {
        prompt: build_fallback_prompt(messages),
        attempted_template_source: attempted.map(|template| template.source_label.clone()),
        attempted_template_text: attempted.map(|template| template.template_text.clone()),
        applied_template_source: None,
        applied_template_text: None,
        resolved_tool_choice: None,
        used_raw_completion_fallback: true,
        raw_completion_fallback_reason: Some(reason),
        prompt_mode: PromptMode::RawCompletion,
        chat_template_result: None,
        native_tool_parse_supported: false,
        additional_stop_sequences: Vec::new(),
        tool_template_diagnostics: None,
    }
}

fn build_oaicompat_prompt(
    model: &LlamaModel,
    request: &PromptRequest<'_>,
    resolved_template: &ResolvedChatTemplate,
) -> Result<BuiltPrompt, PromptError> {
    let mut tool_template_diagnostics =
        (!template_appears_tool_aware(&resolved_template.template_text))
            .then(|| summarize_tool_template_detection(&resolved_template.template_text));

    let messages_json = serde_json::to_string(request.messages)
        .map_err(|error| PromptError::SerializeToolMessages(error.to_string()))?;

    let mut tools_vec = request
        .tools
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let has_tools = !tools_vec.is_empty();
    let normalized_tool_choice = if has_tools {
        normalize_tool_choice_for_llama(&mut tools_vec, request.tool_choice)?
    } else {
        Some("none".to_string())
    };
    let parse_tool_calls = has_tools && normalized_tool_choice.as_deref() != Some("none");
    let tools_json = if has_tools {
        Some(
            serde_json::to_string(&tools_vec)
                .map_err(|error| PromptError::SerializeTools(error.to_string()))?,
        )
    } else {
        None
    };

    let params = OpenAIChatTemplateParams {
        messages_json: &messages_json,
        tools_json: tools_json.as_deref(),
        tool_choice: normalized_tool_choice.as_deref(),
        json_schema: None,
        grammar: None,
        reasoning_format: request.options.reasoning_format.as_deref(),
        chat_template_kwargs: request.options.chat_template_kwargs.as_deref(),
        add_generation_prompt: true,
        use_jinja: true,
        parallel_tool_calls: has_tools && request.options.parallel_tool_calls,
        enable_thinking: request.options.enable_thinking,
        add_bos: false,
        add_eos: false,
        parse_tool_calls,
    };

    let chat_template_result = model
        .apply_chat_template_oaicompat(&resolved_template.template, &params)
        .map_err(|error| PromptError::OpenAICompatTemplate {
            error: error.to_string(),
            diagnostics: tool_template_diagnostics
                .as_ref()
                .map(|diagnostics| format!(" ({diagnostics})"))
                .unwrap_or_default(),
        })?;

    let native_tool_parse_supported =
        oaicompat_result_supports_native_tool_calls(&chat_template_result);
    if parse_tool_calls && !native_tool_parse_supported {
        let parser_diag = format!(
            "oaicompat template exposed no native tool parser metadata (parse_tool_calls={}, parser_present={}, grammar_present={})",
            chat_template_result.parse_tool_calls,
            chat_template_result.parser.is_some(),
            chat_template_result.grammar.is_some(),
        );
        tool_template_diagnostics = Some(match tool_template_diagnostics {
            Some(existing) => format!("{existing}; {parser_diag}"),
            None => parser_diag,
        });
    }

    let mut built = templated_prompt(
        chat_template_result.prompt.clone(),
        resolved_template,
        PromptMode::OpenAICompatChat,
    );
    built.resolved_tool_choice = normalized_tool_choice;
    built.additional_stop_sequences = chat_template_result.additional_stops.clone();
    built.chat_template_result = Some(chat_template_result);
    built.native_tool_parse_supported = native_tool_parse_supported;
    built.tool_template_diagnostics = tool_template_diagnostics;
    Ok(built)
}

fn build_plain_templated_prompt(
    model: &LlamaModel,
    request: &PromptRequest<'_>,
    chat_messages: &[LlamaChatMessage],
    resolved_template: &ResolvedChatTemplate,
) -> Result<BuiltPrompt, PromptError> {
    let messages_json = serde_json::to_string(request.messages)
        .map_err(|error| PromptError::SerializeMessages(error.to_string()))?;

    let params = OpenAIChatTemplateParams {
        messages_json: &messages_json,
        tools_json: None,
        tool_choice: None,
        json_schema: None,
        grammar: None,
        reasoning_format: request.options.reasoning_format.as_deref(),
        chat_template_kwargs: request.options.chat_template_kwargs.as_deref(),
        add_generation_prompt: true,
        use_jinja: true,
        parallel_tool_calls: false,
        enable_thinking: request.options.enable_thinking,
        add_bos: false,
        add_eos: false,
        parse_tool_calls: false,
    };

    match model.apply_chat_template_oaicompat(&resolved_template.template, &params) {
        Ok(result) => {
            let mut built =
                templated_prompt(result.prompt, resolved_template, PromptMode::TemplatedChat);
            built.additional_stop_sequences = result.additional_stops;
            Ok(built)
        }
        Err(oaicompat_error) => {
            match model.apply_chat_template(&resolved_template.template, chat_messages, true) {
                Ok(prompt) => Ok(templated_prompt(
                    prompt,
                    resolved_template,
                    PromptMode::TemplatedChat,
                )),
                Err(legacy_error) => Err(PromptError::Template {
                    source_label: resolved_template.source_label.clone(),
                    oaicompat: oaicompat_error.to_string(),
                    legacy: legacy_error.to_string(),
                }),
            }
        }
    }
}

fn chat_template_text(template: &LlamaChatTemplate) -> String {
    template.as_c_str().to_string_lossy().into_owned()
}

pub fn resolve_chat_template(
    model: &LlamaModel,
    chat_template_override: Option<&str>,
    chat_template_preset: Option<&str>,
) -> Result<ResolvedChatTemplate, PromptError> {
    if let Some(template_override) = chat_template_override.filter(|value| !value.trim().is_empty())
    {
        let template = LlamaChatTemplate::new(template_override)
            .map_err(|error| PromptError::InvalidOverride(error.to_string()))?;
        return Ok(ResolvedChatTemplate {
            template,
            source_label: "explicit override".to_string(),
            template_text: template_override.to_string(),
        });
    }

    if let Ok(template) = model.chat_template(None) {
        return Ok(ResolvedChatTemplate {
            template_text: chat_template_text(&template),
            template,
            source_label: "embedded gguf".to_string(),
        });
    }

    if let Some(template_preset) = chat_template_preset.filter(|value| !value.trim().is_empty()) {
        let template = LlamaChatTemplate::new(template_preset).map_err(|error| {
            PromptError::InvalidPreset {
                preset: template_preset.to_string(),
                error: error.to_string(),
            }
        })?;
        return Ok(ResolvedChatTemplate {
            template_text: template_preset.to_string(),
            template,
            source_label: format!("preset '{template_preset}'"),
        });
    }

    Err(PromptError::NoTemplate)
}

pub fn build_prompt(
    model: &LlamaModel,
    request: &PromptRequest<'_>,
) -> Result<BuiltPrompt, PromptError> {
    let needs_openai_compat = request
        .tools
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty())
        || request.messages.iter().any(message_requires_openai_compat);

    let mut chat_messages = Vec::new();
    for message in request.messages {
        let content = extract_text_content(message);
        if content.is_empty() {
            continue;
        }
        let chat_message = LlamaChatMessage::new(message_role(message).to_string(), content)
            .map_err(|error| PromptError::InvalidMessage(error.to_string()))?;
        chat_messages.push(chat_message);
    }

    if chat_messages.is_empty() {
        return Err(PromptError::NoUsableMessages);
    }

    let resolved_template = match resolve_chat_template(
        model,
        request.chat_template_override,
        request.chat_template_preset,
    ) {
        Ok(resolved) => resolved,
        Err(error) if needs_openai_compat => {
            return Err(PromptError::ToolsNeedTemplate(Box::new(error)));
        }
        Err(error) if request.allow_raw_completion_fallback => {
            return Ok(raw_completion_prompt(
                request.messages,
                None,
                format!("template resolution failed: {error}"),
            ));
        }
        Err(error) => return Err(error),
    };

    if needs_openai_compat {
        return build_oaicompat_prompt(model, request, &resolved_template);
    }

    match build_plain_templated_prompt(model, request, &chat_messages, &resolved_template) {
        Ok(built_prompt) => Ok(built_prompt),
        Err(error) if request.allow_raw_completion_fallback => Ok(raw_completion_prompt(
            request.messages,
            Some(&resolved_template),
            format!("template application failed: {error}"),
        )),
        Err(error) => Err(error),
    }
}

pub fn model_tokenizer_adds_bos(model: &LlamaModel) -> Option<bool> {
    let raw_value = model.meta_val_str(TOKENIZER_ADD_BOS_METADATA_KEY).ok()?;
    match raw_value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" => Some(true),
        "false" | "0" => Some(false),
        _ => None,
    }
}

pub fn prompt_add_bos(prompt_mode: PromptMode, model_tokenizer_adds_bos: Option<bool>) -> AddBos {
    match prompt_mode {
        PromptMode::TemplatedChat | PromptMode::OpenAICompatChat => AddBos::Never,
        PromptMode::RawCompletion => match model_tokenizer_adds_bos {
            Some(false) => AddBos::Never,
            Some(true) | None => AddBos::Always,
        },
    }
}

pub fn resolve_prompt_add_bos(model: &LlamaModel, prompt_mode: PromptMode) -> AddBos {
    match prompt_mode {
        PromptMode::TemplatedChat | PromptMode::OpenAICompatChat => AddBos::Never,
        PromptMode::RawCompletion => prompt_add_bos(prompt_mode, model_tokenizer_adds_bos(model)),
    }
}

pub fn prompt_mode_label(prompt_mode: PromptMode) -> &'static str {
    match prompt_mode {
        PromptMode::TemplatedChat => "templated_chat",
        PromptMode::OpenAICompatChat => "oaicompat_chat",
        PromptMode::RawCompletion => "raw_completion",
    }
}

pub fn add_bos_label(add_bos: AddBos) -> &'static str {
    match add_bos {
        AddBos::Always => "always",
        AddBos::Never => "never",
    }
}

pub fn model_tokenizer_add_bos_label(model_tokenizer_adds_bos: Option<bool>) -> &'static str {
    match model_tokenizer_adds_bos {
        Some(true) => "true",
        Some(false) => "false",
        None => "unknown",
    }
}

pub fn prompt_add_bos_reason(
    prompt_mode: PromptMode,
    model_tokenizer_adds_bos: Option<bool>,
) -> &'static str {
    match prompt_mode {
        PromptMode::TemplatedChat | PromptMode::OpenAICompatChat => {
            "templated chat prompt already carries template/model BOS handling"
        }
        PromptMode::RawCompletion if model_tokenizer_adds_bos == Some(true) => {
            "raw completion follows tokenizer/model BOS default=enabled"
        }
        PromptMode::RawCompletion if model_tokenizer_adds_bos == Some(false) => {
            "raw completion follows tokenizer/model BOS default=disabled"
        }
        PromptMode::RawCompletion => {
            "raw completion metadata missing or invalid; using compatibility fallback add_bos=always"
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn the_think_marker_opens_the_first_system_message() {
        let text = prepend_reasoning_system_prefix(&[
            json!({"role": "user", "content": "hi"}),
            json!({"role": "system", "content": "Be kind."}),
            json!({"role": "system", "content": "Second."}),
        ]);
        assert_eq!(text[1]["content"], "<|think|>\nBe kind.");
        assert_eq!(text[2]["content"], "Second.");
        let parts = prepend_reasoning_system_prefix(&[json!({
            "role": "system",
            "content": [{"type": "text", "text": "Be kind."}]
        })]);
        assert_eq!(parts[0]["content"][0]["text"], "<|think|>\n");
        assert_eq!(parts[0]["content"][1]["text"], "Be kind.");
        let none = prepend_reasoning_system_prefix(&[json!({"role": "user", "content": "hi"})]);
        assert_eq!(none[0], json!({"role": "system", "content": "<|think|>\n"}));
        let empty = prepend_reasoning_system_prefix(&[json!({"role": "system", "content": null})]);
        assert_eq!(empty[0]["content"], "<|think|>\n");
    }

    #[test]
    fn detects_gemma_style_tool_markers() {
        assert!(template_appears_tool_aware(
            "{% if tools %}<|tool_call>{{ tools }}<tool_call|>{% endif %}"
        ));
        assert!(!template_appears_tool_aware("{{ messages }}"));
    }

    #[test]
    fn raw_fallback_transcript_normalizes_roles_and_skips_empty_messages() {
        let messages = vec![
            json!({"role": "developer", "content": "rules\0"}),
            json!({"role": "user", "content": [{"type": "text", "text": "a"}, {"type": "image_url"}, {"type": "text", "text": "b"}]}),
            json!({"role": "assistant", "content": ""}),
            json!({"role": "narrator", "content": "c"}),
            json!({"content": "d"}),
        ];
        assert_eq!(
            build_fallback_prompt(&messages),
            "system: rules\nuser: a\nb\nuser: c\nuser: d\nassistant: "
        );
    }

    #[test]
    fn media_parts_become_markers_in_order() {
        let marker = llama_cpp_2::mtmd::mtmd_default_marker();
        let messages = vec![
            json!({"role": "user", "content": [{"type": "text", "text": "look"}, {"type": "image_url", "image_url": {"url": "x"}}, {"type": "input_audio"}, {"type": "video"}]}),
            json!({"role": "assistant", "content": "plain"}),
        ];
        let injected = inject_media_markers(&messages);
        assert_eq!(
            injected[0]["content"],
            Value::String(format!("look\n{marker}\n{marker}"))
        );
        assert_eq!(injected[1], messages[1]);
    }

    #[test]
    fn tool_messages_require_the_openai_compatible_path() {
        assert!(message_requires_openai_compat(&json!({"role": "tool"})));
        assert!(message_requires_openai_compat(
            &json!({"role": "assistant", "tool_calls": []})
        ));
        assert!(message_requires_openai_compat(
            &json!({"role": "user", "tool_call_id": "1"})
        ));
        assert!(!message_requires_openai_compat(
            &json!({"role": "user", "content": "x"})
        ));
    }

    #[test]
    fn tool_choice_normalization_matches_legacy() {
        let mut tools = vec![
            json!({"function": {"name": "a"}}),
            json!({"function": {"name": "b"}}),
        ];
        assert_eq!(
            normalize_tool_choice_for_llama(&mut tools, None),
            Ok(Some("auto".to_string()))
        );
        assert_eq!(
            normalize_tool_choice_for_llama(&mut tools, Some(&json!("none"))),
            Ok(Some("none".to_string()))
        );
        assert_eq!(
            normalize_tool_choice_for_llama(&mut tools, Some(&json!("any"))),
            Err(PromptError::UnsupportedToolChoice("any".to_string()))
        );
        assert_eq!(
            normalize_tool_choice_for_llama(&mut tools, Some(&json!({"type": "function"}))),
            Err(PromptError::UnsupportedNamedToolChoice)
        );
        assert_eq!(
            normalize_tool_choice_for_llama(&mut tools, Some(&json!(3))),
            Err(PromptError::UnsupportedToolChoicePayload("3".to_string()))
        );
        assert_eq!(
            normalize_tool_choice_for_llama(
                &mut tools,
                Some(&json!({"type": "function", "function": {"name": "b"}}))
            ),
            Ok(Some("required".to_string()))
        );
        assert_eq!(tools, vec![json!({"function": {"name": "b"}})]);
        assert_eq!(
            normalize_tool_choice_for_llama(
                &mut tools,
                Some(&json!({"function": {"name": "zzz"}}))
            ),
            Err(PromptError::ToolNotFound("zzz".to_string()))
        );
    }

    #[test]
    fn bos_follows_tokenizer_only_for_raw_completions() {
        assert_eq!(
            add_bos_label(prompt_add_bos(PromptMode::TemplatedChat, Some(true))),
            "never"
        );
        assert_eq!(
            add_bos_label(prompt_add_bos(PromptMode::OpenAICompatChat, None)),
            "never"
        );
        assert_eq!(
            add_bos_label(prompt_add_bos(PromptMode::RawCompletion, Some(false))),
            "never"
        );
        assert_eq!(
            add_bos_label(prompt_add_bos(PromptMode::RawCompletion, Some(true))),
            "always"
        );
        assert_eq!(
            add_bos_label(prompt_add_bos(PromptMode::RawCompletion, None)),
            "always"
        );
        assert_eq!(
            prompt_add_bos_reason(PromptMode::RawCompletion, None),
            "raw completion metadata missing or invalid; using compatibility fallback add_bos=always"
        );
    }

    #[test]
    fn error_messages_keep_legacy_wording() {
        assert_eq!(
            PromptError::ToolsNeedTemplate(Box::new(PromptError::NoTemplate)).to_string(),
            "llama.cpp tool calling requires a resolved native chat template: No llama chat template resolved. Provide an explicit override, use a GGUF with an embedded template, or select a known preset."
        );
        assert_eq!(
            PromptError::OpenAICompatTemplate {
                error: "boom".to_string(),
                diagnostics: " (diag)".to_string()
            }
            .to_string(),
            "Failed to apply llama.cpp OpenAI-compatible chat template: boom (diag)"
        );
    }

    #[test]
    #[ignore = "needs a local GGUF model in LETTUCE_PLAN_MODEL"]
    fn builds_prompts_from_the_embedded_template() {
        let Ok(path) = std::env::var("LETTUCE_PLAN_MODEL") else {
            return;
        };
        let engine = crate::engine::LlamaEngine::new();
        let loaded = engine
            .load(
                None,
                &crate::engine::EngineLoadRequest {
                    request_id: None,
                    model_path: &path,
                    requested_gpu_layers: Some(0),
                    auto_gpu_layer_candidates: None,
                    native_fit_plan: None,
                    gpu_config: crate::engine::LlamaGpuConfig::default(),
                    strict_mode: false,
                    mmproj_path: None,
                    load_bundled_mtp: false,
                    mtp_model_path: None,
                    mtp_drafter_on_gpu: false,
                    mtp_gpu_fallback_allowed: false,
                    mtp_gpu_device_id: None,
                },
                || {},
            )
            .expect("load");
        let options = OpenAICompatPromptOptions::default();
        let messages = vec![
            json!({"role": "system", "content": "Be brief."}),
            json!({"role": "user", "content": "Hello"}),
        ];
        let plain = build_prompt(
            &loaded.model,
            &PromptRequest {
                messages: &messages,
                chat_template_override: None,
                chat_template_preset: None,
                allow_raw_completion_fallback: false,
                tools: None,
                tool_choice: None,
                options: &options,
            },
        )
        .expect("plain prompt");
        assert_eq!(plain.prompt_mode, PromptMode::TemplatedChat);
        assert_eq!(
            plain.applied_template_source.as_deref(),
            Some("embedded gguf")
        );
        assert!(plain.prompt.contains("Hello"));

        let tools = json!([{"type": "function", "function": {"name": "lookup", "description": "Look up", "parameters": {"type": "object", "properties": {"q": {"type": "string"}}}}}]);
        let with_tools = build_prompt(
            &loaded.model,
            &PromptRequest {
                messages: &messages,
                chat_template_override: None,
                chat_template_preset: None,
                allow_raw_completion_fallback: false,
                tools: Some(&tools),
                tool_choice: None,
                options: &options,
            },
        )
        .expect("tool prompt");
        assert_eq!(with_tools.prompt_mode, PromptMode::OpenAICompatChat);
        assert_eq!(with_tools.resolved_tool_choice.as_deref(), Some("auto"));
        assert!(with_tools.prompt.contains("lookup"));
        assert!(with_tools.native_tool_parse_supported);
        engine.unload().expect("unload");
    }
}
