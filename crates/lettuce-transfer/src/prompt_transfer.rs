//! Prompt templates read from and written to USC system prompt cards and
//! SillyTavern prompt-manager presets.

use std::collections::{BTreeMap, BTreeSet};

use lettuce_context::{
    PromptEntryCondition, PromptEntryDraft, PromptEntryImageSlot, PromptEntryPayload,
    PromptEntryPosition, PromptEntryRole, PromptPurpose,
};
use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PromptTransferError {
    #[error("Invalid prompt file: {0}")]
    InvalidJson(String),
    #[error("PROMPT_NAME_REQUIRED")]
    NameRequired,
    #[error("NO_IMPORTABLE_PROMPTS")]
    NoImportablePrompts,
    #[error("Failed to serialize prompt export")]
    Serialize,
}

/// One stored prompt entry with the id it is exported under.
#[derive(Debug, Clone, PartialEq)]
pub struct PromptTransferEntry {
    pub id: String,
    pub draft: PromptEntryDraft,
}

/// A stored prompt template in the shape its files carry.
#[derive(Debug, Clone, PartialEq)]
pub struct PromptTransfer {
    pub id: String,
    pub name: String,
    pub purpose: PromptPurpose,
    pub entries: Vec<PromptTransferEntry>,
    pub condense: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

/// A prompt template read from a file, ready to be created.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedPrompt {
    pub name: String,
    pub purpose: PromptPurpose,
    pub entries: Vec<PromptEntryDraft>,
    pub condense: bool,
    /// The file named no purpose this app can run, so it became a direct chat
    /// prompt.
    pub purpose_defaulted: bool,
}

/// The preset-level SillyTavern prompts an exported preset carries.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SillyTavernPresetTexts {
    pub impersonation_prompt: String,
    pub new_chat_prompt: String,
    pub new_group_chat_prompt: String,
    pub new_example_chat_prompt: String,
    pub continue_nudge_prompt: String,
    pub scenario_format: String,
    pub personality_format: String,
    pub group_nudge_prompt: String,
    pub wi_format: String,
}

impl SillyTavernPresetTexts {
    /// The texts the old app wrote into every exported preset.
    #[must_use]
    pub fn bundled() -> Self {
        serde_json::from_str(include_str!("../resources/sillytavern-preset.json"))
            .expect("bundled SillyTavern preset texts are valid")
    }
}

const SILLYTAVERN_MARKERS: [(&str, &str); 8] = [
    ("worldInfoBefore", "Lorebook Before"),
    ("personaDescription", "Persona Description"),
    ("charDescription", "Char Description"),
    ("charPersonality", "Char Personality"),
    ("scenario", "Scenario"),
    ("worldInfoAfter", "Lorebook After"),
    ("dialogueExamples", "Chat Examples"),
    ("chatHistory", "Chat History"),
];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UscEntry<'a> {
    id: &'a str,
    name: &'a str,
    role: PromptEntryRole,
    content: &'a str,
    enabled: bool,
    injection_position: PromptEntryPosition,
    injection_depth: u32,
    conditional_min_messages: Option<u32>,
    interval_turns: Option<u32>,
    system_prompt: bool,
    conditions: &'a Option<PromptEntryCondition>,
    prompt_entry_payload: &'a Option<PromptEntryPayload>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UscPromptPayload<'a> {
    id: &'a str,
    name: &'a str,
    prompt_type: PromptPurpose,
    content: &'a str,
    entries: Vec<UscEntry<'a>>,
    condense_prompt_entries: bool,
    created_at: i64,
    updated_at: i64,
}

#[derive(Serialize)]
struct UscSchema {
    name: &'static str,
    version: &'static str,
}

#[derive(Serialize)]
struct UscPromptCard<'a> {
    schema: UscSchema,
    kind: &'static str,
    payload: UscPromptPayload<'a>,
}

/// The template as a USC `system_prompt_template` card, pretty-printed.
pub fn export_prompt_usc(prompt: &PromptTransfer) -> Result<String, PromptTransferError> {
    serde_json::to_string_pretty(&UscPromptCard {
        schema: UscSchema {
            name: "USC",
            version: "1.0",
        },
        kind: "system_prompt_template",
        payload: UscPromptPayload {
            id: &prompt.id,
            name: &prompt.name,
            prompt_type: prompt.purpose,
            content: "",
            entries: prompt
                .entries
                .iter()
                .map(|entry| UscEntry {
                    id: &entry.id,
                    name: &entry.draft.name,
                    role: entry.draft.role,
                    content: &entry.draft.content,
                    enabled: entry.draft.enabled,
                    injection_position: entry.draft.injection_position,
                    injection_depth: entry.draft.depth,
                    conditional_min_messages: entry.draft.conditional_min_messages,
                    interval_turns: entry.draft.interval_turns,
                    system_prompt: entry.draft.system_prompt,
                    conditions: &entry.draft.conditions,
                    prompt_entry_payload: &entry.draft.payload,
                })
                .collect(),
            condense_prompt_entries: prompt.condense,
            created_at: prompt.created_at,
            updated_at: prompt.updated_at,
        },
    })
    .map_err(|_| PromptTransferError::Serialize)
}

fn sillytavern_variables(content: &str) -> String {
    content
        .replace("{{scenario}}", "{{scene}}")
        .replace("{{personality}}", "{{char.desc}}")
}

/// The template as a SillyTavern prompt-manager preset: its entries, then the
/// eight SillyTavern markers, in one prompt order.
pub fn export_prompt_sillytavern(prompt: &PromptTransfer) -> Result<String, PromptTransferError> {
    let texts = SillyTavernPresetTexts::bundled();
    let mut prompts = prompt
        .entries
        .iter()
        .map(|entry| {
            let draft = &entry.draft;
            let mut value = serde_json::Map::new();
            value.insert("identifier".into(), json!(entry.id));
            value.insert("name".into(), json!(draft.name));
            value.insert("system_prompt".into(), json!(draft.system_prompt));
            value.insert("marker".into(), json!(false));
            value.insert(
                "content".into(),
                json!(sillytavern_variables(&draft.content)),
            );
            value.insert("role".into(), json!(draft.role));
            value.insert(
                "injection_position".into(),
                match draft.injection_position {
                    PromptEntryPosition::Relative => json!(0),
                    PromptEntryPosition::InChat => json!(1),
                    PromptEntryPosition::Conditional => json!("conditional"),
                    PromptEntryPosition::Interval => json!("interval"),
                },
            );
            value.insert("injection_depth".into(), json!(draft.depth));
            if let Some(count) = draft.conditional_min_messages {
                value.insert("conditional_min_messages".into(), json!(count));
            }
            if let Some(turns) = draft.interval_turns {
                value.insert("interval_turns".into(), json!(turns));
            }
            value.insert("forbid_overrides".into(), json!(false));
            value.insert("enabled".into(), json!(draft.enabled));
            if let Some(payload) = &draft.payload {
                value.insert(
                    "prompt_entry_payload".into(),
                    serde_json::to_value(payload).map_err(|_| PromptTransferError::Serialize)?,
                );
            }
            Ok(Value::Object(value))
        })
        .collect::<Result<Vec<_>, PromptTransferError>>()?;
    prompts.extend(SILLYTAVERN_MARKERS.iter().map(|(identifier, name)| {
        json!({
            "identifier": identifier,
            "name": name,
            "system_prompt": true,
            "marker": true,
        })
    }));
    let order = prompts
        .iter()
        .map(|prompt| {
            json!({
                "identifier": prompt.get("identifier").cloned().unwrap_or(json!("")),
                "enabled": prompt.get("enabled").cloned().unwrap_or(json!(true)),
            })
        })
        .collect::<Vec<_>>();
    let preset = json!({
        "impersonation_prompt": texts.impersonation_prompt,
        "new_chat_prompt": texts.new_chat_prompt,
        "new_group_chat_prompt": texts.new_group_chat_prompt,
        "new_example_chat_prompt": texts.new_example_chat_prompt,
        "continue_nudge_prompt": texts.continue_nudge_prompt,
        "scenario_format": texts.scenario_format,
        "personality_format": texts.personality_format,
        "group_nudge_prompt": texts.group_nudge_prompt,
        "wi_format": texts.wi_format,
        "prompts": prompts,
        "prompt_order": [{"character_id": 100_001, "order": order}],
    });
    serde_json::to_string_pretty(&preset).map_err(|_| PromptTransferError::Serialize)
}

/// A prompt file: a USC `system_prompt_template` card, else a SillyTavern
/// preset named after `file_stem`. `fallback_entry_name` names an unnamed
/// entry and `fallback_set_name` a preset without a file name.
pub fn parse_prompt_import(
    json: &str,
    file_stem: Option<&str>,
    fallback_entry_name: &str,
    fallback_set_name: &str,
) -> Result<ImportedPrompt, PromptTransferError> {
    let value: Value = serde_json::from_str(json)
        .map_err(|error| PromptTransferError::InvalidJson(error.to_string()))?;
    let payload = (value.pointer("/schema/name").and_then(Value::as_str) == Some("USC")
        && value.get("kind").and_then(Value::as_str) == Some("system_prompt_template"))
    .then(|| value.get("payload"))
    .flatten()
    .filter(|payload| !payload.is_null());
    match payload {
        Some(payload) => usc_prompt(payload, fallback_entry_name),
        None => sillytavern_prompt(&value, file_stem, fallback_entry_name, fallback_set_name),
    }
}

fn usc_prompt(
    payload: &Value,
    fallback_entry_name: &str,
) -> Result<ImportedPrompt, PromptTransferError> {
    let name = payload
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .ok_or(PromptTransferError::NameRequired)?
        .to_owned();
    let (purpose, purpose_defaulted) = match payload
        .get("promptType")
        .and_then(Value::as_str)
        .and_then(crate::legacy_prompt_purpose)
    {
        Some(PromptPurpose::Undefined) | None => (PromptPurpose::DirectChat, true),
        Some(purpose) => (purpose, false),
    };
    let mut entries = payload
        .get("entries")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| usc_entry(entry, purpose, fallback_entry_name))
        .collect::<Vec<_>>();
    let content = payload.get("content").and_then(Value::as_str).unwrap_or("");
    if entries.is_empty() && !content.is_empty() {
        entries.push(PromptEntryDraft {
            built_in_entry_key: None,
            name: "System Prompt".to_owned(),
            role: PromptEntryRole::System,
            content: content.to_owned(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
            conditions: None,
            payload: None,
        });
    }
    Ok(ImportedPrompt {
        name,
        purpose,
        entries,
        condense: payload.get("condensePromptEntries").is_some_and(truthy),
        purpose_defaulted,
    })
}

/// JavaScript truthiness of a JSON value.
pub(crate) fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(flag) => *flag,
        Value::Number(number) => number.as_f64().is_some_and(|number| number != 0.0),
        Value::String(text) => !text.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

fn clamped(value: Option<&Value>, floor: u32) -> Option<u32> {
    value
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .map(|value| value.floor().max(f64::from(floor)).min(f64::from(u32::MAX)) as u32)
}

fn role(value: Option<&Value>) -> PromptEntryRole {
    match value.and_then(Value::as_str) {
        Some("user") => PromptEntryRole::User,
        Some("assistant") => PromptEntryRole::Assistant,
        _ => PromptEntryRole::System,
    }
}

fn image_slot(
    kind: Option<&Value>,
    slot: Option<&Value>,
    slots: &[&str],
) -> Option<PromptEntryPayload> {
    if kind.and_then(Value::as_str) != Some("imageSlot") {
        return None;
    }
    let slot = slot
        .and_then(Value::as_str)
        .filter(|slot| slots.contains(slot))?;
    serde_json::from_value::<PromptEntryImageSlot>(json!(slot))
        .ok()
        .map(|slot| PromptEntryPayload::ImageSlot { slot })
}

/// An interval entry without a turn count never fired; it is kept as an
/// entry that cannot fire.
fn never_firing_interval(mut entry: PromptEntryDraft) -> PromptEntryDraft {
    if entry.injection_position == PromptEntryPosition::Interval && entry.interval_turns.is_none() {
        entry.interval_turns = Some(1);
        entry.enabled = false;
        entry.system_prompt = false;
    }
    entry
}

fn usc_entry(
    entry: &Value,
    purpose: PromptPurpose,
    fallback_name: &str,
) -> Option<PromptEntryDraft> {
    let content = entry.get("content").and_then(Value::as_str).unwrap_or("");
    if content.trim().is_empty() {
        return None;
    }
    let id = entry.get("id").and_then(Value::as_str).unwrap_or_default();
    let payload = entry
        .get("promptEntryPayload")
        .filter(|payload| payload.is_object());
    Some(never_firing_interval(PromptEntryDraft {
        built_in_entry_key: None,
        name: entry
            .get("name")
            .and_then(Value::as_str)
            .filter(|name| !name.trim().is_empty())
            .unwrap_or(fallback_name)
            .to_owned(),
        role: role(entry.get("role")),
        content: content.to_owned(),
        enabled: entry
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        injection_position: match entry.get("injectionPosition").and_then(Value::as_str) {
            Some("conditional") => PromptEntryPosition::Conditional,
            Some("interval") => PromptEntryPosition::Interval,
            Some("inChat") => PromptEntryPosition::InChat,
            _ => PromptEntryPosition::Relative,
        },
        depth: clamped(entry.get("injectionDepth"), 0).unwrap_or(0),
        conditional_min_messages: clamped(entry.get("conditionalMinMessages"), 1),
        interval_turns: clamped(entry.get("intervalTurns"), 1),
        system_prompt: entry
            .get("systemPrompt")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        conditions: lettuce_context::legacy_scene_protocol_conditions(
            purpose,
            id,
            entry
                .get("conditions")
                .filter(|conditions| conditions.is_object())
                .and_then(|conditions| serde_json::from_value(conditions.clone()).ok()),
        ),
        payload: image_slot(
            payload.and_then(|payload| payload.get("type")),
            payload.and_then(|payload| payload.get("slot")),
            &[
                "character",
                "persona",
                "chatBackground",
                "avatar",
                "references",
            ],
        ),
    }))
}

fn prompt_order(value: &Value, into: &mut Vec<(String, Option<bool>)>) {
    match value {
        Value::Array(items) => items.iter().for_each(|item| prompt_order(item, into)),
        Value::Object(object) => {
            if let Some(identifier) = object
                .get("identifier")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|identifier| !identifier.is_empty())
            {
                into.push((
                    identifier.to_owned(),
                    object.get("enabled").and_then(Value::as_bool),
                ));
            }
            if let Some(order) = object.get("order").filter(|order| !order.is_null()) {
                prompt_order(order, into);
            }
        }
        _ => {}
    }
}

fn order_blocks(value: &Value, into: &mut Vec<Vec<(String, Option<bool>)>>) {
    match value {
        Value::Array(items) => items.iter().for_each(|item| order_blocks(item, into)),
        Value::Object(object) => {
            if let Some(order) = object.get("order").filter(|order| !order.is_null()) {
                let mut block = Vec::new();
                prompt_order(order, &mut block);
                if !block.is_empty() {
                    into.push(block);
                }
            }
            object.values().for_each(|item| order_blocks(item, into));
        }
        _ => {}
    }
}

fn sillytavern_prompt(
    value: &Value,
    file_stem: Option<&str>,
    fallback_entry_name: &str,
    fallback_set_name: &str,
) -> Result<ImportedPrompt, PromptTransferError> {
    let prompts = value
        .get("prompts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let identifiers = prompts
        .iter()
        .filter_map(|prompt| prompt.get("identifier").and_then(Value::as_str))
        .map(str::trim)
        .filter(|identifier| !identifier.is_empty())
        .collect::<BTreeSet<_>>();
    let order = value.get("prompt_order").cloned().unwrap_or(Value::Null);
    let mut blocks = Vec::new();
    order_blocks(&order, &mut blocks);
    let refs = if blocks.is_empty() {
        let mut flat = Vec::new();
        prompt_order(&order, &mut flat);
        flat
    } else {
        let matches = |block: &Vec<(String, Option<bool>)>| {
            block
                .iter()
                .filter(|(identifier, _)| identifiers.contains(identifier.as_str()))
                .count()
        };
        let mut best = 0;
        for (index, block) in blocks.iter().enumerate().skip(1) {
            let current = &blocks[best];
            if (matches(block), block.len()) > (matches(current), current.len()) {
                best = index;
            }
        }
        blocks.swap_remove(best)
    };
    let mut order_index = BTreeMap::new();
    let mut enabled = BTreeMap::new();
    for (index, (identifier, flag)) in refs.into_iter().enumerate() {
        order_index.entry(identifier.clone()).or_insert(index);
        if let Some(flag) = flag {
            enabled.insert(identifier, flag);
        }
    }
    let mut entries = prompts
        .iter()
        .enumerate()
        .filter_map(|(index, prompt)| {
            let (id, mut entry) = sillytavern_entry(prompt, index, fallback_entry_name)?;
            if let Some(flag) = enabled.get(&id) {
                entry.enabled = *flag;
            }
            Some((
                order_index.get(&id).copied().unwrap_or(usize::MAX),
                index,
                entry,
            ))
        })
        .collect::<Vec<_>>();
    entries.sort_by_key(|(order, index, _)| (*order, *index));
    if entries.is_empty() {
        return Err(PromptTransferError::NoImportablePrompts);
    }
    let name = file_stem
        .map(str::trim)
        .filter(|stem| !stem.is_empty())
        .unwrap_or(fallback_set_name)
        .to_owned();
    Ok(ImportedPrompt {
        name,
        purpose: PromptPurpose::DirectChat,
        entries: entries.into_iter().map(|(_, _, entry)| entry).collect(),
        condense: false,
        purpose_defaulted: true,
    })
}

fn sillytavern_entry(
    prompt: &Value,
    index: usize,
    fallback_name: &str,
) -> Option<(String, PromptEntryDraft)> {
    let identifier = prompt
        .get("identifier")
        .and_then(Value::as_str)
        .filter(|identifier| !identifier.trim().is_empty());
    if prompt.get("marker").is_some_and(truthy)
        || identifier.is_some_and(|identifier| {
            SILLYTAVERN_MARKERS
                .iter()
                .any(|(marker, _)| *marker == identifier)
        })
    {
        return None;
    }
    let content = sillytavern_variables(prompt.get("content").and_then(Value::as_str)?);
    if content.trim().is_empty() {
        return None;
    }
    let id = identifier.map_or_else(
        || format!("imported_{index}_{}", Uuid::new_v4().simple()),
        str::to_owned,
    );
    let position = prompt.get("injection_position");
    let payload_field = |key: &str| {
        prompt
            .get("prompt_entry_payload")
            .and_then(|payload| payload.get(key))
            .filter(|value| !value.is_null())
            .or_else(|| {
                prompt
                    .get("promptEntryPayload")
                    .and_then(|payload| payload.get(key))
            })
    };
    let entry = never_firing_interval(PromptEntryDraft {
        built_in_entry_key: None,
        name: prompt
            .get("name")
            .and_then(Value::as_str)
            .filter(|name| !name.trim().is_empty())
            .unwrap_or(fallback_name)
            .to_owned(),
        role: role(prompt.get("role")),
        content,
        enabled: prompt
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        injection_position: match position {
            Some(Value::String(position)) if position == "conditional" => {
                PromptEntryPosition::Conditional
            }
            Some(Value::String(position)) if position == "interval" => {
                PromptEntryPosition::Interval
            }
            Some(Value::String(position)) if position == "inChat" => PromptEntryPosition::InChat,
            Some(Value::Number(number)) if number.as_f64() == Some(1.0) => {
                PromptEntryPosition::InChat
            }
            _ => PromptEntryPosition::Relative,
        },
        depth: prompt
            .get("injection_depth")
            .and_then(Value::as_f64)
            .filter(|depth| depth.is_finite())
            .map_or(0, |depth| depth.max(0.0).min(f64::from(u32::MAX)) as u32),
        conditional_min_messages: clamped(prompt.get("conditional_min_messages"), 1),
        interval_turns: clamped(prompt.get("interval_turns"), 1),
        system_prompt: false,
        conditions: lettuce_context::legacy_scene_protocol_conditions(
            PromptPurpose::DirectChat,
            &id,
            None,
        ),
        payload: image_slot(
            payload_field("type"),
            payload_field("slot"),
            &["character", "persona", "avatar", "references"],
        ),
    });
    Some((id, entry))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, content: &str) -> PromptTransferEntry {
        PromptTransferEntry {
            id: id.to_owned(),
            draft: PromptEntryDraft {
                built_in_entry_key: None,
                name: format!("Entry {id}"),
                role: PromptEntryRole::System,
                content: content.to_owned(),
                enabled: true,
                injection_position: PromptEntryPosition::InChat,
                depth: 2,
                conditional_min_messages: None,
                interval_turns: Some(3),
                system_prompt: false,
                conditions: None,
                payload: Some(PromptEntryPayload::ImageSlot {
                    slot: PromptEntryImageSlot::Persona,
                }),
            },
        }
    }

    fn prompt() -> PromptTransfer {
        PromptTransfer {
            id: "prompt-1".to_owned(),
            name: "Storyteller".to_owned(),
            purpose: PromptPurpose::CompanionChat,
            entries: vec![entry("a", "Stay in {{scenario}}."), entry("b", "Be kind.")],
            condense: true,
            created_at: 1,
            updated_at: 2,
        }
    }

    #[test]
    fn a_usc_card_reads_back_as_the_same_template() {
        let exported = export_prompt_usc(&prompt()).expect("export");
        assert!(exported.starts_with("{\n  \"schema\": {\n    \"name\": \"USC\""));
        let imported = parse_prompt_import(&exported, None, "Imported", "Set").expect("import");
        assert_eq!(imported.name, "Storyteller");
        assert_eq!(imported.purpose, PromptPurpose::CompanionChat);
        assert!(!imported.purpose_defaulted);
        assert!(imported.condense);
        assert_eq!(
            imported.entries,
            prompt()
                .entries
                .into_iter()
                .map(|entry| entry.draft)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn a_usc_card_is_normalized_like_the_old_import() {
        let card = json!({
            "schema": {"name": "USC", "version": "1.0"},
            "kind": "system_prompt_template",
            "payload": {
                "name": "  Old  ",
                "promptType": "mystery",
                "content": "Be brief.",
                "entries": [{"content": "   "}],
                "condensePromptEntries": "yes"
            }
        });
        let imported =
            parse_prompt_import(&card.to_string(), None, "Imported", "Set").expect("import");
        assert_eq!(imported.name, "Old");
        assert_eq!(imported.purpose, PromptPurpose::DirectChat);
        assert!(imported.purpose_defaulted);
        assert!(imported.condense);
        assert_eq!(imported.entries.len(), 1);
        assert_eq!(imported.entries[0].content, "Be brief.");
        assert!(imported.entries[0].system_prompt);
        let unnamed = json!({
            "schema": {"name": "USC"},
            "kind": "system_prompt_template",
            "payload": {"name": " "}
        });
        assert_eq!(
            parse_prompt_import(&unnamed.to_string(), None, "Imported", "Set"),
            Err(PromptTransferError::NameRequired)
        );
    }

    #[test]
    fn a_sillytavern_preset_round_trips_through_its_prompt_order() {
        let exported = export_prompt_sillytavern(&prompt()).expect("export");
        let value: Value = serde_json::from_str(&exported).expect("json");
        assert_eq!(value["prompts"].as_array().map(Vec::len), Some(10));
        assert_eq!(value["prompts"][0]["content"], "Stay in {{scene}}.");
        assert_eq!(value["prompts"][0]["injection_position"], 1);
        assert_eq!(value["prompt_order"][0]["character_id"], 100_001);
        let imported =
            parse_prompt_import(&exported, Some("preset"), "Imported", "Set").expect("import");
        assert_eq!(imported.name, "preset");
        assert_eq!(imported.purpose, PromptPurpose::DirectChat);
        assert_eq!(imported.entries.len(), 2);
        assert_eq!(imported.entries[0].content, "Stay in {{scene}}.");
        assert_eq!(
            imported.entries[0].injection_position,
            PromptEntryPosition::InChat
        );
        assert!(!imported.entries[0].system_prompt);
    }

    #[test]
    fn a_sillytavern_preset_follows_its_best_matching_order_block() {
        let preset = json!({
            "prompts": [
                {"identifier": "main", "name": "Main", "content": "Main prompt"},
                {"identifier": "extra", "content": "Extra {{personality}}", "injection_position": 0},
                {"identifier": "chatHistory", "marker": true},
                {"identifier": "blank", "content": "  "}
            ],
            "prompt_order": [
                {"character_id": 100000, "order": [{"identifier": "other"}]},
                {"character_id": 100001, "order": [
                    {"identifier": "extra", "enabled": false},
                    {"identifier": "main", "enabled": true}
                ]}
            ]
        });
        let imported =
            parse_prompt_import(&preset.to_string(), None, "Imported", "Set").expect("import");
        assert_eq!(imported.name, "Set");
        let entries = imported
            .entries
            .iter()
            .map(|entry| (entry.name.as_str(), entry.content.as_str(), entry.enabled))
            .collect::<Vec<_>>();
        assert_eq!(
            entries,
            vec![
                ("Imported", "Extra {{char.desc}}", false),
                ("Main", "Main prompt", true)
            ]
        );
        assert_eq!(
            parse_prompt_import(r#"{"prompts": []}"#, None, "Imported", "Set"),
            Err(PromptTransferError::NoImportablePrompts)
        );
    }

    #[test]
    fn legacy_entries_keep_their_scene_protocol_gate_and_silent_intervals() {
        let card = json!({
            "schema": {"name": "USC"},
            "kind": "system_prompt_template",
            "payload": {
                "name": "Scenes",
                "promptType": "directChat",
                "entries": [
                    {"id": "entry_scene_image_protocol", "name": "Protocol", "content": "Draw scenes."},
                    {"id": "e2", "name": "Nudge", "content": "Nudge.", "injectionPosition": "interval", "systemPrompt": true}
                ]
            }
        });
        let imported =
            parse_prompt_import(&card.to_string(), None, "Imported", "Set").expect("import");
        assert!(matches!(
            imported.entries[0].conditions,
            Some(PromptEntryCondition::All { .. })
        ));
        let nudge = &imported.entries[1];
        assert_eq!(nudge.interval_turns, Some(1));
        assert!(!nudge.enabled);
        assert!(!nudge.system_prompt);
    }
}
