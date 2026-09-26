use quick_xml::{
    Reader,
    escape::{resolve_xml_entity, unescape},
    events::{BytesRef, Event},
};
use serde_json::{Map, Value};

use crate::lorebook_entry::{json_snippet, normalize_fallback_text, xml_attribute};

/// The structured text protocol a creation turn uses in place of native tool
/// calling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreationFallbackFormat {
    Json,
    Xml,
}

/// One tool call read from a structured fallback reply, in reply order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreationFallbackCall {
    pub name: String,
    pub arguments: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CreationFallbackReply {
    pub calls: Vec<CreationFallbackCall>,
    /// The text of a `reply` call, which is shown to the user instead of the
    /// raw envelope.
    pub reply: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CreationFallbackError {
    #[error("creation fallback reply is not a valid envelope")]
    InvalidEnvelope,
    #[error("creation fallback call has no name")]
    MissingName,
}

/// Reads the tool calls of a structured fallback reply. A `reply` call becomes
/// the visible text and is not returned as a call.
pub fn parse_creation_fallback(
    format: CreationFallbackFormat,
    raw: &str,
) -> Result<CreationFallbackReply, CreationFallbackError> {
    match format {
        CreationFallbackFormat::Json => parse_json(raw),
        CreationFallbackFormat::Xml => parse_xml(raw),
    }
}

fn parse_json(raw: &str) -> Result<CreationFallbackReply, CreationFallbackError> {
    let normalized = normalize_fallback_text(raw);
    let value: Value = serde_json::from_str(json_snippet(&normalized).unwrap_or(&normalized))
        .map_err(|_| CreationFallbackError::InvalidEnvelope)?;
    let entries = match value {
        Value::Array(items) => items,
        Value::Object(map) => map
            .get("calls")
            .or_else(|| map.get("operations"))
            .or_else(|| map.get("toolCalls"))
            .and_then(Value::as_array)
            .cloned()
            .ok_or(CreationFallbackError::InvalidEnvelope)?,
        _ => return Err(CreationFallbackError::InvalidEnvelope),
    };
    let mut parsed = CreationFallbackReply::default();
    for item in entries {
        let Value::Object(object) = item else {
            continue;
        };
        let name = object
            .get("name")
            .or_else(|| object.get("tool"))
            .or_else(|| object.get("verb"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .ok_or(CreationFallbackError::MissingName)?
            .to_owned();
        let arguments = object.get("arguments").cloned().unwrap_or_else(|| {
            Value::Object(
                object
                    .iter()
                    .filter(|(key, _)| !matches!(key.as_str(), "name" | "tool" | "verb"))
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect(),
            )
        });
        if name.eq_ignore_ascii_case("reply") {
            if let Some(message) = arguments
                .get("message")
                .or_else(|| arguments.get("text"))
                .and_then(Value::as_str)
            {
                parsed.reply = Some(message.to_owned());
            }
            continue;
        }
        parsed.calls.push(CreationFallbackCall { name, arguments });
    }
    Ok(parsed)
}

fn xml_reference(reference: BytesRef<'_>) -> Result<String, CreationFallbackError> {
    if let Ok(Some(character)) = reference.resolve_char_ref() {
        return Ok(character.to_string());
    }
    let content = reference
        .xml_content()
        .map_err(|_| CreationFallbackError::InvalidEnvelope)?;
    Ok(resolve_xml_entity(&content).map_or_else(|| format!("&{content};"), ToOwned::to_owned))
}

fn commit_xml_call(
    parsed: &mut CreationFallbackReply,
    name: Option<String>,
    arguments: Map<String, Value>,
) {
    let Some(name) = name else {
        return;
    };
    if name.eq_ignore_ascii_case("reply") {
        if let Some(Value::String(message)) = arguments.get("message") {
            parsed.reply = Some(message.clone());
        }
        return;
    }
    parsed.calls.push(CreationFallbackCall {
        name,
        arguments: Value::Object(arguments),
    });
}

fn parse_xml(raw: &str) -> Result<CreationFallbackReply, CreationFallbackError> {
    let normalized = normalize_fallback_text(raw);
    let mut reader = Reader::from_str(&normalized);
    reader.config_mut().trim_text(false);
    let mut buffer = Vec::new();
    let mut root_seen = false;
    let mut in_call = false;
    let mut call_name = None;
    let mut arguments = Map::new();
    let mut argument_key: Option<String> = None;
    let mut argument_text = String::new();
    let mut parsed = CreationFallbackReply::default();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                let tag = String::from_utf8_lossy(event.name().as_ref()).into_owned();
                if !root_seen && matches!(tag.as_str(), "calls" | "operations") {
                    root_seen = true;
                } else if root_seen && !in_call && tag == "call" {
                    in_call = true;
                    call_name = xml_attribute(&event, b"name");
                    arguments = Map::new();
                } else if in_call && tag == "arg" {
                    argument_key = xml_attribute(&event, b"name");
                    argument_text.clear();
                }
            }
            Ok(Event::Empty(event)) => {
                let tag = String::from_utf8_lossy(event.name().as_ref()).into_owned();
                if !root_seen && matches!(tag.as_str(), "calls" | "operations") {
                    root_seen = true;
                } else if root_seen && !in_call && tag == "call" {
                    commit_xml_call(&mut parsed, xml_attribute(&event, b"name"), Map::new());
                }
            }
            Ok(Event::Text(event)) => {
                if argument_key.is_some() {
                    argument_text.push_str(
                        &unescape(&String::from_utf8_lossy(event.as_ref()))
                            .map_err(|_| CreationFallbackError::InvalidEnvelope)?,
                    );
                }
            }
            Ok(Event::CData(event)) => {
                if argument_key.is_some() {
                    argument_text.push_str(&String::from_utf8_lossy(event.as_ref()));
                }
            }
            Ok(Event::GeneralRef(reference)) => {
                if argument_key.is_some() {
                    argument_text.push_str(&xml_reference(reference)?);
                }
            }
            Ok(Event::End(event)) => {
                let tag = String::from_utf8_lossy(event.name().as_ref()).into_owned();
                if tag == "arg" {
                    if let Some(key) = argument_key.take() {
                        arguments.insert(key, Value::String(argument_text.trim().to_owned()));
                        argument_text.clear();
                    }
                } else if tag == "call" {
                    in_call = false;
                    commit_xml_call(
                        &mut parsed,
                        call_name.take(),
                        std::mem::take(&mut arguments),
                    );
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => return Err(CreationFallbackError::InvalidEnvelope),
            _ => {}
        }
        buffer.clear();
    }
    if !root_seen {
        return Err(CreationFallbackError::InvalidEnvelope);
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        CreationFallbackCall, CreationFallbackError, CreationFallbackFormat,
        parse_creation_fallback,
    };

    #[test]
    fn json_envelope_reads_calls_aliases_and_the_reply_like_legacy() {
        let parsed = parse_creation_fallback(
            CreationFallbackFormat::Json,
            "```json\n{\"calls\":[{\"name\":\"set_name\",\"arguments\":{\"name\":\"Ada\"}},\
             {\"tool\":\"write_definition\",\"definition\":\"Maps paths.\"},\
             {\"name\":\"reply\",\"arguments\":{\"message\":\"Named her.\"}}]}\n```",
        )
        .expect("json envelope");
        assert_eq!(
            parsed.calls,
            [
                CreationFallbackCall {
                    name: "set_name".into(),
                    arguments: json!({"name": "Ada"}),
                },
                CreationFallbackCall {
                    name: "write_definition".into(),
                    arguments: json!({"definition": "Maps paths."}),
                },
            ]
        );
        assert_eq!(parsed.reply.as_deref(), Some("Named her."));
        assert_eq!(
            parse_creation_fallback(CreationFallbackFormat::Json, "[{\"arguments\":{}}]"),
            Err(CreationFallbackError::MissingName)
        );
        assert_eq!(
            parse_creation_fallback(CreationFallbackFormat::Json, "All set!"),
            Err(CreationFallbackError::InvalidEnvelope)
        );
    }

    #[test]
    fn xml_envelope_reads_trimmed_args_entities_and_the_reply_like_legacy() {
        let parsed = parse_creation_fallback(
            CreationFallbackFormat::Xml,
            "<calls>\n  <call name=\"set_name\"><arg name=\"name\"> Ada &amp; Bo </arg></call>\n  \
             <call name=\"show_preview\"/>\n  \
             <call name=\"reply\"><arg name=\"message\">Done.</arg></call>\n</calls>",
        )
        .expect("xml envelope");
        assert_eq!(
            parsed.calls,
            [
                CreationFallbackCall {
                    name: "set_name".into(),
                    arguments: json!({"name": "Ada & Bo"}),
                },
                CreationFallbackCall {
                    name: "show_preview".into(),
                    arguments: json!({}),
                },
            ]
        );
        assert_eq!(parsed.reply.as_deref(), Some("Done."));
        assert_eq!(
            parse_creation_fallback(CreationFallbackFormat::Xml, "<reply>hi</reply>"),
            Err(CreationFallbackError::InvalidEnvelope)
        );
    }
}
