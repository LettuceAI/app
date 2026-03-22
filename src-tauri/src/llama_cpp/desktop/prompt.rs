use super::*;

pub(super) struct ResolvedChatTemplate {
    pub(super) template: LlamaChatTemplate,
    pub(super) source_label: String,
    pub(super) template_text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PromptMode {
    TemplatedChat,
    RawCompletion,
}

pub(super) struct BuiltPrompt {
    pub(super) prompt: String,
    pub(super) attempted_template_source: Option<String>,
    pub(super) attempted_template_text: Option<String>,
    pub(super) applied_template_source: Option<String>,
    pub(super) applied_template_text: Option<String>,
    pub(super) used_raw_completion_fallback: bool,
    pub(super) raw_completion_fallback_reason: Option<String>,
    pub(super) prompt_mode: PromptMode,
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
    match model.token_to_piece_bytes(token, 8, false, None) {
        Ok(bytes) => Ok(bytes),
        Err(TokenToStringError::InsufficientBufferSpace(needed)) => {
            let required = usize::try_from(-needed).map_err(|_| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Invalid llama token buffer size hint: {needed}"),
                )
            })?;
            model
                .token_to_piece_bytes(token, required, false, None)
                .map_err(|e| {
                    crate::utils::err_msg(
                        module_path!(),
                        line!(),
                        format!("Failed to decode token bytes: {e}"),
                    )
                })
        }
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
) -> Result<BuiltPrompt, String> {
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
                if allow_raw_completion_fallback {
                    return Ok(BuiltPrompt {
                        prompt: build_fallback_prompt(messages),
                        attempted_template_source: None,
                        attempted_template_text: None,
                        applied_template_source: None,
                        applied_template_text: None,
                        used_raw_completion_fallback: true,
                        raw_completion_fallback_reason: Some(format!(
                            "template resolution failed: {}",
                            err
                        )),
                        prompt_mode: PromptMode::RawCompletion,
                    });
                }
                return Err(err);
            }
        };

    match model.apply_chat_template(&resolved_template.template, &chat_messages, true) {
        Ok(prompt) => Ok(BuiltPrompt {
            prompt,
            attempted_template_source: Some(resolved_template.source_label.clone()),
            attempted_template_text: Some(resolved_template.template_text.clone()),
            applied_template_source: Some(resolved_template.source_label),
            applied_template_text: Some(resolved_template.template_text),
            used_raw_completion_fallback: false,
            raw_completion_fallback_reason: None,
            prompt_mode: PromptMode::TemplatedChat,
        }),
        Err(err) => {
            if allow_raw_completion_fallback {
                Ok(BuiltPrompt {
                    prompt: build_fallback_prompt(messages),
                    attempted_template_source: Some(resolved_template.source_label.clone()),
                    attempted_template_text: Some(resolved_template.template_text.clone()),
                    applied_template_source: None,
                    applied_template_text: None,
                    used_raw_completion_fallback: true,
                    raw_completion_fallback_reason: Some(format!(
                        "template application failed: {}",
                        err
                    )),
                    prompt_mode: PromptMode::RawCompletion,
                })
            } else {
                Err(crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!(
                        "Failed to apply llama chat template from {}: {}",
                        resolved_template.source_label, err
                    ),
                ))
            }
        }
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
        PromptMode::TemplatedChat => AddBos::Never,
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
        PromptMode::TemplatedChat => {
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
