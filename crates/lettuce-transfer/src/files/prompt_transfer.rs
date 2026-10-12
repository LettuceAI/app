//! Prompt templates read from USC system prompt cards and SillyTavern
//! prompt-manager presets.

use std::collections::{BTreeMap, BTreeSet};

use lettuce_context::{
    PromptEntryDraft, PromptEntryImageSlot, PromptEntryPayload, PromptEntryPosition,
    PromptEntryRole, PromptPurpose,
};
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

fn sillytavern_variables(content: &str) -> String {
    content
        .replace("{{scenario}}", "{{scene}}")
        .replace("{{personality}}", "{{char.desc}}")
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
    use lettuce_context::PromptEntryCondition;

    use super::*;

    #[test]
    fn a_usc_card_reads_with_every_entry_field() {
        let card = json!({
            "schema": {"name": "USC", "version": "1.0"},
            "kind": "system_prompt_template",
            "payload": {
                "id": "prompt-1",
                "name": "Storyteller",
                "promptType": "companionChat",
                "content": "",
                "entries": [{
                    "id": "a",
                    "name": "Entry a",
                    "role": "system",
                    "content": "Stay in the scene.",
                    "enabled": true,
                    "injectionPosition": "inChat",
                    "injectionDepth": 2,
                    "conditionalMinMessages": null,
                    "intervalTurns": 3,
                    "systemPrompt": false,
                    "conditions": null,
                    "promptEntryPayload": {"type": "imageSlot", "slot": "persona"}
                }],
                "condensePromptEntries": true,
                "createdAt": 1,
                "updatedAt": 2
            }
        });
        let imported =
            parse_prompt_import(&card.to_string(), None, "Imported", "Set").expect("import");
        assert_eq!(imported.name, "Storyteller");
        assert_eq!(imported.purpose, PromptPurpose::CompanionChat);
        assert!(!imported.purpose_defaulted);
        assert!(imported.condense);
        assert_eq!(
            imported.entries,
            vec![PromptEntryDraft {
                built_in_entry_key: None,
                name: "Entry a".to_owned(),
                role: PromptEntryRole::System,
                content: "Stay in the scene.".to_owned(),
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
            }]
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
    fn a_sillytavern_preset_reads_through_its_prompt_order() {
        let preset = json!({
            "prompts": [
                {"identifier": "a", "name": "Entry a", "system_prompt": false, "marker": false,
                 "content": "Stay in {{scenario}}.", "role": "system", "injection_position": 1,
                 "injection_depth": 2, "enabled": true},
                {"identifier": "chatHistory", "name": "Chat History", "system_prompt": true, "marker": true}
            ],
            "prompt_order": [{"character_id": 100_001, "order": [
                {"identifier": "a", "enabled": true},
                {"identifier": "chatHistory", "enabled": true}
            ]}]
        });
        let imported = parse_prompt_import(&preset.to_string(), Some("preset"), "Imported", "Set")
            .expect("import");
        assert_eq!(imported.name, "preset");
        assert_eq!(imported.purpose, PromptPurpose::DirectChat);
        assert_eq!(imported.entries.len(), 1);
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
