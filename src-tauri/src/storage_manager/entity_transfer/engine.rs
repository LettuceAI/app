use serde::{de::Deserializer, Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value as JsonValue};

use super::super::db::now_ms;
use super::{CharacterExportData, CharacterExportPackage, SceneExport};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CharacterFileFormat {
    Uec,
    LegacyJson,
    CharaCardV3,
    CharaCardV2,
    CharaCardV1,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterFormatInfo {
    pub id: CharacterFileFormat,
    pub label: String,
    pub extension: String,
    pub can_export: bool,
    pub can_import: bool,
    pub read_only: bool,
}

pub fn character_format_info(format: CharacterFileFormat) -> CharacterFormatInfo {
    match format {
        CharacterFileFormat::Uec => CharacterFormatInfo {
            id: format,
            label: "Unified Entity Card (UEC)".to_string(),
            extension: ".uec".to_string(),
            can_export: true,
            can_import: true,
            read_only: false,
        },
        CharacterFileFormat::LegacyJson => CharacterFormatInfo {
            id: format,
            label: "Legacy JSON".to_string(),
            extension: ".json".to_string(),
            can_export: false,
            can_import: true,
            read_only: true,
        },
        CharacterFileFormat::CharaCardV2 => CharacterFormatInfo {
            id: format,
            label: "Character Card V2".to_string(),
            extension: ".json".to_string(),
            can_export: true,
            can_import: true,
            read_only: false,
        },
        CharacterFileFormat::CharaCardV3 => CharacterFormatInfo {
            id: format,
            label: "Character Card V3".to_string(),
            extension: ".json".to_string(),
            can_export: true,
            can_import: true,
            read_only: false,
        },
        CharacterFileFormat::CharaCardV1 => CharacterFormatInfo {
            id: format,
            label: "Character Card V1".to_string(),
            extension: ".json".to_string(),
            can_export: false,
            can_import: true,
            read_only: true,
        },
    }
}

pub fn all_character_formats() -> Vec<CharacterFormatInfo> {
    vec![
        character_format_info(CharacterFileFormat::Uec),
        character_format_info(CharacterFileFormat::CharaCardV3),
        character_format_info(CharacterFileFormat::CharaCardV2),
        character_format_info(CharacterFileFormat::CharaCardV1),
        character_format_info(CharacterFileFormat::LegacyJson),
    ]
}

fn empty_object() -> JsonValue {
    JsonValue::Object(JsonMap::new())
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CharaCardV1 {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub personality: String,
    #[serde(default)]
    pub scenario: String,
    #[serde(default)]
    pub first_mes: String,
    #[serde(default)]
    pub mes_example: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CharaCardV2 {
    #[serde(default)]
    pub spec: String,
    #[serde(default)]
    pub spec_version: String,
    #[serde(default)]
    pub data: CharaCardV2Data,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CharaCardV3 {
    #[serde(default)]
    pub spec: String,
    #[serde(default)]
    pub spec_version: String,
    #[serde(default)]
    pub data: CharaCardV3Data,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CharaCardV3Data {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub personality: String,
    #[serde(default)]
    pub scenario: String,
    #[serde(default)]
    pub first_mes: String,
    #[serde(default)]
    pub mes_example: String,
    #[serde(default)]
    pub creator_notes: String,
    #[serde(default)]
    pub system_prompt: String,
    #[serde(default)]
    pub post_history_instructions: String,
    #[serde(default, deserialize_with = "deserialize_null_as_empty_vec")]
    pub alternate_greetings: Vec<String>,
    #[serde(default)]
    pub character_book: Option<CharaCardCharacterBook>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub creator: String,
    #[serde(default)]
    pub character_version: String,
    #[serde(default = "empty_object")]
    pub extensions: JsonValue,
    #[serde(default)]
    pub assets: Option<Vec<CharaCardAsset>>,
    #[serde(default)]
    pub nickname: Option<String>,
    #[serde(default)]
    pub creator_notes_multilingual: Option<JsonValue>,
    #[serde(default)]
    pub source: Option<Vec<String>>,
    #[serde(default)]
    pub avatar: Option<String>,
    #[serde(default)]
    pub group_only_greetings: Vec<String>,
    #[serde(default)]
    pub creation_date: Option<i64>,
    #[serde(default)]
    pub modification_date: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CharaCardAsset {
    #[serde(rename = "type", default)]
    pub asset_type: String,
    #[serde(default)]
    pub uri: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub ext: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CharaCardV2Data {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub personality: String,
    #[serde(default)]
    pub scenario: String,
    #[serde(default)]
    pub first_mes: String,
    #[serde(default)]
    pub mes_example: String,
    #[serde(default)]
    pub creator_notes: String,
    #[serde(default)]
    pub system_prompt: String,
    #[serde(default)]
    pub post_history_instructions: String,
    #[serde(default)]
    pub alternate_greetings: Vec<String>,
    #[serde(default)]
    pub character_book: Option<CharaCardCharacterBook>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub creator: String,
    #[serde(default)]
    pub character_version: String,
    #[serde(default = "empty_object")]
    pub extensions: JsonValue,
    #[serde(default)]
    pub avatar: Option<String>,
}

fn deserialize_null_as_empty_vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    let value = Option::<Vec<T>>::deserialize(deserializer)?;
    Ok(value.unwrap_or_default())
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
    pub extensions: JsonValue,
    #[serde(default)]
    pub entries: Vec<CharaCardCharacterBookEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CharaCardCharacterBookEntry {
    #[serde(default)]
    pub keys: Vec<String>,
    #[serde(default)]
    pub content: String,
    #[serde(default = "empty_object")]
    pub extensions: JsonValue,
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

pub fn looks_like_chara_card_v2(value: &JsonValue) -> bool {
    value
        .get("spec")
        .and_then(|v| v.as_str())
        .map(|v| v == "chara_card_v2")
        .unwrap_or(false)
        && value.get("data").and_then(|v| v.as_object()).is_some()
}

pub fn looks_like_chara_card_v3(value: &JsonValue) -> bool {
    value
        .get("spec")
        .and_then(|v| v.as_str())
        .map(|v| v == "chara_card_v3")
        .unwrap_or(false)
        && value.get("data").and_then(|v| v.as_object()).is_some()
}

pub fn looks_like_chara_card_v1(value: &JsonValue) -> bool {
    value.get("name").and_then(|v| v.as_str()).is_some()
        && value.get("description").and_then(|v| v.as_str()).is_some()
        && value.get("personality").and_then(|v| v.as_str()).is_some()
        && value.get("scenario").and_then(|v| v.as_str()).is_some()
        && value.get("first_mes").and_then(|v| v.as_str()).is_some()
        && value.get("mes_example").and_then(|v| v.as_str()).is_some()
}

fn push_definition_block(parts: &mut Vec<String>, label: Option<&str>, value: &str) {
    let text = value.trim();
    if text.is_empty() {
        return;
    }

    let block = match label {
        Some(label) => format!("[{}]\n{}", label, text),
        None => text.to_string(),
    };

    parts.push(block);
}

pub fn build_definition_from_fields(
    description: &str,
    personality: &str,
    scenario: &str,
    system_prompt: &str,
    post_history_instructions: &str,
    mes_example: &str,
) -> Option<String> {
    let mut parts = Vec::new();

    push_definition_block(&mut parts, None, description);
    push_definition_block(&mut parts, Some("Personality"), personality);
    push_definition_block(&mut parts, Some("Scenario"), scenario);
    push_definition_block(&mut parts, Some("System Prompt"), system_prompt);
    push_definition_block(
        &mut parts,
        Some("Post History Instructions"),
        post_history_instructions,
    );

    let example = mes_example.trim();
    if !example.is_empty() {
        parts.push(format!(
            "<example_dialogue>\n{}\n</example_dialogue>",
            example
        ));
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n\n"))
    }
}

#[derive(Default)]
pub struct DefinitionSections {
    pub base: String,
    pub personality: String,
    pub scenario: String,
    pub system_prompt: String,
    pub post_history_instructions: String,
    pub mes_example: String,
}

fn extract_example_dialogue(text: &str) -> (String, String) {
    let lower = text.to_ascii_lowercase();
    let start_tag = "<example_dialogue>";
    let end_tag = "</example_dialogue>";
    let start = lower.find(start_tag);
    let end = lower.find(end_tag);

    if let (Some(start_idx), Some(end_idx)) = (start, end) {
        let content_start = start_idx + start_tag.len();
        if end_idx >= content_start {
            let example = text[content_start..end_idx].trim().to_string();
            let mut stripped = String::new();
            stripped.push_str(text[..start_idx].trim_end());
            if !stripped.is_empty() && !text[end_idx + end_tag.len()..].trim().is_empty() {
                stripped.push_str("\n\n");
            }
            stripped.push_str(text[end_idx + end_tag.len()..].trim_start());
            return (stripped.trim().to_string(), example);
        }
    }

    (text.trim().to_string(), String::new())
}

pub fn parse_definition_sections(definition: &str) -> DefinitionSections {
    let (without_examples, example) = extract_example_dialogue(definition);
    let mut sections = DefinitionSections {
        mes_example: example,
        ..DefinitionSections::default()
    };

    let mut current_label: Option<String> = None;
    let mut base_lines: Vec<String> = Vec::new();
    let mut section_lines: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();

    for line in without_examples.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') && trimmed.len() > 2 {
            let label = trimmed.trim_start_matches('[').trim_end_matches(']').trim();
            current_label = Some(label.to_string());
            continue;
        }

        match current_label.as_deref() {
            Some(label) => section_lines
                .entry(label.to_ascii_lowercase())
                .or_default()
                .push(line.to_string()),
            None => base_lines.push(line.to_string()),
        }
    }

    let base = base_lines.join("\n").trim().to_string();
    sections.base = base;

    let take_section = |sections: &std::collections::HashMap<String, Vec<String>>, key: &str| {
        sections
            .get(key)
            .map(|lines| lines.join("\n").trim().to_string())
            .unwrap_or_default()
    };

    sections.personality = take_section(&section_lines, "personality");
    sections.scenario = take_section(&section_lines, "scenario");
    sections.system_prompt = take_section(&section_lines, "system prompt");
    sections.post_history_instructions = take_section(&section_lines, "post history instructions");

    sections
}

pub fn build_scenes_from_greetings(
    first_mes: &str,
    alternate_greetings: &[String],
) -> (Vec<SceneExport>, Option<String>) {
    let mut scenes = Vec::new();
    let now = now_ms() as i64;

    let first_content = first_mes.trim();
    let primary_id = uuid::Uuid::new_v4().to_string();
    scenes.push(SceneExport {
        id: primary_id.clone(),
        content: first_content.to_string(),
        direction: None,
        created_at: Some(now),
        selected_variant_id: None,
        variants: Vec::new(),
    });

    for greeting in alternate_greetings {
        let content = greeting.trim();
        if content.is_empty() {
            continue;
        }
        let id = uuid::Uuid::new_v4().to_string();
        scenes.push(SceneExport {
            id,
            content: content.to_string(),
            direction: None,
            created_at: Some(now),
            selected_variant_id: None,
            variants: Vec::new(),
        });
    }

    if scenes.is_empty() {
        return (scenes, None);
    }

    let default_scene_id = scenes.first().map(|s| s.id.clone());
    (scenes, default_scene_id)
}

pub fn parse_chara_card_v1(value: &JsonValue) -> Result<CharacterExportPackage, String> {
    let card: CharaCardV1 = serde_json::from_value(value.clone()).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Invalid chara card v1: {}", e),
        )
    })?;

    let definition = build_definition_from_fields(
        &card.description,
        &card.personality,
        &card.scenario,
        "",
        "",
        &card.mes_example,
    );
    let (scenes, default_scene_id) = build_scenes_from_greetings(&card.first_mes, &[]);

    Ok(CharacterExportPackage {
        version: 1,
        exported_at: now_ms() as i64,
        character: CharacterExportData {
            name: card.name,
            description: Some(card.description).filter(|v| !v.trim().is_empty()),
            definition,
            scenario: Some(card.scenario).filter(|v| !v.trim().is_empty()),
            nickname: None,
            creator: None,
            creator_notes: None,
            creator_notes_multilingual: None,
            source: None,
            tags: None,
            character_book: None,
            rules: Vec::new(),
            scenes,
            default_scene_id,
            default_model_id: None,
            mode: None,
            companion: None,
            memory_type: Some("manual".to_string()),
            active_lorebook_ids: Vec::new(),
            lorebooks: Vec::new(),
            prompt_template_id: None,
            system_prompt: None,
            voice_config: None,
            voice_autoplay: None,
            disable_avatar_gradient: false,
            avatar_crop: None,
            custom_gradient_enabled: None,
            custom_gradient_colors: None,
            custom_text_color: None,
            custom_text_secondary: None,
            chat_templates: Vec::new(),
            default_chat_template_id: None,
        },
        avatar_data: None,
        background_image_data: None,
    })
}

pub fn parse_chara_card_v2(value: &JsonValue) -> Result<CharacterExportPackage, String> {
    let card: CharaCardV2 = serde_json::from_value(value.clone()).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Invalid chara card v2: {}", e),
        )
    })?;

    let data = card.data;
    let definition = build_definition_from_fields(
        &data.description,
        &data.personality,
        &data.scenario,
        &data.system_prompt,
        &data.post_history_instructions,
        &data.mes_example,
    );

    let (scenes, default_scene_id) =
        build_scenes_from_greetings(&data.first_mes, &data.alternate_greetings);
    let avatar_data = data.avatar.filter(|value| !value.trim().is_empty());

    Ok(CharacterExportPackage {
        version: 1,
        exported_at: now_ms() as i64,
        character: CharacterExportData {
            name: data.name,
            description: Some(data.description).filter(|v| !v.trim().is_empty()),
            definition,
            scenario: Some(data.scenario).filter(|v| !v.trim().is_empty()),
            nickname: None,
            creator: Some(data.creator).filter(|v| !v.trim().is_empty()),
            creator_notes: Some(data.creator_notes).filter(|v| !v.trim().is_empty()),
            creator_notes_multilingual: None,
            source: None,
            tags: Some(data.tags).filter(|v| !v.is_empty()),
            character_book: data
                .character_book
                .as_ref()
                .and_then(|book| serde_json::to_value(book).ok()),
            rules: Vec::new(),
            scenes,
            default_scene_id,
            default_model_id: None,
            mode: None,
            companion: None,
            memory_type: Some("manual".to_string()),
            active_lorebook_ids: Vec::new(),
            lorebooks: Vec::new(),
            prompt_template_id: None,
            system_prompt: None,
            voice_config: None,
            voice_autoplay: None,
            disable_avatar_gradient: false,
            avatar_crop: None,
            custom_gradient_enabled: None,
            custom_gradient_colors: None,
            custom_text_color: None,
            custom_text_secondary: None,
            chat_templates: Vec::new(),
            default_chat_template_id: None,
        },
        avatar_data,
        background_image_data: None,
    })
}

fn resolve_asset_uri(assets: &[CharaCardAsset], asset_type: &str) -> Option<String> {
    let mut selected: Option<&CharaCardAsset> = None;
    for asset in assets.iter().filter(|asset| asset.asset_type == asset_type) {
        if asset.name == "main" {
            return Some(asset.uri.clone());
        }
        if selected.is_none() {
            selected = Some(asset);
        }
    }
    selected.map(|asset| asset.uri.clone())
}

pub fn parse_chara_card_v3(value: &JsonValue) -> Result<CharacterExportPackage, String> {
    let card: CharaCardV3 = serde_json::from_value(value.clone()).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Invalid chara card v3: {}", e),
        )
    })?;

    let data = card.data;
    let definition = build_definition_from_fields(
        &data.description,
        &data.personality,
        &data.scenario,
        &data.system_prompt,
        &data.post_history_instructions,
        &data.mes_example,
    );

    let (scenes, default_scene_id) =
        build_scenes_from_greetings(&data.first_mes, &data.alternate_greetings);

    let asset_icon_uri = data
        .assets
        .as_ref()
        .and_then(|assets| resolve_asset_uri(assets, "icon"));
    let asset_background_uri = data
        .assets
        .as_ref()
        .and_then(|assets| resolve_asset_uri(assets, "background"));
    let avatar_data = data
        .avatar
        .clone()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| asset_icon_uri.filter(|value| !value.trim().is_empty()));
    let background_image_data = asset_background_uri.filter(|uri| uri.starts_with("data:"));

    Ok(CharacterExportPackage {
        version: 1,
        exported_at: now_ms() as i64,
        character: CharacterExportData {
            name: data.name,
            description: Some(data.description).filter(|v| !v.trim().is_empty()),
            definition,
            scenario: Some(data.scenario).filter(|v| !v.trim().is_empty()),
            nickname: data.nickname,
            creator: Some(data.creator).filter(|v| !v.trim().is_empty()),
            creator_notes: Some(data.creator_notes).filter(|v| !v.trim().is_empty()),
            creator_notes_multilingual: data.creator_notes_multilingual,
            source: data.source,
            tags: Some(data.tags).filter(|v| !v.is_empty()),
            character_book: data
                .character_book
                .as_ref()
                .and_then(|book| serde_json::to_value(book).ok()),
            rules: Vec::new(),
            scenes,
            default_scene_id,
            default_model_id: None,
            mode: None,
            companion: None,
            memory_type: Some("manual".to_string()),
            active_lorebook_ids: Vec::new(),
            lorebooks: Vec::new(),
            prompt_template_id: None,
            system_prompt: None,
            voice_config: None,
            voice_autoplay: None,
            disable_avatar_gradient: false,
            avatar_crop: None,
            custom_gradient_enabled: None,
            custom_gradient_colors: None,
            custom_text_color: None,
            custom_text_secondary: None,
            chat_templates: Vec::new(),
            default_chat_template_id: None,
        },
        avatar_data,
        background_image_data,
    })
}

pub fn export_chara_card_v2(package: &CharacterExportPackage) -> CharaCardV2 {
    let definition = package.character.definition.clone().unwrap_or_default();
    let sections = parse_definition_sections(&definition);

    let description = if !package
        .character
        .description
        .as_deref()
        .unwrap_or("")
        .trim()
        .is_empty()
    {
        package.character.description.clone().unwrap_or_default()
    } else {
        sections.base
    };

    let mut first_mes = String::new();
    let mut alternate_greetings = Vec::new();
    if let Some(first_scene) = package.character.scenes.first() {
        first_mes = first_scene.content.clone();
        for scene in package.character.scenes.iter().skip(1) {
            if !scene.content.trim().is_empty() {
                alternate_greetings.push(scene.content.clone());
            }
        }
    }

    CharaCardV2 {
        spec: "chara_card_v2".to_string(),
        spec_version: "2.0".to_string(),
        data: CharaCardV2Data {
            name: package.character.name.clone(),
            description,
            personality: sections.personality,
            scenario: package
                .character
                .scenario
                .clone()
                .unwrap_or_else(|| sections.scenario.clone()),
            first_mes,
            mes_example: sections.mes_example,
            creator_notes: package.character.creator_notes.clone().unwrap_or_default(),
            system_prompt: sections.system_prompt,
            post_history_instructions: sections.post_history_instructions,
            alternate_greetings,
            character_book: package
                .character
                .character_book
                .clone()
                .and_then(|book| serde_json::from_value(book).ok()),
            tags: package.character.tags.clone().unwrap_or_default(),
            creator: package.character.creator.clone().unwrap_or_default(),
            character_version: String::new(),
            extensions: JsonValue::Object(JsonMap::new()),
            avatar: package.avatar_data.clone(),
        },
    }
}

pub fn export_chara_card_v3(
    package: &CharacterExportPackage,
    created_at: Option<i64>,
    updated_at: Option<i64>,
) -> CharaCardV3 {
    let definition = package.character.definition.clone().unwrap_or_default();
    let sections = parse_definition_sections(&definition);

    let description = if !package
        .character
        .description
        .as_deref()
        .unwrap_or("")
        .trim()
        .is_empty()
    {
        package.character.description.clone().unwrap_or_default()
    } else {
        sections.base
    };

    let mut first_mes = String::new();
    let mut alternate_greetings = Vec::new();
    if let Some(first_scene) = package.character.scenes.first() {
        first_mes = first_scene.content.clone();
        for scene in package.character.scenes.iter().skip(1) {
            if !scene.content.trim().is_empty() {
                alternate_greetings.push(scene.content.clone());
            }
        }
    }

    CharaCardV3 {
        spec: "chara_card_v3".to_string(),
        spec_version: "3.0".to_string(),
        data: CharaCardV3Data {
            name: package.character.name.clone(),
            description,
            personality: sections.personality,
            scenario: package
                .character
                .scenario
                .clone()
                .unwrap_or_else(|| sections.scenario.clone()),
            first_mes,
            mes_example: sections.mes_example,
            creator_notes: package.character.creator_notes.clone().unwrap_or_default(),
            system_prompt: sections.system_prompt,
            post_history_instructions: sections.post_history_instructions,
            alternate_greetings,
            character_book: package
                .character
                .character_book
                .clone()
                .and_then(|book| serde_json::from_value(book).ok()),
            tags: package.character.tags.clone().unwrap_or_default(),
            creator: package.character.creator.clone().unwrap_or_default(),
            character_version: String::new(),
            extensions: JsonValue::Object(JsonMap::new()),
            assets: None,
            nickname: package.character.nickname.clone(),
            creator_notes_multilingual: package.character.creator_notes_multilingual.clone(),
            source: package.character.source.clone(),
            avatar: package.avatar_data.clone(),
            group_only_greetings: Vec::new(),
            creation_date: created_at.map(|v| v / 1000),
            modification_date: updated_at.map(|v| v / 1000),
        },
    }
}

#[allow(dead_code)]
pub fn export_chara_card_v1(package: &CharacterExportPackage) -> CharaCardV1 {
    let definition = package.character.definition.clone().unwrap_or_default();
    let sections = parse_definition_sections(&definition);

    let description = if !package
        .character
        .description
        .as_deref()
        .unwrap_or("")
        .trim()
        .is_empty()
    {
        package.character.description.clone().unwrap_or_default()
    } else {
        sections.base
    };

    let mut first_mes = String::new();
    if let Some(first_scene) = package.character.scenes.first() {
        first_mes = first_scene.content.clone();
    }

    CharaCardV1 {
        name: package.character.name.clone(),
        description,
        personality: sections.personality,
        scenario: sections.scenario,
        first_mes,
        mes_example: sections.mes_example,
    }
}

pub fn guess_chara_card_format(value: &JsonValue) -> Option<CharacterFileFormat> {
    if looks_like_chara_card_v3(value) {
        return Some(CharacterFileFormat::CharaCardV3);
    }
    if looks_like_chara_card_v2(value) {
        return Some(CharacterFileFormat::CharaCardV2);
    }
    if looks_like_chara_card_v1(value) {
        return Some(CharacterFileFormat::CharaCardV1);
    }
    None
}
