use std::collections::HashSet;

use lettuce_conversations::{
    ProposedToolCall, ResolvedInferenceProfile, ToolChoice, ToolDefinition, ToolRequest,
    ValidationError,
};
use lettuce_types::{
    CharacterId, ConversationId, JobId, LorebookId, MemoryId, MessageId, PersonaId,
    PromptDocumentId, RequestId, Revision, TimestampMillis,
};
use quick_xml::{
    Reader,
    escape::{resolve_xml_entity, unescape},
    events::{BytesRef, BytesStart, Event},
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

pub const LOREBOOK_ENTRY_WRITE_TOOL_NAME: &str = "write_lorebook_entry";
pub const LOREBOOK_ENTRY_NONE_TOOL_NAME: &str = "no_entry";
pub const MAX_GENERATED_LOREBOOK_KEYWORDS: usize = 24;
pub const DEFAULT_NO_LOREBOOK_ENTRY_REASON: &str =
    "The selected messages do not establish durable lore.";
pub const LOREBOOK_ENTRY_MESSAGES_INSTRUCTION: &str = "Analyze the selected transcript and return exactly one result now. Use the write_lorebook_entry tool when there is a durable lorebook entry to create. Use no_entry when there is not.";
pub const LOREBOOK_ENTRY_MESSAGES_FORCE_INSTRUCTION: &str = "Analyze the selected transcript and return exactly one result now. You MUST call write_lorebook_entry. The no_entry option is disabled — produce the best possible durable lorebook entry even if facts seem weak or already covered.";
pub const LOREBOOK_ENTRY_MEMORY_INSTRUCTION: &str = "Analyze the dynamic memory context summary and the selected memories, then return exactly one result now. Use the write_lorebook_entry tool when there is a durable lorebook entry to create. Use no_entry when there is not.";
pub const LOREBOOK_ENTRY_MEMORY_FORCE_INSTRUCTION: &str = "Analyze the dynamic memory context summary and the selected memories, then return exactly one result now. You MUST call write_lorebook_entry. The no_entry option is disabled — produce the best possible durable lorebook entry even if the memories seem weak or already covered.";
pub const LOREBOOK_ENTRY_MIXED_INSTRUCTION: &str = "Analyze every provided input section that is not marked (none) — selected messages, dynamic memory context summary, and selected memories — and return exactly one result now. Use the write_lorebook_entry tool when there is a durable lorebook entry to create. Use no_entry when there is not.";
pub const LOREBOOK_ENTRY_MIXED_FORCE_INSTRUCTION: &str = "Analyze every provided input section that is not marked (none) — selected messages, dynamic memory context summary, and selected memories — and return exactly one result now. You MUST call write_lorebook_entry. The no_entry option is disabled — produce the best possible durable lorebook entry even if facts seem weak or already covered.";

pub const LOREBOOK_ENTRY_JSON_FALLBACK_PROMPT: &str = r#"Return only JSON. Format: {"result":{"name":"write_lorebook_entry","arguments":{"title":"...","keywords":["..."],"content":"...","alwaysActive":false}}}. If no durable entry should be created, return {"result":{"name":"no_entry","arguments":{"reason":"..."}}}. Do not use markdown."#;
pub const LOREBOOK_ENTRY_XML_FALLBACK_PROMPT: &str = r#"Return only XML. Format: <lorebook_result><write_lorebook_entry alwaysActive="false"><title>...</title><keywords><keyword>...</keyword></keywords><content>...</content></write_lorebook_entry></lorebook_result>. If no durable entry should be created, return <lorebook_result><no_entry><reason>...</reason></no_entry></lorebook_result>. Do not use markdown."#;
pub const LOREBOOK_ENTRY_JSON_FORCE_FALLBACK_PROMPT: &str = r#"Return only JSON. Format: {"result":{"name":"write_lorebook_entry","arguments":{"title":"...","keywords":["..."],"content":"...","alwaysActive":false}}}. You MUST return write_lorebook_entry. The no_entry option is disabled. Do not use markdown."#;
pub const LOREBOOK_ENTRY_XML_FORCE_FALLBACK_PROMPT: &str = r#"Return only XML. Format: <lorebook_result><write_lorebook_entry alwaysActive="false"><title>...</title><keywords><keyword>...</keyword></keywords><content>...</content></write_lorebook_entry></lorebook_result>. You MUST return write_lorebook_entry. The no_entry option is disabled. Do not use markdown."#;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LorebookEntryFallbackFormat {
    Json,
    Xml,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LorebookEntrySource {
    Messages,
    Memory,
    Mixed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LorebookEntryPromptValues {
    pub lorebook_name: String,
    pub character_name: String,
    pub session_title: String,
    pub existing_entries: String,
    pub direction_prompt: String,
    pub selected_messages: String,
    pub memory_summary: String,
    pub selected_memories: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LorebookEntryGenerationRun {
    pub request_id: RequestId,
    pub job_id: JobId,
    pub conversation_id: ConversationId,
    pub lorebook_id: LorebookId,
    pub character_id: CharacterId,
    pub persona_id: Option<PersonaId>,
    pub selected_message_ids: Vec<MessageId>,
    pub selected_memory_ids: Vec<MemoryId>,
    pub source: LorebookEntrySource,
    pub include_memory_summary: bool,
    pub time_awareness_enabled: bool,
    pub force: bool,
    pub profile: ResolvedInferenceProfile,
    pub prompt_id: PromptDocumentId,
    pub prompt_revision: Revision,
    pub prompt_values: LorebookEntryPromptValues,
    pub fallback_format: LorebookEntryFallbackFormat,
    pub created_at: TimestampMillis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LorebookEntryAttemptKind {
    Native,
    StructuredFallback,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum LorebookEntryAttemptDecision {
    Result(LorebookEntryGenerationResult),
    StructuredFallback,
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LorebookEntryAttemptUsage {
    #[serde(default)]
    pub cached_input_tokens: Option<u64>,
    #[serde(default)]
    pub reasoning_tokens: Option<u64>,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LorebookEntryAttemptCheckpoint {
    pub ordinal: u8,
    pub attempt_kind: LorebookEntryAttemptKind,
    pub calls: Vec<ProposedToolCall>,
    pub decision: LorebookEntryAttemptDecision,
    pub usage: Option<LorebookEntryAttemptUsage>,
    pub provider_finish_reason: Option<String>,
    pub provider_request_id: Option<String>,
    pub completed_at: TimestampMillis,
}

impl LorebookEntryGenerationRun {
    pub fn validate(&self) -> Result<(), LorebookEntryRunRepositoryError> {
        let values = &self.prompt_values;
        if self.prompt_revision.get() == 0
            || self.created_at.get() < 0
            || values.lorebook_name.trim().is_empty()
            || values.character_name.trim().is_empty()
            || values.session_title.trim().is_empty()
            || serde_json::to_vec(&self.profile).is_err()
            || has_duplicates(&self.selected_message_ids)
            || has_duplicates(&self.selected_memory_ids)
        {
            return Err(LorebookEntryRunRepositoryError::Invalid);
        }
        match self.source {
            LorebookEntrySource::Messages if self.selected_message_ids.is_empty() => {
                Err(LorebookEntryRunRepositoryError::Invalid)
            }
            LorebookEntrySource::Memory
                if self.selected_memory_ids.is_empty()
                    && (!self.include_memory_summary
                        || values.memory_summary.trim().is_empty()
                        || values.memory_summary.trim() == "(none)") =>
            {
                Err(LorebookEntryRunRepositoryError::Invalid)
            }
            LorebookEntrySource::Mixed
                if self.selected_message_ids.is_empty()
                    && self.selected_memory_ids.is_empty()
                    && (!self.include_memory_summary
                        || values.memory_summary.trim().is_empty()
                        || values.memory_summary.trim() == "(none)") =>
            {
                Err(LorebookEntryRunRepositoryError::Invalid)
            }
            _ => Ok(()),
        }
    }
}

fn has_duplicates<T: Eq + std::hash::Hash>(values: &[T]) -> bool {
    let mut seen = HashSet::with_capacity(values.len());
    values.iter().any(|value| !seen.insert(value))
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LorebookEntryRunRepositoryError {
    #[error("lorebook entry generation run was not found")]
    NotFound,
    #[error("lorebook entry generation run conflicts with durable state")]
    Conflict,
    #[error("lorebook entry generation run is invalid")]
    Invalid,
    #[error("lorebook entry generation run storage failed")]
    Failure,
    #[error("lorebook entry generation run storage is corrupt")]
    Corrupt,
}

pub trait LorebookEntryRunRepository: Send + Sync {
    fn admit_lorebook_entry_run(
        &self,
        run: LorebookEntryGenerationRun,
    ) -> Result<LorebookEntryGenerationRun, LorebookEntryRunRepositoryError>;

    fn load_lorebook_entry_run(
        &self,
        request_id: RequestId,
    ) -> Result<LorebookEntryGenerationRun, LorebookEntryRunRepositoryError>;

    fn load_lorebook_entry_attempts(
        &self,
        request_id: RequestId,
    ) -> Result<Vec<LorebookEntryAttemptCheckpoint>, LorebookEntryRunRepositoryError>;

    fn commit_lorebook_entry_attempt(
        &self,
        request_id: RequestId,
        checkpoint: LorebookEntryAttemptCheckpoint,
    ) -> Result<Vec<LorebookEntryAttemptCheckpoint>, LorebookEntryRunRepositoryError>;
}

impl LorebookEntryAttemptCheckpoint {
    pub fn validate(&self) -> Result<(), LorebookEntryRunRepositoryError> {
        let expected_kind = match self.ordinal {
            0 => LorebookEntryAttemptKind::Native,
            1 => LorebookEntryAttemptKind::StructuredFallback,
            _ => return Err(LorebookEntryRunRepositoryError::Invalid),
        };
        if self.attempt_kind != expected_kind
            || self.completed_at.get() < 0
            || self.calls.iter().any(|call| {
                call.provider_replay.is_some()
                    || call.validate().is_err()
                    || !matches!(self.attempt_kind, LorebookEntryAttemptKind::Native)
            })
            || (self.attempt_kind == LorebookEntryAttemptKind::StructuredFallback
                && matches!(
                    self.decision,
                    LorebookEntryAttemptDecision::StructuredFallback
                ))
            || !attempt_decision_is_normalized(&self.decision)
        {
            return Err(LorebookEntryRunRepositoryError::Invalid);
        }
        Ok(())
    }
}

fn attempt_decision_is_normalized(decision: &LorebookEntryAttemptDecision) -> bool {
    match decision {
        LorebookEntryAttemptDecision::Result(LorebookEntryGenerationResult::Entry { draft }) => {
            !draft.title.is_empty()
                && draft.title == draft.title.trim()
                && !draft.content.is_empty()
                && draft.content == draft.content.trim()
                && draft.keywords.len() <= MAX_GENERATED_LOREBOOK_KEYWORDS
                && draft
                    .keywords
                    .iter()
                    .all(|keyword| !keyword.is_empty() && keyword == keyword.trim())
                && !has_case_insensitive_duplicates(&draft.keywords)
        }
        LorebookEntryAttemptDecision::Result(LorebookEntryGenerationResult::None { reason }) => {
            !reason.is_empty() && reason == reason.trim()
        }
        LorebookEntryAttemptDecision::StructuredFallback
        | LorebookEntryAttemptDecision::Invalid => true,
    }
}

fn has_case_insensitive_duplicates(values: &[String]) -> bool {
    let mut seen = HashSet::with_capacity(values.len());
    values
        .iter()
        .any(|value| !seen.insert(value.to_ascii_lowercase()))
}

pub fn validate_lorebook_entry_attempts(
    attempts: &[LorebookEntryAttemptCheckpoint],
) -> Result<(), LorebookEntryRunRepositoryError> {
    if attempts.len() > 2 {
        return Err(LorebookEntryRunRepositoryError::Invalid);
    }
    for (index, attempt) in attempts.iter().enumerate() {
        attempt.validate()?;
        if usize::from(attempt.ordinal) != index {
            return Err(LorebookEntryRunRepositoryError::Invalid);
        }
    }
    if attempts.len() == 2
        && !matches!(
            attempts[0].decision,
            LorebookEntryAttemptDecision::StructuredFallback
        )
    {
        return Err(LorebookEntryRunRepositoryError::Invalid);
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedLorebookEntryDraft {
    pub title: String,
    pub keywords: Vec<String>,
    pub content: String,
    pub always_active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum LorebookEntryGenerationResult {
    Entry { draft: GeneratedLorebookEntryDraft },
    None { reason: String },
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum LorebookEntryGenerationError {
    #[error("write_lorebook_entry arguments must be an object")]
    ArgumentsNotObject,
    #[error("write_lorebook_entry is missing a non-empty title")]
    MissingTitle,
    #[error("write_lorebook_entry is missing non-empty content")]
    MissingContent,
    #[error("lorebook entry fallback is invalid")]
    InvalidFallback,
    #[error("lorebook entry fallback returned an undeclared operation")]
    UndeclaredFallbackOperation,
}

#[must_use]
pub fn lorebook_entry_tool_request(force: bool) -> ToolRequest {
    let mut definitions = vec![ToolDefinition {
        name: LOREBOOK_ENTRY_WRITE_TOOL_NAME.to_owned(),
        description: Some("Create one lorebook entry draft from the selected transcript.".into()),
        parameters: json!({
            "type": "object",
            "properties": {
                "title": { "type": "string", "description": "Short entry title" },
                "keywords": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Trigger keywords for this entry"
                },
                "content": { "type": "string", "description": "Final lorebook entry content" },
                "alwaysActive": { "type": "boolean", "description": "If true, entry should not require keywords" }
            },
            "required": ["title", "content"]
        }),
        version: 1,
    }];
    if !force {
        definitions.push(ToolDefinition {
            name: LOREBOOK_ENTRY_NONE_TOOL_NAME.to_owned(),
            description: Some(
                "Use this when the selected messages do not justify a durable lorebook entry."
                    .into(),
            ),
            parameters: json!({
                "type": "object",
                "properties": {
                    "reason": { "type": "string", "description": "Short explanation for why no entry should be created" }
                },
                "required": ["reason"]
            }),
            version: 1,
        });
    }
    ToolRequest {
        definitions,
        choice: ToolChoice::Required,
    }
}

#[must_use]
pub const fn lorebook_entry_fallback_prompt(
    format: LorebookEntryFallbackFormat,
    force: bool,
) -> &'static str {
    match (format, force) {
        (LorebookEntryFallbackFormat::Json, false) => LOREBOOK_ENTRY_JSON_FALLBACK_PROMPT,
        (LorebookEntryFallbackFormat::Xml, false) => LOREBOOK_ENTRY_XML_FALLBACK_PROMPT,
        (LorebookEntryFallbackFormat::Json, true) => LOREBOOK_ENTRY_JSON_FORCE_FALLBACK_PROMPT,
        (LorebookEntryFallbackFormat::Xml, true) => LOREBOOK_ENTRY_XML_FORCE_FALLBACK_PROMPT,
    }
}

#[must_use]
pub const fn lorebook_entry_final_instruction(
    source: LorebookEntrySource,
    force: bool,
) -> &'static str {
    match (source, force) {
        (LorebookEntrySource::Messages, false) => LOREBOOK_ENTRY_MESSAGES_INSTRUCTION,
        (LorebookEntrySource::Messages, true) => LOREBOOK_ENTRY_MESSAGES_FORCE_INSTRUCTION,
        (LorebookEntrySource::Memory, false) => LOREBOOK_ENTRY_MEMORY_INSTRUCTION,
        (LorebookEntrySource::Memory, true) => LOREBOOK_ENTRY_MEMORY_FORCE_INSTRUCTION,
        (LorebookEntrySource::Mixed, false) => LOREBOOK_ENTRY_MIXED_INSTRUCTION,
        (LorebookEntrySource::Mixed, true) => LOREBOOK_ENTRY_MIXED_FORCE_INSTRUCTION,
    }
}

pub fn reduce_lorebook_entry_calls(
    calls: &[ProposedToolCall],
    force: bool,
) -> Result<Option<LorebookEntryGenerationResult>, LorebookEntryGenerationError> {
    let mut pending_none = None;
    for call in calls {
        match call.name.as_str() {
            LOREBOOK_ENTRY_WRITE_TOOL_NAME => {
                return normalize_entry_draft(&call.arguments)
                    .map(|draft| Some(LorebookEntryGenerationResult::Entry { draft }));
            }
            LOREBOOK_ENTRY_NONE_TOOL_NAME if !force => {
                pending_none = Some(LorebookEntryGenerationResult::None {
                    reason: call
                        .arguments
                        .get("reason")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .unwrap_or(DEFAULT_NO_LOREBOOK_ENTRY_REASON)
                        .to_owned(),
                });
            }
            _ => {}
        }
    }
    Ok(pending_none)
}

pub fn parse_lorebook_entry_fallback(
    raw: &str,
    format: LorebookEntryFallbackFormat,
    force: bool,
) -> Result<LorebookEntryGenerationResult, LorebookEntryGenerationError> {
    let result = match format {
        LorebookEntryFallbackFormat::Json => parse_json_fallback(raw),
        LorebookEntryFallbackFormat::Xml => parse_xml_fallback(raw),
    }?;
    if force && matches!(result, LorebookEntryGenerationResult::None { .. }) {
        return Err(LorebookEntryGenerationError::UndeclaredFallbackOperation);
    }
    Ok(result)
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
                let key = value.to_ascii_lowercase();
                seen.insert(key).then(|| value.to_owned())
            })
            .collect::<Vec<_>>(),
        Some(Value::String(value)) => {
            let value = value.trim();
            if value.is_empty() {
                Vec::new()
            } else {
                vec![value.to_owned()]
            }
        }
        _ => Vec::new(),
    };
    result.truncate(MAX_GENERATED_LOREBOOK_KEYWORDS);
    result
}

fn normalize_entry_draft(
    arguments: &Value,
) -> Result<GeneratedLorebookEntryDraft, LorebookEntryGenerationError> {
    let object = arguments
        .as_object()
        .ok_or(LorebookEntryGenerationError::ArgumentsNotObject)?;
    let title = object
        .get("title")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(LorebookEntryGenerationError::MissingTitle)?
        .to_owned();
    let content = object
        .get("content")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(LorebookEntryGenerationError::MissingContent)?
        .to_owned();
    Ok(GeneratedLorebookEntryDraft {
        title,
        keywords: normalize_keywords(object.get("keywords")),
        content,
        always_active: object
            .get("alwaysActive")
            .or_else(|| object.get("always_active"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
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

fn parse_json_fallback(
    raw: &str,
) -> Result<LorebookEntryGenerationResult, LorebookEntryGenerationError> {
    let normalized = normalize_fallback_text(raw);
    let value: Value = serde_json::from_str(json_snippet(&normalized).unwrap_or(&normalized))
        .map_err(|_| LorebookEntryGenerationError::InvalidFallback)?;
    let node = value
        .get("result")
        .or_else(|| value.get("response"))
        .unwrap_or(&value);
    let object = node
        .as_object()
        .ok_or(LorebookEntryGenerationError::InvalidFallback)?;
    let name = object
        .get("name")
        .or_else(|| object.get("tool"))
        .or_else(|| object.get("action"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(LorebookEntryGenerationError::InvalidFallback)?;
    let arguments = object
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| Value::Object(Map::new()));
    match name {
        LOREBOOK_ENTRY_WRITE_TOOL_NAME => normalize_entry_draft(&arguments)
            .map(|draft| LorebookEntryGenerationResult::Entry { draft }),
        LOREBOOK_ENTRY_NONE_TOOL_NAME => Ok(LorebookEntryGenerationResult::None {
            reason: arguments
                .get("reason")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or(DEFAULT_NO_LOREBOOK_ENTRY_REASON)
                .to_owned(),
        }),
        _ => Err(LorebookEntryGenerationError::UndeclaredFallbackOperation),
    }
}

fn xml_attribute(element: &BytesStart<'_>, key: &[u8]) -> Option<String> {
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

fn xml_text(raw: &[u8]) -> Result<String, LorebookEntryGenerationError> {
    unescape(&String::from_utf8_lossy(raw))
        .map(|value| value.into_owned())
        .map_err(|_| LorebookEntryGenerationError::InvalidFallback)
}

fn xml_reference(reference: BytesRef<'_>) -> Result<String, LorebookEntryGenerationError> {
    if let Ok(Some(character)) = reference.resolve_char_ref() {
        return Ok(character.to_string());
    }
    let content = reference
        .xml_content()
        .map_err(|_| LorebookEntryGenerationError::InvalidFallback)?;
    Ok(resolve_xml_entity(&content).map_or_else(|| format!("&{content};"), ToOwned::to_owned))
}

fn parse_xml_fallback(
    raw: &str,
) -> Result<LorebookEntryGenerationResult, LorebookEntryGenerationError> {
    let normalized = normalize_fallback_text(raw);
    let mut reader = Reader::from_str(&normalized);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut operation = None;
    let mut field = None;
    let mut title = String::new();
    let mut content = String::new();
    let mut reason = String::new();
    let mut keyword = String::new();
    let mut keywords = Vec::new();
    let mut always_active = false;
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                let tag = String::from_utf8_lossy(event.name().as_ref()).into_owned();
                if operation.is_none()
                    && matches!(
                        tag.as_str(),
                        LOREBOOK_ENTRY_WRITE_TOOL_NAME | LOREBOOK_ENTRY_NONE_TOOL_NAME
                    )
                {
                    if tag == LOREBOOK_ENTRY_WRITE_TOOL_NAME {
                        always_active = xml_attribute(&event, b"alwaysActive")
                            .or_else(|| xml_attribute(&event, b"always_active"))
                            .is_some_and(|value| {
                                matches!(
                                    value.trim().to_ascii_lowercase().as_str(),
                                    "true" | "1" | "yes"
                                )
                            });
                    }
                    operation = Some(tag);
                } else if operation.is_some() {
                    field = Some(tag);
                }
            }
            Ok(Event::Empty(event)) => {
                let tag = String::from_utf8_lossy(event.name().as_ref()).into_owned();
                if operation.is_none() && tag == LOREBOOK_ENTRY_WRITE_TOOL_NAME {
                    always_active = xml_attribute(&event, b"alwaysActive")
                        .or_else(|| xml_attribute(&event, b"always_active"))
                        .is_some_and(|value| {
                            matches!(
                                value.trim().to_ascii_lowercase().as_str(),
                                "true" | "1" | "yes"
                            )
                        });
                    operation = Some(tag);
                } else if operation.is_none() && tag == LOREBOOK_ENTRY_NONE_TOOL_NAME {
                    reason = xml_attribute(&event, b"reason").unwrap_or_default();
                    operation = Some(tag);
                }
            }
            Ok(Event::Text(event)) => append_xml_field(
                field.as_deref(),
                &xml_text(event.as_ref())?,
                &mut title,
                &mut content,
                &mut reason,
                &mut keyword,
            ),
            Ok(Event::GeneralRef(reference)) => append_xml_field(
                field.as_deref(),
                &xml_reference(reference)?,
                &mut title,
                &mut content,
                &mut reason,
                &mut keyword,
            ),
            Ok(Event::End(event)) => {
                let tag = String::from_utf8_lossy(event.name().as_ref()).into_owned();
                if tag == "keyword" {
                    if !keyword.trim().is_empty() {
                        keywords.push(keyword.trim().to_owned());
                    }
                    keyword.clear();
                    field = None;
                } else if matches!(tag.as_str(), "title" | "content" | "reason" | "keywords") {
                    field = None;
                } else if operation.as_deref() == Some(tag.as_str()) {
                    break;
                } else {
                    field = None;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => return Err(LorebookEntryGenerationError::InvalidFallback),
            _ => {}
        }
        buffer.clear();
    }
    match operation.as_deref() {
        Some(LOREBOOK_ENTRY_WRITE_TOOL_NAME) => normalize_entry_draft(&json!({
            "title": title.trim(),
            "keywords": keywords,
            "content": content.trim(),
            "alwaysActive": always_active,
        }))
        .map(|draft| LorebookEntryGenerationResult::Entry { draft }),
        Some(LOREBOOK_ENTRY_NONE_TOOL_NAME) => Ok(LorebookEntryGenerationResult::None {
            reason: if reason.trim().is_empty() {
                DEFAULT_NO_LOREBOOK_ENTRY_REASON.to_owned()
            } else {
                reason.trim().to_owned()
            },
        }),
        _ => Err(LorebookEntryGenerationError::InvalidFallback),
    }
}

fn append_xml_field(
    field: Option<&str>,
    value: &str,
    title: &mut String,
    content: &mut String,
    reason: &mut String,
    keyword: &mut String,
) {
    match field {
        Some("title") => title.push_str(value),
        Some("content") => content.push_str(value),
        Some("reason") => reason.push_str(value),
        Some("keyword") => keyword.push_str(value),
        _ => {}
    }
}

pub fn validate_lorebook_entry_tool_request(force: bool) -> Result<(), ValidationError> {
    lorebook_entry_tool_request(force).validate()
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
    fn tool_contract_copies_force_and_optional_none_modes() {
        let ordinary = lorebook_entry_tool_request(false);
        ordinary.validate().expect("ordinary request");
        assert_eq!(ordinary.choice, ToolChoice::Required);
        assert_eq!(
            ordinary
                .definitions
                .iter()
                .map(|definition| definition.name.as_str())
                .collect::<Vec<_>>(),
            [
                LOREBOOK_ENTRY_WRITE_TOOL_NAME,
                LOREBOOK_ENTRY_NONE_TOOL_NAME
            ]
        );
        let forced = lorebook_entry_tool_request(true);
        forced.validate().expect("forced request");
        assert_eq!(forced.definitions.len(), 1);
        assert_eq!(forced.definitions[0].name, LOREBOOK_ENTRY_WRITE_TOOL_NAME);
    }

    #[test]
    fn write_call_wins_over_earlier_none_and_normalizes_legacy_fields() {
        let calls = vec![
            call(LOREBOOK_ENTRY_NONE_TOOL_NAME, json!({"reason": "weak"})),
            call(
                LOREBOOK_ENTRY_WRITE_TOOL_NAME,
                json!({
                    "title": "  Harbour  ",
                    "keywords": [" Port ", "port", "", "Mira"],
                    "content": "  Mira lives by the harbour.  ",
                    "always_active": true
                }),
            ),
        ];
        assert_eq!(
            reduce_lorebook_entry_calls(&calls, false).expect("reduce"),
            Some(LorebookEntryGenerationResult::Entry {
                draft: GeneratedLorebookEntryDraft {
                    title: "Harbour".into(),
                    keywords: vec!["Port".into(), "Mira".into()],
                    content: "Mira lives by the harbour.".into(),
                    always_active: true,
                }
            })
        );
    }

    #[test]
    fn keyword_limit_and_none_defaults_are_exact() {
        let keywords = (0..30)
            .map(|index| format!("key-{index}"))
            .collect::<Vec<_>>();
        let result = reduce_lorebook_entry_calls(
            &[call(
                LOREBOOK_ENTRY_WRITE_TOOL_NAME,
                json!({"title":"Title", "content":"Body", "keywords": keywords}),
            )],
            false,
        )
        .expect("reduce")
        .expect("result");
        let LorebookEntryGenerationResult::Entry { draft } = result else {
            panic!("expected entry");
        };
        assert_eq!(draft.keywords.len(), MAX_GENERATED_LOREBOOK_KEYWORDS);
        assert_eq!(
            reduce_lorebook_entry_calls(&[call(LOREBOOK_ENTRY_NONE_TOOL_NAME, json!({}))], false)
                .expect("reduce"),
            Some(LorebookEntryGenerationResult::None {
                reason: DEFAULT_NO_LOREBOOK_ENTRY_REASON.into()
            })
        );
        assert_eq!(
            reduce_lorebook_entry_calls(
                &[call(
                    LOREBOOK_ENTRY_NONE_TOOL_NAME,
                    json!({"reason":"ignored"})
                )],
                true
            )
            .expect("forced reduction"),
            None
        );
    }

    #[test]
    fn json_and_xml_fallbacks_copy_legacy_shapes_and_force_rejection() {
        let json = parse_lorebook_entry_fallback(
            r#"```json
            {"response":{"tool":"write_lorebook_entry","arguments":{"title":"Coast","keywords":["Sea"],"content":"Mira moved to the coast.","alwaysActive":true}}}
            ```"#,
            LorebookEntryFallbackFormat::Json,
            false,
        )
        .expect("json");
        let xml = parse_lorebook_entry_fallback(
            r#"<lorebook_result><write_lorebook_entry always_active="yes"><title>Coast &amp; Sea</title><keywords><keyword>Mira</keyword><keyword>mira</keyword></keywords><content>Moved &amp; stayed.</content></write_lorebook_entry></lorebook_result>"#,
            LorebookEntryFallbackFormat::Xml,
            false,
        )
        .expect("xml");
        assert!(matches!(json, LorebookEntryGenerationResult::Entry { .. }));
        let LorebookEntryGenerationResult::Entry { draft } = xml else {
            panic!("expected XML entry");
        };
        assert_eq!(draft.title, "Coast&Sea");
        assert_eq!(draft.keywords, ["Mira"]);
        assert_eq!(draft.content, "Moved&stayed.");
        assert!(draft.always_active);
        assert_eq!(
            parse_lorebook_entry_fallback(
                r#"{"result":{"name":"no_entry","arguments":{"reason":"weak"}}}"#,
                LorebookEntryFallbackFormat::Json,
                true,
            ),
            Err(LorebookEntryGenerationError::UndeclaredFallbackOperation)
        );
    }

    #[test]
    fn malformed_entry_and_fallback_fail_closed() {
        assert_eq!(
            reduce_lorebook_entry_calls(
                &[call(
                    LOREBOOK_ENTRY_WRITE_TOOL_NAME,
                    json!({"title":" ", "content":"ok"})
                )],
                false,
            ),
            Err(LorebookEntryGenerationError::MissingTitle)
        );
        assert_eq!(
            parse_lorebook_entry_fallback(
                "<write_lorebook_entry><title>orphan</title></write_lorebook_entry>",
                LorebookEntryFallbackFormat::Xml,
                false,
            ),
            Err(LorebookEntryGenerationError::MissingContent)
        );
    }
}
