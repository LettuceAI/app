use std::collections::HashSet;

use lettuce_conversations::{
    ProposedToolCall, ResolvedInferenceProfile, ToolChoice, ToolDefinition, ToolRequest,
};
use lettuce_types::{JobId, PromptDocumentId, RequestId, Revision, TimestampMillis};
use quick_xml::Reader;
use quick_xml::escape::{resolve_xml_entity, unescape};
use quick_xml::events::{BytesRef, Event};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

pub const SET_IDENTITY_TOOL_NAME: &str = "set_identity";
pub const SET_AUTHORED_FACTS_TOOL_NAME: &str = "set_authored_facts";
pub const SET_BASELINE_AFFECT_TOOL_NAME: &str = "set_baseline_affect";
pub const SET_REGULATION_STYLE_TOOL_NAME: &str = "set_regulation_style";
pub const SET_RELATIONSHIP_DEFAULTS_TOOL_NAME: &str = "set_relationship_defaults";
pub const SOUL_WRITER_DONE_TOOL_NAME: &str = "done";
pub const SOUL_WRITER_FINAL_INSTRUCTION: &str = "Author the Companion Soul now. Issue tool calls (set_identity, set_authored_facts, set_baseline_affect, set_regulation_style, set_relationship_defaults) across one or more turns, then call done to finish. Populate every identity text field you can ground from the inputs: essence, traits, backstory, appearance, goals, likes, voice, relationalStyle, vulnerabilities, fears, habits, and boundaries. Also extract atomic facts from the backstory and definition: historical events use policy=historical and locked=true; present-state details use policy=current; inferred traits, fears, goals, habits, and vulnerabilities use policy=adaptive and locked=false. Do not turn uncertain implications into facts.";
pub const SOUL_WRITER_JSON_FALLBACK_PROMPT: &str = r#"Return only JSON. Format: {"operations":[{"name":"set_identity","arguments":{"essence":"...","traits":"...","backstory":"...","appearance":"...","goals":"...","likes":"...","voice":"...","relationalStyle":"...","vulnerabilities":"...","fears":"...","habits":"...","boundaries":"..."}},{"name":"set_baseline_affect","arguments":{"warmth":0.5,"trust":0.5,"calm":0.5,"vulnerability":0.5,"longing":0.5,"hurt":0.5,"tension":0.5,"irritation":0.5,"affectionIntensity":0.5,"reassuranceNeed":0.5}},{"name":"set_regulation_style","arguments":{"suppression":0.5,"volatility":0.5,"recoverySpeed":0.5,"conflictAvoidance":0.5,"reassuranceSeeking":0.5,"protestBehavior":0.5,"emotionalTransparency":0.5,"attachmentActivation":0.5,"pride":0.5}},{"name":"set_relationship_defaults","arguments":{"closeness":0.2,"trust":0.3,"affection":0.2,"tension":0.05}},{"name":"done","arguments":{"notes":"optional"}}]}. End with done. Numeric fields are optional; baseline and regulation values clamp to [0,1], and relationship closeness/trust/affection clamp to [-1,1] (negative means the character starts disliking/distrusting/distant from the user) while relationship tension clamps to [0,1]. Do not use markdown."#;
pub const SOUL_WRITER_XML_FALLBACK_PROMPT: &str = r#"Return only XML. Format: <soul_ops><set_identity><essence>...</essence><traits>...</traits><backstory>...</backstory><appearance>...</appearance><goals>...</goals><likes>...</likes><voice>...</voice><relationalStyle>...</relationalStyle><vulnerabilities>...</vulnerabilities><fears>...</fears><habits>...</habits><boundaries>...</boundaries></set_identity><set_baseline_affect warmth="0.5" trust="0.5" calm="0.5" vulnerability="0.5" longing="0.5" hurt="0.5" tension="0.5" irritation="0.5" affectionIntensity="0.5" reassuranceNeed="0.5" /><set_regulation_style suppression="0.5" volatility="0.5" recoverySpeed="0.5" conflictAvoidance="0.5" reassuranceSeeking="0.5" protestBehavior="0.5" emotionalTransparency="0.5" attachmentActivation="0.5" pride="0.5" /><set_relationship_defaults closeness="0.2" trust="0.3" affection="0.2" tension="0.05" /><done summary="optional" /></soul_ops>. End with <done />. Numeric fields are optional; baseline and regulation values clamp to [0,1], and relationship closeness/trust/affection clamp to [-1,1] (negative means the character starts disliking/distrusting/distant from the user) while relationship tension clamps to [0,1]. Do not use markdown."#;
pub const SOUL_WRITER_JSON_FACT_FALLBACK_PROMPT: &str = r#"Also include a set_authored_facts operation. Its arguments must be {"facts":[{"category":"backstory","value":"one atomic fact","policy":"historical","slot":"stable-semantic-slot","confidence":1.0,"weight":1.0,"locked":true}]}. Extract historical, current, and adaptive facts conservatively."#;
pub const SOUL_WRITER_XML_FACT_FALLBACK_PROMPT: &str = r#"Also include <set_authored_facts><facts>[{"category":"backstory","value":"one atomic fact","policy":"historical","slot":"stable-semantic-slot","confidence":1.0,"weight":1.0,"locked":true}]</facts></set_authored_facts>. The facts element contains a JSON array escaped as normal XML text. Extract facts conservatively."#;

const TEXT_FIELDS: &[&str] = &[
    "essence",
    "traits",
    "backstory",
    "appearance",
    "goals",
    "likes",
    "voice",
    "relationalStyle",
    "vulnerabilities",
    "fears",
    "habits",
    "boundaries",
];

const BASELINE_AFFECT_FIELDS: &[&str] = &[
    "warmth",
    "trust",
    "calm",
    "vulnerability",
    "longing",
    "hurt",
    "tension",
    "irritation",
    "affectionIntensity",
    "reassuranceNeed",
];

const REGULATION_STYLE_FIELDS: &[&str] = &[
    "suppression",
    "volatility",
    "recoverySpeed",
    "conflictAvoidance",
    "reassuranceSeeking",
    "protestBehavior",
    "emotionalTransparency",
    "attachmentActivation",
    "pride",
];

const RELATIONSHIP_DEFAULTS_FIELDS: &[&str] = &["closeness", "trust", "affection", "tension"];
const RELATIONSHIP_BIPOLAR_FIELDS: &[&str] = &["closeness", "trust", "affection"];
const SOUL_OPERATION_NAMES: &[&str] = &[
    SET_IDENTITY_TOOL_NAME,
    SET_AUTHORED_FACTS_TOOL_NAME,
    SET_BASELINE_AFFECT_TOOL_NAME,
    SET_REGULATION_STYLE_TOOL_NAME,
    SET_RELATIONSHIP_DEFAULTS_TOOL_NAME,
    SOUL_WRITER_DONE_TOOL_NAME,
];
const SOUL_OPERATION_ROOTS: &[&str] = &["soul_ops", "operations"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SoulWriterFallbackFormat {
    Json,
    Xml,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SoulWriterProfileTarget {
    Primary,
    Fallback,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SoulWriterPromptValues {
    pub character_name: String,
    pub character_definition: String,
    pub character_description: String,
    pub opening_context: String,
    pub current_soul: String,
    pub user_notes: String,
    pub final_instruction: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SoulWriterReduction {
    pub draft: Value,
    pub results: Vec<Value>,
    pub completed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompanionSoulWriterRun {
    pub request_id: RequestId,
    pub job_id: JobId,
    pub primary_profile: ResolvedInferenceProfile,
    pub fallback_profile: Option<ResolvedInferenceProfile>,
    pub prompt_id: PromptDocumentId,
    pub prompt_revision: Revision,
    pub prompt_values: SoulWriterPromptValues,
    pub starting_draft: Value,
    pub fallback_format: SoulWriterFallbackFormat,
    pub created_at: TimestampMillis,
    pub rounds: Vec<CompanionSoulWriterRoundCheckpoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompanionSoulWriterRoundCheckpoint {
    #[serde(default)]
    pub usage: Option<lettuce_conversations::InferenceUsage>,
    #[serde(default)]
    pub fallback_usage: Option<lettuce_conversations::InferenceUsage>,
    pub ordinal: u32,
    pub profile_target: SoulWriterProfileTarget,
    pub calls: Vec<ProposedToolCall>,
    pub resulting_draft: Value,
    pub completed: bool,
    pub reduced_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CompanionSoulWriterRunRepositoryError {
    #[error("companion Soul-writer run was not found")]
    NotFound,
    #[error("companion Soul-writer run conflicts with durable state")]
    Conflict,
    #[error("companion Soul-writer run is invalid")]
    Invalid,
    #[error("companion Soul-writer run storage failed")]
    Failure,
    #[error("companion Soul-writer run storage is corrupt")]
    Corrupt,
}

pub trait CompanionSoulWriterRunRepository: Send + Sync {
    fn admit_companion_soul_writer_run(
        &self,
        run: CompanionSoulWriterRun,
    ) -> Result<CompanionSoulWriterRun, CompanionSoulWriterRunRepositoryError>;

    fn load_companion_soul_writer_run(
        &self,
        request_id: RequestId,
    ) -> Result<CompanionSoulWriterRun, CompanionSoulWriterRunRepositoryError>;

    fn commit_companion_soul_writer_round(
        &self,
        request_id: RequestId,
        checkpoint: CompanionSoulWriterRoundCheckpoint,
    ) -> Result<CompanionSoulWriterRun, CompanionSoulWriterRunRepositoryError>;
}

impl CompanionSoulWriterRun {
    pub fn validate(&self) -> Result<(), CompanionSoulWriterRunRepositoryError> {
        if self.prompt_revision.get() == 0
            || self.prompt_values.character_name.trim().is_empty()
            || !self.starting_draft.is_object()
            || normalize_soul_writer_draft(Some(&self.starting_draft)) != self.starting_draft
            || self.created_at.get() < 0
            || self.rounds.len() > 8
            || self
                .rounds
                .iter()
                .take(self.rounds.len().saturating_sub(1))
                .any(|round| round.completed)
        {
            return Err(CompanionSoulWriterRunRepositoryError::Invalid);
        }
        let mut draft = self.starting_draft.clone();
        let mut fallback_started = false;
        for (index, round) in self.rounds.iter().enumerate() {
            round.validate(index as u32)?;
            if round.profile_target == SoulWriterProfileTarget::Fallback {
                if self.fallback_profile.is_none() {
                    return Err(CompanionSoulWriterRunRepositoryError::Invalid);
                }
                fallback_started = true;
            } else if fallback_started {
                return Err(CompanionSoulWriterRunRepositoryError::Invalid);
            }
            let reduction = reduce_soul_writer_calls(Some(&draft), &round.calls, round.reduced_at);
            if reduction.draft != round.resulting_draft || reduction.completed != round.completed {
                return Err(CompanionSoulWriterRunRepositoryError::Invalid);
            }
            draft = reduction.draft;
        }
        Ok(())
    }
}

#[must_use]
pub const fn soul_writer_fallback_prompt(format: SoulWriterFallbackFormat) -> &'static str {
    match format {
        SoulWriterFallbackFormat::Json => SOUL_WRITER_JSON_FALLBACK_PROMPT,
        SoulWriterFallbackFormat::Xml => SOUL_WRITER_XML_FALLBACK_PROMPT,
    }
}

#[must_use]
pub const fn soul_writer_fact_fallback_prompt(format: SoulWriterFallbackFormat) -> &'static str {
    match format {
        SoulWriterFallbackFormat::Json => SOUL_WRITER_JSON_FACT_FALLBACK_PROMPT,
        SoulWriterFallbackFormat::Xml => SOUL_WRITER_XML_FACT_FALLBACK_PROMPT,
    }
}

impl CompanionSoulWriterRoundCheckpoint {
    fn validate(&self, expected_ordinal: u32) -> Result<(), CompanionSoulWriterRunRepositoryError> {
        if self.ordinal != expected_ordinal
            || self.calls.len() > lettuce_conversations::MAX_TOOL_CALLS_PER_RESPONSE
            || self.calls.iter().any(|call| {
                !SOUL_OPERATION_NAMES.contains(&call.name.as_str()) || call.validate().is_err()
            })
            || !self.resulting_draft.is_object()
            || self.reduced_at.get() < 0
        {
            return Err(CompanionSoulWriterRunRepositoryError::Invalid);
        }
        Ok(())
    }
}

#[must_use]
pub fn soul_writer_prompt_values(
    character_name: &str,
    character_definition: Option<&str>,
    character_description: Option<&str>,
    opening_context: Option<&str>,
    current_soul: Option<&Value>,
    user_notes: Option<&str>,
) -> SoulWriterPromptValues {
    SoulWriterPromptValues {
        character_name: character_name.trim().to_owned(),
        character_definition: nonblank_or(character_definition, "Not provided."),
        character_description: nonblank_or(character_description, "Not provided."),
        opening_context: nonblank_or(opening_context, "Not provided."),
        current_soul: current_soul
            .map(|value| serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".to_owned()))
            .unwrap_or_else(|| "{}".to_owned()),
        user_notes: nonblank_or(user_notes, "No special direction."),
        final_instruction: SOUL_WRITER_FINAL_INSTRUCTION.to_owned(),
    }
}

fn nonblank_or(value: Option<&str>, fallback: &str) -> String {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback)
        .to_owned()
}

pub fn parse_soul_writer_fallback_calls(
    raw: &str,
    format: SoulWriterFallbackFormat,
) -> Result<Vec<ProposedToolCall>, String> {
    match format {
        SoulWriterFallbackFormat::Json => parse_fallback_json(raw),
        SoulWriterFallbackFormat::Xml => parse_fallback_xml(raw),
    }
}

fn normalize_structured_fallback_text(raw: &str) -> String {
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

fn extract_json_snippet(raw: &str) -> Option<&str> {
    let mut start = None;
    let mut stack = Vec::new();
    let mut in_string = false;
    let mut escape = false;
    for (index, character) in raw.char_indices() {
        if in_string {
            if escape {
                escape = false;
            } else if character == '\\' {
                escape = true;
            } else if character == '"' {
                in_string = false;
            }
            continue;
        }
        match character {
            '"' => in_string = true,
            '{' | '[' => {
                if start.is_none() {
                    start = Some(index);
                }
                stack.push(character);
            }
            '}' => {
                if stack.pop() != Some('{') {
                    return None;
                }
                if stack.is_empty() {
                    return start.map(|start| &raw[start..=index]);
                }
            }
            ']' => {
                if stack.pop() != Some('[') {
                    return None;
                }
                if stack.is_empty() {
                    return start.map(|start| &raw[start..=index]);
                }
            }
            _ => {}
        }
    }
    None
}

fn parse_fallback_json(raw: &str) -> Result<Vec<ProposedToolCall>, String> {
    let normalized = normalize_structured_fallback_text(raw);
    let snippet = extract_json_snippet(&normalized).unwrap_or(&normalized);
    let value: Value = serde_json::from_str(snippet)
        .map_err(|error| format!("fallback JSON parse error: {error}"))?;
    let operations = match &value {
        Value::Array(items) => items.clone(),
        Value::Object(map) => map
            .get("operations")
            .or_else(|| map.get("ops"))
            .and_then(Value::as_array)
            .cloned()
            .ok_or_else(|| "fallback JSON missing operations array".to_owned())?,
        _ => return Err("fallback JSON must be object or array".to_owned()),
    };
    operations
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let Some(object) = item.as_object() else {
                return Ok(None);
            };
            let name = match object
                .get("name")
                .or_else(|| object.get("tool"))
                .or_else(|| object.get("op"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|name| !name.is_empty())
            {
                Some(name) => name,
                None => {
                    return Err(format!(
                        "fallback JSON operation {} is missing a name",
                        index + 1
                    ));
                }
            };
            if !SOUL_OPERATION_NAMES.contains(&name) {
                return Ok(None);
            }
            let arguments = match object.get("arguments") {
                Some(Value::Object(arguments)) => Value::Object(arguments.clone()),
                Some(arguments) => arguments.clone(),
                None => Value::Object(
                    object
                        .iter()
                        .filter(|(key, _)| !matches!(key.as_str(), "name" | "tool" | "op"))
                        .map(|(key, value)| (key.clone(), value.clone()))
                        .collect(),
                ),
            };
            Ok(Some(ProposedToolCall {
                provider_call_id: Some(format!("json_op_{}", index + 1)),
                name: name.to_owned(),
                arguments,
                raw_arguments: None,
                provider_replay: None,
            }))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|calls| calls.into_iter().flatten().collect())
}

fn parse_fallback_xml(raw: &str) -> Result<Vec<ProposedToolCall>, String> {
    let normalized = normalize_structured_fallback_text(raw);
    let mut reader = Reader::from_str(&normalized);
    reader.config_mut().trim_text(false);
    let mut buffer = Vec::new();
    let mut root_seen = false;
    let mut current_operation: Option<String> = None;
    let mut current_arguments = Map::new();
    let mut current_field: Option<String> = None;
    let mut calls = Vec::new();
    let mut operation_index = 0usize;
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                let tag = String::from_utf8_lossy(event.name().as_ref()).into_owned();
                if !root_seen && SOUL_OPERATION_ROOTS.contains(&tag.as_str()) {
                    root_seen = true;
                } else if root_seen
                    && current_operation.is_none()
                    && SOUL_OPERATION_NAMES.contains(&tag.as_str())
                {
                    current_operation = Some(tag);
                    current_arguments = Map::new();
                    ingest_xml_attributes(&mut current_arguments, &event);
                } else if current_operation.is_some() {
                    current_field = Some(tag);
                }
            }
            Ok(Event::Empty(event)) => {
                let tag = String::from_utf8_lossy(event.name().as_ref()).into_owned();
                if !root_seen && SOUL_OPERATION_ROOTS.contains(&tag.as_str()) {
                    root_seen = true;
                } else if root_seen
                    && current_operation.is_none()
                    && SOUL_OPERATION_NAMES.contains(&tag.as_str())
                {
                    let mut arguments = Map::new();
                    ingest_xml_attributes(&mut arguments, &event);
                    operation_index += 1;
                    calls.push(fallback_call(
                        format!("xml_op_{operation_index}"),
                        tag,
                        arguments,
                    ));
                }
            }
            Ok(Event::Text(event)) => {
                if let (Some(field), Some(_)) =
                    (current_field.as_deref(), current_operation.as_ref())
                {
                    let text = String::from_utf8_lossy(event.as_ref());
                    let text = unescape(&text)
                        .map_err(|error| format!("fallback XML text decode error: {error}"))?;
                    append_text_field(&mut current_arguments, field, &text);
                }
            }
            Ok(Event::CData(event)) => {
                if let (Some(field), Some(_)) =
                    (current_field.as_deref(), current_operation.as_ref())
                {
                    append_text_field(
                        &mut current_arguments,
                        field,
                        &String::from_utf8_lossy(event.as_ref()),
                    );
                }
            }
            Ok(Event::GeneralRef(event)) => {
                if let (Some(field), Some(_)) =
                    (current_field.as_deref(), current_operation.as_ref())
                {
                    let text = decode_xml_reference(event)?;
                    append_text_field(&mut current_arguments, field, &text);
                }
            }
            Ok(Event::End(event)) => {
                let tag = String::from_utf8_lossy(event.name().as_ref()).into_owned();
                if current_field.as_deref() == Some(tag.as_str()) {
                    current_field = None;
                } else if current_operation.as_deref() == Some(tag.as_str()) {
                    coerce_numeric_strings(&mut current_arguments);
                    operation_index += 1;
                    calls.push(fallback_call(
                        format!("xml_op_{operation_index}"),
                        current_operation.take().unwrap_or_default(),
                        std::mem::take(&mut current_arguments),
                    ));
                }
            }
            Ok(Event::Eof) => break,
            Err(error) => return Err(format!("fallback XML parse error: {error}")),
            _ => {}
        }
        buffer.clear();
    }
    if !root_seen {
        return Err("fallback response did not contain a soul_ops root".to_owned());
    }
    Ok(calls)
}

fn ingest_xml_attributes(
    arguments: &mut Map<String, Value>,
    event: &quick_xml::events::BytesStart<'_>,
) {
    for attribute in event.attributes().flatten() {
        let key = String::from_utf8_lossy(attribute.key.as_ref()).into_owned();
        let Ok(raw) = attribute.unescape_value() else {
            continue;
        };
        if let Ok(number) = raw.trim().parse::<f64>()
            && let Some(number) = serde_json::Number::from_f64(number)
        {
            arguments.insert(key, Value::Number(number));
            continue;
        }
        arguments.insert(key, Value::String(raw.into_owned()));
    }
}

fn decode_xml_reference(reference: BytesRef<'_>) -> Result<String, String> {
    if let Ok(Some(character)) = reference.resolve_char_ref() {
        return Ok(character.to_string());
    }
    let content = reference
        .xml_content()
        .map_err(|error| format!("fallback XML reference decode error: {error}"))?;
    Ok(resolve_xml_entity(&content)
        .map(str::to_owned)
        .unwrap_or_else(|| format!("&{content};")))
}

fn append_text_field(arguments: &mut Map<String, Value>, key: &str, fragment: &str) {
    if fragment.is_empty() {
        return;
    }
    match arguments.get_mut(key) {
        Some(Value::String(existing)) => existing.push_str(fragment),
        _ => {
            arguments.insert(key.to_owned(), Value::String(fragment.to_owned()));
        }
    }
}

fn coerce_numeric_strings(arguments: &mut Map<String, Value>) {
    let numeric_fields = BASELINE_AFFECT_FIELDS
        .iter()
        .copied()
        .chain(REGULATION_STYLE_FIELDS.iter().copied())
        .chain(RELATIONSHIP_DEFAULTS_FIELDS.iter().copied())
        .collect::<HashSet<_>>();
    for (key, value) in arguments.iter_mut() {
        if !numeric_fields.contains(key.as_str()) {
            if let Value::String(text) = value {
                *text = text.trim().to_owned();
            }
            continue;
        }
        if let Value::String(text) = value
            && let Ok(number) = text.trim().parse::<f64>()
            && let Some(number) = serde_json::Number::from_f64(number)
        {
            *value = Value::Number(number);
        }
    }
    arguments.retain(|_, value| !matches!(value, Value::String(text) if text.is_empty()));
}

fn fallback_call(
    provider_call_id: String,
    name: String,
    arguments: Map<String, Value>,
) -> ProposedToolCall {
    ProposedToolCall {
        provider_call_id: Some(provider_call_id),
        name,
        arguments: Value::Object(arguments),
        raw_arguments: None,
        provider_replay: None,
    }
}

#[must_use]
pub fn soul_writer_tool_request() -> ToolRequest {
    let identity_properties = json!({
        "essence": { "type": "string" },
        "traits": { "type": "string" },
        "backstory": { "type": "string" },
        "appearance": { "type": "string" },
        "goals": { "type": "string" },
        "likes": { "type": "string" },
        "voice": { "type": "string" },
        "relationalStyle": { "type": "string" },
        "vulnerabilities": { "type": "string" },
        "fears": { "type": "string" },
        "habits": { "type": "string" },
        "boundaries": { "type": "string" }
    });
    ToolRequest {
        definitions: vec![
            ToolDefinition {
                name: SET_IDENTITY_TOOL_NAME.to_owned(),
                description: Some(
                    "Set or refine the durable identity text fields. All fields optional; later calls overwrite earlier values for the same field."
                        .to_owned(),
                ),
                parameters: json!({
                    "type": "object",
                    "properties": identity_properties
                }),
                version: 1,
            },
            ToolDefinition {
                name: SET_AUTHORED_FACTS_TOOL_NAME.to_owned(),
                description: Some(
                    "Replace the atomic facts extracted from the authored definition and backstory. Historical events are immutable and locked. Current facts can be superseded by a newer value in the same slot. Adaptive facts are evidence about traits, fears, goals, habits, or vulnerabilities and remain unlocked."
                        .to_owned(),
                ),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "facts": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "category": { "type": "string", "enum": TEXT_FIELDS },
                                    "value": { "type": "string" },
                                    "policy": { "type": "string", "enum": ["historical", "current", "adaptive"] },
                                    "slot": { "type": "string" },
                                    "confidence": { "type": "number", "minimum": 0, "maximum": 1 },
                                    "weight": { "type": "number", "minimum": 0, "maximum": 1 },
                                    "locked": { "type": "boolean" }
                                },
                                "required": ["category", "value", "policy", "slot", "confidence", "weight", "locked"]
                            }
                        }
                    },
                    "required": ["facts"]
                }),
                version: 1,
            },
            ToolDefinition {
                name: SET_BASELINE_AFFECT_TOOL_NAME.to_owned(),
                description: Some(
                    "Set or refine baseline affect floats in [0,1]. All fields optional; values are clamped."
                        .to_owned(),
                ),
                parameters: numeric_parameters(BASELINE_AFFECT_FIELDS),
                version: 1,
            },
            ToolDefinition {
                name: SET_REGULATION_STYLE_TOOL_NAME.to_owned(),
                description: Some(
                    "Set or refine regulation style floats in [0,1]. All fields optional; values are clamped."
                        .to_owned(),
                ),
                parameters: numeric_parameters(REGULATION_STYLE_FIELDS),
                version: 1,
            },
            ToolDefinition {
                name: SET_RELATIONSHIP_DEFAULTS_TOOL_NAME.to_owned(),
                description: Some(
                    "Set the starting relationship-with-user defaults. closeness/trust/affection are bidirectional in [-1,1] (negative = the character starts disliking/distrusting/distant from the user, 0 = neutral, positive = warm); tension is in [0,1]. All fields optional."
                        .to_owned(),
                ),
                parameters: numeric_parameters(RELATIONSHIP_DEFAULTS_FIELDS),
                version: 1,
            },
            ToolDefinition {
                name: SOUL_WRITER_DONE_TOOL_NAME.to_owned(),
                description: Some(
                    "Call once the Companion Soul is finalized. Terminal — no more setters after this."
                        .to_owned(),
                ),
                parameters: json!({
                    "type": "object",
                    "properties": { "notes": { "type": "string" } }
                }),
                version: 1,
            },
        ],
        choice: ToolChoice::Required,
    }
}

fn numeric_parameters(fields: &[&str]) -> Value {
    let properties = fields
        .iter()
        .map(|field| {
            (
                (*field).to_owned(),
                json!({ "type": "number", "minimum": 0, "maximum": 1 }),
            )
        })
        .collect::<Map<_, _>>();
    json!({ "type": "object", "properties": properties })
}

#[must_use]
pub fn normalize_soul_writer_draft(current: Option<&Value>) -> Value {
    let mut working = default_soul_value();
    let Some(current) = current.and_then(Value::as_object) else {
        return working;
    };

    if let Some(soul_in) = current.get("soul").and_then(Value::as_object)
        && let Some(soul_out) = working.get_mut("soul").and_then(Value::as_object_mut)
    {
        for key in TEXT_FIELDS {
            if let Some(Value::String(value)) = soul_in.get(*key) {
                soul_out.insert((*key).to_owned(), Value::String(value.clone()));
            }
        }
        copy_numeric_section(
            soul_in,
            soul_out,
            "baselineAffect",
            BASELINE_AFFECT_FIELDS,
            &[],
        );
        copy_numeric_section(
            soul_in,
            soul_out,
            "regulationStyle",
            REGULATION_STYLE_FIELDS,
            &[],
        );
    }
    if let Some(relationship_in) = current
        .get("relationshipDefaults")
        .and_then(Value::as_object)
        && let Some(relationship_out) = working
            .get_mut("relationshipDefaults")
            .and_then(Value::as_object_mut)
    {
        copy_numeric_values(
            relationship_in,
            relationship_out,
            RELATIONSHIP_DEFAULTS_FIELDS,
            RELATIONSHIP_BIPOLAR_FIELDS,
        );
    }
    if let Some(facts) = current.get("authoredFacts").and_then(Value::as_array)
        && let Some(root) = working.as_object_mut()
    {
        root.insert("authoredFacts".to_owned(), Value::Array(facts.clone()));
    }
    working
}

#[must_use]
pub fn reduce_soul_writer_calls(
    current: Option<&Value>,
    calls: &[ProposedToolCall],
    now: TimestampMillis,
) -> SoulWriterReduction {
    let mut draft = normalize_soul_writer_draft(current);
    let mut results = Vec::new();
    let mut completed = false;
    for call in calls {
        let (done, result) = apply_call(&mut draft, call, now);
        results.push(result);
        if done {
            completed = true;
            break;
        }
    }
    SoulWriterReduction {
        draft,
        results,
        completed,
    }
}

fn default_soul_value() -> Value {
    let text = TEXT_FIELDS
        .iter()
        .map(|field| ((*field).to_owned(), Value::String(String::new())))
        .collect::<Map<_, _>>();
    let baseline = zero_numeric_values(BASELINE_AFFECT_FIELDS);
    let regulation = zero_numeric_values(REGULATION_STYLE_FIELDS);
    let relationship = zero_numeric_values(RELATIONSHIP_DEFAULTS_FIELDS);
    let mut soul = text;
    soul.insert("baselineAffect".to_owned(), Value::Object(baseline));
    soul.insert("regulationStyle".to_owned(), Value::Object(regulation));
    json!({
        "soul": soul,
        "authoredFacts": [],
        "relationshipDefaults": relationship
    })
}

fn zero_numeric_values(fields: &[&str]) -> Map<String, Value> {
    fields
        .iter()
        .map(|field| ((*field).to_owned(), json!(0.0)))
        .collect()
}

fn copy_numeric_section(
    source: &Map<String, Value>,
    target: &mut Map<String, Value>,
    section: &str,
    fields: &[&str],
    signed_fields: &[&str],
) {
    let Some(source) = source.get(section).and_then(Value::as_object) else {
        return;
    };
    let Some(target) = target.get_mut(section).and_then(Value::as_object_mut) else {
        return;
    };
    copy_numeric_values(source, target, fields, signed_fields);
}

fn copy_numeric_values(
    source: &Map<String, Value>,
    target: &mut Map<String, Value>,
    fields: &[&str],
    signed_fields: &[&str],
) {
    for key in fields {
        if let Some(value) = source.get(*key).and_then(Value::as_f64) {
            insert_clamped(target, key, value, signed_fields.contains(key));
        }
    }
}

fn insert_clamped(target: &mut Map<String, Value>, key: &str, value: f64, signed: bool) {
    let value = if signed {
        value.clamp(-1.0, 1.0)
    } else {
        value.clamp(0.0, 1.0)
    };
    if let Some(value) = serde_json::Number::from_f64(value) {
        target.insert(key.to_owned(), Value::Number(value));
    }
}

fn apply_call(draft: &mut Value, call: &ProposedToolCall, now: TimestampMillis) -> (bool, Value) {
    match call.name.as_str() {
        SET_IDENTITY_TOOL_NAME => {
            let applied = apply_identity(draft, &call.arguments);
            (false, json!({ "ok": true, "applied": applied }))
        }
        SET_AUTHORED_FACTS_TOOL_NAME => {
            let applied = apply_authored_facts(draft, &call.arguments, now);
            (false, json!({ "ok": true, "applied": applied }))
        }
        SET_BASELINE_AFFECT_TOOL_NAME => {
            let applied = apply_numeric_section(
                draft,
                &["soul", "baselineAffect"],
                BASELINE_AFFECT_FIELDS,
                &[],
                &call.arguments,
            );
            (false, json!({ "ok": true, "applied": applied }))
        }
        SET_REGULATION_STYLE_TOOL_NAME => {
            let applied = apply_numeric_section(
                draft,
                &["soul", "regulationStyle"],
                REGULATION_STYLE_FIELDS,
                &[],
                &call.arguments,
            );
            (false, json!({ "ok": true, "applied": applied }))
        }
        SET_RELATIONSHIP_DEFAULTS_TOOL_NAME => {
            let applied = apply_numeric_section(
                draft,
                &["relationshipDefaults"],
                RELATIONSHIP_DEFAULTS_FIELDS,
                RELATIONSHIP_BIPOLAR_FIELDS,
                &call.arguments,
            );
            (false, json!({ "ok": true, "applied": applied }))
        }
        SOUL_WRITER_DONE_TOOL_NAME => (true, json!({ "ok": true, "done": true })),
        _ => (
            false,
            json!({ "ok": false, "error": "unknown_tool", "name": call.name }),
        ),
    }
}

fn apply_identity(draft: &mut Value, arguments: &Value) -> Vec<String> {
    let mut applied = Vec::new();
    let Some(arguments) = arguments.as_object() else {
        return applied;
    };
    let Some(soul) = draft.get_mut("soul").and_then(Value::as_object_mut) else {
        return applied;
    };
    for key in TEXT_FIELDS {
        if let Some(text) = arguments.get(*key).and_then(Value::as_str) {
            let text = text.trim();
            if !text.is_empty() {
                soul.insert((*key).to_owned(), Value::String(text.to_owned()));
                applied.push((*key).to_owned());
            }
        }
    }
    applied
}

fn apply_authored_facts(draft: &mut Value, arguments: &Value, now: TimestampMillis) -> usize {
    let Some(raw_facts) = arguments.get("facts") else {
        return 0;
    };
    let parsed;
    let facts = match raw_facts {
        Value::Array(facts) => facts,
        Value::String(value) => {
            parsed = serde_json::from_str::<Vec<Value>>(value).unwrap_or_default();
            &parsed
        }
        _ => return 0,
    };
    let facts = facts
        .iter()
        .filter_map(|fact| parse_authored_fact(fact, now))
        .filter_map(|fact| serde_json::to_value(fact).ok())
        .collect::<Vec<_>>();
    let count = facts.len();
    if let Some(root) = draft.as_object_mut() {
        root.insert("authoredFacts".to_owned(), Value::Array(facts));
    }
    count
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthoredFact {
    id: String,
    category: String,
    value: String,
    kind: String,
    policy: String,
    slot: String,
    confidence: f64,
    evidence_count: u32,
    weight: f64,
    valid_from: TimestampMillis,
    locked: bool,
    source_memory_ids: Vec<String>,
    created_at: TimestampMillis,
}

fn parse_authored_fact(fact: &Value, now: TimestampMillis) -> Option<AuthoredFact> {
    let category = fact.get("category")?.as_str()?.trim();
    if !TEXT_FIELDS.contains(&category) {
        return None;
    }
    let value = fact.get("value")?.as_str()?.trim();
    let policy = fact.get("policy")?.as_str()?.trim();
    let slot = fact.get("slot")?.as_str()?.trim();
    if value.is_empty()
        || slot.is_empty()
        || !matches!(policy, "historical" | "current" | "adaptive")
    {
        return None;
    }
    let confidence = fact
        .get("confidence")
        .and_then(Value::as_f64)
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);
    if confidence < 0.7 {
        return None;
    }
    Some(AuthoredFact {
        id: uuid::Uuid::new_v4().to_string(),
        category: category.to_owned(),
        value: value.to_owned(),
        kind: "authored".to_owned(),
        policy: policy.to_owned(),
        slot: slot.to_owned(),
        confidence,
        evidence_count: 1,
        weight: fact
            .get("weight")
            .and_then(Value::as_f64)
            .unwrap_or(1.0)
            .clamp(0.0, 1.0),
        valid_from: now,
        locked: policy == "historical"
            || fact.get("locked").and_then(Value::as_bool).unwrap_or(false),
        source_memory_ids: Vec::new(),
        created_at: now,
    })
}

fn apply_numeric_section(
    draft: &mut Value,
    path: &[&str],
    fields: &[&str],
    signed_fields: &[&str],
    arguments: &Value,
) -> Vec<String> {
    let mut applied = Vec::new();
    let Some(arguments) = arguments.as_object() else {
        return applied;
    };
    let mut node = draft;
    for segment in path {
        let Some(map) = node.as_object_mut() else {
            return applied;
        };
        let Some(next) = map.get_mut(*segment) else {
            return applied;
        };
        node = next;
    }
    let Some(target) = node.as_object_mut() else {
        return applied;
    };
    for key in fields {
        if let Some(value) = arguments.get(*key).and_then(Value::as_f64) {
            insert_clamped(target, key, value, signed_fields.contains(key));
            applied.push((*key).to_owned());
        }
    }
    applied
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(name: &str, arguments: Value) -> ProposedToolCall {
        ProposedToolCall {
            provider_call_id: None,
            name: name.to_owned(),
            arguments,
            raw_arguments: None,
            provider_replay: None,
        }
    }

    #[test]
    fn required_tool_contract_matches_legacy() {
        let request = soul_writer_tool_request();
        assert_eq!(request.choice, ToolChoice::Required);
        assert_eq!(
            request
                .definitions
                .iter()
                .map(|definition| definition.name.as_str())
                .collect::<Vec<_>>(),
            [
                "set_identity",
                "set_authored_facts",
                "set_baseline_affect",
                "set_regulation_style",
                "set_relationship_defaults",
                "done"
            ]
        );
        request.validate().expect("valid tool request");
    }

    #[test]
    fn partial_current_value_is_completed_and_clamped_like_legacy() {
        let existing_fact = json!({ "id": "keep-verbatim" });
        let current = json!({
            "soul": {
                "essence": "Quiet",
                "baselineAffect": { "warmth": 2.0 },
                "regulationStyle": { "pride": -0.5 }
            },
            "relationshipDefaults": { "closeness": -2.0, "tension": 2.0 },
            "authoredFacts": [existing_fact.clone()]
        });
        let draft = normalize_soul_writer_draft(Some(&current));
        assert_eq!(draft["soul"]["essence"], "Quiet");
        assert_eq!(draft["soul"]["traits"], "");
        assert_eq!(draft["soul"]["baselineAffect"]["warmth"], 1.0);
        assert_eq!(draft["soul"]["baselineAffect"]["trust"], 0.0);
        assert_eq!(draft["soul"]["regulationStyle"]["pride"], 0.0);
        assert_eq!(draft["relationshipDefaults"]["closeness"], -1.0);
        assert_eq!(draft["relationshipDefaults"]["tension"], 1.0);
        assert_eq!(draft["authoredFacts"], json!([existing_fact]));
    }

    #[test]
    fn calls_apply_in_order_and_done_stops_later_calls() {
        let reduction = reduce_soul_writer_calls(
            None,
            &[
                call("set_identity", json!({ "traits": " First " })),
                call("set_identity", json!({ "traits": "Second" })),
                call("done", json!({})),
                call("set_identity", json!({ "traits": "Ignored" })),
            ],
            TimestampMillis::new(7),
        );
        assert!(reduction.completed);
        assert_eq!(reduction.results.len(), 3);
        assert_eq!(reduction.draft["soul"]["traits"], "Second");
        assert_eq!(reduction.results[2], json!({ "ok": true, "done": true }));
    }

    #[test]
    fn numeric_sections_use_the_legacy_bounds() {
        let reduction = reduce_soul_writer_calls(
            None,
            &[
                call(
                    "set_baseline_affect",
                    json!({ "warmth": 2.0, "hurt": -1.0 }),
                ),
                call(
                    "set_regulation_style",
                    json!({ "recoverySpeed": 1.5, "pride": -0.5 }),
                ),
                call(
                    "set_relationship_defaults",
                    json!({ "closeness": -2.0, "trust": 2.0, "tension": -1.0 }),
                ),
            ],
            TimestampMillis::new(7),
        );
        assert_eq!(reduction.draft["soul"]["baselineAffect"]["warmth"], 1.0);
        assert_eq!(reduction.draft["soul"]["baselineAffect"]["hurt"], 0.0);
        assert_eq!(
            reduction.draft["soul"]["regulationStyle"]["recoverySpeed"],
            1.0
        );
        assert_eq!(reduction.draft["soul"]["regulationStyle"]["pride"], 0.0);
        assert_eq!(reduction.draft["relationshipDefaults"]["closeness"], -1.0);
        assert_eq!(reduction.draft["relationshipDefaults"]["trust"], 1.0);
        assert_eq!(reduction.draft["relationshipDefaults"]["tension"], 0.0);
    }

    #[test]
    fn authored_facts_preserve_legacy_defaults_and_historical_locking() {
        let reduction = reduce_soul_writer_calls(
            None,
            &[call(
                "set_authored_facts",
                json!({
                    "facts": [
                        {
                            "category": "backstory",
                            "value": " Survived the winter evacuation ",
                            "policy": "historical",
                            "slot": " winter-evacuation ",
                            "confidence": 0.98,
                            "weight": 2.0,
                            "locked": false
                        },
                        {
                            "category": "fears",
                            "value": "Probably fears snow",
                            "policy": "adaptive",
                            "slot": "snow",
                            "confidence": 0.4,
                            "weight": 0.5,
                            "locked": false
                        }
                    ]
                }),
            )],
            TimestampMillis::new(7),
        );
        let facts = reduction.draft["authoredFacts"].as_array().expect("facts");
        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0]["category"], "backstory");
        assert_eq!(facts[0]["value"], "Survived the winter evacuation");
        assert_eq!(facts[0]["kind"], "authored");
        assert_eq!(facts[0]["locked"], true);
        assert_eq!(facts[0]["weight"], 1.0);
        assert_eq!(facts[0]["evidenceCount"], 1);
        assert_eq!(facts[0]["validFrom"], 7);
        assert_eq!(reduction.results[0], json!({ "ok": true, "applied": 1 }));
    }

    #[test]
    fn prompt_values_copy_legacy_missing_value_and_json_rules() {
        let current = json!({ "soul": { "traits": "Careful" } });
        let values = soul_writer_prompt_values(
            "  Mira  ",
            Some("  Canon  "),
            Some(" "),
            None,
            Some(&current),
            Some("  Be reserved  "),
        );
        assert_eq!(values.character_name, "Mira");
        assert_eq!(values.character_definition, "Canon");
        assert_eq!(values.character_description, "Not provided.");
        assert_eq!(values.opening_context, "Not provided.");
        assert_eq!(
            values.current_soul,
            serde_json::to_string_pretty(&current).expect("json")
        );
        assert_eq!(values.user_notes, "Be reserved");
        assert_eq!(values.final_instruction, SOUL_WRITER_FINAL_INSTRUCTION);

        let empty = soul_writer_prompt_values("Mira", None, None, None, None, None);
        assert_eq!(empty.current_soul, "{}");
        assert_eq!(empty.user_notes, "No special direction.");
    }

    #[test]
    fn json_fallback_preserves_aliases_inline_arguments_and_order() {
        let calls = parse_soul_writer_fallback_calls(
            r#"```json
            {"ops":[
                {"tool":"ignored","value":1},
                {"op":"set_identity","traits":"Careful"},
                {"name":"set_baseline_affect","arguments":{"warmth":0.8}},
                {"name":"done","arguments":{}}
            ]}
            ```"#,
            SoulWriterFallbackFormat::Json,
        )
        .expect("json fallback");
        assert_eq!(
            calls
                .iter()
                .map(|call| call.name.as_str())
                .collect::<Vec<_>>(),
            ["set_identity", "set_baseline_affect", "done"]
        );
        assert_eq!(calls[0].provider_call_id.as_deref(), Some("json_op_2"));
        assert_eq!(calls[0].arguments, json!({ "traits": "Careful" }));
        assert_eq!(calls[1].arguments, json!({ "warmth": 0.8 }));
    }

    #[test]
    fn xml_fallback_copies_numeric_text_entity_and_authored_fact_handling() {
        let calls = parse_soul_writer_fallback_calls(
            r#"<soul_ops>
                <set_identity><traits> Calm &amp; careful </traits></set_identity>
                <set_authored_facts><facts>[{"category":"backstory","value":"Moved to the coast","policy":"historical","slot":"coast-move","confidence":1,"weight":1,"locked":true}]</facts></set_authored_facts>
                <set_relationship_defaults closeness="-0.5" tension="1.5" />
                <done />
            </soul_ops>"#,
            SoulWriterFallbackFormat::Xml,
        )
        .expect("xml fallback");
        assert_eq!(
            calls
                .iter()
                .map(|call| call.name.as_str())
                .collect::<Vec<_>>(),
            [
                "set_identity",
                "set_authored_facts",
                "set_relationship_defaults",
                "done"
            ]
        );
        assert_eq!(calls[0].arguments["traits"], "Calm & careful");
        assert!(
            calls[1].arguments["facts"]
                .as_str()
                .is_some_and(|facts| facts.contains("coast-move"))
        );
        assert_eq!(calls[2].arguments["closeness"], -0.5);
        assert_eq!(calls[2].arguments["tension"], 1.5);

        let reduction = reduce_soul_writer_calls(None, &calls, TimestampMillis::new(9));
        assert!(reduction.completed);
        assert_eq!(reduction.draft["soul"]["traits"], "Calm & careful");
        assert_eq!(reduction.draft["authoredFacts"][0]["locked"], true);
        assert_eq!(reduction.draft["relationshipDefaults"]["closeness"], -0.5);
        assert_eq!(reduction.draft["relationshipDefaults"]["tension"], 1.0);
    }

    #[test]
    fn malformed_fallbacks_are_rejected() {
        assert!(
            parse_soul_writer_fallback_calls(
                r#"{"operations":[{"value":1}]}"#,
                SoulWriterFallbackFormat::Json,
            )
            .is_err()
        );
        assert!(
            parse_soul_writer_fallback_calls(
                "<set_identity><traits>orphan</traits></set_identity>",
                SoulWriterFallbackFormat::Xml,
            )
            .is_err()
        );
    }
}
