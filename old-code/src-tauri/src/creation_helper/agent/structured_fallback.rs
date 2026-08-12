use quick_xml::escape::{resolve_xml_entity, unescape};
use quick_xml::events::{BytesRef, BytesStart, Event};
use quick_xml::Reader;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::chat_manager::tooling::ToolCall;

use super::state::TargetKind;
use super::tool_defs::schema_summary_for_target;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum CreationHelperFallbackFormat {
    #[default]
    Native,
    Json,
    Xml,
}

impl CreationHelperFallbackFormat {
    pub fn from_setting(value: Option<&str>) -> Self {
        match value.map(str::to_ascii_lowercase).as_deref() {
            Some("json") => CreationHelperFallbackFormat::Json,
            Some("xml") => CreationHelperFallbackFormat::Xml,
            _ => CreationHelperFallbackFormat::Native,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            CreationHelperFallbackFormat::Native => "native",
            CreationHelperFallbackFormat::Json => "json",
            CreationHelperFallbackFormat::Xml => "xml",
        }
    }

    pub fn is_native(self) -> bool {
        matches!(self, CreationHelperFallbackFormat::Native)
    }
}

pub fn fallback_system_prompt(format: CreationHelperFallbackFormat, target: TargetKind) -> String {
    match format {
        CreationHelperFallbackFormat::Native => String::new(),
        CreationHelperFallbackFormat::Json => {
            let mut s = String::new();
            s.push_str(
                "Your model does not support native tool calling. Return ONE JSON object \
                 wrapping the tool calls you want to execute on this turn. Schema:\n\n\
                 {\"calls\":[{\"name\":\"tool_name\",\"arguments\":{...}}, ...]}\n\n\
                 Use {\"calls\":[]} when you only want to send a plain message back to the \
                 user with no tool actions. Do not wrap in markdown. No commentary outside \
                 the JSON. The user-facing message (if any) goes in a final \
                 {\"name\":\"reply\",\"arguments\":{\"message\":\"...\"}} entry.\n\n",
            );
            s.push_str(&schema_summary_for_target(target));
            s
        }
        CreationHelperFallbackFormat::Xml => {
            let mut s = String::new();
            s.push_str(
                "Your model does not support native tool calling. Return ONE XML envelope \
                 wrapping the tool calls. Schema:\n\n\
                 <calls>\n  <call name=\"tool_name\"><arg name=\"key\">value</arg></call>\n  ...\n</calls>\n\n\
                 Use an empty <calls/> when no tools are needed. Do not use markdown. To \
                 send a plain message to the user, emit \
                 <call name=\"reply\"><arg name=\"message\">...</arg></call>.\n\n",
            );
            s.push_str(&schema_summary_for_target(target));
            s
        }
    }
}

fn normalize(raw: &str) -> String {
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

#[derive(Clone, Debug, Default)]
pub struct ParsedFallback {
    pub calls: Vec<ToolCall>,
    pub reply: Option<String>,
}

pub fn parse(format: CreationHelperFallbackFormat, raw: &str) -> Result<ParsedFallback, String> {
    match format {
        CreationHelperFallbackFormat::Native => Err("native format has no fallback parse".into()),
        CreationHelperFallbackFormat::Json => parse_json(raw),
        CreationHelperFallbackFormat::Xml => parse_xml(raw),
    }
}

fn parse_json(raw: &str) -> Result<ParsedFallback, String> {
    let normalized = normalize(raw);
    let snippet = extract_json_snippet(&normalized).unwrap_or(normalized.as_str());
    let value: Value = serde_json::from_str(snippet)
        .map_err(|err| format!("fallback JSON parse error: {}", err))?;

    let entries = match &value {
        Value::Array(items) => items.clone(),
        Value::Object(map) => map
            .get("calls")
            .or_else(|| map.get("operations"))
            .or_else(|| map.get("toolCalls"))
            .and_then(Value::as_array)
            .cloned()
            .ok_or_else(|| "fallback JSON missing 'calls' array".to_string())?,
        _ => return Err("fallback JSON must be an object or array".to_string()),
    };

    let mut out = ParsedFallback::default();
    for (i, item) in entries.into_iter().enumerate() {
        let Some(obj) = item.as_object().cloned() else {
            continue;
        };
        let name = obj
            .get("name")
            .or_else(|| obj.get("tool"))
            .or_else(|| obj.get("verb"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| format!("fallback JSON entry {} missing name", i + 1))?
            .to_string();

        let arguments = match obj.get("arguments").cloned() {
            Some(Value::Object(args)) => Value::Object(args),
            Some(other) => other,
            None => {
                let mut args = Map::new();
                for (k, v) in obj.iter() {
                    if matches!(k.as_str(), "name" | "tool" | "verb") {
                        continue;
                    }
                    args.insert(k.clone(), v.clone());
                }
                Value::Object(args)
            }
        };

        if name.eq_ignore_ascii_case("reply") {
            if let Some(msg) = arguments
                .get("message")
                .or_else(|| arguments.get("text"))
                .and_then(Value::as_str)
            {
                out.reply = Some(msg.to_string());
            }
            continue;
        }

        out.calls.push(ToolCall {
            id: format!("fallback_{}", i + 1),
            name,
            arguments,
            raw_arguments: None,
            thought_signature: None,
        });
    }
    Ok(out)
}

fn attr_value(element: &BytesStart<'_>, key: &[u8]) -> Option<String> {
    for attr in element.attributes().flatten() {
        if attr.key.as_ref() == key {
            return attr.unescape_value().ok().map(|v| v.into_owned());
        }
    }
    None
}

fn decode_text(raw: &[u8]) -> Result<String, String> {
    let text = String::from_utf8_lossy(raw);
    unescape(&text)
        .map(|cow| cow.into_owned())
        .map_err(|err| format!("fallback XML text decode: {}", err))
}

fn decode_general_ref(raw: BytesRef<'_>) -> Result<String, String> {
    if let Ok(Some(ch)) = raw.resolve_char_ref() {
        return Ok(ch.to_string());
    }
    let content = raw
        .xml_content()
        .map_err(|err| format!("fallback XML ref decode: {}", err))?;
    if let Some(entity) = resolve_xml_entity(&content) {
        Ok(entity.to_string())
    } else {
        Ok(format!("&{};", content))
    }
}

fn parse_xml(raw: &str) -> Result<ParsedFallback, String> {
    let normalized = normalize(raw);
    let mut reader = Reader::from_str(&normalized);
    reader.config_mut().trim_text(false);

    let mut buf = Vec::new();
    let mut root_seen = false;
    let mut in_call = false;
    let mut call_name: Option<String> = None;
    let mut current_args: Map<String, Value> = Map::new();
    let mut arg_key: Option<String> = None;
    let mut arg_buf = String::new();
    let mut out = ParsedFallback::default();
    let mut idx = 0usize;

    let commit_call = |out: &mut ParsedFallback,
                       idx: &mut usize,
                       name: Option<String>,
                       args: Map<String, Value>| {
        if let Some(n) = name {
            if n.eq_ignore_ascii_case("reply") {
                if let Some(Value::String(msg)) = args.get("message").cloned() {
                    out.reply = Some(msg);
                }
            } else {
                *idx += 1;
                out.calls.push(ToolCall {
                    id: format!("fallback_{}", *idx),
                    name: n,
                    arguments: Value::Object(args),
                    raw_arguments: None,
                    thought_signature: None,
                });
            }
        }
    };

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(event)) => {
                let tag = String::from_utf8_lossy(event.name().as_ref()).into_owned();
                if !root_seen && (tag == "calls" || tag == "operations") {
                    root_seen = true;
                } else if root_seen && !in_call && tag == "call" {
                    in_call = true;
                    call_name = attr_value(&event, b"name");
                    current_args = Map::new();
                } else if in_call && tag == "arg" {
                    arg_key = attr_value(&event, b"name");
                    arg_buf.clear();
                }
            }
            Ok(Event::Empty(event)) => {
                let tag = String::from_utf8_lossy(event.name().as_ref()).into_owned();
                if !root_seen && (tag == "calls" || tag == "operations") {
                    root_seen = true;
                } else if root_seen && !in_call && tag == "call" {
                    let name = attr_value(&event, b"name");
                    commit_call(&mut out, &mut idx, name, Map::new());
                }
            }
            Ok(Event::Text(event)) => {
                if arg_key.is_some() {
                    arg_buf.push_str(&decode_text(event.as_ref())?);
                }
            }
            Ok(Event::CData(event)) => {
                if arg_key.is_some() {
                    arg_buf.push_str(&String::from_utf8_lossy(event.as_ref()));
                }
            }
            Ok(Event::GeneralRef(event)) => {
                if arg_key.is_some() {
                    arg_buf.push_str(&decode_general_ref(event)?);
                }
            }
            Ok(Event::End(event)) => {
                let tag = String::from_utf8_lossy(event.name().as_ref()).into_owned();
                if tag == "arg" {
                    if let Some(key) = arg_key.take() {
                        current_args.insert(key, Value::String(arg_buf.trim().to_string()));
                        arg_buf.clear();
                    }
                } else if tag == "call" {
                    in_call = false;
                    let name = call_name.take();
                    let args = std::mem::take(&mut current_args);
                    commit_call(&mut out, &mut idx, name, args);
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(format!("fallback XML parse error: {}", err)),
            _ => {}
        }
        buf.clear();
    }

    if !root_seen {
        return Err("fallback response did not contain <calls>...</calls>".into());
    }
    Ok(out)
}
