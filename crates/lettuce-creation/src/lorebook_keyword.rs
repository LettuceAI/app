use std::collections::HashSet;

use lettuce_conversations::{ProposedToolCall, ToolChoice, ToolDefinition, ToolRequest};
use quick_xml::{
    Reader,
    escape::{resolve_xml_entity, unescape},
    events::{BytesRef, Event},
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::{LorebookEntryFallbackFormat, MAX_GENERATED_LOREBOOK_KEYWORDS};

pub const LOREBOOK_KEYWORD_WRITE_TOOL_NAME: &str = "write_lorebook_keywords";
pub const LOREBOOK_KEYWORD_FINAL_INSTRUCTION: &str = "Analyze the lorebook entry content and return exactly one result now. You MUST call write_lorebook_keywords with a concise, deduplicated keyword list.";
pub const LOREBOOK_KEYWORD_JSON_FALLBACK_PROMPT: &str = r#"Return only JSON. Format: {"result":{"name":"write_lorebook_keywords","arguments":{"keywords":["..."]}}}. You MUST return write_lorebook_keywords. Do not use markdown."#;
pub const LOREBOOK_KEYWORD_XML_FALLBACK_PROMPT: &str = r#"Return only XML. Format: <lorebook_result><write_lorebook_keywords><keywords><keyword>...</keyword></keywords></write_lorebook_keywords></lorebook_result>. You MUST return write_lorebook_keywords. Do not use markdown."#;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LorebookKeywordDraft {
    pub keywords: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LorebookKeywordGenerationError {
    #[error("lorebook keyword fallback is invalid")]
    InvalidFallback,
    #[error("lorebook keyword fallback returned an undeclared operation")]
    UndeclaredFallbackOperation,
}

#[must_use]
pub fn lorebook_keyword_tool_request() -> ToolRequest {
    ToolRequest {
        definitions: vec![ToolDefinition {
            name: LOREBOOK_KEYWORD_WRITE_TOOL_NAME.to_owned(),
            description: Some(
                "Generate one deduplicated keyword list for the lorebook entry draft.".into(),
            ),
            parameters: json!({
                "type": "object",
                "properties": {
                    "keywords": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Trigger keywords, aliases, names, locations, and other durable lookup terms"
                    }
                },
                "required": ["keywords"]
            }),
            version: 1,
        }],
        choice: ToolChoice::Required,
    }
}

#[must_use]
pub const fn lorebook_keyword_fallback_prompt(format: LorebookEntryFallbackFormat) -> &'static str {
    match format {
        LorebookEntryFallbackFormat::Json => LOREBOOK_KEYWORD_JSON_FALLBACK_PROMPT,
        LorebookEntryFallbackFormat::Xml => LOREBOOK_KEYWORD_XML_FALLBACK_PROMPT,
    }
}

#[must_use]
pub fn reduce_lorebook_keyword_calls(calls: &[ProposedToolCall]) -> Option<LorebookKeywordDraft> {
    calls
        .iter()
        .find(|call| call.name == LOREBOOK_KEYWORD_WRITE_TOOL_NAME)
        .map(|call| LorebookKeywordDraft {
            keywords: normalize_keywords(call.arguments.get("keywords")),
        })
}

pub fn parse_lorebook_keyword_fallback(
    raw: &str,
    format: LorebookEntryFallbackFormat,
) -> Result<LorebookKeywordDraft, LorebookKeywordGenerationError> {
    match format {
        LorebookEntryFallbackFormat::Json => parse_json_fallback(raw),
        LorebookEntryFallbackFormat::Xml => parse_xml_fallback(raw),
    }
}

fn normalize_keywords(value: Option<&Value>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut result = match value {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .filter_map(|value| {
                let normalized = value.to_owned();
                let key = normalized.to_ascii_lowercase();
                seen.insert(key).then_some(normalized)
            })
            .collect::<Vec<_>>(),
        Some(Value::String(value)) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                Vec::new()
            } else {
                vec![trimmed.to_owned()]
            }
        }
        _ => Vec::new(),
    };
    result.truncate(MAX_GENERATED_LOREBOOK_KEYWORDS);
    result
}

fn parse_json_fallback(raw: &str) -> Result<LorebookKeywordDraft, LorebookKeywordGenerationError> {
    let normalized = normalize_fallback_text(raw);
    let value: Value = serde_json::from_str(json_snippet(&normalized).unwrap_or(&normalized))
        .map_err(|_| LorebookKeywordGenerationError::InvalidFallback)?;
    let node = value
        .get("result")
        .or_else(|| value.get("response"))
        .unwrap_or(&value);
    let object = node
        .as_object()
        .ok_or(LorebookKeywordGenerationError::InvalidFallback)?;
    let name = object
        .get("name")
        .or_else(|| object.get("tool"))
        .or_else(|| object.get("action"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(LorebookKeywordGenerationError::InvalidFallback)?;
    if name != LOREBOOK_KEYWORD_WRITE_TOOL_NAME {
        return Err(LorebookKeywordGenerationError::UndeclaredFallbackOperation);
    }
    let arguments = object
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| Value::Object(Map::new()));
    Ok(LorebookKeywordDraft {
        keywords: normalize_keywords(arguments.get("keywords")),
    })
}

fn parse_xml_fallback(raw: &str) -> Result<LorebookKeywordDraft, LorebookKeywordGenerationError> {
    let normalized = normalize_fallback_text(raw);
    let mut reader = Reader::from_str(&normalized);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut operation = None;
    let mut field = None;
    let mut keyword = String::new();
    let mut keywords = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                let tag = String::from_utf8_lossy(event.name().as_ref()).into_owned();
                if operation.is_none() && tag == LOREBOOK_KEYWORD_WRITE_TOOL_NAME {
                    operation = Some(tag);
                } else if operation.is_some() {
                    field = Some(tag);
                }
            }
            Ok(Event::Text(event)) if field.as_deref() == Some("keyword") => {
                keyword.push_str(&xml_text(event.as_ref())?);
            }
            Ok(Event::GeneralRef(reference)) if field.as_deref() == Some("keyword") => {
                keyword.push_str(&xml_reference(reference)?);
            }
            Ok(Event::End(event)) => {
                let tag = String::from_utf8_lossy(event.name().as_ref()).into_owned();
                if tag == "keyword" {
                    if !keyword.trim().is_empty() {
                        keywords.push(keyword.trim().to_owned());
                    }
                    keyword.clear();
                    field = None;
                } else if matches!(tag.as_str(), "keywords" | LOREBOOK_KEYWORD_WRITE_TOOL_NAME) {
                    field = None;
                    if tag == LOREBOOK_KEYWORD_WRITE_TOOL_NAME {
                        break;
                    }
                } else {
                    field = None;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => return Err(LorebookKeywordGenerationError::InvalidFallback),
            _ => {}
        }
        buffer.clear();
    }
    if operation.as_deref() != Some(LOREBOOK_KEYWORD_WRITE_TOOL_NAME) {
        return Err(LorebookKeywordGenerationError::InvalidFallback);
    }
    Ok(LorebookKeywordDraft {
        keywords: normalize_keywords(Some(&json!(keywords))),
    })
}

fn normalize_fallback_text(raw: &str) -> String {
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

fn xml_text(raw: &[u8]) -> Result<String, LorebookKeywordGenerationError> {
    unescape(&String::from_utf8_lossy(raw))
        .map(|value| value.into_owned())
        .map_err(|_| LorebookKeywordGenerationError::InvalidFallback)
}

fn xml_reference(reference: BytesRef<'_>) -> Result<String, LorebookKeywordGenerationError> {
    if let Ok(Some(character)) = reference.resolve_char_ref() {
        return Ok(character.to_string());
    }
    let content = reference
        .xml_content()
        .map_err(|_| LorebookKeywordGenerationError::InvalidFallback)?;
    Ok(resolve_xml_entity(&content).map_or_else(|| format!("&{content};"), ToOwned::to_owned))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(name: &str, arguments: Value) -> ProposedToolCall {
        ProposedToolCall {
            provider_call_id: None,
            name: name.into(),
            arguments,
            raw_arguments: None,
            provider_replay: None,
        }
    }

    #[test]
    fn required_tool_contract_matches_legacy() {
        let request = lorebook_keyword_tool_request();
        assert_eq!(request.choice, ToolChoice::Required);
        assert_eq!(request.definitions.len(), 1);
        assert_eq!(
            request.definitions[0].name,
            LOREBOOK_KEYWORD_WRITE_TOOL_NAME
        );
        assert_eq!(
            request.definitions[0].parameters["required"],
            json!(["keywords"])
        );
    }

    #[test]
    fn first_writer_call_normalizes_and_caps_keywords() {
        let mut keywords = vec![json!(" Mira "), json!("mira"), json!(""), json!(7)];
        keywords.extend((0..30).map(|index| json!(format!("key-{index}"))));
        let result = reduce_lorebook_keyword_calls(&[
            call("ignored", json!({})),
            call(
                LOREBOOK_KEYWORD_WRITE_TOOL_NAME,
                json!({"keywords": keywords}),
            ),
            call(
                LOREBOOK_KEYWORD_WRITE_TOOL_NAME,
                json!({"keywords": ["later"]}),
            ),
        ])
        .expect("writer result");
        assert_eq!(result.keywords.len(), MAX_GENERATED_LOREBOOK_KEYWORDS);
        assert_eq!(result.keywords[0], "Mira");
        assert!(!result.keywords.iter().any(|keyword| keyword == "later"));
    }

    #[test]
    fn json_and_xml_fallbacks_copy_legacy_normalization() {
        assert_eq!(
            parse_lorebook_keyword_fallback(
                "```json\n{\"response\":{\"tool\":\"write_lorebook_keywords\",\"arguments\":{\"keywords\":[\" Port \",\"port\",\"Mira\"]}}}\n```",
                LorebookEntryFallbackFormat::Json,
            )
            .expect("json fallback")
            .keywords,
            ["Port", "Mira"]
        );
        assert_eq!(
            parse_lorebook_keyword_fallback(
                "<lorebook_result><write_lorebook_keywords><keywords><keyword>Coast &amp; Sea</keyword><keyword>coast &amp; sea</keyword></keywords></write_lorebook_keywords></lorebook_result>",
                LorebookEntryFallbackFormat::Xml,
            )
            .expect("xml fallback")
            .keywords,
            ["Coast&Sea"]
        );
    }

    #[test]
    fn undeclared_fallback_is_rejected() {
        assert_eq!(
            parse_lorebook_keyword_fallback(
                r#"{"result":{"name":"no_entry","arguments":{}}}"#,
                LorebookEntryFallbackFormat::Json,
            ),
            Err(LorebookKeywordGenerationError::UndeclaredFallbackOperation)
        );
    }
}
