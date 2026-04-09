use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Canonical tool definition used internally (OpenAI-style).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDefinition {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub parameters: Value,
}

/// High-level tool choice; mapped per-provider.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ToolChoice {
    Auto,
    None,
    Required,
    Any,
    Tool { name: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolConfig {
    pub tools: Vec<ToolDefinition>,
    #[serde(default)]
    pub choice: Option<ToolChoice>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub arguments: Value,
    #[serde(default)]
    pub raw_arguments: Option<String>,
}

fn has_tools(cfg: &ToolConfig) -> bool {
    !cfg.tools.is_empty()
}

fn arguments_value_from_str(raw: &str) -> (Value, Option<String>) {
    match serde_json::from_str::<Value>(raw) {
        Ok(val) => (val, Some(raw.to_string())),
        Err(_) => (Value::String(raw.to_string()), Some(raw.to_string())),
    }
}

/// Convert canonical tools into OpenAI-style `tools` array.
pub fn openai_tools(cfg: &ToolConfig) -> Option<Vec<Value>> {
    if !has_tools(cfg) {
        return None;
    }

    let tools: Vec<Value> = cfg
        .tools
        .iter()
        .map(|tool| {
            let mut function = json!({
                "name": tool.name,
                "parameters": tool.parameters
            });
            if let Some(desc) = &tool.description {
                if let Some(obj) = function.as_object_mut() {
                    obj.insert("description".to_string(), Value::String(desc.clone()));
                }
            }
            json!({
                "type": "function",
                "function": function
            })
        })
        .collect();

    if tools.is_empty() {
        None
    } else {
        Some(tools)
    }
}

/// Map canonical choice to OpenAI semantics.
pub fn openai_tool_choice(choice: Option<&ToolChoice>) -> Option<Value> {
    match choice {
        None => None,
        Some(ToolChoice::Auto) => Some(json!("auto")),
        Some(ToolChoice::None) => Some(json!("none")),
        Some(ToolChoice::Required) | Some(ToolChoice::Any) => Some(json!("required")),
        Some(ToolChoice::Tool { name }) => Some(json!({
            "type": "function",
            "function": { "name": name }
        })),
    }
}

/// Mistral-specific choice mapping (uses "any" instead of "required").
pub fn mistral_tool_choice(choice: Option<&ToolChoice>) -> Option<Value> {
    match choice {
        None => None,
        Some(ToolChoice::Auto) => Some(json!("auto")),
        Some(ToolChoice::None) => Some(json!("none")),
        Some(ToolChoice::Required) | Some(ToolChoice::Any) => Some(json!("any")),
        Some(ToolChoice::Tool { name }) => Some(json!({
            "type": "function",
            "function": { "name": name }
        })),
    }
}

/// Anthropic messages API uses `input_schema` and a structured tool_choice.
pub fn anthropic_tools(cfg: &ToolConfig) -> Option<Vec<Value>> {
    if !has_tools(cfg) {
        return None;
    }

    let tools: Vec<Value> = cfg
        .tools
        .iter()
        .map(|tool| {
            json!({
                "name": tool.name,
                "description": tool.description,
                "input_schema": tool.parameters
            })
        })
        .collect();

    if tools.is_empty() {
        None
    } else {
        Some(tools)
    }
}

pub fn anthropic_tool_choice(choice: Option<&ToolChoice>) -> Option<Value> {
    match choice {
        None => None,
        Some(ToolChoice::Auto) => Some(json!({ "type": "auto" })),
        Some(ToolChoice::None) => Some(json!({ "type": "none" })),
        Some(ToolChoice::Required) | Some(ToolChoice::Any) => Some(json!({ "type": "any" })),
        Some(ToolChoice::Tool { name }) => Some(json!({ "type": "tool", "name": name })),
    }
}

/// Gemini needs a wrapped `tools` list plus a `tool_config.function_calling_config`.
pub fn gemini_tools(cfg: &ToolConfig) -> Option<Vec<Value>> {
    if !has_tools(cfg) {
        return None;
    }

    let declarations: Vec<Value> = cfg
        .tools
        .iter()
        .map(|tool| {
            json!({
                "name": tool.name,
                "description": tool.description,
                "parameters": tool.parameters
            })
        })
        .collect();

    if declarations.is_empty() {
        None
    } else {
        Some(vec![json!({ "function_declarations": declarations })])
    }
}

pub fn gemini_tool_config(choice: Option<&ToolChoice>) -> Option<Value> {
    match choice {
        None | Some(ToolChoice::Auto) => Some(json!({
            "function_calling_config": { "mode": "AUTO" }
        })),
        Some(ToolChoice::None) => Some(json!({
            "function_calling_config": { "mode": "NONE" }
        })),
        Some(ToolChoice::Required) | Some(ToolChoice::Any) => Some(json!({
            "function_calling_config": { "mode": "ANY" }
        })),
        Some(ToolChoice::Tool { name }) => Some(json!({
            "function_calling_config": {
                "mode": "ANY",
                "allowed_function_names": [name]
            }
        })),
    }
}

/// Z.AI exposes OpenAI-style tools but only supports auto mode today.
pub fn zai_tool_choice(choice: Option<&ToolChoice>) -> Option<Value> {
    if choice.is_none() {
        Some(json!("auto"))
    } else {
        Some(json!("auto"))
    }
}

pub fn parse_tool_calls(provider_id: &str, payload: &Value) -> Vec<ToolCall> {
    let mut calls: Vec<ToolCall> = Vec::new();

    // 1) OpenAI-style responses
    if let Some(choices) = payload.get("choices").and_then(|v| v.as_array()) {
        for choice in choices {
            if let Some(message) = choice.get("message") {
                extract_openai_calls(message, &mut calls);
            }
            if let Some(delta) = choice.get("delta") {
                extract_openai_calls(delta, &mut calls);
            }
        }
    } else {
        extract_openai_calls(payload, &mut calls);
    }

    // 2) Anthropic tool_use blocks
    if provider_id.eq_ignore_ascii_case("anthropic") || calls.is_empty() {
        if let Some(content) = payload.get("content").and_then(|v| v.as_array()) {
            for part in content {
                if part
                    .get("type")
                    .and_then(|v| v.as_str())
                    .map(|t| t == "tool_use")
                    .unwrap_or(false)
                {
                    if let Some(name) = part.get("name").and_then(|v| v.as_str()) {
                        let id = part
                            .get("id")
                            .and_then(|v| v.as_str())
                            .unwrap_or_else(|| "tool_use");
                        let (arguments, raw_arguments) = match part.get("input") {
                            Some(Value::String(raw)) => arguments_value_from_str(raw),
                            Some(other) => (other.clone(), None),
                            None => (Value::Null, None),
                        };
                        calls.push(ToolCall {
                            id: id.to_string(),
                            name: name.to_string(),
                            arguments,
                            raw_arguments,
                        });
                    }
                }
            }
        }
    }

    // 3) Gemini function call parts
    if provider_id.starts_with("google") || provider_id.contains("gemini") || calls.is_empty() {
        if let Some(candidates) = payload.get("candidates").and_then(|v| v.as_array()) {
            for candidate in candidates {
                if let Some(content) = candidate.get("content").and_then(|v| v.as_object()) {
                    if let Some(parts) = content.get("parts").and_then(|v| v.as_array()) {
                        for part in parts {
                            if let Some(function_call) = part
                                .get("function_call")
                                .or_else(|| part.get("functionCall"))
                            {
                                if let Some(name) =
                                    function_call.get("name").and_then(|v| v.as_str())
                                {
                                    let args = function_call
                                        .get("args")
                                        .or_else(|| function_call.get("arguments"))
                                        .cloned()
                                        .unwrap_or_else(|| Value::Object(Default::default()));
                                    let call_id = function_call
                                        .get("id")
                                        .or_else(|| function_call.get("call_id"))
                                        .or_else(|| function_call.get("callId"))
                                        .and_then(|v| v.as_str())
                                        .map(|s| s.to_string())
                                        .unwrap_or_else(|| {
                                            format!("func_call_{}", calls.len() + 1)
                                        });
                                    calls.push(ToolCall {
                                        id: call_id,
                                        name: name.to_string(),
                                        arguments: args,
                                        raw_arguments: None,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    calls
}

fn extract_openai_legacy_function_call(node: &Value, out: &mut Vec<ToolCall>) {
    let Some(function_call) = node
        .get("function_call")
        .or_else(|| node.get("functionCall"))
    else {
        return;
    };

    let Some(name) = function_call.get("name").and_then(|v| v.as_str()) else {
        return;
    };

    let (arguments, raw_arguments) = match function_call.get("arguments") {
        Some(Value::String(raw)) => arguments_value_from_str(raw),
        Some(other) => (other.clone(), None),
        None => (Value::Null, None),
    };
    let id = function_call
        .get("id")
        .or_else(|| function_call.get("call_id"))
        .or_else(|| function_call.get("callId"))
        .and_then(|v| v.as_str())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("function_call_{}", out.len() + 1));

    out.push(ToolCall {
        id,
        name: name.to_string(),
        arguments,
        raw_arguments,
    });
}

fn extract_openai_calls(node: &Value, out: &mut Vec<ToolCall>) {
    if let Some(tool_calls) = node.get("tool_calls").and_then(|v| v.as_array()) {
        for raw_call in tool_calls {
            if let Some(function) = raw_call.get("function") {
                if let Some(name) = function.get("name").and_then(|v| v.as_str()) {
                    let (arguments, raw_arguments) = match function.get("arguments") {
                        Some(Value::String(raw)) => arguments_value_from_str(raw),
                        Some(other) => (other.clone(), None),
                        None => (Value::Null, None),
                    };
                    let id = raw_call
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or_else(|| "tool_call");
                    out.push(ToolCall {
                        id: id.to_string(),
                        name: name.to_string(),
                        arguments,
                        raw_arguments,
                    });
                }
            }
        }
    }

    extract_openai_legacy_function_call(node, out);

    if let Some(message) = node.get("message") {
        extract_openai_calls(message, out);
    }

    if let Some(delta) = node.get("delta") {
        extract_openai_calls(delta, out);
    }
}

#[cfg(test)]
mod tests {
    use super::parse_tool_calls;
    use serde_json::json;

    #[test]
    fn parses_ollama_non_streaming_tool_calls_from_message() {
        let payload = json!({
            "model": "qwen3",
            "message": {
                "role": "assistant",
                "content": "",
                "tool_calls": [{
                    "function": {
                        "name": "get_weather",
                        "arguments": {
                            "city": "Istanbul"
                        }
                    }
                }]
            },
            "done": true
        });

        let calls = parse_tool_calls("ollama", &payload);

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "get_weather");
        assert_eq!(calls[0].arguments, json!({ "city": "Istanbul" }));
    }

    #[test]
    fn parses_ollama_streaming_tool_calls_from_message() {
        let payload = json!({
            "message": {
                "role": "assistant",
                "tool_calls": [{
                    "id": "call_1",
                    "function": {
                        "name": "add_two_numbers",
                        "arguments": {
                            "a": 3,
                            "b": 1
                        }
                    }
                }]
            },
            "done": false
        });

        let calls = parse_tool_calls("ollama", &payload);

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].id, "call_1");
        assert_eq!(calls[0].name, "add_two_numbers");
        assert_eq!(calls[0].arguments, json!({ "a": 3, "b": 1 }));
    }

    #[test]
    fn parses_openai_legacy_function_call_from_message() {
        let payload = json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "function_call": {
                        "name": "write_summary",
                        "arguments": "{\"summary\":\"short recap\"}"
                    }
                }
            }]
        });

        let calls = parse_tool_calls("openai", &payload);

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "write_summary");
        assert_eq!(calls[0].arguments, json!({ "summary": "short recap" }));
        assert_eq!(calls[0].raw_arguments.as_deref(), Some("{\"summary\":\"short recap\"}"));
    }

    #[test]
    fn parses_openai_legacy_function_call_camel_case() {
        let payload = json!({
            "message": {
                "role": "assistant",
                "functionCall": {
                    "name": "create_memory",
                    "arguments": {
                        "text": "Likes tea",
                        "category": "preference"
                    }
                }
            }
        });

        let calls = parse_tool_calls("local", &payload);

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "create_memory");
        assert_eq!(
            calls[0].arguments,
            json!({ "text": "Likes tea", "category": "preference" })
        );
    }
}
