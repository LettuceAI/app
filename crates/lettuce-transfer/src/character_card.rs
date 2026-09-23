//! Character Card V1/V2/V3 files (JSON or PNG) read into a draft, and V2/V3
//! written from a character.

use std::io::Read;

use base64::Engine;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharacterFileFormat {
    Uec,
    LegacyJson,
    CharaCardV3,
    CharaCardV2,
    CharaCardV1,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CharacterCardError {
    #[error("Invalid PNG file")]
    InvalidPng,
    #[error("Corrupted PNG metadata")]
    CorruptedPng,
    #[error("PNG does not contain a supported character card payload")]
    NoCardInPng,
    #[error("Invalid UTF-8 import file")]
    InvalidUtf8,
    #[error("Invalid JSON")]
    InvalidJson,
    #[error("Invalid chara card {0}: {1}")]
    InvalidCard(&'static str, String),
}

/// A card read into the fields a character is created from. `greetings`
/// holds the first message then each non-blank alternate greeting; each
/// becomes a scene, the first the default.
#[derive(Debug, Clone, PartialEq)]
pub struct CharacterCardDraft {
    pub format: CharacterFileFormat,
    pub name: String,
    pub description: Option<String>,
    pub definition: Option<String>,
    pub scenario: Option<String>,
    pub nickname: Option<String>,
    pub creator: Option<String>,
    pub creator_notes: Option<String>,
    pub creator_notes_multilingual: Option<Value>,
    pub source: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
    pub character_book: Option<CharaCardCharacterBook>,
    pub greetings: Vec<String>,
    pub avatar: Option<String>,
    pub background: Option<String>,
}

/// What a card export reads from a character. `definition` is the one
/// `export_definition` resolves.
#[derive(Debug, Clone, PartialEq)]
pub struct CharacterCardSource {
    pub name: String,
    pub description: Option<String>,
    pub definition: Option<String>,
    pub character_book: Option<CharaCardCharacterBook>,
    pub scenario: Option<String>,
    pub nickname: Option<String>,
    pub creator: Option<String>,
    pub creator_notes: Option<String>,
    pub creator_notes_multilingual: Option<Value>,
    pub source: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
    pub scene_contents: Vec<String>,
    pub avatar: Option<String>,
    pub created_at: Option<i64>,
    pub updated_at: Option<i64>,
}

fn empty_object() -> Value {
    Value::Object(Map::new())
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CharaCardV1 {
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    personality: String,
    #[serde(default)]
    scenario: String,
    #[serde(default)]
    first_mes: String,
    #[serde(default)]
    mes_example: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CharaCardV2 {
    #[serde(default)]
    spec: String,
    #[serde(default)]
    spec_version: String,
    #[serde(default)]
    data: CharaCardV2Data,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CharaCardV3 {
    #[serde(default)]
    spec: String,
    #[serde(default)]
    spec_version: String,
    #[serde(default)]
    data: CharaCardV3Data,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CharaCardV3Data {
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    personality: String,
    #[serde(default)]
    scenario: String,
    #[serde(default)]
    first_mes: String,
    #[serde(default)]
    mes_example: String,
    #[serde(default)]
    creator_notes: String,
    #[serde(default)]
    system_prompt: String,
    #[serde(default)]
    post_history_instructions: String,
    #[serde(default, deserialize_with = "deserialize_null_as_empty_vec")]
    alternate_greetings: Vec<String>,
    #[serde(default)]
    character_book: Option<CharaCardCharacterBook>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    creator: String,
    #[serde(default)]
    character_version: String,
    #[serde(default = "empty_object")]
    extensions: Value,
    #[serde(default)]
    assets: Option<Vec<CharaCardAsset>>,
    #[serde(default)]
    nickname: Option<String>,
    #[serde(default)]
    creator_notes_multilingual: Option<Value>,
    #[serde(default)]
    source: Option<Vec<String>>,
    #[serde(default)]
    avatar: Option<String>,
    #[serde(default)]
    group_only_greetings: Vec<String>,
    #[serde(default)]
    creation_date: Option<i64>,
    #[serde(default)]
    modification_date: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CharaCardAsset {
    #[serde(rename = "type", default)]
    asset_type: String,
    #[serde(default)]
    uri: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    ext: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CharaCardV2Data {
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    personality: String,
    #[serde(default)]
    scenario: String,
    #[serde(default)]
    first_mes: String,
    #[serde(default)]
    mes_example: String,
    #[serde(default)]
    creator_notes: String,
    #[serde(default)]
    system_prompt: String,
    #[serde(default)]
    post_history_instructions: String,
    #[serde(default)]
    alternate_greetings: Vec<String>,
    #[serde(default)]
    character_book: Option<CharaCardCharacterBook>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    creator: String,
    #[serde(default)]
    character_version: String,
    #[serde(default = "empty_object")]
    extensions: Value,
    #[serde(default)]
    avatar: Option<String>,
}

fn deserialize_null_as_empty_vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Option::<Vec<T>>::deserialize(deserializer)?.unwrap_or_default())
}

/// A card's embedded lorebook; fields outside it are dropped.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CharaCardCharacterBook {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub scan_depth: Option<i64>,
    #[serde(default)]
    pub token_budget: Option<i64>,
    #[serde(default)]
    pub recursive_scanning: Option<bool>,
    #[serde(default = "empty_object")]
    pub extensions: Value,
    #[serde(default)]
    pub entries: Vec<CharaCardCharacterBookEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CharaCardCharacterBookEntry {
    #[serde(default)]
    pub keys: Vec<String>,
    #[serde(default)]
    pub content: String,
    #[serde(default = "empty_object")]
    pub extensions: Value,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub insertion_order: i64,
    #[serde(default)]
    pub case_sensitive: Option<bool>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub priority: Option<i64>,
    #[serde(default)]
    pub id: Option<i64>,
    #[serde(default)]
    pub comment: Option<String>,
    #[serde(default)]
    pub selective: Option<bool>,
    #[serde(default)]
    pub secondary_keys: Option<Vec<String>>,
    #[serde(default)]
    pub constant: Option<bool>,
    #[serde(default)]
    pub position: Option<String>,
}

fn is_spec(value: &Value, spec: &str) -> bool {
    value.get("spec").and_then(Value::as_str) == Some(spec)
        && value.get("data").is_some_and(Value::is_object)
}

fn looks_like_chara_card_v1(value: &Value) -> bool {
    [
        "name",
        "description",
        "personality",
        "scenario",
        "first_mes",
        "mes_example",
    ]
    .iter()
    .all(|field| value.get(*field).and_then(Value::as_str).is_some())
}

/// A UEC schema name, or any string `kind`.
#[must_use]
pub fn looks_like_uec(value: &Value) -> bool {
    value
        .get("schema")
        .and_then(|schema| schema.get("name"))
        .and_then(Value::as_str)
        == Some("UEC")
        || value.get("kind").and_then(Value::as_str).is_some()
}

/// UEC, then V3, V2 and V1; `None` leaves the value to the legacy package
/// reader.
#[must_use]
pub fn detect_character_card_format(value: &Value) -> Option<CharacterFileFormat> {
    if looks_like_uec(value) {
        Some(CharacterFileFormat::Uec)
    } else if is_spec(value, "chara_card_v3") {
        Some(CharacterFileFormat::CharaCardV3)
    } else if is_spec(value, "chara_card_v2") {
        Some(CharacterFileFormat::CharaCardV2)
    } else if looks_like_chara_card_v1(value) {
        Some(CharacterFileFormat::CharaCardV1)
    } else {
        None
    }
}

/// The JSON text of an import file: a PNG's embedded card, else the file as
/// UTF-8.
pub fn character_import_json(filename: &str, data: &[u8]) -> Result<String, CharacterCardError> {
    if filename.to_ascii_lowercase().ends_with(".png") {
        return extract_character_json_from_png(data);
    }
    String::from_utf8(data.to_vec()).map_err(|_| CharacterCardError::InvalidUtf8)
}

/// Reads a V1, V2 or V3 card; `None` for other formats.
pub fn parse_character_card(
    value: &Value,
) -> Result<Option<CharacterCardDraft>, CharacterCardError> {
    let Some(format) = detect_character_card_format(value) else {
        return Ok(None);
    };
    let draft = match format {
        CharacterFileFormat::CharaCardV1 => {
            let card: CharaCardV1 = serde_json::from_value(value.clone())
                .map_err(|error| CharacterCardError::InvalidCard("v1", error.to_string()))?;
            CharacterCardDraft {
                format,
                definition: build_definition_from_fields(
                    &card.description,
                    &card.personality,
                    &card.scenario,
                    &card.mes_example,
                ),
                name: card.name,
                description: non_blank(card.description),
                scenario: non_blank(card.scenario),
                nickname: None,
                creator: None,
                creator_notes: None,
                creator_notes_multilingual: None,
                source: None,
                tags: None,
                character_book: None,
                greetings: greetings(&card.first_mes, &[]),
                avatar: None,
                background: None,
            }
        }
        CharacterFileFormat::CharaCardV2 => {
            let data = serde_json::from_value::<CharaCardV2>(value.clone())
                .map_err(|error| CharacterCardError::InvalidCard("v2", error.to_string()))?
                .data;
            CharacterCardDraft {
                format,
                definition: build_definition_from_fields(
                    &data.description,
                    &data.personality,
                    &data.scenario,
                    &data.mes_example,
                ),
                greetings: greetings(&data.first_mes, &data.alternate_greetings),
                name: data.name,
                description: non_blank(data.description),
                scenario: non_blank(data.scenario),
                nickname: None,
                creator: non_blank(data.creator),
                creator_notes: non_blank(data.creator_notes),
                creator_notes_multilingual: None,
                source: None,
                tags: Some(data.tags).filter(|tags| !tags.is_empty()),
                character_book: data.character_book,
                avatar: data.avatar.filter(|value| !value.trim().is_empty()),
                background: None,
            }
        }
        CharacterFileFormat::CharaCardV3 => {
            let data = serde_json::from_value::<CharaCardV3>(value.clone())
                .map_err(|error| CharacterCardError::InvalidCard("v3", error.to_string()))?
                .data;
            let icon = data
                .assets
                .as_deref()
                .and_then(|assets| resolve_asset_uri(assets, "icon"));
            let background = data
                .assets
                .as_deref()
                .and_then(|assets| resolve_asset_uri(assets, "background"))
                .filter(|uri| uri.starts_with("data:"));
            CharacterCardDraft {
                format,
                definition: build_definition_from_fields(
                    &data.description,
                    &data.personality,
                    &data.scenario,
                    &data.mes_example,
                ),
                greetings: greetings(&data.first_mes, &data.alternate_greetings),
                avatar: data
                    .avatar
                    .filter(|value| !value.trim().is_empty())
                    .or_else(|| icon.filter(|value| !value.trim().is_empty())),
                name: data.name,
                description: non_blank(data.description),
                scenario: non_blank(data.scenario),
                nickname: data.nickname,
                creator: non_blank(data.creator),
                creator_notes: non_blank(data.creator_notes),
                creator_notes_multilingual: data.creator_notes_multilingual,
                source: data.source,
                tags: Some(data.tags).filter(|tags| !tags.is_empty()),
                character_book: data.character_book,
                background,
            }
        }
        CharacterFileFormat::Uec | CharacterFileFormat::LegacyJson => return Ok(None),
    };
    Ok(Some(draft))
}

fn non_blank(value: String) -> Option<String> {
    Some(value).filter(|value| !value.trim().is_empty())
}

fn greetings(first: &str, alternates: &[String]) -> Vec<String> {
    std::iter::once(first.trim().to_owned())
        .chain(
            alternates
                .iter()
                .map(|greeting| greeting.trim())
                .filter(|greeting| !greeting.is_empty())
                .map(str::to_owned),
        )
        .collect()
}

fn resolve_asset_uri(assets: &[CharaCardAsset], asset_type: &str) -> Option<String> {
    let mut selected = None;
    for asset in assets.iter().filter(|asset| asset.asset_type == asset_type) {
        if asset.name == "main" {
            return Some(asset.uri.clone());
        }
        selected.get_or_insert(asset);
    }
    selected.map(|asset| asset.uri.clone())
}

fn push_definition_block(parts: &mut Vec<String>, label: Option<&str>, value: &str) {
    let text = value.trim();
    if text.is_empty() {
        return;
    }
    parts.push(match label {
        Some(label) => format!("[{label}]\n{text}"),
        None => text.to_owned(),
    });
}

/// A card's description, personality, scenario and example dialogue as one
/// definition; its system prompt and post-history instructions are dropped.
#[must_use]
pub fn build_definition_from_fields(
    description: &str,
    personality: &str,
    scenario: &str,
    mes_example: &str,
) -> Option<String> {
    let mut parts = Vec::new();
    push_definition_block(&mut parts, None, description);
    push_definition_block(&mut parts, Some("Personality"), personality);
    push_definition_block(&mut parts, Some("Scenario"), scenario);
    let example = mes_example.trim();
    if !example.is_empty() {
        parts.push(format!(
            "<example_dialogue>\n{example}\n</example_dialogue>"
        ));
    }
    (!parts.is_empty()).then(|| parts.join("\n\n"))
}

/// Removes `[System Prompt]` and `[Post History Instructions]` sections from a
/// definition.
#[must_use]
pub fn strip_legacy_card_prompt_sections(definition: &str) -> String {
    let mut output: Vec<&str> = Vec::new();
    let mut skip = false;
    for line in definition.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') && trimmed.len() > 2 {
            let label = trimmed[1..trimmed.len() - 1].trim();
            skip = label.eq_ignore_ascii_case("system prompt")
                || label.eq_ignore_ascii_case("post history instructions");
            if skip {
                while output.last().is_some_and(|line| line.trim().is_empty()) {
                    output.pop();
                }
                continue;
            }
        }
        if !skip {
            output.push(line);
        }
    }
    output.join("\n").trim().to_owned()
}

/// A stored definition without its prompt sections, or the description when
/// nothing else is left.
#[must_use]
pub fn export_definition(definition: Option<&str>, description: Option<&str>) -> Option<String> {
    definition
        .map(strip_legacy_card_prompt_sections)
        .filter(|value| !value.is_empty())
        .or_else(|| description.map(str::to_owned))
}

#[derive(Default)]
struct DefinitionSections {
    base: String,
    personality: String,
    scenario: String,
    system_prompt: String,
    post_history_instructions: String,
    mes_example: String,
}

fn extract_example_dialogue(text: &str) -> (String, String) {
    let lower = text.to_ascii_lowercase();
    let start_tag = "<example_dialogue>";
    let end_tag = "</example_dialogue>";
    if let (Some(start), Some(end)) = (lower.find(start_tag), lower.find(end_tag)) {
        let content_start = start + start_tag.len();
        if end >= content_start {
            let example = text[content_start..end].trim().to_owned();
            let mut stripped = String::new();
            stripped.push_str(text[..start].trim_end());
            if !stripped.is_empty() && !text[end + end_tag.len()..].trim().is_empty() {
                stripped.push_str("\n\n");
            }
            stripped.push_str(text[end + end_tag.len()..].trim_start());
            return (stripped.trim().to_owned(), example);
        }
    }
    (text.trim().to_owned(), String::new())
}

fn parse_definition_sections(definition: &str) -> DefinitionSections {
    let (without_examples, example) = extract_example_dialogue(definition);
    let mut current: Option<String> = None;
    let mut base = Vec::new();
    let mut sections: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for line in without_examples.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') && trimmed.len() > 2 {
            current = Some(
                trimmed
                    .trim_start_matches('[')
                    .trim_end_matches(']')
                    .trim()
                    .to_owned(),
            );
            continue;
        }
        match current.as_deref() {
            Some(label) => sections
                .entry(label.to_ascii_lowercase())
                .or_default()
                .push(line.to_owned()),
            None => base.push(line.to_owned()),
        }
    }
    let take = |key: &str| {
        sections
            .get(key)
            .map(|lines| lines.join("\n").trim().to_owned())
            .unwrap_or_default()
    };
    DefinitionSections {
        base: base.join("\n").trim().to_owned(),
        personality: take("personality"),
        scenario: take("scenario"),
        system_prompt: take("system prompt"),
        post_history_instructions: take("post history instructions"),
        mes_example: example,
    }
}

struct ExportFields {
    sections: DefinitionSections,
    description: String,
    scenario: String,
    first_mes: String,
    alternate_greetings: Vec<String>,
}

fn export_fields(source: &CharacterCardSource) -> ExportFields {
    let sections = parse_definition_sections(source.definition.as_deref().unwrap_or_default());
    let description = match source.description.as_deref() {
        Some(description) if !description.trim().is_empty() => description.to_owned(),
        _ => sections.base.clone(),
    };
    let scenario = source
        .scenario
        .clone()
        .unwrap_or_else(|| sections.scenario.clone());
    let first_mes = source.scene_contents.first().cloned().unwrap_or_default();
    let alternate_greetings = source
        .scene_contents
        .iter()
        .skip(1)
        .filter(|content| !content.trim().is_empty())
        .cloned()
        .collect();
    ExportFields {
        sections,
        description,
        scenario,
        first_mes,
        alternate_greetings,
    }
}

/// Pretty-printed Character Card V2.
pub fn export_chara_card_v2(source: &CharacterCardSource) -> Result<String, CharacterCardError> {
    let fields = export_fields(source);
    serde_json::to_string_pretty(&CharaCardV2 {
        spec: "chara_card_v2".to_owned(),
        spec_version: "2.0".to_owned(),
        data: CharaCardV2Data {
            name: source.name.clone(),
            description: fields.description,
            personality: fields.sections.personality,
            scenario: fields.scenario,
            first_mes: fields.first_mes,
            mes_example: fields.sections.mes_example,
            creator_notes: source.creator_notes.clone().unwrap_or_default(),
            system_prompt: fields.sections.system_prompt,
            post_history_instructions: fields.sections.post_history_instructions,
            alternate_greetings: fields.alternate_greetings,
            character_book: source.character_book.clone(),
            tags: source.tags.clone().unwrap_or_default(),
            creator: source.creator.clone().unwrap_or_default(),
            character_version: String::new(),
            extensions: empty_object(),
            avatar: source.avatar.clone(),
        },
    })
    .map_err(|_| CharacterCardError::InvalidJson)
}

/// Pretty-printed Character Card V3; dates are in seconds.
pub fn export_chara_card_v3(source: &CharacterCardSource) -> Result<String, CharacterCardError> {
    let fields = export_fields(source);
    serde_json::to_string_pretty(&CharaCardV3 {
        spec: "chara_card_v3".to_owned(),
        spec_version: "3.0".to_owned(),
        data: CharaCardV3Data {
            name: source.name.clone(),
            description: fields.description,
            personality: fields.sections.personality,
            scenario: fields.scenario,
            first_mes: fields.first_mes,
            mes_example: fields.sections.mes_example,
            creator_notes: source.creator_notes.clone().unwrap_or_default(),
            system_prompt: fields.sections.system_prompt,
            post_history_instructions: fields.sections.post_history_instructions,
            alternate_greetings: fields.alternate_greetings,
            character_book: source.character_book.clone(),
            tags: source.tags.clone().unwrap_or_default(),
            creator: source.creator.clone().unwrap_or_default(),
            character_version: String::new(),
            extensions: empty_object(),
            assets: None,
            nickname: source.nickname.clone(),
            creator_notes_multilingual: source.creator_notes_multilingual.clone(),
            source: source.source.clone(),
            avatar: source.avatar.clone(),
            group_only_greetings: Vec::new(),
            creation_date: source.created_at.map(|value| value / 1000),
            modification_date: source.updated_at.map(|value| value / 1000),
        },
    })
    .map_err(|_| CharacterCardError::InvalidJson)
}

fn decode_base64_json_candidate(candidate: &str) -> Option<String> {
    use base64::engine::general_purpose;
    [
        &general_purpose::STANDARD,
        &general_purpose::STANDARD_NO_PAD,
        &general_purpose::URL_SAFE,
        &general_purpose::URL_SAFE_NO_PAD,
    ]
    .into_iter()
    .find_map(|engine| {
        let text = String::from_utf8(engine.decode(candidate).ok()?).ok()?;
        serde_json::from_str::<Value>(&text).is_ok().then_some(text)
    })
}

fn try_parse_character_json(candidate: &str) -> Option<String> {
    let trimmed = candidate.trim();
    if trimmed.is_empty() {
        return None;
    }
    if serde_json::from_str::<Value>(trimmed).is_ok() {
        return Some(trimmed.to_owned());
    }
    decode_base64_json_candidate(trimmed)
}

fn inflate(bytes: &[u8]) -> Option<String> {
    let mut text = String::new();
    flate2::read::ZlibDecoder::new(bytes)
        .read_to_string(&mut text)
        .ok()?;
    Some(text)
}

fn decode_png_text_chunk(chunk_type: &[u8], chunk: &[u8]) -> Option<(String, String)> {
    let keyword_end = chunk.iter().position(|byte| *byte == 0)?;
    if keyword_end == 0 {
        return None;
    }
    let keyword = String::from_utf8(chunk[..keyword_end].to_vec()).ok()?;
    match chunk_type {
        b"tEXt" => Some((
            keyword,
            String::from_utf8(chunk[keyword_end + 1..].to_vec()).ok()?,
        )),
        b"zTXt" => {
            if keyword_end + 2 > chunk.len() || chunk[keyword_end + 1] != 0 {
                return None;
            }
            Some((keyword, inflate(&chunk[keyword_end + 2..])?))
        }
        b"iTXt" => {
            let mut cursor = keyword_end + 1;
            if cursor + 1 >= chunk.len() {
                return None;
            }
            let compressed = chunk[cursor];
            let method = chunk[cursor + 1];
            cursor += 2;
            for _ in 0..2 {
                cursor += chunk.get(cursor..)?.iter().position(|byte| *byte == 0)? + 1;
            }
            let text = chunk.get(cursor..)?;
            if compressed == 1 {
                if method != 0 {
                    return None;
                }
                return Some((keyword, inflate(text)?));
            }
            Some((keyword, String::from_utf8(text.to_vec()).ok()?))
        }
        _ => None,
    }
}

/// Walks the chunks (without checking CRCs) and prefers the `ccv3`, `chara`
/// then `ccv2` text chunk, as raw or base64 JSON.
pub fn extract_character_json_from_png(data: &[u8]) -> Result<String, CharacterCardError> {
    const SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if data.len() < SIGNATURE.len() || &data[..SIGNATURE.len()] != SIGNATURE {
        return Err(CharacterCardError::InvalidPng);
    }
    let mut candidates = Vec::new();
    let mut offset = SIGNATURE.len();
    while offset + 12 <= data.len() {
        let length = u32::from_be_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]) as usize;
        offset += 4;
        if offset + 8 > data.len() || offset + length + 8 > data.len() {
            return Err(CharacterCardError::CorruptedPng);
        }
        let chunk_type = &data[offset..offset + 4];
        if std::str::from_utf8(chunk_type).is_err() {
            return Err(CharacterCardError::CorruptedPng);
        }
        offset += 4;
        let chunk = &data[offset..offset + length];
        offset += length + 4;
        if chunk_type == b"IEND" {
            break;
        }
        if let Some(candidate) = decode_png_text_chunk(chunk_type, chunk) {
            candidates.push(candidate);
        }
    }
    for preferred in ["ccv3", "chara", "ccv2"] {
        if let Some(json) = candidates
            .iter()
            .filter(|(keyword, _)| keyword.eq_ignore_ascii_case(preferred))
            .find_map(|(_, text)| try_parse_character_json(text))
        {
            return Ok(json);
        }
    }
    candidates
        .iter()
        .find_map(|(_, text)| try_parse_character_json(text))
        .ok_or(CharacterCardError::NoCardInPng)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use serde_json::json;

    use super::*;

    #[test]
    fn card_instruction_fields_are_not_buried_in_definition() {
        let definition =
            build_definition_from_fields("Description", "Personality", "Scenario", "Example")
                .expect("definition");
        assert_eq!(
            definition,
            "Description\n\n[Personality]\nPersonality\n\n[Scenario]\nScenario\n\n<example_dialogue>\nExample\n</example_dialogue>"
        );
    }

    #[test]
    fn legacy_card_instruction_sections_are_removed_without_losing_other_sections() {
        let definition = "Description\n\n[Personality]\nWarm\n\n[System Prompt]\nOld system prompt\n\n[Post History Instructions]\nOld post-history prompt\n\n[Scenario]\nCafe\n\n[Custom]\nKeep me";
        assert_eq!(
            strip_legacy_card_prompt_sections(definition),
            "Description\n\n[Personality]\nWarm\n[Scenario]\nCafe\n\n[Custom]\nKeep me"
        );
    }

    #[test]
    fn detection_follows_the_legacy_order() {
        assert_eq!(
            detect_character_card_format(&json!({"kind": "anything"})),
            Some(CharacterFileFormat::Uec)
        );
        assert_eq!(
            detect_character_card_format(&json!({"spec": "chara_card_v3", "data": {}})),
            Some(CharacterFileFormat::CharaCardV3)
        );
        assert_eq!(
            detect_character_card_format(&json!({"spec": "chara_card_v2", "data": {}})),
            Some(CharacterFileFormat::CharaCardV2)
        );
        assert_eq!(
            detect_character_card_format(&json!({
                "name": "A", "description": "", "personality": "", "scenario": "",
                "first_mes": "", "mes_example": ""
            })),
            Some(CharacterFileFormat::CharaCardV1)
        );
        assert_eq!(detect_character_card_format(&json!({"version": 1})), None);
        assert_eq!(
            export_definition(Some("[System Prompt]\nX"), Some("D")).as_deref(),
            Some("D")
        );
    }

    #[test]
    fn a_v3_card_reads_like_legacy() {
        let draft = parse_character_card(&json!({
            "spec": "chara_card_v3",
            "data": {
                "name": "Ada",
                "description": " Keeper ",
                "personality": "Warm",
                "scenario": "",
                "first_mes": "",
                "mes_example": "Hi",
                "system_prompt": "hidden",
                "alternate_greetings": null,
                "tags": ["a"],
                "creator": " ",
                "nickname": "A",
                "assets": [
                    {"type": "icon", "uri": "https://example.test/one.png", "name": "other"},
                    {"type": "icon", "uri": "data:image/png;base64,AA==", "name": "main"},
                    {"type": "background", "uri": "https://example.test/bg.png", "name": "main"}
                ],
                "character_book": {"entries": [{"keys": ["k"], "content": "c", "use_regex": true}]}
            }
        }))
        .expect("parse")
        .expect("card");
        assert_eq!(draft.format, CharacterFileFormat::CharaCardV3);
        assert_eq!(draft.greetings, vec![String::new()]);
        assert_eq!(draft.avatar.as_deref(), Some("data:image/png;base64,AA=="));
        assert_eq!(draft.background, None);
        assert_eq!(draft.creator, None);
        assert_eq!(draft.description.as_deref(), Some(" Keeper "));
        assert_eq!(
            draft.definition.as_deref(),
            Some("Keeper\n\n[Personality]\nWarm\n\n<example_dialogue>\nHi\n</example_dialogue>")
        );
        let book = draft.character_book.expect("book");
        assert!(!book.entries[0].enabled);
        assert_eq!(
            serde_json::to_value(&book.entries[0]).expect("entry")["keys"],
            json!(["k"])
        );
    }

    #[test]
    fn exports_keep_legacy_field_order_and_values() {
        let source = CharacterCardSource {
            name: "Ada".into(),
            description: None,
            definition: Some(
                "Base\n\n[Personality]\nWarm\n[System Prompt]\nSys\n\n<example_dialogue>\nEx\n</example_dialogue>"
                    .into(),
            ),
            character_book: None,
            scenario: None,
            nickname: Some("A".into()),
            creator: None,
            creator_notes: None,
            creator_notes_multilingual: None,
            source: None,
            tags: None,
            scene_contents: vec!["Hello".into(), " ".into(), "Again".into()],
            avatar: None,
            created_at: Some(5_500),
            updated_at: None,
        };
        let v2 = export_chara_card_v2(&source).expect("v2");
        let value: Value = serde_json::from_str(&v2).expect("json");
        assert_eq!(value["data"]["description"], "Base");
        assert_eq!(value["data"]["personality"], "Warm");
        assert_eq!(value["data"]["system_prompt"], "Sys");
        assert_eq!(value["data"]["mes_example"], "Ex");
        assert_eq!(value["data"]["alternate_greetings"], json!(["Again"]));
        assert_eq!(value["data"]["character_book"], Value::Null);
        assert_eq!(value["data"]["avatar"], Value::Null);
        assert!(v2.starts_with("{\n  \"spec\": \"chara_card_v2\",\n  \"spec_version\": \"2.0\",\n  \"data\": {\n    \"name\": \"Ada\",\n    \"description\": \"Base\","));
        let v3: Value =
            serde_json::from_str(&export_chara_card_v3(&source).expect("v3")).expect("json");
        assert_eq!(v3["data"]["creation_date"], 5);
        assert_eq!(v3["data"]["modification_date"], Value::Null);
        assert_eq!(v3["data"]["nickname"], "A");
        assert_eq!(v3["data"]["assets"], Value::Null);
    }

    fn chunk(kind: &[u8], body: &[u8]) -> Vec<u8> {
        let mut out = (body.len() as u32).to_be_bytes().to_vec();
        out.extend_from_slice(kind);
        out.extend_from_slice(body);
        out.extend_from_slice(&[0, 0, 0, 0]);
        out
    }

    #[test]
    fn png_cards_prefer_ccv3_and_read_compressed_and_base64_text() {
        let v2 = json!({"spec": "chara_card_v2", "data": {"name": "Two"}}).to_string();
        let v3 = json!({"spec": "chara_card_v3", "data": {"name": "Three"}}).to_string();
        let mut encoder =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(v3.as_bytes()).expect("compress");
        let compressed = encoder.finish().expect("finish");
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        let mut chara = b"chara\0".to_vec();
        chara.extend_from_slice(
            base64::engine::general_purpose::STANDARD
                .encode(v2.as_bytes())
                .as_bytes(),
        );
        png.extend(chunk(b"tEXt", &chara));
        let mut itxt = b"CCV3\0\x01\x00\0\0".to_vec();
        itxt.extend_from_slice(&compressed);
        png.extend(chunk(b"iTXt", &itxt));
        png.extend(chunk(b"IEND", &[]));
        let json = character_import_json("card.PNG", &png).expect("card");
        assert_eq!(json, v3);
        let mut truncated = b"\x89PNG\r\n\x1a\n".to_vec();
        truncated.extend_from_slice(&[0, 0, 1, 0, b't', b'E', b'X', b't', 0, 0, 0, 0]);
        assert_eq!(
            extract_character_json_from_png(&truncated),
            Err(CharacterCardError::CorruptedPng)
        );
        assert_eq!(
            extract_character_json_from_png(b"nope"),
            Err(CharacterCardError::InvalidPng)
        );
    }
}
