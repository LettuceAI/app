use super::*;

pub(super) struct ResolvedChatTemplate {
    pub(super) template: LlamaChatTemplate,
    pub(super) source_label: String,
    pub(super) template_text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PromptMode {
    TemplatedChat,
    OpenAICompatChat,
    RawCompletion,
}

pub(super) struct BuiltPrompt {
    pub(super) prompt: String,
    pub(super) attempted_template_source: Option<String>,
    pub(super) attempted_template_text: Option<String>,
    pub(super) applied_template_source: Option<String>,
    pub(super) applied_template_text: Option<String>,
    pub(super) resolved_tool_choice: Option<String>,
    pub(super) used_raw_completion_fallback: bool,
    pub(super) raw_completion_fallback_reason: Option<String>,
    pub(super) prompt_mode: PromptMode,
    pub(super) chat_template_result: Option<llama_cpp_2::model::ChatTemplateResult>,
    pub(super) additional_stop_sequences: Vec<String>,
    pub(super) tool_template_diagnostics: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct OpenAICompatPromptOptions {
    pub(super) reasoning_format: Option<String>,
    pub(super) parallel_tool_calls: bool,
    pub(super) enable_thinking: bool,
}

fn normalize_role(role: &str) -> &'static str {
    match role {
        "system" | "developer" => "system",
        "assistant" => "assistant",
        "user" => "user",
        _ => "user",
    }
}

fn sanitize_text(value: &str) -> String {
    value.replace('\0', "")
}

pub(super) fn token_piece_bytes(
    model: &LlamaModel,
    token: llama_cpp_2::token::LlamaToken,
) -> Result<Vec<u8>, String> {
    fn decode_with_special_mode(
        model: &LlamaModel,
        token: llama_cpp_2::token::LlamaToken,
        special: bool,
    ) -> Result<Vec<u8>, TokenToStringError> {
        match model.token_to_piece_bytes(token, 8, special, None) {
            Ok(bytes) => Ok(bytes),
            Err(TokenToStringError::InsufficientBufferSpace(needed)) => {
                let required =
                    usize::try_from(-needed).map_err(|_| TokenToStringError::InsufficientBufferSpace(needed))?;
                model.token_to_piece_bytes(token, required, special, None)
            }
            Err(e) => Err(e),
        }
    }

    match decode_with_special_mode(model, token, false) {
        Ok(bytes) => Ok(bytes),
        Err(TokenToStringError::UnknownTokenType) => decode_with_special_mode(model, token, true)
            .map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to decode token bytes: {e}"),
                )
            }),
        Err(e) => Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to decode token bytes: {e}"),
        )),
    }
}

fn extract_text_content(message: &Value) -> String {
    let content = message.get("content");
    match content {
        Some(Value::String(text)) => sanitize_text(text),
        Some(Value::Array(parts)) => {
            let mut out: Vec<String> = Vec::new();
            for part in parts {
                let part_type = part.get("type").and_then(|v| v.as_str());
                if part_type == Some("text") {
                    if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
                        let cleaned = sanitize_text(text);
                        if !cleaned.is_empty() {
                            out.push(cleaned);
                        }
                    }
                }
            }
            out.join("\n")
        }
        _ => String::new(),
    }
}

fn build_fallback_prompt(messages: &[Value]) -> String {
    let mut prompt = String::new();
    for message in messages {
        let role = message
            .get("role")
            .and_then(|v| v.as_str())
            .map(normalize_role)
            .unwrap_or("user");
        let content = extract_text_content(message);
        if content.is_empty() {
            continue;
        }
        prompt.push_str(role);
        prompt.push_str(": ");
        prompt.push_str(&content);
        prompt.push('\n');
    }
    prompt.push_str("assistant: ");
    prompt
}

pub(super) fn inject_media_markers(messages: &[Value]) -> Vec<Value> {
    messages
        .iter()
        .map(|message| {
            let Some(parts) = message.get("content").and_then(|v| v.as_array()) else {
                return message.clone();
            };

            let mut text_parts = Vec::new();
            for part in parts {
                match part.get("type").and_then(|v| v.as_str()) {
                    Some("text") => {
                        if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
                            let cleaned = sanitize_text(text);
                            if !cleaned.is_empty() {
                                text_parts.push(cleaned);
                            }
                        }
                    }
                    Some("image_url") => {
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
    if message
        .get("role")
        .and_then(|v| v.as_str())
        .map(|role| role == "tool")
        .unwrap_or(false)
    {
        return true;
    }

    if message.get("tool_calls").is_some() || message.get("tool_call_id").is_some() {
        return true;
    }

    false
}

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
        format!(
            "tool-template heuristic found no known markers; template_preview=\"{}\"",
            preview
        )
    } else {
        format!(
            "tool-template heuristic matched markers=[{}] template_preview=\"{}\"",
            matched.join(", "),
            preview
        )
    }
}

fn oaicompat_result_supports_native_tool_calls(
    result: &llama_cpp_2::model::ChatTemplateResult,
) -> bool {
    result.parse_tool_calls || result.parser.is_some() || result.grammar.is_some()
}

fn normalize_tool_choice_for_llama(
    tools: &mut Vec<Value>,
    tool_choice: Option<&Value>,
) -> Result<Option<String>, String> {
    match tool_choice {
        None => Ok(Some("auto".to_string())),
        Some(Value::String(choice)) => match choice.as_str() {
            "auto" | "none" | "required" => Ok(Some(choice.clone())),
            other => Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Unsupported llama.cpp tool_choice '{}'", other),
            )),
        },
        Some(Value::Object(object)) => {
            let Some(name) = object
                .get("function")
                .and_then(|v| v.get("name"))
                .and_then(|v| v.as_str())
            else {
                return Err(crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    "Unsupported llama.cpp named tool choice payload",
                ));
            };

            tools.retain(|tool| {
                tool.get("function")
                    .and_then(|v| v.get("name"))
                    .and_then(|v| v.as_str())
                    == Some(name)
            });

            if tools.is_empty() {
                return Err(crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!(
                        "Requested llama.cpp tool '{}' was not found in the tools array",
                        name
                    ),
                ));
            }

            Ok(Some("required".to_string()))
        }
        Some(other) => Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Unsupported llama.cpp tool_choice payload: {}", other),
        )),
    }
}

fn build_oaicompat_prompt(
    model: &LlamaModel,
    messages: &[Value],
    resolved_template: &ResolvedChatTemplate,
    tools: Option<&Value>,
    tool_choice: Option<&Value>,
    options: &OpenAICompatPromptOptions,
) -> Result<BuiltPrompt, String> {
    let tool_template_diagnostics = (!template_appears_tool_aware(&resolved_template.template_text))
        .then(|| summarize_tool_template_detection(&resolved_template.template_text));

    let messages_json = serde_json::to_string(messages).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to serialize llama.cpp messages for tool calling: {e}"),
        )
    })?;

    let mut tools_vec = tools
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    let has_tools = !tools_vec.is_empty();
    let normalized_tool_choice = if has_tools {
        normalize_tool_choice_for_llama(&mut tools_vec, tool_choice)?
    } else {
        Some("none".to_string())
    };
    let parse_tool_calls = has_tools && normalized_tool_choice.as_deref() != Some("none");
    let tools_json = if has_tools {
        Some(serde_json::to_string(&tools_vec).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to serialize llama.cpp tools for tool calling: {e}"),
            )
        })?)
    } else {
        None
    };

    let params = llama_cpp_2::openai::OpenAIChatTemplateParams {
        messages_json: &messages_json,
        tools_json: tools_json.as_deref(),
        tool_choice: normalized_tool_choice.as_deref(),
        json_schema: None,
        grammar: None,
        reasoning_format: options.reasoning_format.as_deref(),
        chat_template_kwargs: None,
        add_generation_prompt: true,
        use_jinja: true,
        parallel_tool_calls: has_tools && options.parallel_tool_calls,
        enable_thinking: options.enable_thinking,
        add_bos: false,
        add_eos: false,
        parse_tool_calls,
    };

    let chat_template_result = model
        .apply_chat_template_oaicompat(&resolved_template.template, &params)
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!(
                    "Failed to apply llama.cpp OpenAI-compatible chat template: {e}{}",
                    tool_template_diagnostics
                        .as_ref()
                        .map(|diag| format!(" ({diag})"))
                        .unwrap_or_default()
                ),
            )
        })?;

    if parse_tool_calls && !oaicompat_result_supports_native_tool_calls(&chat_template_result) {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!(
                "Resolved llama.cpp oaicompat template did not expose native tool-call parsing metadata (parse_tool_calls={}, parser_present={}, grammar_present={}){}",
                chat_template_result.parse_tool_calls,
                chat_template_result.parser.is_some(),
                chat_template_result.grammar.is_some(),
                tool_template_diagnostics
                    .as_ref()
                    .map(|diag| format!(" ({diag})"))
                    .unwrap_or_default()
            ),
        ));
    }

    Ok(BuiltPrompt {
        prompt: chat_template_result.prompt.clone(),
        attempted_template_source: Some(resolved_template.source_label.clone()),
        attempted_template_text: Some(resolved_template.template_text.clone()),
        applied_template_source: Some(resolved_template.source_label.clone()),
        applied_template_text: Some(resolved_template.template_text.clone()),
        resolved_tool_choice: normalized_tool_choice,
        used_raw_completion_fallback: false,
        raw_completion_fallback_reason: None,
        prompt_mode: PromptMode::OpenAICompatChat,
        additional_stop_sequences: chat_template_result.additional_stops.clone(),
        chat_template_result: Some(chat_template_result),
        tool_template_diagnostics,
    })
}

fn build_plain_templated_prompt(
    model: &LlamaModel,
    messages: &[Value],
    chat_messages: &[LlamaChatMessage],
    resolved_template: &ResolvedChatTemplate,
    options: &OpenAICompatPromptOptions,
) -> Result<BuiltPrompt, String> {
    let messages_json = serde_json::to_string(messages).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to serialize llama.cpp messages for chat templating: {e}"),
        )
    })?;

    let params = llama_cpp_2::openai::OpenAIChatTemplateParams {
        messages_json: &messages_json,
        tools_json: None,
        tool_choice: None,
        json_schema: None,
        grammar: None,
        reasoning_format: options.reasoning_format.as_deref(),
        chat_template_kwargs: None,
        add_generation_prompt: true,
        use_jinja: true,
        parallel_tool_calls: false,
        enable_thinking: options.enable_thinking,
        add_bos: false,
        add_eos: false,
        parse_tool_calls: false,
    };

    let chat_template_result = match model.apply_chat_template_oaicompat(&resolved_template.template, &params) {
        Ok(result) => result,
        Err(oaicompat_err) => {
            match model.apply_chat_template(&resolved_template.template, chat_messages, true) {
                Ok(prompt) => {
                    return Ok(BuiltPrompt {
                        prompt,
                        attempted_template_source: Some(resolved_template.source_label.clone()),
                        attempted_template_text: Some(resolved_template.template_text.clone()),
                        applied_template_source: Some(resolved_template.source_label.clone()),
                        applied_template_text: Some(resolved_template.template_text.clone()),
                        resolved_tool_choice: None,
                        used_raw_completion_fallback: false,
                        raw_completion_fallback_reason: None,
                        prompt_mode: PromptMode::TemplatedChat,
                        chat_template_result: None,
                        additional_stop_sequences: Vec::new(),
                        tool_template_diagnostics: None,
                    });
                }
                Err(legacy_err) => {
                    return Err(crate::utils::err_msg(
                        module_path!(),
                        line!(),
                        format!(
                            "Failed to apply llama chat template from {} via oaicompat ({}) and legacy ({})",
                            resolved_template.source_label, oaicompat_err, legacy_err
                        ),
                    ));
                }
            }
        }
    };

    Ok(BuiltPrompt {
        prompt: chat_template_result.prompt.clone(),
        attempted_template_source: Some(resolved_template.source_label.clone()),
        attempted_template_text: Some(resolved_template.template_text.clone()),
        applied_template_source: Some(resolved_template.source_label.clone()),
        applied_template_text: Some(resolved_template.template_text.clone()),
        resolved_tool_choice: None,
        used_raw_completion_fallback: false,
        raw_completion_fallback_reason: None,
        prompt_mode: PromptMode::TemplatedChat,
        chat_template_result: None,
        additional_stop_sequences: chat_template_result.additional_stops.clone(),
        tool_template_diagnostics: None,
    })
}

fn chat_template_text(template: &LlamaChatTemplate) -> String {
    template.as_c_str().to_string_lossy().into_owned()
}

fn resolve_chat_template(
    model: &LlamaModel,
    chat_template_override: Option<&str>,
    chat_template_preset: Option<&str>,
) -> Result<ResolvedChatTemplate, String> {
    if let Some(template_override) = chat_template_override.filter(|v| !v.trim().is_empty()) {
        let template = LlamaChatTemplate::new(template_override).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Invalid explicit llama chat template override: {e}"),
            )
        })?;
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

    if let Some(template_preset) = chat_template_preset.filter(|v| !v.trim().is_empty()) {
        let template = LlamaChatTemplate::new(template_preset).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!(
                    "Invalid llama chat template preset '{}': {e}",
                    template_preset
                ),
            )
        })?;
        return Ok(ResolvedChatTemplate {
            template_text: template_preset.to_string(),
            template,
            source_label: format!("preset '{}'", template_preset),
        });
    }

    Err(crate::utils::err_msg(
        module_path!(),
        line!(),
        "No llama chat template resolved. Provide an explicit override, use a GGUF with an embedded template, or select a known preset.",
    ))
}

pub(super) fn build_prompt(
    model: &LlamaModel,
    messages: &[Value],
    chat_template_override: Option<&str>,
    chat_template_preset: Option<&str>,
    allow_raw_completion_fallback: bool,
    tools: Option<&Value>,
    tool_choice: Option<&Value>,
    options: &OpenAICompatPromptOptions,
) -> Result<BuiltPrompt, String> {
    let needs_openai_compat = tools
        .and_then(|value| value.as_array())
        .map(|items| !items.is_empty())
        .unwrap_or(false)
        || messages.iter().any(message_requires_openai_compat);

    let mut chat_messages = Vec::new();
    for message in messages {
        let role = message
            .get("role")
            .and_then(|v| v.as_str())
            .map(normalize_role)
            .unwrap_or("user");
        let content = extract_text_content(message);
        if content.is_empty() {
            continue;
        }
        let chat_message = LlamaChatMessage::new(role.to_string(), content).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Invalid chat message: {e}"),
            )
        })?;
        chat_messages.push(chat_message);
    }

    if chat_messages.is_empty() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "No usable chat messages for llama.cpp",
        ));
    }

    let resolved_template =
        match resolve_chat_template(model, chat_template_override, chat_template_preset) {
            Ok(resolved) => resolved,
            Err(err) => {
                if needs_openai_compat {
                    return Err(crate::utils::err_msg(
                        module_path!(),
                        line!(),
                        format!(
                            "llama.cpp tool calling requires a resolved native chat template: {}",
                            err
                        ),
                    ));
                }
                if allow_raw_completion_fallback {
                    return Ok(BuiltPrompt {
                        prompt: build_fallback_prompt(messages),
                        attempted_template_source: None,
                        attempted_template_text: None,
                        applied_template_source: None,
                        applied_template_text: None,
                        resolved_tool_choice: None,
                        used_raw_completion_fallback: true,
                        raw_completion_fallback_reason: Some(format!(
                            "template resolution failed: {}",
                            err
                        )),
                        prompt_mode: PromptMode::RawCompletion,
                        chat_template_result: None,
                        additional_stop_sequences: Vec::new(),
                        tool_template_diagnostics: None,
                    });
                }
                return Err(err);
            }
        };

    if needs_openai_compat {
        return build_oaicompat_prompt(
            model,
            messages,
            &resolved_template,
            tools,
            tool_choice,
            options,
        );
    }

    match build_plain_templated_prompt(
        model,
        messages,
        &chat_messages,
        &resolved_template,
        options,
    ) {
        Ok(built_prompt) => Ok(built_prompt),
        Err(err) => {
            if allow_raw_completion_fallback {
                Ok(BuiltPrompt {
                    prompt: build_fallback_prompt(messages),
                    attempted_template_source: Some(resolved_template.source_label.clone()),
                    attempted_template_text: Some(resolved_template.template_text.clone()),
                    applied_template_source: None,
                    applied_template_text: None,
                    resolved_tool_choice: None,
                    used_raw_completion_fallback: true,
                    raw_completion_fallback_reason: Some(format!(
                        "template application failed: {}",
                        err
                    )),
                    prompt_mode: PromptMode::RawCompletion,
                    chat_template_result: None,
                    additional_stop_sequences: Vec::new(),
                    tool_template_diagnostics: None,
                })
            } else {
                Err(err)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::template_appears_tool_aware;

    #[test]
    fn detects_gemma_style_tool_markers() {
        assert!(template_appears_tool_aware(
            "{% if tools %}<|tool_call>{{ tools }}<tool_call|>{% endif %}"
        ));
    }
}

pub(super) fn model_tokenizer_adds_bos(model: &LlamaModel) -> Option<bool> {
    let raw_value = model
        .meta_val_str(super::super::TOKENIZER_ADD_BOS_METADATA_KEY)
        .ok()?;
    match raw_value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" => Some(true),
        "false" | "0" => Some(false),
        _ => None,
    }
}

pub(super) fn resolve_prompt_add_bos(model: &LlamaModel, prompt_mode: PromptMode) -> AddBos {
    match prompt_mode {
        PromptMode::TemplatedChat | PromptMode::OpenAICompatChat => AddBos::Never,
        PromptMode::RawCompletion => match model_tokenizer_adds_bos(model) {
            Some(true) => AddBos::Always,
            Some(false) => AddBos::Never,
            None => AddBos::Always,
        },
    }
}

pub(super) fn prompt_mode_label(prompt_mode: PromptMode) -> &'static str {
    match prompt_mode {
        PromptMode::TemplatedChat => "templated_chat",
        PromptMode::OpenAICompatChat => "oaicompat_chat",
        PromptMode::RawCompletion => "raw_completion",
    }
}

pub(super) fn add_bos_label(add_bos: AddBos) -> &'static str {
    match add_bos {
        AddBos::Always => "always",
        AddBos::Never => "never",
    }
}

pub(super) fn model_tokenizer_add_bos_label(
    model_tokenizer_adds_bos: Option<bool>,
) -> &'static str {
    match model_tokenizer_adds_bos {
        Some(true) => "true",
        Some(false) => "false",
        None => "unknown",
    }
}

pub(super) fn prompt_add_bos_reason(
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
