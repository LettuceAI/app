//! Lorebooks as SillyTavern World Info JSON (read and write) and as a USC
//! lorebook card (read).

use std::collections::BTreeMap;

use lettuce_context::{SecondaryKeywordLogic, is_phrase_pattern, literal_keyword_regex};
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
    pub secondary_keywords: Vec<String>,
    pub secondary_logic: SecondaryKeywordLogic,
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
    pub secondary_keywords: Vec<String>,
    pub secondary_logic: SecondaryKeywordLogic,
    pub case_sensitive: bool,
    pub keyword_match_mode: PackagedKeywordMatchMode,
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

pub(crate) const fn secondary_logic_name(logic: SecondaryKeywordLogic) -> &'static str {
    match logic {
        SecondaryKeywordLogic::AndAny => "and_any",
        SecondaryKeywordLogic::NotAll => "not_all",
        SecondaryKeywordLogic::NotAny => "not_any",
        SecondaryKeywordLogic::AndAll => "and_all",
    }
}

/// SillyTavern's `selectiveLogic` number; a missing or unknown value is
/// `and_any`.
pub(crate) const fn secondary_logic_from_value(value: Option<i64>) -> SecondaryKeywordLogic {
    match value {
        Some(1) => SecondaryKeywordLogic::NotAll,
        Some(2) => SecondaryKeywordLogic::NotAny,
        Some(3) => SecondaryKeywordLogic::AndAll,
        _ => SecondaryKeywordLogic::AndAny,
    }
}

const fn secondary_logic_value(logic: SecondaryKeywordLogic) -> i32 {
    match logic {
        SecondaryKeywordLogic::AndAny => 0,
        SecondaryKeywordLogic::NotAll => 1,
        SecondaryKeywordLogic::NotAny => 2,
        SecondaryKeywordLogic::AndAll => 3,
    }
}

const fn detection_mode_name(mode: PackagedKeywordDetectionMode) -> &'static str {
    match mode {
        PackagedKeywordDetectionMode::RecentMessageWindow => "recentMessageWindow",
        PackagedKeywordDetectionMode::LatestUserMessage => "latestUserMessage",
    }
}

/// Pretty-printed World Info; entries are keyed `"1"`, `"2"`, … in lexical
/// order and the keyword match mode is not written. Secondary keywords are
/// written as `keysecondary` and `secondary_keys` with `selective` set when
/// there are any, and the logic as `selectiveLogic`.
pub fn export_world_info(lorebook: &LorebookTransfer) -> Result<String, LorebookTransferError> {
    let entries = lorebook
        .entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let seq = index as i64 + 1;
            let keys = export_keys(entry, &entry.keywords);
            let secondary = export_keys(entry, &entry.secondary_keywords);
            (
                seq.to_string(),
                WorldInfoExportEntry {
                    uid: seq,
                    key: keys.clone(),
                    keysecondary: secondary.clone(),
                    comment: String::new(),
                    content: entry.content.clone(),
                    constant: entry.always_active,
                    selective: !entry.secondary_keywords.is_empty(),
                    selective_logic: secondary_logic_value(entry.secondary_logic),
                    order: entry.priority,
                    position: 1,
                    disable: !entry.enabled,
                    add_memo: true,
                    exclude_recursion: true,
                    probability: 100,
                    display_index: index as i32 + 1,
                    use_probability: true,
                    secondary_keys: secondary,
                    keys,
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

/// A SillyTavern `/pattern/flags` key as its pattern and whether it ignores case.
fn slash_regex(key: &str) -> Option<(String, bool)> {
    let rest = key.strip_prefix('/')?;
    let end = rest.rfind('/')?;
    let (pattern, flags) = (&rest[..end], &rest[end + 1..]);
    (!pattern.is_empty() && flags.chars().all(|flag| "dgimsuvy".contains(flag)))
        .then(|| (pattern.to_owned(), flags.contains('i')))
}

/// Keys as SillyTavern reads them: a regex entry, or a literal entry that uses
/// phrase patterns, is written as `/pattern/flags` keys, every other entry as
/// plain keys.
fn export_keys(entry: &LorebookTransferEntry, keys: &[String]) -> Vec<String> {
    let ignore_case = if entry.case_sensitive { "" } else { "i" };
    match entry.keyword_match_mode {
        PackagedKeywordMatchMode::Regex => keys
            .iter()
            .map(|key| format!("/{key}/{ignore_case}"))
            .collect(),
        PackagedKeywordMatchMode::Literal
            if entry
                .keywords
                .iter()
                .chain(&entry.secondary_keywords)
                .any(|key| is_phrase_pattern(key)) =>
        {
            keys.iter()
                .map(|key| format!("/{}/{ignore_case}u", literal_keyword_regex(key, entry.case_sensitive)))
                .collect()
        }
        PackagedKeywordMatchMode::Literal => keys.to_vec(),
    }
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

/// Reads World Info: `keys`, else `key`; secondary keywords from
/// `keysecondary`, else `secondary_keys`, count unless `selective` is false,
/// with `selectiveLogic` (else `extensions.selectiveLogic`) as the logic;
/// entries without content are skipped; every keyword matches literally.
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
            let secondary_keywords = if entry.get("selective") == Some(&Value::Bool(false)) {
                Vec::new()
            } else {
                let mut secondary = string_list(entry.get("keysecondary"));
                if secondary.is_empty() {
                    secondary = string_list(entry.get("secondary_keys"));
                }
                secondary
            };
            let secondary_logic = secondary_logic_from_value(
                entry
                    .get("selectiveLogic")
                    .or_else(|| {
                        entry
                            .get("extensions")
                            .and_then(|value| value.get("selectiveLogic"))
                    })
                    .and_then(Value::as_i64),
            );
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
            let mut case_sensitive = entry
                .get("case_sensitive")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let mut keyword_match_mode = PackagedKeywordMatchMode::Literal;
            let (mut keywords, mut secondary_keywords) = (keywords, secondary_keywords);
            if !keywords.is_empty()
                && keywords
                    .iter()
                    .chain(&secondary_keywords)
                    .all(|keyword| slash_regex(keyword).is_some())
            {
                let parsed = keywords
                    .iter()
                    .chain(&secondary_keywords)
                    .filter_map(|keyword| slash_regex(keyword))
                    .collect::<Vec<_>>();
                case_sensitive = !parsed.iter().any(|(_, insensitive)| *insensitive);
                keyword_match_mode = PackagedKeywordMatchMode::Regex;
                let strip = |list: &[String]| {
                    list.iter()
                        .filter_map(|keyword| slash_regex(keyword).map(|(pattern, _)| pattern))
                        .collect::<Vec<_>>()
                };
                (keywords, secondary_keywords) = (strip(&keywords), strip(&secondary_keywords));
            }
            Some(ImportedWorldInfoEntry {
                title,
                enabled,
                always_active: entry
                    .get("constant")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                keywords,
                secondary_keywords,
                secondary_logic,
                case_sensitive,
                keyword_match_mode,
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

pub fn parse_lorebook_import(json: &str) -> Result<ImportedWorldInfo, LorebookTransferError> {
    let value: Value = serde_json::from_str(json)
        .map_err(|error| LorebookTransferError::InvalidImport(error.to_string()))?;
    if value.pointer("/schema/name").and_then(Value::as_str) != Some("USC") {
        return parse_world_info(json);
    }
    if value.get("kind").and_then(Value::as_str) != Some("lorebook")
        || value.pointer("/schema/version").and_then(Value::as_str) != Some("1.0")
    {
        return Err(LorebookTransferError::InvalidImport(
            "unsupported USC lorebook".into(),
        ));
    }
    let payload: UscLorebookImport = serde_json::from_value(value["payload"].clone())
        .map_err(|error| LorebookTransferError::InvalidImport(error.to_string()))?;
    let name = payload.name.trim().to_owned();
    if name.is_empty() {
        return Err(LorebookTransferError::MissingName);
    }
    let keyword_detection_mode = match payload.keyword_detection_mode.as_deref() {
        None | Some("recentMessageWindow") => PackagedKeywordDetectionMode::RecentMessageWindow,
        Some("latestUserMessage") => PackagedKeywordDetectionMode::LatestUserMessage,
        Some(_) => {
            return Err(LorebookTransferError::InvalidImport(
                "unknown keyword detection mode".into(),
            ));
        }
    };
    let mut entries = payload.entries;
    entries.sort_by_key(|entry| entry.display_order);
    Ok(ImportedWorldInfo {
        name,
        keyword_detection_mode,
        entries: entries
            .into_iter()
            .enumerate()
            .map(|(index, entry)| {
                Ok(ImportedWorldInfoEntry {
                    title: entry.title,
                    enabled: entry.enabled,
                    always_active: entry.always_active,
                    keywords: entry.keywords,
                    secondary_keywords: Vec::new(),
                    secondary_logic: SecondaryKeywordLogic::AndAny,
                    case_sensitive: entry.case_sensitive,
                    keyword_match_mode: PackagedKeywordMatchMode::Literal,
                    content: entry.content,
                    priority: entry.priority,
                    display_order: i32::try_from(index).map_err(|_| {
                        LorebookTransferError::InvalidImport("entry order exceeds i32".into())
                    })?,
                })
            })
            .collect::<Result<_, LorebookTransferError>>()?,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UscLorebookImport {
    name: String,
    keyword_detection_mode: Option<String>,
    entries: Vec<UscLorebookEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UscLorebookEntry {
    #[serde(rename = "id")]
    _id: String,
    title: String,
    enabled: bool,
    always_active: bool,
    keywords: Vec<String>,
    case_sensitive: bool,
    content: String,
    priority: i32,
    display_order: i32,
    #[serde(rename = "createdAt")]
    _created_at: i64,
    #[serde(rename = "updatedAt")]
    _updated_at: i64,
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
            secondary_keywords: Vec::new(),
            secondary_logic: SecondaryKeywordLogic::AndAny,
            case_sensitive: false,
            keyword_match_mode: PackagedKeywordMatchMode::Literal,
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
    fn world_info_writes_regex_and_phrase_pattern_entries_as_slash_keys_and_reads_them_back() {
        let mut book = lorebook();
        book.entries.truncate(2);
        book.entries[0].keyword_match_mode = PackagedKeywordMatchMode::Regex;
        book.entries[0].keywords = vec!["doc(tor)?".into()];
        book.entries[0].case_sensitive = true;
        book.entries[1].keywords = vec!["Doc * medic".into(), "Lazaro".into()];
        book.entries[1].secondary_keywords = vec!["(field|combat) work".into()];
        let exported = export_world_info(&book).expect("export");
        let value: Value = serde_json::from_str(&exported).expect("json");
        assert_eq!(value["entries"]["1"]["key"], json!(["/doc(tor)?/"]));
        let pattern_keys = value["entries"]["2"]["key"].as_array().expect("keys");
        assert!(pattern_keys.iter().all(|key| {
            key.as_str()
                .is_some_and(|key| key.starts_with('/') && key.ends_with("/iu"))
        }));
        let imported = parse_world_info(&exported).expect("import");
        assert_eq!(
            imported.entries[0].keyword_match_mode,
            PackagedKeywordMatchMode::Regex
        );
        assert_eq!(imported.entries[0].keywords, ["doc(tor)?"]);
        assert!(imported.entries[0].case_sensitive);
        let phrase = &imported.entries[1];
        assert_eq!(phrase.keyword_match_mode, PackagedKeywordMatchMode::Regex);
        assert!(!phrase.case_sensitive);
        let fires = |keywords: &[String], text: &str| {
            keywords.iter().any(|keyword| {
                lettuce_context::keyword_matches_with_mode(
                    keyword,
                    text,
                    false,
                    lettuce_context::KeywordMatchMode::Regex,
                )
                .expect("valid regex")
            })
        };
        assert!(fires(&phrase.keywords, "Doc is our medic"));
        assert!(fires(&phrase.keywords, "lazaro"));
        assert!(!fires(&phrase.keywords, "The medic saw Doc"));
        assert!(fires(&phrase.secondary_keywords, "He does combat work"));
    }

    #[test]
    fn world_info_keys_are_regex_only_when_every_key_is_a_slash_pattern() {
        let imported = parse_world_info(
            &json!({"name": "Mixed", "entries": {"1": {"key": ["/a+/i", "plain"], "content": "x"}}}).to_string(),
        )
        .expect("import");
        assert_eq!(
            imported.entries[0].keyword_match_mode,
            PackagedKeywordMatchMode::Literal
        );
        assert_eq!(imported.entries[0].keywords, ["/a+/i", "plain"]);
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

    fn secondary_entry(
        keywords: &[&str],
        secondary: &[&str],
        logic: SecondaryKeywordLogic,
    ) -> LorebookTransferEntry {
        LorebookTransferEntry {
            keywords: keywords.iter().map(|value| (*value).to_owned()).collect(),
            secondary_keywords: secondary.iter().map(|value| (*value).to_owned()).collect(),
            secondary_logic: logic,
            ..lorebook().entries.remove(0)
        }
    }

    #[test]
    fn world_info_round_trips_secondary_keywords_with_every_logic() {
        for (logic, value) in [
            (SecondaryKeywordLogic::AndAny, 0),
            (SecondaryKeywordLogic::NotAll, 1),
            (SecondaryKeywordLogic::NotAny, 2),
            (SecondaryKeywordLogic::AndAll, 3),
        ] {
            let mut book = lorebook();
            book.entries = vec![secondary_entry(&["a"], &["b", "c"], logic)];
            let exported = export_world_info(&book).expect("export");
            let json: Value = serde_json::from_str(&exported).expect("json");
            let entry = &json["entries"]["1"];
            assert_eq!(entry["keysecondary"], json!(["b", "c"]));
            assert_eq!(entry["secondary_keys"], json!(["b", "c"]));
            assert_eq!(entry["selective"], true);
            assert_eq!(entry["selectiveLogic"], value);
            let read = parse_world_info(&exported).expect("read back");
            assert_eq!(read.entries[0].keywords, vec!["a".to_owned()]);
            assert_eq!(
                read.entries[0].secondary_keywords,
                vec!["b".to_owned(), "c".to_owned()]
            );
            assert_eq!(read.entries[0].secondary_logic, logic);
        }
    }

    #[test]
    fn world_info_without_secondary_keywords_exports_not_selective() {
        let mut book = lorebook();
        book.entries = vec![secondary_entry(&["a"], &[], SecondaryKeywordLogic::AndAny)];
        let json: Value =
            serde_json::from_str(&export_world_info(&book).expect("export")).expect("json");
        assert_eq!(json["entries"]["1"]["selective"], false);
        assert_eq!(json["entries"]["1"]["selectiveLogic"], 0);
        assert_eq!(json["entries"]["1"]["keysecondary"], json!([]));
    }

    #[test]
    fn world_info_import_reads_secondary_keywords_and_selective_flag() {
        let read = parse_world_info(
            &json!({
                "name": "Notes",
                "entries": {
                    "0": {"keys": ["a"], "keysecondary": ["x"], "selective": true, "selectiveLogic": 2, "content": "one"},
                    "1": {"keys": ["b"], "secondary_keys": ["y", " "], "content": "two", "extensions": {"selectiveLogic": 1}},
                    "2": {"keys": ["c"], "keysecondary": ["z"], "selective": false, "selectiveLogic": 3, "content": "three"},
                    "3": {"keys": ["d"], "keysecondary": [], "selective": true, "selectiveLogic": 3, "content": "four"},
                    "4": {"keys": ["e"], "keysecondary": ["w"], "selective": true, "selectiveLogic": 9, "content": "five"}
                }
            })
            .to_string(),
        )
        .expect("world info");
        let view = |index: usize| {
            (
                read.entries[index].secondary_keywords.clone(),
                read.entries[index].secondary_logic,
            )
        };
        assert_eq!(
            view(0),
            (vec!["x".to_owned()], SecondaryKeywordLogic::NotAny)
        );
        assert_eq!(
            view(1),
            (vec!["y".to_owned()], SecondaryKeywordLogic::NotAll)
        );
        assert_eq!(view(2).0, Vec::<String>::new());
        assert_eq!(view(3).0, Vec::<String>::new());
        assert_eq!(
            view(4),
            (vec!["w".to_owned()], SecondaryKeywordLogic::AndAny)
        );
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
}
