use quick_xml::escape::{resolve_xml_entity, unescape};
use quick_xml::events::{BytesRef, BytesStart, Event};
use quick_xml::Reader;
use serde_json::{Map, Value};
use std::collections::HashMap;

use crate::chat_manager::tooling::ToolCall;
use crate::chat_manager::types::DynamicMemoryStructuredFallbackFormat;

const OPERATION_ROOT_TAGS: &[&str] = &["memory_ops", "operations"];
const REPAIR_ROOT_TAGS: &[&str] = &["memory_repairs", "items"];
const OPERATION_TAGS: &[&str] = &[
    "create_memory",
    "delete_memory",
    "pin_memory",
    "unpin_memory",
    "done",
];

pub const MEMORY_OPERATIONS_XML_FALLBACK_PROMPT: &str = r#"Return only XML. Format: <memory_ops><create_memory important="false"><text>...</text><category>plot_event</category></create_memory><delete_memory confidence="0.9"><text>123456</text></delete_memory><pin_memory><id>123456</id></pin_memory><unpin_memory><id>123456</id></unpin_memory><done><summary>optional note</summary></done></memory_ops>. Use an empty <memory_ops /> when no changes are needed. Do not use markdown."#;

pub const MEMORY_REPAIRS_XML_FALLBACK_PROMPT: &str = r#"Return only XML. Format: <memory_repairs><item><text>...</text><category>other</category></item></memory_repairs>. Use exactly one <item> per input text. Do not use markdown."#;

pub const MEMORY_OPERATIONS_JSON_FALLBACK_PROMPT: &str = r#"Return only JSON. Format: {"operations":[{"name":"create_memory","arguments":{"text":"...","category":"plot_event","important":false}},{"name":"delete_memory","arguments":{"text":"123456","confidence":0.9}},{"name":"pin_memory","arguments":{"id":"123456"}},{"name":"unpin_memory","arguments":{"id":"123456"}},{"name":"done","arguments":{"summary":"optional note"}}]}. Use {"operations":[]} when no changes are needed. Do not use markdown."#;

pub const MEMORY_REPAIRS_JSON_FALLBACK_PROMPT: &str = r#"Return only JSON. Format: {"items":[{"text":"...","category":"other"}]}. Use exactly one item per input text. Do not use markdown."#;

pub fn structured_fallback_format_label(format: DynamicMemoryStructuredFallbackFormat) -> &'static str {
    match format {
        DynamicMemoryStructuredFallbackFormat::Json => "json",
        DynamicMemoryStructuredFallbackFormat::Xml => "xml",
    }
}

pub fn memory_operations_fallback_prompt(
    format: DynamicMemoryStructuredFallbackFormat,
) -> &'static str {
    match format {
        DynamicMemoryStructuredFallbackFormat::Json => MEMORY_OPERATIONS_JSON_FALLBACK_PROMPT,
        DynamicMemoryStructuredFallbackFormat::Xml => MEMORY_OPERATIONS_XML_FALLBACK_PROMPT,
    }
}

pub fn memory_repairs_fallback_prompt(
    format: DynamicMemoryStructuredFallbackFormat,
) -> &'static str {
    match format {
        DynamicMemoryStructuredFallbackFormat::Json => MEMORY_REPAIRS_JSON_FALLBACK_PROMPT,
        DynamicMemoryStructuredFallbackFormat::Xml => MEMORY_REPAIRS_XML_FALLBACK_PROMPT,
    }
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

fn attr_value(element: &BytesStart<'_>, key: &[u8]) -> Option<String> {
    for attr in element.attributes().flatten() {
        if attr.key.as_ref() == key {
            return attr.unescape_value().ok().map(|value| value.into_owned());
        }
    }
    None
}

fn insert_if_present(args: &mut Map<String, Value>, key: &str, value: Option<String>) {
    if let Some(value) = value.map(|v| v.trim().to_string()).filter(|v| !v.is_empty()) {
        args.insert(key.to_string(), Value::String(value));
    }
}

fn insert_bool_attr(args: &mut Map<String, Value>, key: &str, value: Option<String>) {
    if let Some(value) = value {
        let normalized = value.trim().to_ascii_lowercase();
        if matches!(normalized.as_str(), "true" | "1" | "yes") {
            args.insert(key.to_string(), Value::Bool(true));
        } else if matches!(normalized.as_str(), "false" | "0" | "no") {
            args.insert(key.to_string(), Value::Bool(false));
        }
    }
}

fn insert_number_attr(args: &mut Map<String, Value>, key: &str, value: Option<String>) {
    if let Some(value) = value.and_then(|v| v.trim().parse::<f64>().ok()) {
        if let Some(number) = serde_json::Number::from_f64(value) {
            args.insert(key.to_string(), Value::Number(number));
        }
    }
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

fn append_json_string_field(args: &mut Map<String, Value>, key: &str, fragment: &str) {
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

fn append_optional_string(slot: &mut Option<String>, fragment: &str) {
    if fragment.is_empty() {
        return;
    }

    match slot {
        Some(existing) => existing.push_str(fragment),
        None => *slot = Some(fragment.to_string()),
    }
}

fn trim_json_string_fields(args: &mut Map<String, Value>) {
    args.retain(|_, value| match value {
        Value::String(text) => {
            *text = text.trim().to_string();
            !text.is_empty()
        }
        _ => true,
    });
}

fn trimmed_option_string(slot: &mut Option<String>) -> Option<String> {
    slot.take()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn parse_memory_operations_from_json(raw: &str) -> Result<Vec<ToolCall>, String> {
    let normalized = normalize_structured_fallback_text(raw);
    let snippet = extract_json_snippet(&normalized).unwrap_or(normalized.as_str());
    let value: Value =
        serde_json::from_str(snippet).map_err(|err| format!("fallback JSON parse error: {}", err))?;

    let operations = match &value {
        Value::Array(items) => items,
        Value::Object(map) => map
            .get("operations")
            .or_else(|| map.get("toolCalls"))
            .or_else(|| map.get("calls"))
            .and_then(Value::as_array)
            .ok_or_else(|| "fallback response did not contain valid JSON operations".to_string())?,
        _ => {
            return Err("fallback response did not contain valid JSON operations".to_string());
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
            .or_else(|| obj.get("action"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| format!("fallback JSON operation {} is missing a name", index + 1))?;

        let arguments = match obj.get("arguments") {
            Some(Value::Object(args)) => Value::Object(args.clone()),
            Some(other) => other.clone(),
            None => {
                let mut args = Map::new();
                for (key, value) in obj {
                    if matches!(key.as_str(), "name" | "tool" | "op" | "action") {
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
            raw_arguments: None,
            arguments,
        });
    }

    Ok(calls)
}

fn parse_memory_tag_repairs_from_json(
    raw: &str,
    allowed_categories: &[&str],
) -> Result<HashMap<String, String>, String> {
    let normalized = normalize_structured_fallback_text(raw);
    let snippet = extract_json_snippet(&normalized).unwrap_or(normalized.as_str());
    let value: Value =
        serde_json::from_str(snippet).map_err(|err| format!("fallback JSON parse error: {}", err))?;

    let items = match &value {
        Value::Array(items) => items,
        Value::Object(map) => map
            .get("items")
            .or_else(|| map.get("repairs"))
            .or_else(|| map.get("results"))
            .and_then(Value::as_array)
            .ok_or_else(|| "fallback response did not contain valid JSON repairs".to_string())?,
        _ => return Err("fallback response did not contain valid JSON repairs".to_string()),
    };

    let mut repaired = HashMap::new();
    for item in items {
        let Some(obj) = item.as_object() else {
            continue;
        };
        let Some(text) = obj.get("text").and_then(Value::as_str).map(str::trim) else {
            continue;
        };
        let Some(category) = obj.get("category").and_then(Value::as_str).map(str::trim) else {
            continue;
        };
        if !text.is_empty() && allowed_categories.contains(&category) {
            repaired.insert(text.to_string(), category.to_string());
        }
    }

    Ok(repaired)
}

fn parse_memory_operations_from_xml(raw: &str) -> Result<Vec<ToolCall>, String> {
    let normalized = normalize_structured_fallback_text(raw);
    let mut reader = Reader::from_str(&normalized);
    reader.config_mut().trim_text(false);

    let mut buf = Vec::new();
    let mut root_seen = false;
    let mut current_op_name: Option<String> = None;
    let mut current_args = Map::new();
    let mut current_field: Option<String> = None;
    let mut calls = Vec::new();
    let mut op_index = 0usize;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(event)) => {
                let tag = String::from_utf8_lossy(event.name().as_ref()).into_owned();
                if !root_seen && OPERATION_ROOT_TAGS.contains(&tag.as_str()) {
                    root_seen = true;
                } else if root_seen && current_op_name.is_none() {
                    let op_name = if tag == "operation" {
                        attr_value(&event, b"name")
                            .or_else(|| attr_value(&event, b"op"))
                            .unwrap_or_default()
                    } else {
                        tag.clone()
                    };
                    if OPERATION_TAGS.contains(&op_name.as_str()) {
                        current_op_name = Some(op_name);
                        current_args = Map::new();
                        insert_if_present(&mut current_args, "text", attr_value(&event, b"text"));
                        insert_if_present(
                            &mut current_args,
                            "category",
                            attr_value(&event, b"category"),
                        );
                        insert_if_present(&mut current_args, "id", attr_value(&event, b"id"));
                        insert_if_present(
                            &mut current_args,
                            "summary",
                            attr_value(&event, b"summary"),
                        );
                        insert_bool_attr(
                            &mut current_args,
                            "important",
                            attr_value(&event, b"important"),
                        );
                        insert_number_attr(
                            &mut current_args,
                            "confidence",
                            attr_value(&event, b"confidence"),
                        );
                    }
                } else if current_op_name.is_some() {
                    current_field = Some(tag);
                }
            }
            Ok(Event::Empty(event)) => {
                let tag = String::from_utf8_lossy(event.name().as_ref()).into_owned();
                if !root_seen && OPERATION_ROOT_TAGS.contains(&tag.as_str()) {
                    root_seen = true;
                } else if root_seen && current_op_name.is_none() {
                    let op_name = if tag == "operation" {
                        attr_value(&event, b"name")
                            .or_else(|| attr_value(&event, b"op"))
                            .unwrap_or_default()
                    } else {
                        tag.clone()
                    };
                    if OPERATION_TAGS.contains(&op_name.as_str()) {
                        let mut args = Map::new();
                        insert_if_present(&mut args, "text", attr_value(&event, b"text"));
                        insert_if_present(&mut args, "category", attr_value(&event, b"category"));
                        insert_if_present(&mut args, "id", attr_value(&event, b"id"));
                        insert_if_present(&mut args, "summary", attr_value(&event, b"summary"));
                        insert_bool_attr(&mut args, "important", attr_value(&event, b"important"));
                        insert_number_attr(
                            &mut args,
                            "confidence",
                            attr_value(&event, b"confidence"),
                        );
                        op_index += 1;
                        calls.push(ToolCall {
                            id: format!("xml_op_{}", op_index),
                            name: op_name,
                            arguments: Value::Object(args),
                            raw_arguments: None,
                        });
                    }
                }
            }
            Ok(Event::Text(event)) => {
                if let (Some(field), Some(_)) = (current_field.as_deref(), current_op_name.as_ref()) {
                    let text = decode_xml_text(event.as_ref())?;
                    append_json_string_field(&mut current_args, field, &text);
                }
            }
            Ok(Event::CData(event)) => {
                if let (Some(field), Some(_)) = (current_field.as_deref(), current_op_name.as_ref()) {
                    let text = String::from_utf8_lossy(event.as_ref());
                    append_json_string_field(&mut current_args, field, &text);
                }
            }
            Ok(Event::GeneralRef(event)) => {
                if let (Some(field), Some(_)) = (current_field.as_deref(), current_op_name.as_ref()) {
                    let text = decode_xml_general_ref(event)?;
                    append_json_string_field(&mut current_args, field, &text);
                }
            }
            Ok(Event::End(event)) => {
                let tag = String::from_utf8_lossy(event.name().as_ref()).into_owned();
                if current_field.as_deref() == Some(tag.as_str()) {
                    current_field = None;
                } else if current_op_name.as_deref() == Some(tag.as_str())
                    || (tag == "operation" && current_op_name.is_some())
                {
                    trim_json_string_fields(&mut current_args);
                    op_index += 1;
                    calls.push(ToolCall {
                        id: format!("xml_op_{}", op_index),
                        name: current_op_name.take().unwrap_or_default(),
                        arguments: Value::Object(std::mem::take(&mut current_args)),
                        raw_arguments: None,
                    });
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => {
                return Err(format!("fallback XML parse error: {}", err));
            }
            _ => {}
        }
        buf.clear();
    }

    if !root_seen {
        return Err("fallback response did not contain valid XML".to_string());
    }

    Ok(calls)
}

fn parse_memory_tag_repairs_from_xml(
    raw: &str,
    allowed_categories: &[&str],
) -> Result<HashMap<String, String>, String> {
    let normalized = normalize_structured_fallback_text(raw);
    let mut reader = Reader::from_str(&normalized);
    reader.config_mut().trim_text(false);

    let mut buf = Vec::new();
    let mut root_seen = false;
    let mut in_item = false;
    let mut current_text: Option<String> = None;
    let mut current_category: Option<String> = None;
    let mut current_field: Option<String> = None;
    let mut repaired = HashMap::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(event)) => {
                let tag = String::from_utf8_lossy(event.name().as_ref()).into_owned();
                if !root_seen && REPAIR_ROOT_TAGS.contains(&tag.as_str()) {
                    root_seen = true;
                } else if root_seen && tag == "item" {
                    in_item = true;
                    current_text = attr_value(&event, b"text");
                    current_category = attr_value(&event, b"category");
                } else if in_item {
                    current_field = Some(tag);
                }
            }
            Ok(Event::Empty(event)) => {
                let tag = String::from_utf8_lossy(event.name().as_ref()).into_owned();
                if !root_seen && REPAIR_ROOT_TAGS.contains(&tag.as_str()) {
                    root_seen = true;
                } else if root_seen && tag == "item" {
                    let text = attr_value(&event, b"text");
                    let category = attr_value(&event, b"category");
                    if let (Some(text), Some(category)) = (text, category) {
                        if allowed_categories.contains(&category.as_str()) {
                            repaired.insert(text, category);
                        }
                    }
                }
            }
            Ok(Event::Text(event)) => {
                if let Some(field) = current_field.as_deref() {
                    let text = decode_xml_text(event.as_ref())?;
                    match field {
                        "text" => append_optional_string(&mut current_text, &text),
                        "category" => append_optional_string(&mut current_category, &text),
                        _ => {}
                    }
                }
            }
            Ok(Event::CData(event)) => {
                if let Some(field) = current_field.as_deref() {
                    let text = String::from_utf8_lossy(event.as_ref());
                    match field {
                        "text" => append_optional_string(&mut current_text, &text),
                        "category" => append_optional_string(&mut current_category, &text),
                        _ => {}
                    }
                }
            }
            Ok(Event::GeneralRef(event)) => {
                if let Some(field) = current_field.as_deref() {
                    let text = decode_xml_general_ref(event)?;
                    match field {
                        "text" => append_optional_string(&mut current_text, &text),
                        "category" => append_optional_string(&mut current_category, &text),
                        _ => {}
                    }
                }
            }
            Ok(Event::End(event)) => {
                let tag = String::from_utf8_lossy(event.name().as_ref()).into_owned();
                if current_field.as_deref() == Some(tag.as_str()) {
                    current_field = None;
                } else if tag == "item" {
                    in_item = false;
                    if let (Some(text), Some(category)) = (
                        trimmed_option_string(&mut current_text),
                        trimmed_option_string(&mut current_category),
                    ) {
                        if allowed_categories.contains(&category.as_str()) {
                            repaired.insert(text, category);
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(format!("fallback XML parse error: {}", err)),
            _ => {}
        }
        buf.clear();
    }

    if !root_seen {
        return Err("fallback response did not contain valid XML".to_string());
    }

    Ok(repaired)
}

pub fn parse_memory_operations_from_text(
    raw: &str,
    format: DynamicMemoryStructuredFallbackFormat,
) -> Result<Vec<ToolCall>, String> {
    match format {
        DynamicMemoryStructuredFallbackFormat::Json => parse_memory_operations_from_json(raw),
        DynamicMemoryStructuredFallbackFormat::Xml => parse_memory_operations_from_xml(raw),
    }
}

pub fn parse_memory_tag_repairs_from_text(
    raw: &str,
    allowed_categories: &[&str],
    format: DynamicMemoryStructuredFallbackFormat,
) -> Result<HashMap<String, String>, String> {
    match format {
        DynamicMemoryStructuredFallbackFormat::Json => {
            parse_memory_tag_repairs_from_json(raw, allowed_categories)
        }
        DynamicMemoryStructuredFallbackFormat::Xml => {
            parse_memory_tag_repairs_from_xml(raw, allowed_categories)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_memory_operations_from_text, parse_memory_tag_repairs_from_text};
    use crate::chat_manager::types::DynamicMemoryStructuredFallbackFormat;
    use serde_json::json;

    #[test]
    fn parses_operations_with_wrapper_text_and_entities() {
        let raw = r#"Here you go:
```xml
<memory_ops>
  <create_memory important="true">
    <text>Sam &amp; Elias reconciled</text>
    <category>relationship</category>
  </create_memory>
  <done summary="all set" />
</memory_ops>
```"#;

        let calls = parse_memory_operations_from_text(raw, DynamicMemoryStructuredFallbackFormat::Xml)
            .expect("should parse xml");

        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].name, "create_memory");
        assert_eq!(
            calls[0].arguments,
            json!({
                "important": true,
                "text": "Sam & Elias reconciled",
                "category": "relationship"
            })
        );
        assert_eq!(calls[1].name, "done");
        assert_eq!(calls[1].arguments, json!({ "summary": "all set" }));
    }

    #[test]
    fn parses_repairs_with_prose_around_xml() {
        let raw = r#"I fixed the categories.
<memory_repairs>
  <item>
    <text>Likes tea</text>
    <category>preference</category>
  </item>
</memory_repairs>"#;

        let repaired = parse_memory_tag_repairs_from_text(
            raw,
            &["preference", "other"],
            DynamicMemoryStructuredFallbackFormat::Xml,
        )
        .expect("should parse repairs");

        assert_eq!(repaired.get("Likes tea"), Some(&"preference".to_string()));
    }

    #[test]
    fn parses_attribute_entities() {
        let raw = r#"<memory_ops><done summary="Sam &amp; Elias synced" /></memory_ops>"#;

        let calls = parse_memory_operations_from_text(raw, DynamicMemoryStructuredFallbackFormat::Xml)
            .expect("should parse xml");

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].arguments, json!({ "summary": "Sam & Elias synced" }));
    }

    #[test]
    fn parses_operations_from_wrapped_json() {
        let raw = r#"Answer:
```json
{"operations":[{"name":"create_memory","arguments":{"text":"Sam apologized","category":"plot_event","important":true}},{"name":"done","arguments":{"summary":"captured"}}]}
```"#;

        let calls =
            parse_memory_operations_from_text(raw, DynamicMemoryStructuredFallbackFormat::Json)
                .expect("should parse json");

        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].name, "create_memory");
        assert_eq!(
            calls[0].arguments,
            json!({
                "text": "Sam apologized",
                "category": "plot_event",
                "important": true
            })
        );
        assert_eq!(calls[1].name, "done");
    }

    #[test]
    fn parses_repairs_from_json() {
        let raw = r#"{"items":[{"text":"Likes tea","category":"preference"}]}"#;

        let repaired = parse_memory_tag_repairs_from_text(
            raw,
            &["preference", "other"],
            DynamicMemoryStructuredFallbackFormat::Json,
        )
        .expect("should parse repairs");

        assert_eq!(repaired.get("Likes tea"), Some(&"preference".to_string()));
    }
}
