//! Tool calls in a local model's reply: OpenAI `tool_calls`/`function_call`
//! shapes in the parsed message (with Anthropic `tool_use` parts and Gemini
//! `functionCall` parts as fallbacks), and raw text recovery for models that
//! wrote their calls as `<tool_call>` blocks, JSON or `<function=name>` tags
//! instead.

use serde_json::{Map, Value, json};

#[derive(Clone, Debug, PartialEq)]
pub struct LocalToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
    pub raw_arguments: Option<String>,
}

const TEXT_BLOCK_TAGS: [(&str, &str); 4] = [
    ("<tool_call>", "</tool_call>"),
    ("<tool_calls>", "</tool_calls>"),
    ("<function_call>", "</function_call>"),
    ("<function_calls>", "</function_calls>"),
];

/// The tool calls in a parsed assistant message.
#[must_use]
pub fn parse_tool_calls(payload: &Value) -> Vec<LocalToolCall> {
    let mut calls = Vec::new();

    if let Some(choices) = payload.get("choices").and_then(Value::as_array) {
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

    if calls.is_empty() {
        for part in payload
            .get("content")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if part.get("type").and_then(Value::as_str) != Some("tool_use") {
                continue;
            }
            let Some(name) = part.get("name").and_then(Value::as_str) else {
                continue;
            };
            let id = part.get("id").and_then(Value::as_str).unwrap_or("tool_use");
            let (arguments, raw_arguments) = match part.get("input") {
                Some(Value::String(raw)) => arguments_value_from_str(raw),
                Some(other) => (other.clone(), None),
                None => (Value::Null, None),
            };
            calls.push(LocalToolCall {
                id: id.to_string(),
                name: name.to_string(),
                arguments,
                raw_arguments,
            });
        }
    }

    if calls.is_empty() {
        for candidate in payload
            .get("candidates")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let parts = candidate
                .get("content")
                .and_then(Value::as_object)
                .and_then(|content| content.get("parts"))
                .and_then(Value::as_array);
            for part in parts.into_iter().flatten() {
                let Some(function_call) = part
                    .get("function_call")
                    .or_else(|| part.get("functionCall"))
                else {
                    continue;
                };
                let Some(name) = function_call.get("name").and_then(Value::as_str) else {
                    continue;
                };
                let arguments = function_call
                    .get("args")
                    .or_else(|| function_call.get("arguments"))
                    .cloned()
                    .unwrap_or_else(|| Value::Object(Map::new()));
                let id = function_call
                    .get("id")
                    .or_else(|| function_call.get("call_id"))
                    .or_else(|| function_call.get("callId"))
                    .and_then(Value::as_str)
                    .map_or_else(
                        || format!("func_call_{}", calls.len() + 1),
                        ToString::to_string,
                    );
                calls.push(LocalToolCall {
                    id,
                    name: name.to_string(),
                    arguments,
                    raw_arguments: None,
                });
            }
        }
    }

    calls
}

/// Tool calls a model wrote into its text.
#[must_use]
pub fn parse_tool_calls_from_text(raw: &str) -> Vec<LocalToolCall> {
    let mut calls = Vec::new();
    let normalized = raw.trim();

    for (open_tag, close_tag) in TEXT_BLOCK_TAGS {
        let mut cursor = 0usize;
        while let Some(start_rel) = normalized[cursor..].find(open_tag) {
            let start = cursor + start_rel + open_tag.len();
            let Some(end_rel) = normalized[start..].find(close_tag) else {
                break;
            };
            let end = start + end_rel;
            parse_tool_call_block_into(normalized[start..end].trim(), &mut calls);
            cursor = end + close_tag.len();
        }
    }

    if calls.is_empty() {
        parse_tool_call_block_into(normalized, &mut calls);
    }

    calls
}

/// The text with tool-call blocks and stray ChatML turn markers removed.
#[must_use]
pub fn strip_tool_call_blocks(raw: &str) -> String {
    let mut out = raw.to_string();
    for (open_tag, close_tag) in TEXT_BLOCK_TAGS {
        out = strip_tagged_blocks(&out, open_tag, close_tag);
    }
    out = strip_tagged_blocks(&out, "<function=", "</function>");
    out.replace("<|im_start|>assistant", "")
        .replace("<|im_end|>", "")
        .trim()
        .to_string()
}

/// An assistant message rebuilt from calls written into the text.
#[must_use]
pub fn recover_message_from_raw_tool_output(output: &str) -> Option<Value> {
    let tool_calls = parse_tool_calls_from_text(output);
    if tool_calls.is_empty() {
        return None;
    }

    let tool_calls_value = tool_calls
        .iter()
        .map(|call| {
            json!({
                "id": call.id,
                "type": "function",
                "function": {
                    "name": call.name,
                    "arguments": call
                        .raw_arguments
                        .clone()
                        .unwrap_or_else(|| call.arguments.to_string()),
                }
            })
        })
        .collect::<Vec<_>>();

    Some(json!({
        "role": "assistant",
        "content": strip_tool_call_blocks(output),
        "tool_calls": tool_calls_value,
    }))
}

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

fn arguments_value_from_str(raw: &str) -> (Value, Option<String>) {
    if let Some(parsed) = parse_parameter_tag_arguments(raw) {
        return (parsed, Some(raw.to_string()));
    }
    match serde_json::from_str::<Value>(raw) {
        Ok(value) => (value, Some(raw.to_string())),
        Err(_) => (Value::String(raw.to_string()), Some(raw.to_string())),
    }
}

fn parse_parameter_tag_arguments(raw: &str) -> Option<Value> {
    let trimmed = raw.trim();
    if !trimmed.contains("<parameter") || !trimmed.contains("</parameter>") {
        return None;
    }

    let mut map = Map::new();
    let mut cursor = 0usize;

    while let Some(start_rel) = trimmed[cursor..].find("<parameter") {
        let start = cursor + start_rel;
        let after_start = &trimmed[start + "<parameter".len()..];
        let Some(name_end_rel) = after_start.find('>') else {
            break;
        };
        let name = after_start[..name_end_rel]
            .trim()
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
        map.insert(
            name.to_string(),
            coerce_parameter_value(trimmed[content_start..content_end].trim()),
        );
        cursor = content_end + "</parameter>".len();
    }

    (!map.is_empty()).then_some(Value::Object(map))
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
    serde_json::from_str::<Value>(trimmed).unwrap_or_else(|_| Value::String(trimmed.to_string()))
}

fn parse_tool_call_block_into(block: &str, out: &mut Vec<LocalToolCall>) {
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

fn parse_tool_call_block_function_tag(block: &str, index: usize) -> Option<LocalToolCall> {
    let rest = block.trim().strip_prefix("<function=")?;
    let name_end = rest.find('>')?;
    let name = rest[..name_end].trim().trim_matches('"').trim_matches('\'');
    if name.is_empty() {
        return None;
    }
    let inner = rest[name_end + 1..]
        .strip_suffix("</function>")
        .unwrap_or("")
        .trim();
    let (arguments, raw_arguments) = if inner.is_empty() {
        (Value::Object(Map::new()), None)
    } else if let Ok(value) = serde_json::from_str::<Value>(inner) {
        (value, Some(inner.to_string()))
    } else {
        (Value::String(inner.to_string()), Some(inner.to_string()))
    };
    Some(LocalToolCall {
        id: format!("text_tool_call_{index}"),
        name: name.to_string(),
        arguments,
        raw_arguments,
    })
}

fn extract_tool_calls_from_json_value(value: &Value, out: &mut Vec<LocalToolCall>) {
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
                .and_then(Value::as_array)
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

fn parse_json_tool_call_object(value: &Value, index: usize) -> Option<LocalToolCall> {
    let function = value.get("function").unwrap_or(value);
    let name = function
        .get("name")
        .or_else(|| value.get("name"))
        .and_then(Value::as_str)?;
    let arguments_node = function
        .get("arguments")
        .or_else(|| function.get("args"))
        .or_else(|| value.get("arguments"))
        .or_else(|| value.get("args"));
    let (arguments, raw_arguments) = match arguments_node {
        Some(Value::String(raw)) => arguments_value_from_str(raw),
        Some(other) => (other.clone(), None),
        None => (Value::Object(Map::new()), None),
    };
    let id = value
        .get("id")
        .or_else(|| value.get("call_id"))
        .or_else(|| value.get("callId"))
        .and_then(Value::as_str)
        .map_or_else(|| format!("text_tool_call_{index}"), ToOwned::to_owned);
    Some(LocalToolCall {
        id,
        name: name.to_string(),
        arguments,
        raw_arguments,
    })
}

fn extract_openai_legacy_function_call(node: &Value, out: &mut Vec<LocalToolCall>) {
    let Some(function_call) = node
        .get("function_call")
        .or_else(|| node.get("functionCall"))
    else {
        return;
    };
    let Some(name) = function_call.get("name").and_then(Value::as_str) else {
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
        .and_then(Value::as_str)
        .map_or_else(
            || format!("function_call_{}", out.len() + 1),
            ToOwned::to_owned,
        );
    out.push(LocalToolCall {
        id,
        name: name.to_string(),
        arguments,
        raw_arguments,
    });
}

fn extract_openai_calls(node: &Value, out: &mut Vec<LocalToolCall>) {
    for raw_call in node
        .get("tool_calls")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(function) = raw_call.get("function") else {
            continue;
        };
        let Some(name) = function.get("name").and_then(Value::as_str) else {
            continue;
        };
        let (arguments, raw_arguments) = match function.get("arguments") {
            Some(Value::String(raw)) => arguments_value_from_str(raw),
            Some(other) => (other.clone(), None),
            None => (Value::Null, None),
        };
        let id = raw_call
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("tool_call");
        out.push(LocalToolCall {
            id: id.to_string(),
            name: name.to_string(),
            arguments,
            raw_arguments,
        });
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
    use super::*;

    #[test]
    fn parsed_messages_yield_openai_tool_calls() {
        let message = json!({
            "role": "assistant",
            "content": "",
            "tool_calls": [
                {"id": "c1", "type": "function", "function": {"name": "lookup", "arguments": "{\"q\":\"x\"}"}},
                {"type": "function", "function": {"name": "time", "arguments": {"tz": "UTC"}}},
                {"type": "function", "function": {"arguments": "{}"}}
            ]
        });
        let calls = parse_tool_calls(&message);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].id, "c1");
        assert_eq!(calls[0].arguments, json!({"q": "x"}));
        assert_eq!(calls[0].raw_arguments.as_deref(), Some("{\"q\":\"x\"}"));
        assert_eq!(calls[1].id, "tool_call");
        assert_eq!(calls[1].raw_arguments, None);
        assert!(parse_tool_calls(&json!({"role": "assistant", "content": "hi"})).is_empty());
    }

    #[test]
    fn parameter_tags_become_typed_arguments() {
        let (value, raw) = arguments_value_from_str(
            "<parameter=city>Paris</parameter><parameter=days>3</parameter><parameter=hot>TRUE</parameter>",
        );
        assert_eq!(value, json!({"city": "Paris", "days": 3, "hot": true}));
        assert!(raw.is_some());
    }

    #[test]
    fn text_blocks_are_recovered_and_stripped() {
        let output = "Let me check.\n<tool_call>\n{\"name\": \"lookup\", \"arguments\": {\"q\": \"tea\"}}\n</tool_call><|im_end|>";
        let message = recover_message_from_raw_tool_output(output).expect("recovered");
        assert_eq!(message["content"], json!("Let me check."));
        assert_eq!(message["tool_calls"][0]["id"], json!("text_tool_call_1"));
        assert_eq!(
            message["tool_calls"][0]["function"]["arguments"],
            json!("{\"q\":\"tea\"}")
        );
        let tagged = parse_tool_calls_from_text("<function=search>{\"q\": 1}</function>");
        assert_eq!(tagged[0].name, "search");
        assert_eq!(tagged[0].arguments, json!({"q": 1}));
        assert!(recover_message_from_raw_tool_output("Just words.").is_none());
        assert_eq!(
            strip_tool_call_blocks("a <function=x>{}</function> b <tool_call>open"),
            "a  b <tool_call>open"
        );
    }
}
