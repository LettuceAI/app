use lettuce_conversations::ProposedToolCall;
use quick_xml::{
    Reader,
    escape::{resolve_xml_entity, unescape},
    events::{BytesRef, BytesStart, Event},
};
use serde_json::{Map, Value};

use crate::DynamicMemoryStructuredFallbackFormat;

const ROOT_TAGS: &[&str] = &["memory_ops", "operations"];
const OPERATION_TAGS: &[&str] = &[
    "create_memory",
    "delete_memory",
    "pin_memory",
    "unpin_memory",
    "done",
];

pub const MEMORY_OPERATIONS_XML_FALLBACK_PROMPT: &str = r#"Return only XML. Format: <memory_ops><create_memory important="false"><text>...</text><category>plot_event</category></create_memory><delete_memory confidence="0.9"><text>memory ID</text></delete_memory><pin_memory><id>memory ID</id></pin_memory><unpin_memory><id>memory ID</id></unpin_memory><done><summary>optional note</summary></done></memory_ops>. Use an empty <memory_ops /> when no changes are needed. Do not use markdown."#;

pub const MEMORY_OPERATIONS_JSON_FALLBACK_PROMPT: &str = r#"Return only JSON. Format: {"operations":[{"name":"create_memory","arguments":{"text":"...","category":"plot_event","important":false}},{"name":"delete_memory","arguments":{"text":"memory ID","confidence":0.9}},{"name":"pin_memory","arguments":{"id":"memory ID"}},{"name":"unpin_memory","arguments":{"id":"memory ID"}},{"name":"done","arguments":{"summary":"optional note"}}]}. Use {"operations":[]} when no changes are needed. Do not use markdown."#;

#[must_use]
pub const fn memory_operations_fallback_prompt(
    format: DynamicMemoryStructuredFallbackFormat,
) -> &'static str {
    match format {
        DynamicMemoryStructuredFallbackFormat::Json => MEMORY_OPERATIONS_JSON_FALLBACK_PROMPT,
        DynamicMemoryStructuredFallbackFormat::Xml => MEMORY_OPERATIONS_XML_FALLBACK_PROMPT,
    }
}

pub fn parse_memory_operations_from_text(
    raw: &str,
    format: DynamicMemoryStructuredFallbackFormat,
) -> Result<Vec<ProposedToolCall>, StructuredFallbackError> {
    match format {
        DynamicMemoryStructuredFallbackFormat::Json => parse_json(raw),
        DynamicMemoryStructuredFallbackFormat::Xml => parse_xml(raw),
    }
}

fn normalize(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.starts_with("```") {
        let mut lines = trimmed.lines();
        let _ = lines.next();
        let mut body = lines.collect::<Vec<_>>();
        if body.last().is_some_and(|line| line.trim() == "```") {
            body.pop();
        }
        return body.join("\n").trim().to_owned();
    }
    trimmed.to_owned()
}

fn json_snippet(raw: &str) -> Option<&str> {
    let mut start = None;
    let mut stack = Vec::new();
    let mut in_string = false;
    let mut escape = false;
    for (index, ch) in raw.char_indices() {
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
                start.get_or_insert(index);
                stack.push(ch);
            }
            '}' if stack.pop() == Some('{') => {
                if stack.is_empty() {
                    return start.map(|begin| &raw[begin..=index]);
                }
            }
            ']' if stack.pop() == Some('[') => {
                if stack.is_empty() {
                    return start.map(|begin| &raw[begin..=index]);
                }
            }
            '}' | ']' => return None,
            _ => {}
        }
    }
    None
}

fn parse_json(raw: &str) -> Result<Vec<ProposedToolCall>, StructuredFallbackError> {
    let normalized = normalize(raw);
    let value: Value = serde_json::from_str(json_snippet(&normalized).unwrap_or(&normalized))
        .map_err(|_| StructuredFallbackError::InvalidJson)?;
    let operations = match &value {
        Value::Array(items) => items,
        Value::Object(map) => map
            .get("operations")
            .or_else(|| map.get("toolCalls"))
            .or_else(|| map.get("calls"))
            .and_then(Value::as_array)
            .ok_or(StructuredFallbackError::InvalidJson)?,
        _ => return Err(StructuredFallbackError::InvalidJson),
    };
    operations
        .iter()
        .enumerate()
        .filter_map(|(index, item)| item.as_object().map(|item| (index, item)))
        .map(|(index, item)| {
            let name = item
                .get("name")
                .or_else(|| item.get("tool"))
                .or_else(|| item.get("op"))
                .or_else(|| item.get("action"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .ok_or(StructuredFallbackError::MissingOperationName)?;
            let arguments = match item.get("arguments") {
                Some(value) => value.clone(),
                None => Value::Object(
                    item.iter()
                        .filter(|(key, _)| {
                            !matches!(key.as_str(), "name" | "tool" | "op" | "action")
                        })
                        .map(|(key, value)| (key.clone(), value.clone()))
                        .collect(),
                ),
            };
            Ok(ProposedToolCall {
                provider_call_id: Some(format!("json_op_{}", index + 1)),
                name: name.to_owned(),
                arguments,
                raw_arguments: None,
                provider_replay: None,
            })
        })
        .collect()
}

fn attribute(element: &BytesStart<'_>, key: &[u8]) -> Option<String> {
    element.attributes().flatten().find_map(|attribute| {
        (attribute.key.as_ref() == key)
            .then(|| {
                attribute
                    .unescape_value()
                    .ok()
                    .map(|value| value.into_owned())
            })
            .flatten()
    })
}

fn insert_string(arguments: &mut Map<String, Value>, key: &str, value: Option<String>) {
    if let Some(value) = value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
    {
        arguments.insert(key.to_owned(), Value::String(value));
    }
}

fn insert_bool(arguments: &mut Map<String, Value>, key: &str, value: Option<String>) {
    let value = value.as_deref().map(str::trim);
    if matches!(value, Some("true" | "1" | "yes")) {
        arguments.insert(key.to_owned(), Value::Bool(true));
    } else if matches!(value, Some("false" | "0" | "no")) {
        arguments.insert(key.to_owned(), Value::Bool(false));
    }
}

fn insert_number(arguments: &mut Map<String, Value>, key: &str, value: Option<String>) {
    if let Some(number) = value
        .and_then(|value| value.trim().parse::<f64>().ok())
        .and_then(serde_json::Number::from_f64)
    {
        arguments.insert(key.to_owned(), Value::Number(number));
    }
}

fn operation_arguments(event: &BytesStart<'_>) -> Map<String, Value> {
    let mut arguments = Map::new();
    insert_string(&mut arguments, "text", attribute(event, b"text"));
    insert_string(&mut arguments, "category", attribute(event, b"category"));
    insert_string(&mut arguments, "id", attribute(event, b"id"));
    insert_string(&mut arguments, "summary", attribute(event, b"summary"));
    insert_bool(&mut arguments, "important", attribute(event, b"important"));
    insert_number(
        &mut arguments,
        "confidence",
        attribute(event, b"confidence"),
    );
    arguments
}

fn operation_name(event: &BytesStart<'_>) -> String {
    let tag = String::from_utf8_lossy(event.name().as_ref()).into_owned();
    if tag == "operation" {
        attribute(event, b"name")
            .or_else(|| attribute(event, b"op"))
            .unwrap_or_default()
    } else {
        tag
    }
}

fn append(arguments: &mut Map<String, Value>, key: &str, fragment: &str) {
    if fragment.is_empty() {
        return;
    }
    match arguments.get_mut(key) {
        Some(Value::String(value)) => value.push_str(fragment),
        _ => {
            arguments.insert(key.to_owned(), Value::String(fragment.to_owned()));
        }
    }
}

fn decode_reference(reference: BytesRef<'_>) -> Result<String, StructuredFallbackError> {
    if let Ok(Some(character)) = reference.resolve_char_ref() {
        return Ok(character.to_string());
    }
    let content = reference
        .xml_content()
        .map_err(|_| StructuredFallbackError::InvalidXml)?;
    Ok(resolve_xml_entity(&content).map_or_else(|| format!("&{content};"), str::to_owned))
}

fn push_xml_call(
    calls: &mut Vec<ProposedToolCall>,
    name: String,
    mut arguments: Map<String, Value>,
) {
    arguments.retain(|_, value| match value {
        Value::String(text) => {
            *text = text.trim().to_owned();
            !text.is_empty()
        }
        _ => true,
    });
    calls.push(ProposedToolCall {
        provider_call_id: Some(format!("xml_op_{}", calls.len() + 1)),
        name,
        arguments: Value::Object(arguments),
        raw_arguments: None,
        provider_replay: None,
    });
}

fn parse_xml(raw: &str) -> Result<Vec<ProposedToolCall>, StructuredFallbackError> {
    let normalized = normalize(raw);
    let mut reader = Reader::from_str(&normalized);
    reader.config_mut().trim_text(false);
    let mut buffer = Vec::new();
    let mut root_seen = false;
    let mut current_name = None;
    let mut current_arguments = Map::new();
    let mut current_field = None;
    let mut calls = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                let tag = String::from_utf8_lossy(event.name().as_ref()).into_owned();
                if !root_seen && ROOT_TAGS.contains(&tag.as_str()) {
                    root_seen = true;
                } else if root_seen && current_name.is_none() {
                    let name = operation_name(&event);
                    if OPERATION_TAGS.contains(&name.as_str()) {
                        current_arguments = operation_arguments(&event);
                        current_name = Some(name);
                    }
                } else if current_name.is_some() {
                    current_field = Some(tag);
                }
            }
            Ok(Event::Empty(event)) => {
                let tag = String::from_utf8_lossy(event.name().as_ref()).into_owned();
                if !root_seen && ROOT_TAGS.contains(&tag.as_str()) {
                    root_seen = true;
                } else if root_seen && current_name.is_none() {
                    let name = operation_name(&event);
                    if OPERATION_TAGS.contains(&name.as_str()) {
                        push_xml_call(&mut calls, name, operation_arguments(&event));
                    }
                }
            }
            Ok(Event::Text(event)) => {
                if let Some(field) = current_field.as_deref() {
                    let encoded = String::from_utf8_lossy(event.as_ref());
                    let text =
                        unescape(&encoded).map_err(|_| StructuredFallbackError::InvalidXml)?;
                    append(&mut current_arguments, field, &text);
                }
            }
            Ok(Event::CData(event)) => {
                if let Some(field) = current_field.as_deref() {
                    append(
                        &mut current_arguments,
                        field,
                        &String::from_utf8_lossy(event.as_ref()),
                    );
                }
            }
            Ok(Event::GeneralRef(event)) => {
                if let Some(field) = current_field.as_deref() {
                    append(&mut current_arguments, field, &decode_reference(event)?);
                }
            }
            Ok(Event::End(event)) => {
                let tag = String::from_utf8_lossy(event.name().as_ref()).into_owned();
                if current_field.as_deref() == Some(tag.as_str()) {
                    current_field = None;
                } else if current_name.as_deref() == Some(tag.as_str())
                    || (tag == "operation" && current_name.is_some())
                {
                    push_xml_call(
                        &mut calls,
                        current_name.take().unwrap_or_default(),
                        std::mem::take(&mut current_arguments),
                    );
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => return Err(StructuredFallbackError::InvalidXml),
            _ => {}
        }
        buffer.clear();
    }
    if !root_seen {
        return Err(StructuredFallbackError::InvalidXml);
    }
    Ok(calls)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum StructuredFallbackError {
    #[error("fallback response did not contain valid JSON operations")]
    InvalidJson,
    #[error("fallback JSON operation is missing a name")]
    MissingOperationName,
    #[error("fallback response did not contain valid XML operations")]
    InvalidXml,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn parses_legacy_xml_wrapper_text_and_entities() {
        let calls = parse_memory_operations_from_text(
            "Here you go:\n```xml\n<memory_ops><create_memory important=\"true\"><text>Sam &amp; Elias reconciled</text><category>relationship</category></create_memory><done summary=\"all set\" /></memory_ops>\n```",
            DynamicMemoryStructuredFallbackFormat::Xml,
        )
        .expect("xml");
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].name, "create_memory");
        assert_eq!(
            calls[0].arguments,
            json!({"important":true,"text":"Sam & Elias reconciled","category":"relationship"})
        );
        assert_eq!(calls[1].arguments, json!({"summary":"all set"}));
    }

    #[test]
    fn parses_legacy_json_wrapper_and_aliases() {
        let calls = parse_memory_operations_from_text(
            "Answer:\n```json\n{\"operations\":[{\"name\":\"create_memory\",\"arguments\":{\"text\":\"Sam apologized\",\"category\":\"plot_event\",\"important\":true}},{\"action\":\"done\",\"summary\":\"captured\"}]}\n```",
            DynamicMemoryStructuredFallbackFormat::Json,
        )
        .expect("json");
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].provider_call_id.as_deref(), Some("json_op_1"));
        assert_eq!(calls[1].arguments, json!({"summary":"captured"}));
    }
}
