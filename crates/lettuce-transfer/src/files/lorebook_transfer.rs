//! Lorebooks as SillyTavern World Info JSON (read and write) and as a USC
//! lorebook card (write).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{PackagedKeywordDetectionMode, PackagedKeywordMatchMode};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LorebookTransferError {
    #[error("Invalid lorebook import JSON: {0}")]
    InvalidImport(String),
    #[error("Lorebook name is required")]
    MissingName,
    #[error("Failed to serialize lorebook export")]
    Serialize,
}

/// A lorebook as its files describe it.
#[derive(Debug, Clone, PartialEq)]
pub struct LorebookTransfer {
    pub id: String,
    pub name: String,
    pub keyword_detection_mode: PackagedKeywordDetectionMode,
    pub entries: Vec<LorebookTransferEntry>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LorebookTransferEntry {
    pub id: String,
    pub title: String,
    pub enabled: bool,
    pub always_active: bool,
    pub keywords: Vec<String>,
    pub case_sensitive: bool,
    pub keyword_match_mode: PackagedKeywordMatchMode,
    pub content: String,
    pub priority: i32,
    pub display_order: i32,
    pub created_at: i64,
    pub updated_at: i64,
}

/// A World Info file read into a new lorebook; entry ids and times belong to
/// the import. Entries are in their order, numbered from 0.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedWorldInfo {
    pub name: String,
    pub keyword_detection_mode: PackagedKeywordDetectionMode,
    pub entries: Vec<ImportedWorldInfoEntry>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportedWorldInfoEntry {
    pub title: String,
    pub enabled: bool,
    pub always_active: bool,
    pub keywords: Vec<String>,
    pub case_sensitive: bool,
    pub content: String,
    pub priority: i32,
    pub display_order: i32,
}

#[derive(Serialize)]
struct WorldInfoExport {
    name: String,
    description: String,
    is_creation: bool,
    scan_depth: i64,
    token_budget: i64,
    recursive_scanning: bool,
    extensions: Value,
    entries: BTreeMap<String, WorldInfoExportEntry>,
}

#[derive(Serialize)]
struct WorldInfoExportEntry {
    uid: i64,
    key: Vec<String>,
    keysecondary: Vec<String>,
    comment: String,
    content: String,
    constant: bool,
    selective: bool,
    #[serde(rename = "selectiveLogic")]
    selective_logic: i32,
    order: i32,
    position: i32,
    disable: bool,
    #[serde(rename = "addMemo")]
    add_memo: bool,
    #[serde(rename = "excludeRecursion")]
    exclude_recursion: bool,
    probability: i32,
    #[serde(rename = "displayIndex")]
    display_index: i32,
    #[serde(rename = "useProbability")]
    use_probability: bool,
    secondary_keys: Vec<String>,
    keys: Vec<String>,
    id: i64,
    priority: i32,
    insertion_order: i32,
    enabled: bool,
    name: String,
    extensions: Value,
    case_sensitive: bool,
    depth: i32,
    character_filter: Option<Value>,
}

#[derive(Deserialize)]
struct WorldInfoImport {
    name: String,
    #[serde(default)]
    extensions: Value,
    #[serde(default)]
    entries: Value,
}

const fn detection_mode_name(mode: PackagedKeywordDetectionMode) -> &'static str {
    match mode {
        PackagedKeywordDetectionMode::RecentMessageWindow => "recentMessageWindow",
        PackagedKeywordDetectionMode::LatestUserMessage => "latestUserMessage",
    }
}

/// Pretty-printed World Info; entries are keyed `"1"`, `"2"`, … in lexical
/// order and the keyword match mode is not written.
pub fn export_world_info(lorebook: &LorebookTransfer) -> Result<String, LorebookTransferError> {
    let entries = lorebook
        .entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let seq = index as i64 + 1;
            (
                seq.to_string(),
                WorldInfoExportEntry {
                    uid: seq,
                    key: entry.keywords.clone(),
                    keysecondary: Vec::new(),
                    comment: String::new(),
                    content: entry.content.clone(),
                    constant: entry.always_active,
                    selective: false,
                    selective_logic: 0,
                    order: entry.priority,
                    position: 1,
                    disable: !entry.enabled,
                    add_memo: true,
                    exclude_recursion: true,
                    probability: 100,
                    display_index: index as i32 + 1,
                    use_probability: true,
                    secondary_keys: Vec::new(),
                    keys: entry.keywords.clone(),
                    id: seq,
                    priority: entry.priority,
                    insertion_order: entry.display_order,
                    enabled: entry.enabled,
                    name: entry.title.clone(),
                    extensions: Value::Object(Map::new()),
                    case_sensitive: entry.case_sensitive,
                    depth: 4,
                    character_filter: None,
                },
            )
        })
        .collect();
    let mut extensions = Map::new();
    extensions.insert(
        "lettuceai".to_owned(),
        serde_json::json!({
            "keywordDetectionMode": detection_mode_name(lorebook.keyword_detection_mode)
        }),
    );
    serde_json::to_string_pretty(&WorldInfoExport {
        name: lorebook.name.clone(),
        description: String::new(),
        is_creation: false,
        scan_depth: 4,
        token_budget: 0,
        recursive_scanning: false,
        extensions: Value::Object(extensions),
        entries,
    })
    .map_err(|_| LorebookTransferError::Serialize)
}

fn number_to_i32(value: Option<&Value>) -> Option<i32> {
    value
        .and_then(|value| value.as_i64().or_else(|| value.as_u64().map(|n| n as i64)))
        .and_then(|value| i32::try_from(value).ok())
}

fn string_list(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(|item| item.trim().to_owned())
                .filter(|item| !item.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// Reads World Info: `keys`, else `key`; secondary keys are ignored; entries
/// without content are skipped; every keyword matches literally.
pub fn parse_world_info(json: &str) -> Result<ImportedWorldInfo, LorebookTransferError> {
    let parsed: WorldInfoImport = serde_json::from_str(json)
        .map_err(|error| LorebookTransferError::InvalidImport(error.to_string()))?;
    let raw: Vec<(Option<i64>, &Value)> = if let Some(map) = parsed.entries.as_object() {
        map.iter()
            .map(|(key, value)| (key.parse::<i64>().ok(), value))
            .collect()
    } else if let Some(list) = parsed.entries.as_array() {
        list.iter()
            .enumerate()
            .map(|(index, value)| (Some(index as i64), value))
            .collect()
    } else {
        Vec::new()
    };
    let mut entries = raw
        .into_iter()
        .enumerate()
        .filter_map(|(index, (map_index, value))| {
            let entry = value.as_object()?;
            let mut keywords = string_list(entry.get("keys"));
            if keywords.is_empty() {
                keywords = string_list(entry.get("key"));
            }
            let content = entry
                .get("content")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            if content.trim().is_empty() {
                return None;
            }
            let title = entry
                .get("name")
                .and_then(Value::as_str)
                .map(|name| name.trim().to_owned())
                .filter(|name| !name.is_empty())
                .or_else(|| keywords.first().cloned())
                .unwrap_or_else(|| format!("Entry {}", index + 1));
            let enabled = entry
                .get("enabled")
                .and_then(Value::as_bool)
                .unwrap_or_else(|| {
                    !entry
                        .get("disable")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                });
            let display_order = number_to_i32(entry.get("insertion_order"))
                .or_else(|| number_to_i32(entry.get("displayIndex")).map(|n| n.saturating_sub(1)))
                .or_else(|| {
                    map_index
                        .and_then(|n| i32::try_from(n).ok())
                        .map(|n| n.saturating_sub(1))
                })
                .unwrap_or(index as i32);
            Some(ImportedWorldInfoEntry {
                title,
                enabled,
                always_active: entry
                    .get("constant")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                keywords,
                case_sensitive: entry
                    .get("case_sensitive")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                content,
                priority: number_to_i32(entry.get("priority"))
                    .or_else(|| number_to_i32(entry.get("order")))
                    .unwrap_or(0),
                display_order,
            })
        })
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.display_order);
    for (index, entry) in entries.iter_mut().enumerate() {
        entry.display_order = index as i32;
    }
    let name = parsed.name.trim().to_owned();
    if name.is_empty() {
        return Err(LorebookTransferError::MissingName);
    }
    let keyword_detection_mode = match parsed
        .extensions
        .get("lettuceai")
        .and_then(|value| value.get("keywordDetectionMode"))
        .and_then(Value::as_str)
    {
        Some("latestUserMessage") => PackagedKeywordDetectionMode::LatestUserMessage,
        _ => PackagedKeywordDetectionMode::RecentMessageWindow,
    };
    Ok(ImportedWorldInfo {
        name,
        keyword_detection_mode,
        entries,
    })
}

#[derive(Serialize)]
struct UscLorebookCard {
    schema: UscSchema,
    kind: &'static str,
    payload: UscLorebookPayload,
}

#[derive(Serialize)]
struct UscSchema {
    name: &'static str,
    version: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UscLorebookPayload {
    id: String,
    name: String,
    keyword_detection_mode: &'static str,
    entries: Vec<UscLorebookEntry>,
    created_at: i64,
    updated_at: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UscLorebookEntry {
    id: String,
    title: String,
    enabled: bool,
    always_active: bool,
    keywords: Vec<String>,
    case_sensitive: bool,
    content: String,
    priority: i32,
    display_order: i32,
    created_at: i64,
    updated_at: i64,
}

/// Pretty-printed USC 1.0 lorebook card; entries carry no lorebook id or
/// keyword match mode.
pub fn export_lorebook_usc(lorebook: &LorebookTransfer) -> Result<String, LorebookTransferError> {
    serde_json::to_string_pretty(&UscLorebookCard {
        schema: UscSchema {
            name: "USC",
            version: "1.0",
        },
        kind: "lorebook",
        payload: UscLorebookPayload {
            id: lorebook.id.clone(),
            name: lorebook.name.clone(),
            keyword_detection_mode: detection_mode_name(lorebook.keyword_detection_mode),
            entries: lorebook
                .entries
                .iter()
                .map(|entry| UscLorebookEntry {
                    id: entry.id.clone(),
                    title: entry.title.clone(),
                    enabled: entry.enabled,
                    always_active: entry.always_active,
                    keywords: entry.keywords.clone(),
                    case_sensitive: entry.case_sensitive,
                    content: entry.content.clone(),
                    priority: entry.priority,
                    display_order: entry.display_order,
                    created_at: entry.created_at,
                    updated_at: entry.updated_at,
                })
                .collect(),
            created_at: lorebook.created_at,
            updated_at: lorebook.updated_at,
        },
    })
    .map_err(|_| LorebookTransferError::Serialize)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn lorebook() -> LorebookTransfer {
        let entry = |index: i32, title: &str| LorebookTransferEntry {
            id: format!("entry-{index}"),
            title: title.into(),
            enabled: index != 2,
            always_active: index == 1,
            keywords: vec![format!("k{index}")],
            case_sensitive: false,
            keyword_match_mode: PackagedKeywordMatchMode::Regex,
            content: format!("content {index}"),
            priority: index * 10,
            display_order: index,
            created_at: 1,
            updated_at: 2,
        };
        LorebookTransfer {
            id: "book".into(),
            name: "Harbour".into(),
            keyword_detection_mode: PackagedKeywordDetectionMode::LatestUserMessage,
            entries: (0..11).map(|index| entry(index, "Title")).collect(),
            created_at: 1,
            updated_at: 2,
        }
    }

    #[test]
    fn world_info_export_keeps_the_old_layout() {
        let exported = export_world_info(&lorebook()).expect("export");
        let value: Value = serde_json::from_str(&exported).expect("json");
        let keys = value["entries"]
            .as_object()
            .expect("entries")
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(&keys[..3], ["1", "10", "11"]);
        assert_eq!(
            value["extensions"]["lettuceai"]["keywordDetectionMode"],
            "latestUserMessage"
        );
        assert_eq!(value["entries"]["3"]["disable"], true);
        assert_eq!(value["entries"]["2"]["constant"], true);
        assert!(exported.starts_with(
            "{\n  \"name\": \"Harbour\",\n  \"description\": \"\",\n  \"is_creation\": false,"
        ));
        assert!(exported.contains(
            "\"uid\": 1,\n      \"key\": [\n        \"k0\"\n      ],\n      \"keysecondary\": [],"
        ));
        let read = parse_world_info(&exported).expect("read back");
        assert_eq!(read.name, "Harbour");
        assert_eq!(
            read.keyword_detection_mode,
            PackagedKeywordDetectionMode::LatestUserMessage
        );
        assert_eq!(read.entries.len(), 11);
        assert_eq!(read.entries[2].content, "content 2");
        assert!(!read.entries[2].enabled);
        assert!(read.entries[1].always_active);
        assert_eq!(read.entries[3].priority, 30);
    }

    #[test]
    fn world_info_import_follows_the_old_rules() {
        let read = parse_world_info(
            &json!({
                "name": " Notes ",
                "entries": [
                    {"key": ["a", " "], "keysecondary": ["x"], "content": "second", "displayIndex": 3, "order": 4},
                    {"keys": [], "content": "  "},
                    {"name": "First", "keys": ["b"], "content": "first", "insertion_order": 0, "disable": true, "constant": true, "case_sensitive": true}
                ]
            })
            .to_string(),
        )
        .expect("world info");
        assert_eq!(read.name, "Notes");
        assert_eq!(read.entries.len(), 2);
        assert_eq!(read.entries[0].title, "First");
        assert!(
            !read.entries[0].enabled
                && read.entries[0].always_active
                && read.entries[0].case_sensitive
        );
        assert_eq!(read.entries[1].title, "a");
        assert_eq!(read.entries[1].keywords, vec!["a".to_owned()]);
        assert_eq!(read.entries[1].priority, 4);
        assert_eq!(read.entries[1].display_order, 1);
        assert_eq!(
            parse_world_info(&json!({"name": "  "}).to_string()),
            Err(LorebookTransferError::MissingName)
        );
    }

    #[test]
    fn usc_lorebook_export_omits_the_lorebook_id_and_match_mode() {
        let value: Value =
            serde_json::from_str(&export_lorebook_usc(&lorebook()).expect("usc")).expect("json");
        assert_eq!(value["schema"], json!({"name": "USC", "version": "1.0"}));
        assert_eq!(value["kind"], "lorebook");
        assert_eq!(
            value["payload"]["keywordDetectionMode"],
            "latestUserMessage"
        );
        let entry = value["payload"]["entries"][0].as_object().expect("entry");
        assert!(!entry.contains_key("lorebookId") && !entry.contains_key("keywordMatchMode"));
        assert!(value.get("app_specific_settings").is_none());
    }
}
