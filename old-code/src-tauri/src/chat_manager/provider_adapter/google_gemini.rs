use std::collections::HashMap;

use serde::Serialize;
use serde_json::{json, Map, Value};

use super::{
    extract_image_data_urls, extract_input_audio, extract_text_content, parse_data_url,
    visible_chat_system_instruction_text, ProviderAdapter,
};
use crate::chat_manager::tooling::{gemini_tool_config, gemini_tools, ToolConfig};

pub struct GoogleGeminiAdapter;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeminiThinkingMode {
    Level,
    Budget,
    Unknown,
}

pub fn gemini_thinking_mode(model_name: &str) -> GeminiThinkingMode {
    let normalized = model_name.trim().to_ascii_lowercase();

    if normalized.contains("gemini-2.5") || normalized.contains("robotics-er-1.5") {
        GeminiThinkingMode::Budget
    } else if normalized.contains("gemini-3") {
        GeminiThinkingMode::Level
    } else {
        GeminiThinkingMode::Unknown
    }
}

fn gemini_api_base(base_url: &str) -> String {
    let base = base_url.trim_end_matches('/');
    if let Some(prefix) = base.strip_suffix("/v1") {
        format!("{prefix}/v1beta")
    } else {
        base.to_string()
    }
}

#[derive(Serialize)]
struct GeminiThinkingConfig {
    #[serde(rename = "includeThoughts")]
    include_thoughts: bool,
    #[serde(rename = "thinkingBudget", skip_serializing_if = "Option::is_none")]
    thinking_budget: Option<i32>,
    #[serde(rename = "thinkingLevel", skip_serializing_if = "Option::is_none")]
    thinking_level: Option<String>,
}

#[derive(Serialize)]
struct GeminiGenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(rename = "topP", skip_serializing_if = "Option::is_none")]
    top_p: Option<f64>,
    #[serde(rename = "maxOutputTokens")]
    max_output_tokens: u32,
    #[serde(rename = "topK", skip_serializing_if = "Option::is_none")]
    top_k: Option<u32>,
    #[serde(rename = "thinkingConfig", skip_serializing_if = "Option::is_none")]
    thinking_config: Option<GeminiThinkingConfig>,
}

impl ProviderAdapter for GoogleGeminiAdapter {
    fn endpoint(&self, base_url: &str) -> String {
        base_url.trim_end_matches('/').to_string()
    }

    fn build_url(
        &self,
        base_url: &str,
        model_name: &str,
        api_key: &str,
        should_stream: bool,
    ) -> String {
        let base = gemini_api_base(base_url);
        if should_stream {
            format!(
                "{}/models/{}:streamGenerateContent?alt=sse&key={}",
                base, model_name, api_key
            )
        } else {
            format!(
                "{}/models/{}:generateContent?key={}",
                base, model_name, api_key
            )
        }
    }

    fn system_role(&self) -> std::borrow::Cow<'static, str> {
        "system".into()
    }

    fn supports_stream(&self) -> bool {
        true
    }

    fn requires_api_key(&self) -> bool {
        true
    }

    fn required_auth_headers(&self) -> &'static [&'static str] {
        &[]
    }

    fn default_headers_template(&self) -> HashMap<String, String> {
        let mut out = HashMap::new();
        out.insert("Content-Type".into(), "application/json".into());
        out
    }

    fn headers(
        &self,
        api_key: &str,
        extra: Option<&HashMap<String, String>>,
    ) -> HashMap<String, String> {
        let mut out: HashMap<String, String> = HashMap::new();
        out.insert("Content-Type".into(), "application/json".into());
        out.insert("x-goog-api-key".into(), api_key.to_string());
        out.entry("User-Agent".into())
            .or_insert_with(|| "LettuceAI/0.1".into());
        if let Some(extra) = extra {
            for (k, v) in extra.iter() {
                out.insert(k.clone(), v.clone());
            }
        }
        out
    }

    fn body(
        &self,
        _model_name: &str,
        messages_for_api: &Vec<Value>,
        system_prompt: Option<String>,
        temperature: Option<f64>,
        top_p: Option<f64>,
        max_tokens: u32,
        _context_length: Option<u32>,
        _should_stream: bool,
        _frequency_penalty: Option<f64>,
        _presence_penalty: Option<f64>,
        top_k: Option<u32>,
        tool_config: Option<&ToolConfig>,
        reasoning_enabled: bool,
        _reasoning_effort: Option<String>,
        reasoning_budget: Option<u32>,
    ) -> Value {
        let mut contents: Vec<Value> = Vec::new();
        let mut tool_call_name_by_id: HashMap<String, String> = HashMap::new();
        let mut system_instruction_chunks: Vec<String> = Vec::new();

        for msg in messages_for_api {
            if let Some(raw_content) = msg.get("gemini_content") {
                if let Some(parts) = raw_content.get("parts").and_then(|v| v.as_array()) {
                    if !parts.is_empty() {
                        for part in parts {
                            let Some(function_call) = part
                                .get("functionCall")
                                .or_else(|| part.get("function_call"))
                            else {
                                continue;
                            };
                            let (Some(id), Some(name)) = (
                                function_call.get("id").and_then(|v| v.as_str()),
                                function_call.get("name").and_then(|v| v.as_str()),
                            ) else {
                                continue;
                            };
                            tool_call_name_by_id.insert(id.to_string(), name.to_string());
                        }
                        contents.push(json!({
                            "role": raw_content
                                .get("role")
                                .and_then(|v| v.as_str())
                                .unwrap_or("model"),
                            "parts": parts
                        }));
                        continue;
                    }
                }
            }

            let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("user");
            if role == "system" || role == "developer" {
                if let Some(visible_instruction) = visible_chat_system_instruction_text(msg) {
                    contents.push(json!({
                        "role": "user",
                        "parts": [{ "text": visible_instruction }]
                    }));
                    continue;
                }
                if let Some(content) = extract_text_content(msg.get("content")) {
                    let trimmed = content.trim();
                    if !trimmed.is_empty() {
                        system_instruction_chunks.push(trimmed.to_string());
                    }
                }
                continue;
            }

            if role == "assistant" {
                if let Some(tool_calls) = msg.get("tool_calls").and_then(|v| v.as_array()) {
                    let mut parts: Vec<Value> = Vec::new();

                    if let Some(content) = extract_text_content(msg.get("content")) {
                        if !content.trim().is_empty() {
                            parts.push(json!({ "text": content }));
                        }
                    }

                    for tool_call in tool_calls {
                        let id = tool_call
                            .get("id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let function = tool_call.get("function").unwrap_or(tool_call);
                        let name = function
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("tool_call")
                            .to_string();
                        let args = function
                            .get("arguments")
                            .map(parse_jsonish_value)
                            .unwrap_or_else(|| Value::Object(Map::new()));

                        if !id.is_empty() {
                            tool_call_name_by_id.insert(id.clone(), name.clone());
                        }

                        let mut function_call = json!({
                            "name": name,
                            "args": args
                        });
                        if !id.is_empty() {
                            function_call["id"] = Value::String(id);
                        }

                        let mut part = json!({ "functionCall": function_call });
                        // echo thought signature, else thinking models reject the replay
                        if let Some(sig) = tool_call
                            .get("thoughtSignature")
                            .or_else(|| tool_call.get("thought_signature"))
                            .and_then(|v| v.as_str())
                            .filter(|s| !s.is_empty())
                        {
                            part["thoughtSignature"] = Value::String(sig.to_string());
                        }
                        parts.push(part);
                    }

                    if !parts.is_empty() {
                        contents.push(json!({
                            "role": "model",
                            "parts": parts
                        }));
                    }
                    continue;
                }
            }

            if role == "tool" {
                let tool_call_id = msg
                    .get("tool_call_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let tool_name = tool_call_name_by_id
                    .get(tool_call_id)
                    .cloned()
                    .unwrap_or_else(|| "tool_call".to_string());

                let content_value = msg
                    .get("content")
                    .map(parse_jsonish_value)
                    .unwrap_or_else(|| Value::Object(Map::new()));
                let response = match content_value {
                    Value::Object(_) => content_value,
                    other => json!({ "result": other }),
                };

                let mut function_response = json!({
                    "name": tool_name,
                    "response": response
                });
                if !tool_call_id.is_empty() {
                    function_response["id"] = Value::String(tool_call_id.to_string());
                }

                contents.push(json!({
                    "role": "user",
                    "parts": [{
                        "functionResponse": function_response
                    }]
                }));
                continue;
            }

            let text = extract_text_content(msg.get("content")).unwrap_or_default();
            let image_urls = extract_image_data_urls(msg.get("content"));
            let audio_inputs = extract_input_audio(msg.get("content"));
            if text.trim().is_empty() && image_urls.is_empty() && audio_inputs.is_empty() {
                continue;
            }

            let gem_role = match role {
                "assistant" | "model" => "model",
                _ => "user",
            };

            let mut parts: Vec<Value> = Vec::new();
            if !text.trim().is_empty() {
                parts.push(json!({ "text": text }));
            }

            for image_url in image_urls {
                if let Some((mime_type, data)) = parse_data_url(&image_url) {
                    parts.push(json!({
                        "inline_data": {
                            "mime_type": mime_type,
                            "data": data,
                        }
                    }));
                }
            }

            for (format, data) in audio_inputs {
                parts.push(json!({
                    "inline_data": {
                        "mime_type": gemini_audio_mime(&format),
                        "data": data,
                    }
                }));
            }

            if parts.is_empty() {
                continue;
            }

            contents.push(json!({
                "role": gem_role,
                "parts": parts
            }));
        }

        let thinking_config = if reasoning_enabled {
            let thinking_mode = gemini_thinking_mode(_model_name);
            let thinking_level = match thinking_mode {
                GeminiThinkingMode::Level => _reasoning_effort.as_ref().map(|s| s.to_uppercase()),
                GeminiThinkingMode::Budget | GeminiThinkingMode::Unknown => None,
            };
            let thinking_budget = match thinking_mode {
                GeminiThinkingMode::Budget => {
                    Some(reasoning_budget.map(|b| b as i32).unwrap_or(-1))
                }
                GeminiThinkingMode::Level | GeminiThinkingMode::Unknown => None,
            };

            Some(GeminiThinkingConfig {
                include_thoughts: true,
                thinking_budget,
                thinking_level,
            })
        } else {
            None
        };

        let generation_config = GeminiGenerationConfig {
            temperature,
            top_p,
            max_output_tokens: max_tokens,
            top_k,
            thinking_config,
        };

        let tools = tool_config.and_then(gemini_tools);
        let gemini_tool_config = if tools.is_some() {
            tool_config.and_then(|cfg| gemini_tool_config(cfg.choice.as_ref()))
        } else {
            None
        };

        let mut body = json!({
            "contents": contents,
            "generationConfig": serde_json::to_value(generation_config).unwrap_or_else(|_| json!({}))
        });

        if let Some(system) = system_prompt
            .into_iter()
            .chain(system_instruction_chunks)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .reduce(|mut combined, chunk| {
                combined.push_str("\n\n");
                combined.push_str(&chunk);
                combined
            })
        {
            body["systemInstruction"] = json!({
                "parts": [{ "text": system }]
            });
        }
        if let Some(tools) = tools {
            body["tools"] = Value::Array(tools);
        }
        if let Some(cfg) = gemini_tool_config {
            body["toolConfig"] = cfg;
        }

        if let Some(gen_config) = body.get("generationConfig") {
            crate::utils::log_debug_global(
                "gemini_request",
                format!("Gemini generationConfig: {:?}", gen_config),
            );
        }

        body
    }

    fn list_models_endpoint(&self, base_url: &str) -> String {
        let base = gemini_api_base(base_url);
        format!("{}/models", base)
    }

    fn parse_models_list(
        &self,
        response: Value,
    ) -> Vec<crate::chat_manager::provider_adapter::ModelInfo> {
        let mut models = Vec::new();
        if let Some(list) = response.get("models").and_then(|d| d.as_array()) {
            for item in list {
                if let Some(name) = item.get("name").and_then(|n| n.as_str()) {
                    let id = name.strip_prefix("models/").unwrap_or(name).to_string();
                    models.push(crate::chat_manager::provider_adapter::ModelInfo {
                        id,
                        display_name: item
                            .get("displayName")
                            .and_then(|n| n.as_str())
                            .map(|s| s.to_string()),
                        description: item
                            .get("description")
                            .and_then(|n| n.as_str())
                            .map(|s| s.to_string()),
                        context_length: item.get("inputTokenLimit").and_then(|c| c.as_u64()),
                        input_modalities: None,
                        output_modalities: None,
                        supported_endpoints: None,
                        input_price: None,
                        output_price: None,
                    });
                }
            }
        }
        models
    }
}

fn gemini_audio_mime(format: &str) -> &'static str {
    match format.to_ascii_lowercase().as_str() {
        "mp3" | "mpeg" => "audio/mp3",
        "ogg" => "audio/ogg",
        "flac" => "audio/flac",
        "aac" => "audio/aac",
        "aiff" | "aif" => "audio/aiff",
        "m4a" | "mp4" => "audio/aac",
        _ => "audio/wav",
    }
}

fn parse_jsonish_value(value: &Value) -> Value {
    match value {
        Value::String(raw) => {
            serde_json::from_str::<Value>(raw).unwrap_or_else(|_| Value::String(raw.clone()))
        }
        other => other.clone(),
    }
}
