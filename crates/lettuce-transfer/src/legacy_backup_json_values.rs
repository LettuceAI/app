use std::{collections::BTreeSet, str::FromStr};

use lettuce_characters::{
    CharacterDefaults, ChatAppearanceV1, InteractionMode, LegacyVoiceLocatorV1, VoicePreference,
    WidgetNode,
};
use lettuce_companions::{
    CompanionPromptingConfig, CompanionSoulConfig, CompanionSoulIdentity, RelationshipDefaults,
    SoulCategory, SoulFact, initial_soul_state,
};
use lettuce_types::{CharacterId, TimestampMillis, VoiceProfileId};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Map, Value, json};
use uuid::Uuid;

use crate::{
    LegacyBackupConfigurationPlan, LegacyBackupDocumentKind, LegacyImportSkip,
    LegacyImportSkipKind, LegacyImportSkipReason, legacy_backup_configuration::LEGACY_ID_NAMESPACE,
    legacy_value_skip,
};

const COMPANION_SECTIONS: [&str; 7] = [
    "soul",
    "authoredFacts",
    "relationshipDefaults",
    "prompting",
    "timeAwareness",
    "context",
    "memory",
];
const SOUL_FACT_KEYS: [&str; 17] = [
    "id",
    "category",
    "value",
    "kind",
    "policy",
    "slot",
    "confidence",
    "evidenceCount",
    "weight",
    "validFrom",
    "validUntil",
    "locked",
    "sourceMemoryIds",
    "createdAt",
    "supersedes",
    "supersededBy",
    "supersededAt",
];
const GLOBAL_APPEARANCE_FIELD: &str = "settings.advanced_settings.chatAppearance";

/// Values every legacy character and group JSON mapping shares: the legacy
/// global chat appearance already merged over the defaults, and the user
/// voices the configuration plan imports.
pub(crate) struct LegacyJsonContext {
    base_chat_appearance: Map<String, Value>,
    user_voice_ids: BTreeSet<VoiceProfileId>,
}

impl LegacyJsonContext {
    pub(crate) fn new(
        configuration: &LegacyBackupConfigurationPlan,
        skipped: &mut Vec<LegacyImportSkip>,
    ) -> Self {
        let mut base_chat_appearance = default_appearance();
        if let Some(global) = configuration
            .source
            .documents
            .iter()
            .find(|document| document.kind == LegacyBackupDocumentKind::Settings)
            .and_then(|document| serde_json::from_slice::<Value>(&document.bytes).ok())
            .and_then(|settings| {
                settings
                    .get("advanced_settings")?
                    .get("chatAppearance")
                    .cloned()
            })
            .filter(|value| !value.is_null())
        {
            apply_appearance(
                &mut base_chat_appearance,
                &global,
                GLOBAL_APPEARANCE_FIELD,
                "settings",
                skipped,
            );
        }
        Self {
            base_chat_appearance,
            user_voice_ids: configuration
                .user_voices
                .iter()
                .map(|voice| voice.id)
                .collect(),
        }
    }
}

pub(crate) struct LegacyCompanion {
    pub(crate) soul: Option<CompanionSoulConfig>,
    pub(crate) prompt_source_id: Option<String>,
}

/// Legacy `characters.companion`: keys the rewrite has no field for are
/// dropped and recorded, missing values take the legacy defaults, and authored
/// facts are normalized the way legacy stored them.
pub(crate) fn legacy_companion(
    value: Option<Value>,
    character_key: &str,
    companion_mode: bool,
    created_at: TimestampMillis,
    skipped: &mut Vec<LegacyImportSkip>,
) -> LegacyCompanion {
    const FIELD: &str = "characters.companion";
    let none = || LegacyCompanion {
        soul: None,
        prompt_source_id: None,
    };
    let private_memory = || LegacyCompanion {
        soul: Some(CompanionSoulConfig {
            share_memory_across_chats: false,
            ..CompanionSoulConfig::default()
        }),
        prompt_source_id: None,
    };
    let Some(value) = value.filter(|value| !value.is_null()) else {
        return if companion_mode {
            private_memory()
        } else {
            none()
        };
    };
    let Value::Object(mut object) = value else {
        skipped.push(malformed(FIELD, character_key));
        return if companion_mode {
            private_memory()
        } else {
            none()
        };
    };
    if !companion_mode {
        skipped.push(unknown(FIELD, character_key));
        return none();
    }
    let extra = object
        .keys()
        .filter(|key| !COMPANION_SECTIONS.contains(&key.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    for key in extra {
        object.remove(&key);
        skipped.push(unknown(&format!("{FIELD}.{key}"), character_key));
    }
    let prompt_source_id = match object
        .get_mut("prompting")
        .and_then(Value::as_object_mut)
        .and_then(|prompting| prompting.remove("promptTemplateId"))
    {
        None | Some(Value::Null) => None,
        Some(Value::String(value)) => Some(value).filter(|value| !value.trim().is_empty()),
        Some(_) => {
            skipped.push(malformed(
                &format!("{FIELD}.prompting.promptTemplateId"),
                character_key,
            ));
            None
        }
    };
    let mut config = CompanionSoulConfig::default();
    let mut flag = |value: Option<Value>, field: &str| match value {
        None | Some(Value::Null) => false,
        Some(Value::Bool(value)) => value,
        Some(_) => {
            skipped.push(malformed(&format!("{FIELD}.{field}"), character_key));
            false
        }
    };
    let top_level = flag(object.remove("timeAwareness"), "timeAwareness");
    let context = match object.remove("context") {
        None | Some(Value::Null) => false,
        Some(Value::Object(mut context)) => {
            let enabled = flag(context.remove("timeAwareness"), "context.timeAwareness");
            for key in context.keys() {
                skipped.push(unknown(&format!("{FIELD}.context.{key}"), character_key));
            }
            enabled
        }
        Some(_) => {
            skipped.push(malformed(&format!("{FIELD}.context"), character_key));
            false
        }
    };
    config.time_awareness = top_level || context;
    config.share_memory_across_chats = match object.remove("memory") {
        None | Some(Value::Null) => true,
        Some(Value::Object(mut memory)) => {
            let shared = match memory.remove("sharedAcrossSessions") {
                None | Some(Value::Null) => true,
                Some(Value::Bool(value)) => value,
                Some(_) => {
                    skipped.push(malformed(
                        &format!("{FIELD}.memory.sharedAcrossSessions"),
                        character_key,
                    ));
                    false
                }
            };
            for key in memory.keys() {
                skipped.push(unknown(&format!("{FIELD}.memory.{key}"), character_key));
            }
            shared
        }
        Some(_) => {
            skipped.push(malformed(&format!("{FIELD}.memory"), character_key));
            false
        }
    };
    if let Some(soul) = object.remove("soul") {
        config.soul = merged_section::<CompanionSoulIdentity>(
            soul,
            &format!("{FIELD}.soul"),
            character_key,
            skipped,
        );
    }
    if let Some(defaults) = object.remove("relationshipDefaults") {
        config.relationship_defaults = merged_section::<RelationshipDefaults>(
            defaults,
            &format!("{FIELD}.relationshipDefaults"),
            character_key,
            skipped,
        );
    }
    if let Some(prompting) = object.remove("prompting") {
        config.prompting = merged_section::<CompanionPromptingConfig>(
            prompting,
            &format!("{FIELD}.prompting"),
            character_key,
            skipped,
        );
    }
    match object.remove("authoredFacts") {
        Some(Value::Array(items)) => {
            let mut ids = BTreeSet::new();
            for (index, item) in items.into_iter().enumerate() {
                let Some(fact) = legacy_soul_fact(item, character_key, index, created_at, skipped)
                else {
                    continue;
                };
                if ids.insert(fact.id.clone()) {
                    config.authored_facts.push(fact);
                } else {
                    skipped.push(malformed(
                        &format!("{FIELD}.authoredFacts[{index}].id"),
                        character_key,
                    ));
                }
            }
        }
        None | Some(Value::Null) => {}
        Some(_) => skipped.push(malformed(&format!("{FIELD}.authoredFacts"), character_key)),
    }
    if !valid_companion(&config, created_at) && !config.authored_facts.is_empty() {
        config.authored_facts.clear();
        skipped.push(malformed(&format!("{FIELD}.authoredFacts"), character_key));
    }
    if !valid_companion(&config, created_at) {
        config.relationship_defaults = RelationshipDefaults::default();
        skipped.push(malformed(
            &format!("{FIELD}.relationshipDefaults"),
            character_key,
        ));
    }
    if !valid_companion(&config, created_at) {
        config.prompting = CompanionPromptingConfig::default();
        skipped.push(malformed(&format!("{FIELD}.prompting"), character_key));
    }
    if !valid_companion(&config, created_at) {
        config.soul = CompanionSoulIdentity::default();
        skipped.push(malformed(&format!("{FIELD}.soul"), character_key));
    }
    if !valid_companion(&config, created_at) {
        skipped.push(malformed(FIELD, character_key));
        return private_memory();
    }
    LegacyCompanion {
        soul: Some(config),
        prompt_source_id,
    }
}

fn valid_companion(config: &CompanionSoulConfig, created_at: TimestampMillis) -> bool {
    CharacterDefaults {
        interaction_mode: InteractionMode::Companion,
        companion_soul: Some(config.clone()),
        ..CharacterDefaults::default()
    }
    .validate()
    .is_ok()
        && initial_soul_state(Some(config), created_at).is_ok()
}

fn legacy_soul_fact(
    item: Value,
    character_key: &str,
    index: usize,
    created_at: TimestampMillis,
    skipped: &mut Vec<LegacyImportSkip>,
) -> Option<SoulFact> {
    let field = format!("characters.companion.authoredFacts[{index}]");
    let Value::Object(mut object) = item else {
        skipped.push(malformed(&field, character_key));
        return None;
    };
    let extra = object
        .keys()
        .filter(|key| !SOUL_FACT_KEYS.contains(&key.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    for key in extra {
        object.remove(&key);
        skipped.push(unknown(&format!("{field}.{key}"), character_key));
    }
    let Some(category) = object
        .get("category")
        .and_then(Value::as_str)
        .and_then(|value| serde_json::from_value::<SoulCategory>(json!(value)).ok())
    else {
        skipped.push(unknown(&format!("{field}.category"), character_key));
        return None;
    };
    if object
        .get("value")
        .and_then(Value::as_str)
        .is_none_or(|value| value.trim().is_empty())
    {
        skipped.push(malformed(&format!("{field}.value"), character_key));
        return None;
    }
    match object.get("kind").and_then(Value::as_str) {
        Some("add" | "adjust" | "authored" | "consolidated") => {}
        Some(kind) if !kind.trim().is_empty() => {
            skipped.push(unknown(&format!("{field}.kind"), character_key));
            object.insert("kind".into(), json!("authored"));
        }
        _ => {
            object.insert("kind".into(), json!("authored"));
        }
    }
    let default_policy = match category {
        SoulCategory::Appearance
        | SoulCategory::Goals
        | SoulCategory::Likes
        | SoulCategory::Voice
        | SoulCategory::Boundaries => "current",
        _ => "adaptive",
    };
    match object.get("policy").and_then(Value::as_str) {
        Some("current" | "adaptive" | "historical") => {}
        Some(policy) if !policy.trim().is_empty() => {
            skipped.push(unknown(&format!("{field}.policy"), character_key));
            object.insert("policy".into(), json!(default_policy));
        }
        _ => {
            object.insert("policy".into(), json!(default_policy));
        }
    }
    if object
        .get("slot")
        .and_then(Value::as_str)
        .is_none_or(|slot| slot.trim().is_empty())
    {
        object.insert("slot".into(), json!(category.as_str()));
    }
    if object
        .get("id")
        .and_then(Value::as_str)
        .is_none_or(|id| id.trim().is_empty())
    {
        let identity = format!("soul-fact:{character_key}:{index}");
        object.insert(
            "id".into(),
            json!(Uuid::new_v5(&LEGACY_ID_NAMESPACE, identity.as_bytes()).to_string()),
        );
    }
    for key in [
        "supersedes",
        "sourceMemoryIds",
        "validUntil",
        "supersededBy",
        "supersededAt",
    ] {
        if object.get(key).is_some_and(Value::is_null) {
            object.remove(key);
        }
    }
    for key in ["sourceMemoryIds", "supersedes"] {
        if let Some(Value::Array(ids)) = object.get_mut(key) {
            let before = ids.len();
            ids.retain(|id| id.as_str().is_some_and(|id| !id.trim().is_empty()));
            if ids.len() != before {
                skipped.push(malformed(&format!("{field}.{key}"), character_key));
            }
        }
    }
    let Ok(mut fact) = serde_json::from_value::<SoulFact>(Value::Object(object)) else {
        skipped.push(malformed(&field, character_key));
        return None;
    };
    if fact.superseded_by.is_some() && fact.superseded_at.is_none() {
        fact.superseded_at = Some(if fact.created_at.get() > 0 {
            fact.created_at
        } else {
            created_at
        });
        skipped.push(malformed(&format!("{field}.supersededAt"), character_key));
    } else if fact.superseded_by.is_none() && fact.superseded_at.is_some() {
        fact.superseded_at = None;
        skipped.push(malformed(&format!("{field}.supersededAt"), character_key));
    }
    if !valid_fact(&fact, created_at) && fact.valid_until.is_some() {
        fact.valid_until = None;
        skipped.push(malformed(&format!("{field}.validUntil"), character_key));
    }
    if !valid_fact(&fact, created_at) {
        skipped.push(malformed(&field, character_key));
        return None;
    }
    Some(fact)
}

fn valid_fact(fact: &SoulFact, created_at: TimestampMillis) -> bool {
    let config = CompanionSoulConfig {
        authored_facts: vec![fact.clone()],
        ..CompanionSoulConfig::default()
    };
    initial_soul_state(Some(&config), TimestampMillis::new(1)).is_ok()
        && initial_soul_state(Some(&config), created_at).is_ok()
}

fn merged_section<T: Default + Serialize + DeserializeOwned>(
    value: Value,
    field: &str,
    row: &str,
    skipped: &mut Vec<LegacyImportSkip>,
) -> T {
    let template = serde_json::to_value(T::default()).unwrap_or(Value::Null);
    let merged = merge_known(template, value, field, row, skipped);
    serde_json::from_value(merged).unwrap_or_else(|_| {
        skipped.push(malformed(field, row));
        T::default()
    })
}

fn merge_known(
    template: Value,
    value: Value,
    field: &str,
    row: &str,
    skipped: &mut Vec<LegacyImportSkip>,
) -> Value {
    match (template, value) {
        (Value::Object(mut template), Value::Object(value)) => {
            for (key, item) in value {
                let path = format!("{field}.{key}");
                match template.remove(&key) {
                    Some(default) => {
                        let merged = merge_known(default, item, &path, row, skipped);
                        template.insert(key, merged);
                    }
                    None => skipped.push(unknown(&path, row)),
                }
            }
            Value::Object(template)
        }
        (template, Value::Null) => template,
        (_, value) => value,
    }
}

/// Legacy `characters.voice_config`: a user voice resolves to the imported
/// voice profile, a provider voice has no profile row and is kept verbatim as
/// an unresolved legacy locator.
pub(crate) fn legacy_voice(
    raw: Option<String>,
    character_key: &str,
    context: &LegacyJsonContext,
    skipped: &mut Vec<LegacyImportSkip>,
) -> Option<VoicePreference> {
    const FIELD: &str = "characters.voice_config";
    let raw = raw?;
    let value = match serde_json::from_str::<Value>(&raw) {
        Ok(Value::Null) => return None,
        Ok(value) => value,
        Err(_) => {
            skipped.push(malformed(FIELD, character_key));
            return None;
        }
    };
    match value.get("source").and_then(Value::as_str) {
        Some("user") => {
            let Some(voice) = value
                .get("userVoiceId")
                .and_then(Value::as_str)
                .filter(|voice| !voice.trim().is_empty())
            else {
                skipped.push(malformed(FIELD, character_key));
                return None;
            };
            let id = VoiceProfileId::from_str(voice).unwrap_or_else(|_| {
                VoiceProfileId::from_uuid(Uuid::new_v5(
                    &LEGACY_ID_NAMESPACE,
                    format!("voice:{voice}").as_bytes(),
                ))
            });
            if context.user_voice_ids.contains(&id) {
                return Some(VoicePreference::VoiceProfile(id));
            }
            skipped.push(LegacyImportSkip {
                kind: LegacyImportSkipKind::VoiceReference,
                source_key: character_key.to_owned(),
                reason: LegacyImportSkipReason::MissingUserVoice,
            });
            None
        }
        Some("provider") => Some(VoicePreference::UnresolvedLegacy(LegacyVoiceLocatorV1 {
            locator: raw,
        })),
        _ => {
            skipped.push(unknown(FIELD, character_key));
            None
        }
    }
}

/// The complete appearance legacy displayed: its sparse override merged over
/// the legacy global appearance. A key that cannot be represented keeps the
/// inherited value and is recorded.
pub(crate) fn legacy_chat_appearance(
    raw: Option<&str>,
    field: &str,
    owner_key: &str,
    context: &LegacyJsonContext,
    skipped: &mut Vec<LegacyImportSkip>,
) -> ChatAppearanceV1 {
    let mut current = context.base_chat_appearance.clone();
    match raw.map(serde_json::from_str::<Value>) {
        None | Some(Ok(Value::Null)) => {}
        Some(Ok(value)) => apply_appearance(&mut current, &value, field, owner_key, skipped),
        Some(Err(_)) => skipped.push(malformed(field, owner_key)),
    }
    appearance(&current).unwrap_or_default()
}

fn default_appearance() -> Map<String, Value> {
    match serde_json::to_value(ChatAppearanceV1::default()) {
        Ok(Value::Object(map)) => map,
        _ => Map::new(),
    }
}

fn appearance(map: &Map<String, Value>) -> Option<ChatAppearanceV1> {
    serde_json::from_value::<ChatAppearanceV1>(Value::Object(map.clone()))
        .ok()
        .filter(|value| value.validate().is_ok())
}

fn apply_appearance(
    current: &mut Map<String, Value>,
    value: &Value,
    field: &str,
    row: &str,
    skipped: &mut Vec<LegacyImportSkip>,
) {
    let Value::Object(object) = value else {
        skipped.push(malformed(field, row));
        return;
    };
    let mut pending = Vec::new();
    for (key, item) in object {
        let snake = camel_to_snake(key);
        if snake == "format_version" || !current.contains_key(&snake) {
            skipped.push(unknown(&format!("{field}.{key}"), row));
            continue;
        }
        let converted = match (snake.as_str(), item) {
            ("chat_widget_slots", _) => {
                legacy_widget_slots(item, &format!("{field}.{key}"), row, skipped)
            }
            ("message_info_placement", Value::String(value)) => json!(camel_to_snake(value)),
            (name, Value::String(value))
                if name.ends_with("_color_hex") && value.trim().is_empty() =>
            {
                Value::Null
            }
            _ => item.clone(),
        };
        pending.push((key.clone(), snake, converted));
    }
    let custom_width = pending
        .iter()
        .any(|(_, snake, item)| snake == "chat_column_width" && item.as_str() == Some("custom"));
    if custom_width
        && current
            .get("chat_column_width_px")
            .is_none_or(Value::is_null)
        && !pending
            .iter()
            .any(|(_, snake, _)| snake == "chat_column_width_px")
    {
        pending.push((
            "chatColumnWidthPx".to_owned(),
            "chat_column_width_px".to_owned(),
            json!(800),
        ));
    }
    loop {
        let before = pending.len();
        pending.retain(|(_, snake, item)| {
            let mut candidate = current.clone();
            candidate.insert(snake.clone(), item.clone());
            if appearance(&candidate).is_some() {
                *current = candidate;
                false
            } else {
                true
            }
        });
        if pending.is_empty() || pending.len() == before {
            break;
        }
    }
    for (key, _, _) in pending {
        skipped.push(malformed(&format!("{field}.{key}"), row));
    }
}

fn legacy_widget_slots(
    value: &Value,
    field: &str,
    row: &str,
    skipped: &mut Vec<LegacyImportSkip>,
) -> Value {
    if !value.is_object() {
        skipped.push(malformed(field, row));
        return json!({ "left": [], "right": [] });
    }
    let mut slots = Map::new();
    for side in ["left", "right"] {
        let nodes = match value.get(side) {
            None | Some(Value::Null) => Vec::new(),
            Some(Value::Array(items)) => {
                let mut nodes = Vec::with_capacity(items.len());
                for (index, item) in items.iter().enumerate() {
                    let path = format!("{field}.{side}[{index}]");
                    let node = legacy_widget_node(item, &path, row, skipped);
                    if valid_widget(&node) {
                        nodes.push(node);
                    } else {
                        skipped.push(malformed(&path, row));
                    }
                }
                nodes
            }
            Some(_) => {
                skipped.push(malformed(&format!("{field}.{side}"), row));
                Vec::new()
            }
        };
        slots.insert(side.to_owned(), Value::Array(nodes));
    }
    Value::Object(slots)
}

fn valid_widget(node: &Value) -> bool {
    serde_json::from_value::<WidgetNode>(node.clone()).is_ok_and(|parsed| parsed.validate().is_ok())
}

fn legacy_widget_node(
    value: &Value,
    field: &str,
    row: &str,
    skipped: &mut Vec<LegacyImportSkip>,
) -> Value {
    let Value::Object(object) = value else {
        return value.clone();
    };
    let mut node = Map::new();
    for (key, item) in object {
        match key.as_str() {
            "children" => {
                let mut children = Vec::new();
                for (index, child) in item.as_array().into_iter().flatten().enumerate() {
                    let path = format!("{field}.children[{index}]");
                    let converted = legacy_widget_node(child, &path, row, skipped);
                    if valid_widget(&converted) {
                        children.push(converted);
                    } else {
                        skipped.push(malformed(&path, row));
                    }
                }
                node.insert("children".into(), Value::Array(children));
            }
            "stats" | "snippets" => {
                let mut entries = Vec::new();
                for (index, entry) in item.as_array().into_iter().flatten().enumerate() {
                    if ["id", "label"].iter().all(|name| {
                        entry
                            .get(name)
                            .and_then(Value::as_str)
                            .is_some_and(|text| !text.trim().is_empty())
                    }) {
                        entries.push(entry.clone());
                    } else {
                        skipped.push(malformed(&format!("{field}.{key}[{index}]"), row));
                    }
                }
                node.insert(key.clone(), Value::Array(entries));
            }
            "source" => {
                node.insert("source".into(), legacy_widget_source(item));
            }
            "hourFormat" => {
                let format = match item.as_str() {
                    Some("12h") => json!("h12"),
                    Some("24h") => json!("h24"),
                    _ => item.clone(),
                };
                node.insert("hour_format".into(), format);
            }
            "characterId" => {
                if item
                    .as_str()
                    .is_some_and(|id| CharacterId::from_str(id).is_ok())
                {
                    node.insert("character_id".into(), item.clone());
                } else if !item.is_null() {
                    skipped.push(malformed(&format!("{field}.characterId"), row));
                }
            }
            _ => {
                node.insert(camel_to_snake(key), item.clone());
            }
        }
    }
    let list = match node.get("type").and_then(Value::as_str) {
        Some("box") => Some("children"),
        Some("stat_tracker") => Some("stats"),
        Some("quick_snippets") => Some("snippets"),
        _ => None,
    };
    if let Some(list) = list
        && !node.contains_key(list)
    {
        node.insert(list.into(), Value::Array(Vec::new()));
    }
    Value::Object(node)
}

fn legacy_widget_source(value: &Value) -> Value {
    match (
        value.get("kind").and_then(Value::as_str),
        value.get("path").and_then(Value::as_str),
    ) {
        (Some("library" | "upload"), Some(path)) => {
            json!({ "kind": "unresolved_legacy", "token": path })
        }
        _ => value.clone(),
    }
}

fn camel_to_snake(key: &str) -> String {
    let mut snake = String::with_capacity(key.len() + 4);
    for character in key.chars() {
        if character.is_ascii_uppercase() {
            snake.push('_');
            snake.push(character.to_ascii_lowercase());
        } else {
            snake.push(character);
        }
    }
    snake
}

fn malformed(field: &str, row: &str) -> LegacyImportSkip {
    legacy_value_skip(field, row, LegacyImportSkipReason::MalformedLegacyValue)
}

fn unknown(field: &str, row: &str) -> LegacyImportSkip {
    legacy_value_skip(field, row, LegacyImportSkipReason::UnknownLegacyValue)
}

#[cfg(test)]
mod tests {
    use lettuce_characters::{HourFormat, WidgetImageSource};
    use lettuce_companions::SoulFactPolicy;

    use super::*;

    fn context(global: Option<Value>) -> (LegacyJsonContext, Vec<LegacyImportSkip>) {
        let mut skipped = Vec::new();
        let mut base_chat_appearance = default_appearance();
        if let Some(global) = global {
            apply_appearance(
                &mut base_chat_appearance,
                &global,
                GLOBAL_APPEARANCE_FIELD,
                "settings",
                &mut skipped,
            );
        }
        (
            LegacyJsonContext {
                base_chat_appearance,
                user_voice_ids: BTreeSet::new(),
            },
            skipped,
        )
    }

    #[test]
    fn legacy_companion_keeps_the_soul_with_legacy_defaults_and_records_dropped_keys() {
        let mut skipped = Vec::new();
        let companion = legacy_companion(
            Some(json!({
                "soul": {"essence": "Kind", "baselineAffect": {"warmth": 0.9}, "mood": "calm"},
                "authoredFacts": [
                    {"category": "likes", "value": "tea"},
                    {"category": "hobbies", "value": "chess"}
                ],
                "relationshipDefaults": {"trust": 0.4},
                "prompting": {"promptTemplateId": "prompt-1", "styleNotes": "soft"},
                "memory": {"enabled": true},
                "timeAwareness": true
            })),
            "character-1",
            true,
            TimestampMillis::new(1),
            &mut skipped,
        );

        let soul = companion.soul.expect("companion soul");
        assert_eq!(soul.soul.essence, "Kind");
        assert_eq!(soul.soul.baseline_affect.warmth, 0.9);
        assert_eq!(soul.soul.baseline_affect.trust, 0.35);
        assert_eq!(soul.authored_facts.len(), 1);
        assert_eq!(soul.authored_facts[0].policy, SoulFactPolicy::Current);
        assert_eq!(soul.authored_facts[0].slot, "likes");
        assert!(!soul.authored_facts[0].id.is_empty());
        assert_eq!(soul.relationship_defaults.trust, 0.4);
        assert_eq!(soul.relationship_defaults.closeness, 0.1);
        assert_eq!(soul.prompting.style_notes, "soft");
        assert!(soul.prompting.prompt_template_id.is_none());
        assert_eq!(companion.prompt_source_id.as_deref(), Some("prompt-1"));
        assert!(soul.time_awareness);
        assert_eq!(skipped.len(), 3);

        assert!(soul.share_memory_across_chats);
        assert!(soul.share_soul_growth_across_chats);

        let mut memory_skips = Vec::new();
        let private = legacy_companion(
            Some(json!({"memory": {"sharedAcrossSessions": false, "maxEntries": 5}})),
            "character-4",
            true,
            TimestampMillis::new(1),
            &mut memory_skips,
        );
        assert!(
            !private
                .soul
                .expect("companion soul")
                .share_memory_across_chats
        );
        assert_eq!(memory_skips.len(), 1);
        let unset = legacy_companion(
            None,
            "character-5",
            true,
            TimestampMillis::new(1),
            &mut memory_skips,
        );
        assert!(
            !unset
                .soul
                .expect("companion soul")
                .share_memory_across_chats
        );

        let mut context_skips = Vec::new();
        let context = legacy_companion(
            Some(json!({"context": {"timeAwareness": true, "extra": 1}, "timeAwareness": false})),
            "character-3",
            true,
            TimestampMillis::new(1),
            &mut context_skips,
        );
        assert!(context.soul.expect("companion soul").time_awareness);
        assert_eq!(context_skips.len(), 1);

        let mut roleplay = Vec::new();
        let dropped = legacy_companion(
            Some(json!({"soul": {}})),
            "character-2",
            false,
            TimestampMillis::new(1),
            &mut roleplay,
        );
        assert!(dropped.soul.is_none());
        assert_eq!(
            roleplay[0].reason,
            LegacyImportSkipReason::UnknownLegacyValue
        );
    }

    #[test]
    fn legacy_voice_resolves_imported_user_voices_and_keeps_provider_voices_verbatim() {
        let (mut context, _) = context(None);
        let voice = VoiceProfileId::new();
        context.user_voice_ids.insert(voice);
        let mut skipped = Vec::new();

        assert_eq!(
            legacy_voice(
                Some(format!(r#"{{"source":"user","userVoiceId":"{voice}"}}"#)),
                "character-1",
                &context,
                &mut skipped,
            ),
            Some(VoicePreference::VoiceProfile(voice))
        );
        let provider = r#"{"source":"provider","providerId":"p","voiceId":"v"}"#.to_owned();
        assert_eq!(
            legacy_voice(
                Some(provider.clone()),
                "character-1",
                &context,
                &mut skipped
            ),
            Some(VoicePreference::UnresolvedLegacy(LegacyVoiceLocatorV1 {
                locator: provider
            }))
        );
        assert!(skipped.is_empty());
        assert_eq!(
            legacy_voice(
                Some(format!(
                    r#"{{"source":"user","userVoiceId":"{}"}}"#,
                    VoiceProfileId::new()
                )),
                "character-2",
                &context,
                &mut skipped,
            ),
            None
        );
        assert_eq!(skipped[0].kind, LegacyImportSkipKind::VoiceReference);
        assert_eq!(skipped[0].reason, LegacyImportSkipReason::MissingUserVoice);
    }

    #[test]
    fn legacy_chat_appearance_merges_the_override_over_the_global_appearance() {
        let (context, global_skips) =
            context(Some(json!({"fontSize": "large", "bubbleOpacity": 50})));
        assert!(global_skips.is_empty());
        let mut skipped = Vec::new();

        let appearance = legacy_chat_appearance(
            Some(
                r#"{
                    "chatColumnWidthPx": 900,
                    "chatColumnWidth": "custom",
                    "backgroundDim": 500,
                    "unknownKey": 1,
                    "chatWidgetSlots": {"left": [
                        {"id": "clock", "type": "time", "hourFormat": "24h", "showSeconds": true},
                        {"id": "art", "type": "image", "source": {"kind": "library", "path": "img-1"}},
                        {"type": "dice"}
                    ]}
                }"#,
            ),
            "characters.chat_appearance",
            "character-1",
            &context,
            &mut skipped,
        );

        assert_eq!(
            serde_json::to_value(appearance.font_size).expect("font size"),
            json!("large")
        );
        assert_eq!(appearance.bubble_opacity, 50.0);
        assert_eq!(
            serde_json::to_value(appearance.chat_column_width).expect("width"),
            json!("custom")
        );
        assert_eq!(appearance.chat_column_width_px, Some(900));
        assert_eq!(appearance.background_dim, 0.0);
        assert_eq!(appearance.chat_widget_slots.left.len(), 2);
        assert!(matches!(
            &appearance.chat_widget_slots.left[0],
            WidgetNode::Time {
                hour_format: Some(HourFormat::H24),
                show_seconds: Some(true),
                ..
            }
        ));
        assert!(matches!(
            &appearance.chat_widget_slots.left[1],
            WidgetNode::Image {
                source: WidgetImageSource::UnresolvedLegacy { token },
                ..
            } if token == "img-1"
        ));
        assert_eq!(skipped.len(), 3);
    }

    #[test]
    fn legacy_appearance_converts_placements_blank_colors_and_unsized_custom_widths() {
        let (context, _) = context(None);
        let mut skipped = Vec::new();

        let appearance = legacy_chat_appearance(
            Some(
                r#"{"messageInfoPlacement": "belowHeaderOutside", "userBubbleColorHex": "", "chatColumnWidth": "custom"}"#,
            ),
            "group_characters.chat_appearance",
            "group-1",
            &context,
            &mut skipped,
        );

        assert_eq!(
            serde_json::to_value(appearance.message_info_placement).expect("placement"),
            json!("below_header_outside")
        );
        assert_eq!(appearance.user_bubble_color_hex, None);
        assert_eq!(appearance.chat_column_width_px, Some(800));
        assert!(skipped.is_empty());
    }

    #[test]
    fn legacy_widget_conversion_drops_only_the_invalid_child_or_entry() {
        let (context, _) = context(None);
        let mut skipped = Vec::new();

        let appearance = legacy_chat_appearance(
            Some(
                r#"{"chatWidgetSlots": {"right": [
                    {"id": "group", "type": "box", "children": [
                        {"id": "who", "type": "character_info", "characterId": "not-a-uuid"},
                        {"type": "divider"}
                    ]},
                    {"id": "stats", "type": "stat_tracker", "stats": [
                        {"id": "hp", "label": "HP", "value": 3},
                        {"id": "mp", "label": "", "value": 1}
                    ]}
                ]}}"#,
            ),
            "characters.chat_appearance",
            "character-1",
            &context,
            &mut skipped,
        );

        let right = &appearance.chat_widget_slots.right;
        assert_eq!(right.len(), 2);
        assert!(matches!(&right[0], WidgetNode::Box { children, .. } if children.len() == 1));
        assert!(matches!(&right[1], WidgetNode::StatTracker { stats, .. } if stats.len() == 1));
        assert_eq!(skipped.len(), 3);
    }

    #[test]
    fn legacy_companion_keeps_the_soul_when_facts_collide_or_expire_before_creation() {
        let mut skipped = Vec::new();
        let companion = legacy_companion(
            Some(json!({
                "soul": {"essence": "Kind"},
                "prompting": {"promptTemplateId": 7},
                "authoredFacts": [
                    {"id": "fact-1", "category": "likes", "value": "tea"},
                    {"id": "fact-1", "category": "goals", "value": "travel"},
                    {"id": "fact-2", "category": "habits", "value": "reads", "validUntil": 5},
                    {
                        "id": "fact-3",
                        "category": "fears",
                        "value": "storms",
                        "supersededBy": "fact-9",
                        "supersedes": ["", "fact-0"]
                    }
                ]
            })),
            "character-1",
            true,
            TimestampMillis::new(1_000),
            &mut skipped,
        );

        let soul = companion.soul.expect("companion soul");
        assert_eq!(soul.soul.essence, "Kind");
        assert_eq!(soul.authored_facts.len(), 3);
        assert!(soul.authored_facts[1].valid_until.is_none());
        assert_eq!(
            soul.authored_facts[2].superseded_at,
            Some(TimestampMillis::new(1_000))
        );
        assert_eq!(soul.authored_facts[2].supersedes, ["fact-0"]);
        assert!(companion.prompt_source_id.is_none());
        assert_eq!(skipped.len(), 5);
    }
}
