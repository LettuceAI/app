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
    if let Some(parsed) = parse_parameter_tag_arguments(raw) {
        return (parsed, Some(raw.to_string()));
    }
    match serde_json::from_str::<Value>(raw) {
        Ok(val) => (val, Some(raw.to_string())),
        Err(_) => (Value::String(raw.to_string()), Some(raw.to_string())),
    }
}

fn parse_parameter_tag_arguments(raw: &str) -> Option<Value> {
    let trimmed = raw.trim();
    if !trimmed.contains("<parameter") || !trimmed.contains("</parameter>") {
        return None;
    }

    let mut map = serde_json::Map::new();
    let mut cursor = 0usize;

    while let Some(start_rel) = trimmed[cursor..].find("<parameter") {
        let start = cursor + start_rel;
        let after_start = &trimmed[start + "<parameter".len()..];

        let Some(name_end_rel) = after_start.find('>') else {
            break;
        };
        let name_fragment = after_start[..name_end_rel].trim();
        let name = name_fragment
            .trim_start_matches('=')
            .trim_start_matches('-')
            .trim()
            .trim_matches('"')
            .trim_matches('\'');
        if name.is_empty() {
            break;
        }

        let content_start = start + "<parameter".len() + name_end_rel + 1;
        let Some(end_rel) = trimmed[content_start..].find("</parameter>") else {
            break;
        };
        let content_end = content_start + end_rel;
        let value_raw = trimmed[content_start..content_end].trim();
        map.insert(name.to_string(), coerce_parameter_value(value_raw));
        cursor = content_end + "</parameter>".len();
    }

    if map.is_empty() {
        None
    } else {
        Some(Value::Object(map))
    }
}

fn coerce_parameter_value(raw: &str) -> Value {
    let trimmed = raw.trim();
    if trimmed.eq_ignore_ascii_case("true") {
        return Value::Bool(true);
    }
    if trimmed.eq_ignore_ascii_case("false") {
        return Value::Bool(false);
    }
    if trimmed.eq_ignore_ascii_case("null") {
        return Value::Null;
    }
    if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
        return parsed;
    }
    Value::String(trimmed.to_string())
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

/// Gemini needs a wrapped `tools` list plus a `toolConfig.functionCallingConfig`.
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
        Some(vec![json!({ "functionDeclarations": declarations })])
    }
}

pub fn gemini_tool_config(choice: Option<&ToolChoice>) -> Option<Value> {
    match choice {
        None | Some(ToolChoice::Auto) => Some(json!({
            "functionCallingConfig": { "mode": "AUTO" }
        })),
        Some(ToolChoice::None) => Some(json!({
            "functionCallingConfig": { "mode": "NONE" }
        })),
        Some(ToolChoice::Required) | Some(ToolChoice::Any) => Some(json!({
            "functionCallingConfig": { "mode": "ANY" }
        })),
        Some(ToolChoice::Tool { name }) => Some(json!({
            "functionCallingConfig": {
                "mode": "ANY",
                "allowedFunctionNames": [name]
            }
        })),
    }
}

/// Z.AI exposes OpenAI-style tools but only supports auto mode today.
pub fn zai_tool_choice(_choice: Option<&ToolChoice>) -> Option<Value> {
    Some(json!("auto"))
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
                            .unwrap_or("tool_use");
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
    if crate::chat_manager::provider_adapter::is_gemini_format_provider(provider_id)
        || calls.is_empty()
    {
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

pub fn parse_tool_calls_from_text(raw: &str) -> Vec<ToolCall> {
    let mut calls = Vec::new();
    let normalized = raw.trim();

    for (open_tag, close_tag) in [
        ("<tool_call>", "</tool_call>"),
        ("<tool_calls>", "</tool_calls>"),
        ("<function_call>", "</function_call>"),
        ("<function_calls>", "</function_calls>"),
    ] {
        let mut cursor = 0usize;
        while let Some(start_rel) = normalized[cursor..].find(open_tag) {
            let start = cursor + start_rel + open_tag.len();
            let Some(end_rel) = normalized[start..].find(close_tag) else {
                break;
            };
            let end = start + end_rel;
            let block = normalized[start..end].trim();
            parse_tool_call_block_into(block, &mut calls);
            cursor = end + close_tag.len();
        }
    }

    if calls.is_empty() {
        parse_tool_call_block_into(normalized, &mut calls);
    }

    calls
}

#[cfg(any(test, not(mobile)))]
pub fn strip_tool_call_blocks(raw: &str) -> String {
    let mut out = raw.to_string();

    for (open_tag, close_tag) in [
        ("<tool_call>", "</tool_call>"),
        ("<tool_calls>", "</tool_calls>"),
        ("<function_call>", "</function_call>"),
        ("<function_calls>", "</function_calls>"),
    ] {
        out = strip_tagged_blocks(&out, open_tag, close_tag);
    }

    out = strip_inline_function_blocks(&out);

    out.replace("<|im_start|>assistant", "")
        .replace("<|im_end|>", "")
        .trim()
        .to_string()
}

#[cfg(any(test, not(mobile)))]
fn strip_tagged_blocks(raw: &str, open_tag: &str, close_tag: &str) -> String {
    let mut out = String::new();
    let mut cursor = 0usize;

    while let Some(start_rel) = raw[cursor..].find(open_tag) {
        let start = cursor + start_rel;
        out.push_str(&raw[cursor..start]);

        let block_start = start + open_tag.len();
        let Some(end_rel) = raw[block_start..].find(close_tag) else {
            cursor = start;
            break;
        };
        cursor = block_start + end_rel + close_tag.len();
    }

    out.push_str(&raw[cursor..]);
    out
}

#[cfg(any(test, not(mobile)))]
fn strip_inline_function_blocks(raw: &str) -> String {
    let mut out = String::new();
    let mut cursor = 0usize;
    let open_tag = "<function=";
    let close_tag = "</function>";

    while let Some(start_rel) = raw[cursor..].find(open_tag) {
        let start = cursor + start_rel;
        out.push_str(&raw[cursor..start]);

        let block_start = start + open_tag.len();
        let Some(end_rel) = raw[block_start..].find(close_tag) else {
            cursor = start;
            break;
        };
        cursor = block_start + end_rel + close_tag.len();
    }

    out.push_str(&raw[cursor..]);
    out
}

fn parse_tool_call_block_into(block: &str, out: &mut Vec<ToolCall>) {
    if block.is_empty() {
        return;
    }

    if let Ok(value) = serde_json::from_str::<Value>(block) {
        extract_tool_calls_from_json_value(&value, out);
        if !out.is_empty() {
            return;
        }
    }

    if let Some(call) = parse_tool_call_block_function_tag(block, out.len() + 1) {
        out.push(call);
    }
}

fn parse_tool_call_block_function_tag(block: &str, index: usize) -> Option<ToolCall> {
    let prefix = "<function=";
    let suffix = "</function>";
    let trimmed = block.trim();
    let rest = trimmed.strip_prefix(prefix)?;
    let name_end = rest.find('>')?;
    let name = rest[..name_end].trim().trim_matches('"').trim_matches('\'');
    if name.is_empty() {
        return None;
    }
    let inner = rest[name_end + 1..]
        .strip_suffix(suffix)
        .unwrap_or("")
        .trim();
    let (arguments, raw_arguments) = if inner.is_empty() {
        (Value::Object(Default::default()), None)
    } else if let Ok(value) = serde_json::from_str::<Value>(inner) {
        (value, Some(inner.to_string()))
    } else {
        (Value::String(inner.to_string()), Some(inner.to_string()))
    };

    Some(ToolCall {
        id: format!("text_tool_call_{}", index),
        name: name.to_string(),
        arguments,
        raw_arguments,
    })
}

fn extract_tool_calls_from_json_value(value: &Value, out: &mut Vec<ToolCall>) {
    match value {
        Value::Array(items) => {
            for item in items {
                extract_tool_calls_from_json_value(item, out);
            }
        }
        Value::Object(map) => {
            if let Some(tool_calls) = map
                .get("tool_calls")
                .or_else(|| map.get("toolCalls"))
                .or_else(|| map.get("calls"))
                .and_then(|v| v.as_array())
            {
                for item in tool_calls {
                    extract_tool_calls_from_json_value(item, out);
                }
                return;
            }

            if let Some(function_call) =
                map.get("function_call").or_else(|| map.get("functionCall"))
            {
                extract_tool_calls_from_json_value(function_call, out);
                return;
            }

            if let Some(call) = parse_json_tool_call_object(value, out.len() + 1) {
                out.push(call);
            }
        }
        _ => {}
    }
}

fn parse_json_tool_call_object(value: &Value, index: usize) -> Option<ToolCall> {
    let function = value.get("function").unwrap_or(value);
    let name = function
        .get("name")
        .or_else(|| value.get("name"))
        .and_then(|v| v.as_str())?;
    let arguments_node = function
        .get("arguments")
        .or_else(|| function.get("args"))
        .or_else(|| value.get("arguments"))
        .or_else(|| value.get("args"));
    let (arguments, raw_arguments) = match arguments_node {
        Some(Value::String(raw)) => arguments_value_from_str(raw),
        Some(other) => (other.clone(), None),
        None => (Value::Object(Default::default()), None),
    };
    let id = value
        .get("id")
        .or_else(|| value.get("call_id"))
        .or_else(|| value.get("callId"))
        .and_then(|v| v.as_str())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("text_tool_call_{}", index));

    Some(ToolCall {
        id,
        name: name.to_string(),
        arguments,
        raw_arguments,
    })
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
                        .unwrap_or("tool_call");
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
