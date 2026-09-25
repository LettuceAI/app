//! The character and persona package every entity file reads into and UEC
//! writes from: UEC v1/v2 cards, character cards and the pre-UEC package JSON.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use unified_entity_card::{
    SCHEMA_VERSION, SCHEMA_VERSION_V2, UecKind, assert_uec, convert_uec_v1_to_v2,
    create_character_uec, create_persona_uec, downgrade_uec,
};

use crate::{
    CharacterCardDraft, CharacterFileFormat, detect_character_card_format, looks_like_uec,
    parse_character_card,
};

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum EntityPackageError {
    #[error("Invalid import data: {0}")]
    InvalidImport(String),
    #[error("{0}")]
    InvalidUec(String),
    #[error("{0}")]
    InvalidCard(String),
    #[error("Failed to downgrade UEC v2 for legacy parser: {0}")]
    Downgrade(String),
    #[error("Invalid downgraded UEC payload: {0}")]
    InvalidDowngrade(String),
    #[error("Failed to upgrade UEC v1 payload to v2: {0}")]
    Upgrade(String),
    #[error("Invalid import: This is not a {0} UEC")]
    WrongKind(&'static str),
    #[error("Invalid UEC payload: {0}")]
    InvalidPayload(&'static str),
    #[error("Unsupported export version: {0}. Please update your app.")]
    UnsupportedVersion(u32),
    #[error("Failed to serialize export")]
    Serialize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterPackage {
    pub version: u32,
    #[serde(default)]
    pub exported_at: i64,
    pub character: CharacterPackageData,
    pub avatar_data: Option<String>,
    pub background_image_data: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageCrop {
    pub x: f64,
    pub y: f64,
    pub scale: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterPackageData {
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub definition: Option<String>,
    #[serde(default)]
    pub scenario: Option<String>,
    #[serde(default)]
    pub nickname: Option<String>,
    #[serde(default)]
    pub creator: Option<String>,
    #[serde(default)]
    pub creator_notes: Option<String>,
    #[serde(default)]
    pub creator_notes_multilingual: Option<Value>,
    #[serde(default)]
    pub source: Option<Vec<String>>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub character_book: Option<Value>,
    pub rules: Vec<String>,
    pub scenes: Vec<ScenePackage>,
    pub default_scene_id: Option<String>,
    pub default_model_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub companion: Option<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub companion_scheduled_notes: Vec<CompanionScheduledNotePackage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub companion_shared_memory: Option<CompanionSharedMemoryPackage>,
    #[serde(default)]
    pub memory_type: Option<String>,
    #[serde(default)]
    pub active_lorebook_ids: Vec<String>,
    #[serde(default)]
    pub lorebooks: Vec<LorebookPackage>,
    pub prompt_template_id: Option<String>,
    pub system_prompt: Option<String>,
    pub voice_config: Option<Value>,
    pub voice_autoplay: Option<bool>,
    pub disable_avatar_gradient: bool,
    pub avatar_crop: Option<PackageCrop>,
    #[serde(default)]
    pub banner_crop: Option<PackageCrop>,
    pub custom_gradient_enabled: Option<bool>,
    pub custom_gradient_colors: Option<Vec<String>>,
    pub custom_text_color: Option<String>,
    pub custom_text_secondary: Option<String>,
    #[serde(default)]
    pub chat_templates: Vec<ChatTemplatePackage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_chat_template_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanionScheduledNotePackage {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub character_id: Option<String>,
    #[serde(default)]
    pub label: String,
    pub content: String,
    pub available_at: i64,
    #[serde(default)]
    pub expires_at: Option<i64>,
    #[serde(default = "default_recurrence")]
    pub recurrence: String,
    #[serde(default)]
    pub recurrence_window_ms: Option<i64>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanionSharedMemoryPackage {
    #[serde(default)]
    pub memories: Value,
    #[serde(default)]
    pub memory_summary: Option<String>,
    #[serde(default)]
    pub memory_summary_token_count: i64,
    #[serde(default)]
    pub memory_tool_events: Value,
    #[serde(default)]
    pub memory_status: Option<String>,
    #[serde(default)]
    pub memory_error: Option<String>,
    #[serde(default)]
    pub memory_progress_step: Option<i64>,
    #[serde(default)]
    pub soul_growth: Value,
    #[serde(default)]
    pub relationship_states: Value,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub updated_at: i64,
}

fn default_recurrence() -> String {
    "none".to_owned()
}

const fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LorebookPackage {
    pub lorebook: PackagedLorebook,
    #[serde(default)]
    pub entries: Vec<PackagedLorebookEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackagedLorebook {
    pub id: String,
    pub name: String,
    pub avatar_path: Option<String>,
    #[serde(default)]
    pub keyword_detection_mode: PackagedKeywordDetectionMode,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PackagedKeywordDetectionMode {
    #[default]
    RecentMessageWindow,
    LatestUserMessage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackagedKeywordMatchMode {
    #[default]
    Literal,
    Regex,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackagedLorebookEntry {
    pub id: String,
    pub lorebook_id: String,
    pub title: String,
    pub enabled: bool,
    pub always_active: bool,
    pub keywords: Vec<String>,
    pub case_sensitive: bool,
    #[serde(default)]
    pub keyword_match_mode: PackagedKeywordMatchMode,
    pub content: String,
    pub priority: i32,
    pub display_order: i32,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenePackage {
    pub id: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background_image_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<i64>,
    pub selected_variant_id: Option<String>,
    pub variants: Vec<SceneVariantPackage>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneVariantPackage {
    pub id: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatTemplatePackage {
    pub id: String,
    pub name: String,
    pub messages: Vec<ChatTemplateMessagePackage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatTemplateMessagePackage {
    pub id: String,
    pub role: String,
    pub content: String,
}

/// Everything a persona file writes: the new persona, the lorebooks it is
/// bound to in order and whether it becomes the default.
#[derive(Debug, Clone, PartialEq)]
pub struct PersonaFileImport {
    pub persona: lettuce_characters::Persona,
    pub lorebook_ids: Vec<lettuce_types::LorebookId>,
    pub make_default: bool,
}

pub trait PersonaFileRepository: Send + Sync {
    /// Writes the whole import in one transaction, so a failure leaves nothing
    /// behind.
    fn import_persona_file(
        &self,
        import: &PersonaFileImport,
    ) -> Result<lettuce_characters::Persona, lettuce_characters::RepositoryError>;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonaPackage {
    pub version: u32,
    #[serde(default)]
    pub exported_at: i64,
    pub persona: PersonaPackageData,
    pub avatar_data: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonaPackageData {
    pub title: String,
    pub description: String,
    pub nickname: Option<String>,
    pub is_default: Option<bool>,
    pub avatar_crop: Option<PackageCrop>,
    #[serde(default)]
    pub active_lorebook_ids: Vec<String>,
}

/// UEC, a character card, then the pre-UEC package; `None` when none of them
/// reads the value.
#[must_use]
pub fn detect_character_format(value: &Value) -> Option<CharacterFileFormat> {
    detect_character_card_format(value).or_else(|| {
        serde_json::from_value::<CharacterPackage>(value.clone())
            .is_ok()
            .then_some(CharacterFileFormat::LegacyJson)
    })
}

/// Reads any character file into a package; cards get a fresh scene id per
/// greeting from `new_id`, created at `now`.
pub fn parse_character_import(
    value: &Value,
    now: i64,
    mut new_id: impl FnMut() -> String,
) -> Result<(CharacterPackage, CharacterFileFormat), EntityPackageError> {
    if looks_like_uec(value) {
        return Ok((parse_uec_character(value, now)?, CharacterFileFormat::Uec));
    }
    if let Some(draft) = parse_character_card(value)
        .map_err(|error| EntityPackageError::InvalidCard(error.to_string()))?
    {
        let format = draft.format;
        return Ok((package_from_card(draft, now, &mut new_id), format));
    }
    serde_json::from_value::<CharacterPackage>(value.clone())
        .map(|package| (package, CharacterFileFormat::LegacyJson))
        .map_err(|error| EntityPackageError::InvalidImport(error.to_string()))
}

fn package_from_card(
    draft: CharacterCardDraft,
    now: i64,
    new_id: &mut impl FnMut() -> String,
) -> CharacterPackage {
    let scenes = draft
        .greetings
        .into_iter()
        .map(|content| ScenePackage {
            id: new_id(),
            content,
            direction: None,
            background_image_path: None,
            created_at: Some(now),
            selected_variant_id: None,
            variants: Vec::new(),
        })
        .collect::<Vec<_>>();
    CharacterPackage {
        version: 1,
        exported_at: now,
        character: CharacterPackageData {
            name: draft.name,
            description: draft.description,
            definition: draft.definition,
            scenario: draft.scenario,
            nickname: draft.nickname,
            creator: draft.creator,
            creator_notes: draft.creator_notes,
            creator_notes_multilingual: draft.creator_notes_multilingual,
            source: draft.source,
            tags: draft.tags,
            character_book: draft
                .character_book
                .as_ref()
                .and_then(|book| serde_json::to_value(book).ok()),
            rules: Vec::new(),
            default_scene_id: scenes.first().map(|scene| scene.id.clone()),
            scenes,
            default_model_id: None,
            mode: None,
            companion: None,
            companion_scheduled_notes: Vec::new(),
            companion_shared_memory: None,
            memory_type: Some("manual".to_owned()),
            active_lorebook_ids: Vec::new(),
            lorebooks: Vec::new(),
            prompt_template_id: None,
            system_prompt: None,
            voice_config: None,
            voice_autoplay: None,
            disable_avatar_gradient: false,
            avatar_crop: None,
            banner_crop: None,
            custom_gradient_enabled: None,
            custom_gradient_colors: None,
            custom_text_color: None,
            custom_text_secondary: None,
            chat_templates: Vec::new(),
            default_chat_template_id: None,
        },
        avatar_data: draft.avatar,
        background_image_data: draft.background,
    }
}

fn number_to_i64(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().map(|value| value as i64))
        .or_else(|| value.as_f64().map(|value| value as i64))
}

fn string(map: &Map<String, Value>, key: &str) -> Option<String> {
    map.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn strings(value: Option<&Value>) -> Option<Vec<String>> {
    value.and_then(Value::as_array).map(|items| {
        items
            .iter()
            .filter_map(|item| item.as_str().map(str::to_owned))
            .collect()
    })
}

fn parse_crop(value: Option<&Value>) -> Option<PackageCrop> {
    let crop = value?.as_object()?;
    Some(PackageCrop {
        x: crop.get("x")?.as_f64()?,
        y: crop.get("y")?.as_f64()?,
        scale: crop.get("scale")?.as_f64()?,
    })
}

fn asset_string_to_v2_locator(value: &str) -> Value {
    if let Some(rest) = value.strip_prefix("data:") {
        let (mime_type, data) = rest.split_once(";base64,").unwrap_or(("", rest));
        let mut locator = Map::new();
        locator.insert("type".into(), Value::String("inline_base64".to_owned()));
        if !mime_type.is_empty() {
            locator.insert("mimeType".into(), Value::String(mime_type.to_owned()));
        }
        locator.insert("data".into(), Value::String(data.to_owned()));
        return Value::Object(locator);
    }
    if value.starts_with("http://") || value.starts_with("https://") {
        let mut locator = Map::new();
        locator.insert("type".into(), Value::String("remote_url".to_owned()));
        locator.insert("url".into(), Value::String(value.to_owned()));
        return Value::Object(locator);
    }
    Value::String(value.to_owned())
}

fn asset_locator_to_string(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(content) => Some(content.clone()),
        Value::Object(map) => match map.get("type").and_then(Value::as_str) {
            Some("inline_base64") => {
                let data = map.get("data").and_then(Value::as_str)?;
                let mime_type = map
                    .get("mimeType")
                    .and_then(Value::as_str)
                    .unwrap_or("application/octet-stream");
                Some(format!("data:{mime_type};base64,{data}"))
            }
            Some("remote_url") => map.get("url").and_then(Value::as_str).map(str::to_owned),
            _ => None,
        },
        _ => None,
    }
}

fn normalize_asset_fields(card: &mut Value, to_v2: bool) {
    let Some(payload) = card.get_mut("payload").and_then(Value::as_object_mut) else {
        return;
    };
    for key in ["avatar", "chatBackground"] {
        let Some(current) = payload.get(key).cloned() else {
            continue;
        };
        if to_v2 {
            if let Value::String(text) = current {
                payload.insert(key.to_owned(), asset_string_to_v2_locator(&text));
            }
        } else if let Some(text) = asset_locator_to_string(Some(&current)) {
            payload.insert(key.to_owned(), Value::String(text));
        }
    }
}

fn normalize_uec_for_read(value: &Value) -> Result<unified_entity_card::Uec, EntityPackageError> {
    let uec = assert_uec(value, false).map_err(EntityPackageError::InvalidUec)?;
    if uec.schema.version != SCHEMA_VERSION_V2 {
        return Ok(uec);
    }
    let mut downgraded = downgrade_uec(value, SCHEMA_VERSION, false)
        .map_err(EntityPackageError::Downgrade)?
        .card;
    normalize_asset_fields(&mut downgraded, false);
    assert_uec(&downgraded, false).map_err(EntityPackageError::InvalidDowngrade)
}

/// The v2 payload fields a v1 downgrade removes, read from the card itself.
fn v2_payload(value: &Value) -> Option<&Map<String, Value>> {
    (value
        .get("schema")
        .and_then(|schema| schema.get("version"))
        .and_then(Value::as_str)
        == Some(SCHEMA_VERSION_V2))
    .then(|| value.get("payload").and_then(Value::as_object))
    .flatten()
}

fn scene_from_value(scene: &Value) -> Option<ScenePackage> {
    let map = scene.as_object()?;
    Some(ScenePackage {
        id: map.get("id")?.as_str()?.to_owned(),
        content: map.get("content")?.as_str()?.to_owned(),
        direction: string(map, "direction"),
        background_image_path: string(map, "backgroundImagePath"),
        created_at: map.get("createdAt").and_then(number_to_i64),
        selected_variant_id: string(map, "selectedVariantId"),
        variants: map
            .get("variants")
            .and_then(Value::as_array)
            .map(|variants| {
                variants
                    .iter()
                    .filter_map(|variant| {
                        let map = variant.as_object()?;
                        Some(SceneVariantPackage {
                            id: map.get("id")?.as_str()?.to_owned(),
                            content: map.get("content")?.as_str()?.to_owned(),
                            direction: string(map, "direction"),
                            created_at: map.get("createdAt").and_then(number_to_i64),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default(),
    })
}

fn v2_scene_variants_as_scenes(value: &Value) -> Option<(Vec<ScenePackage>, Option<String>)> {
    let scene = v2_payload(value)?.get("scene")?.as_object()?;
    let base_id = scene.get("id")?.as_str()?.to_owned();
    let mut scenes = vec![ScenePackage {
        id: base_id.clone(),
        content: scene.get("content")?.as_str()?.to_owned(),
        direction: string(scene, "direction"),
        background_image_path: None,
        created_at: scene.get("createdAt").and_then(number_to_i64),
        selected_variant_id: None,
        variants: Vec::new(),
    }];
    for variant in scene
        .get("variants")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(map) = variant.as_object() else {
            continue;
        };
        let (Some(id), Some(content)) = (
            map.get("id").and_then(Value::as_str),
            map.get("content").and_then(Value::as_str),
        ) else {
            continue;
        };
        scenes.push(ScenePackage {
            id: id.to_owned(),
            content: content.to_owned(),
            direction: string(map, "direction"),
            background_image_path: None,
            created_at: map.get("createdAt").and_then(number_to_i64),
            selected_variant_id: None,
            variants: Vec::new(),
        });
    }
    let default_scene_id = match scene.get("selectedVariant") {
        Some(Value::String(selected)) => Some(selected.clone()),
        _ => Some(base_id),
    };
    Some((scenes, default_scene_id))
}

/// A character UEC (v1, or v2 read through its v1 downgrade) as a package; a
/// v2 card keeps the nickname, creator fields, source and embedded lorebook
/// the downgrade removes.
pub fn parse_uec_character(
    value: &Value,
    now: i64,
) -> Result<CharacterPackage, EntityPackageError> {
    let uec = normalize_uec_for_read(value)?;
    if uec.kind != UecKind::Character {
        return Err(EntityPackageError::WrongKind("character"));
    }
    let payload = uec
        .payload
        .as_object()
        .ok_or(EntityPackageError::InvalidPayload("expected object"))?;
    let original = v2_payload(value).unwrap_or(payload);
    let name = string(payload, "name").ok_or(EntityPackageError::InvalidPayload("missing name"))?;
    let description = string(payload, "description");
    let definition = string(payload, "definitions").or_else(|| description.clone());
    let (prompt_template_id, system_prompt) =
        match payload.get("systemPrompt").and_then(Value::as_str) {
            Some(value) if value.starts_with("_ID:") => {
                (Some(value.trim_start_matches("_ID:").to_owned()), None)
            }
            Some(value) => (None, Some(value.to_owned())),
            None => (None, None),
        };
    let app = uec
        .app_specific_settings
        .as_ref()
        .and_then(Value::as_object);
    let setting = |key: &str| app.and_then(|map| map.get(key));
    let setting_or_payload = |key: &str| setting(key).or_else(|| payload.get(key));
    let mode = setting("interaction_mode")
        .and_then(Value::as_str)
        .or_else(|| setting("mode").and_then(Value::as_str))
        .or_else(|| payload.get("mode").and_then(Value::as_str))
        .map_or("roleplay", |value| {
            if value.eq_ignore_ascii_case("companion") {
                "companion"
            } else {
                "roleplay"
            }
        })
        .to_owned();
    let mut scenes = payload
        .get("scenes")
        .and_then(Value::as_array)
        .map(|scenes| scenes.iter().filter_map(scene_from_value).collect())
        .unwrap_or_default();
    let mut default_scene_id = string(payload, "defaultSceneId");
    if let Some((v2_scenes, v2_default)) = v2_scene_variants_as_scenes(value) {
        scenes = v2_scenes;
        default_scene_id = v2_default;
    }
    Ok(CharacterPackage {
        version: 1,
        exported_at: now,
        character: CharacterPackageData {
            name,
            description,
            definition,
            scenario: string(payload, "scenario"),
            nickname: string(original, "nickname"),
            creator: string(original, "creator"),
            creator_notes: original
                .get("creatorNotes")
                .or_else(|| original.get("creator_notes"))
                .and_then(Value::as_str)
                .map(str::to_owned),
            creator_notes_multilingual: original
                .get("creatorNotesMultilingual")
                .or_else(|| original.get("creator_notes_multilingual"))
                .cloned(),
            source: strings(original.get("source")),
            tags: strings(payload.get("tags")),
            character_book: original
                .get("characterBook")
                .or_else(|| original.get("character_book"))
                .cloned(),
            rules: strings(payload.get("rules")).unwrap_or_default(),
            scenes,
            default_scene_id,
            default_model_id: string(payload, "defaultModelId"),
            mode: Some(mode),
            companion: setting_or_payload("companion").cloned(),
            companion_scheduled_notes: setting_or_payload("companionScheduledNotes")
                .and_then(|value| serde_json::from_value(value.clone()).ok())
                .unwrap_or_default(),
            companion_shared_memory: setting_or_payload("companionSharedMemory")
                .and_then(|value| serde_json::from_value(value.clone()).ok()),
            memory_type: setting("memoryType")
                .and_then(Value::as_str)
                .map(str::to_owned),
            active_lorebook_ids: strings(setting_or_payload("activeLorebookIds"))
                .unwrap_or_default(),
            lorebooks: setting_or_payload("lorebooks")
                .and_then(|value| serde_json::from_value(value.clone()).ok())
                .unwrap_or_default(),
            prompt_template_id,
            system_prompt,
            voice_config: payload.get("voiceConfig").cloned(),
            voice_autoplay: payload.get("voiceAutoplay").and_then(Value::as_bool),
            disable_avatar_gradient: setting("disableAvatarGradient")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            avatar_crop: parse_crop(setting("avatarCrop")),
            banner_crop: parse_crop(setting("bannerCrop")),
            custom_gradient_enabled: setting("customGradientEnabled").and_then(Value::as_bool),
            custom_gradient_colors: strings(setting("customGradientColors")),
            custom_text_color: setting("customTextColor")
                .and_then(Value::as_str)
                .map(str::to_owned),
            custom_text_secondary: setting("customTextSecondary")
                .and_then(Value::as_str)
                .map(str::to_owned),
            chat_templates: setting("chatTemplates")
                .and_then(|value| serde_json::from_value(value.clone()).ok())
                .unwrap_or_default(),
            default_chat_template_id: setting("defaultChatTemplateId")
                .and_then(Value::as_str)
                .map(str::to_owned),
        },
        avatar_data: asset_locator_to_string(payload.get("avatar")),
        background_image_data: asset_locator_to_string(payload.get("chatBackground")),
    })
}

/// A persona UEC or pre-UEC persona package; the version must be at most 1.
pub fn parse_persona_import(value: &Value, now: i64) -> Result<PersonaPackage, EntityPackageError> {
    let package = if looks_like_uec(value) {
        parse_uec_persona(value, now)?
    } else {
        serde_json::from_value::<PersonaPackage>(value.clone())
            .map_err(|error| EntityPackageError::InvalidImport(error.to_string()))?
    };
    if package.version > 1 {
        return Err(EntityPackageError::UnsupportedVersion(package.version));
    }
    Ok(package)
}

fn parse_uec_persona(value: &Value, now: i64) -> Result<PersonaPackage, EntityPackageError> {
    let uec = normalize_uec_for_read(value)?;
    if uec.kind != UecKind::Persona {
        return Err(EntityPackageError::WrongKind("persona"));
    }
    let payload = uec
        .payload
        .as_object()
        .ok_or(EntityPackageError::InvalidPayload("expected object"))?;
    let original = v2_payload(value).unwrap_or(payload);
    let app = uec.app_specific_settings.as_ref();
    Ok(PersonaPackage {
        version: 1,
        exported_at: now,
        persona: PersonaPackageData {
            title: string(payload, "title")
                .ok_or(EntityPackageError::InvalidPayload("missing title"))?,
            description: string(payload, "description").unwrap_or_default(),
            nickname: string(original, "nickname"),
            is_default: payload.get("isDefault").and_then(Value::as_bool),
            avatar_crop: parse_crop(app.and_then(|value| value.get("avatarCrop"))),
            active_lorebook_ids: strings(
                app.and_then(|value| value.get("activeLorebookIds"))
                    .or_else(|| payload.get("activeLorebookIds")),
            )
            .unwrap_or_default(),
        },
        avatar_data: asset_locator_to_string(payload.get("avatar")),
    })
}

fn resolve_v1_scene_for_v2(card: &Value) -> Option<Value> {
    let payload = card.get("payload")?.as_object()?;
    let scenes = payload.get("scenes")?.as_array()?;
    if scenes.is_empty() {
        return None;
    }
    let default_scene_id = payload.get("defaultSceneId").and_then(Value::as_str);
    let picked = default_scene_id
        .and_then(|id| {
            scenes
                .iter()
                .find(|scene| scene.get("id").and_then(Value::as_str) == Some(id))
        })
        .or_else(|| scenes.first())?;
    let selected_scene_id = picked.get("id").and_then(Value::as_str)?.to_owned();
    let mut scene = picked.as_object()?.clone();
    let selected_variant_id = scene
        .remove("selectedVariantId")
        .and_then(|value| value.as_str().map(str::to_owned));
    let mut merged = scene
        .get("variants")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for alternate in scenes.iter().filter(|scene| {
        scene
            .get("id")
            .and_then(Value::as_str)
            .is_some_and(|id| id != selected_scene_id)
    }) {
        let Some(map) = alternate.as_object() else {
            continue;
        };
        let mut variant = Map::new();
        for key in ["id", "content", "direction"] {
            if let Some(value) = map.get(key).cloned() {
                variant.insert(key.to_owned(), value);
            }
        }
        if let Some(created_at) = map
            .get("createdAt")
            .or_else(|| map.get("created_at"))
            .cloned()
        {
            variant.insert("createdAt".to_owned(), created_at);
        }
        if variant.contains_key("id") && variant.contains_key("content") {
            merged.push(Value::Object(variant));
        }
        if let Some(extra) = map.get("variants").and_then(Value::as_array) {
            merged.extend(extra.iter().cloned());
        }
    }
    if !merged.is_empty() {
        scene.insert("variants".to_owned(), Value::Array(merged.clone()));
    }
    let selected = selected_variant_id
        .filter(|selected| {
            merged
                .iter()
                .any(|variant| variant.get("id").and_then(Value::as_str) == Some(selected))
        })
        .map_or_else(|| Value::from(0), Value::String);
    scene.insert("selectedVariant".to_owned(), selected);
    Some(Value::Object(scene))
}

fn stringify_v2_uec(card: &Value) -> Result<String, EntityPackageError> {
    let resolved_scene = resolve_v1_scene_for_v2(card);
    let mut upgraded = convert_uec_v1_to_v2(card).map_err(EntityPackageError::Upgrade)?;
    if let Some(scene) = resolved_scene
        && let Some(payload) = upgraded.get_mut("payload").and_then(Value::as_object_mut)
    {
        payload.insert("scene".to_owned(), scene);
    }
    normalize_asset_fields(&mut upgraded, true);
    serde_json::to_string_pretty(&upgraded).map_err(|_| EntityPackageError::Serialize)
}

fn crop_value(crop: &PackageCrop) -> Value {
    serde_json::json!({ "x": crop.x, "y": crop.y, "scale": crop.scale })
}

/// The pretty-printed v2 character UEC of a package.
pub fn build_character_uec(
    package: &CharacterPackage,
    character_id: &str,
    created_at: Option<i64>,
    updated_at: Option<i64>,
) -> Result<String, EntityPackageError> {
    let character = &package.character;
    let mut payload = Map::new();
    payload.insert("id".into(), Value::String(character_id.to_owned()));
    payload.insert("name".into(), Value::String(character.name.clone()));
    if let Some(description) = &character.description {
        payload.insert("description".into(), Value::String(description.clone()));
    }
    if let Some(definition) = character
        .definition
        .clone()
        .or_else(|| character.description.clone())
    {
        payload.insert("definitions".into(), Value::String(definition));
    }
    if let Some(data) = &package.avatar_data {
        payload.insert("avatar".into(), Value::String(data.clone()));
    }
    if let Some(data) = &package.background_image_data {
        payload.insert("chatBackground".into(), Value::String(data.clone()));
    }
    payload.insert("rules".into(), serde_json::json!(character.rules));
    payload.insert(
        "scenes".into(),
        serde_json::to_value(&character.scenes).map_err(|_| EntityPackageError::Serialize)?,
    );
    let optional = [
        ("defaultSceneId", character.default_scene_id.clone()),
        ("defaultModelId", character.default_model_id.clone()),
        ("scenario", character.scenario.clone()),
        ("nickname", character.nickname.clone()),
        ("creator", character.creator.clone()),
        ("creatorNotes", character.creator_notes.clone()),
    ];
    for (key, value) in optional {
        if let Some(value) = value {
            payload.insert(key.into(), Value::String(value));
        }
    }
    if let Some(value) = &character.creator_notes_multilingual {
        payload.insert("creatorNotesMultilingual".into(), value.clone());
    }
    if let Some(value) = &character.source {
        payload.insert("source".into(), serde_json::json!(value));
    }
    if let Some(value) = &character.tags {
        payload.insert("tags".into(), serde_json::json!(value));
    }
    if let Some(value) = &character.character_book {
        payload.insert("characterBook".into(), value.clone());
    }
    let mut system_prompt_is_id = false;
    if let Some(template) = &character.prompt_template_id {
        payload.insert("systemPrompt".into(), Value::String(template.clone()));
        system_prompt_is_id = true;
    } else if let Some(prompt) = &character.system_prompt {
        payload.insert("systemPrompt".into(), Value::String(prompt.clone()));
    }
    if let Some(voice) = character
        .voice_config
        .as_ref()
        .filter(|voice| !voice.is_null())
    {
        payload.insert("voiceConfig".into(), voice.clone());
    }
    payload.insert(
        "voiceAutoplay".into(),
        Value::Bool(character.voice_autoplay.unwrap_or(false)),
    );
    let created_at = created_at.unwrap_or(package.exported_at);
    let updated_at = updated_at.unwrap_or(package.exported_at);
    payload.insert("createdAt".into(), Value::from(created_at));
    payload.insert("updatedAt".into(), Value::from(updated_at));

    let mut app = Map::new();
    app.insert(
        "disableAvatarGradient".into(),
        Value::Bool(character.disable_avatar_gradient),
    );
    if let Some(mode) = &character.mode {
        app.insert("interaction_mode".into(), Value::String(mode.clone()));
        app.insert("mode".into(), Value::String(mode.clone()));
    }
    if let Some(companion) = &character.companion {
        app.insert("companion".into(), companion.clone());
    }
    if !character.companion_scheduled_notes.is_empty() {
        app.insert(
            "companionScheduledNotes".into(),
            serde_json::to_value(&character.companion_scheduled_notes)
                .unwrap_or_else(|_| Value::Array(Vec::new())),
        );
    }
    if let Some(memory) = &character.companion_shared_memory {
        app.insert(
            "companionSharedMemory".into(),
            serde_json::to_value(memory).unwrap_or_else(|_| Value::Object(Map::new())),
        );
    }
    app.insert(
        "memoryType".into(),
        Value::String(
            character
                .memory_type
                .clone()
                .unwrap_or_else(|| "manual".to_owned()),
        ),
    );
    if !character.active_lorebook_ids.is_empty() {
        app.insert(
            "activeLorebookIds".into(),
            serde_json::json!(character.active_lorebook_ids),
        );
    }
    if !character.lorebooks.is_empty() {
        app.insert(
            "lorebooks".into(),
            serde_json::to_value(&character.lorebooks).unwrap_or_else(|_| Value::Array(Vec::new())),
        );
    }
    app.insert(
        "customGradientEnabled".into(),
        Value::Bool(character.custom_gradient_enabled.unwrap_or(false)),
    );
    if let Some(colors) = &character.custom_gradient_colors {
        app.insert("customGradientColors".into(), serde_json::json!(colors));
    }
    if let Some(color) = &character.custom_text_color {
        app.insert("customTextColor".into(), Value::String(color.clone()));
    }
    if let Some(color) = &character.custom_text_secondary {
        app.insert("customTextSecondary".into(), Value::String(color.clone()));
    }
    if let Some(crop) = &character.avatar_crop {
        app.insert("avatarCrop".into(), crop_value(crop));
    }
    if let Some(crop) = &character.banner_crop {
        app.insert("bannerCrop".into(), crop_value(crop));
    }
    if !character.chat_templates.is_empty() {
        app.insert(
            "chatTemplates".into(),
            serde_json::to_value(&character.chat_templates)
                .unwrap_or_else(|_| Value::Array(Vec::new())),
        );
    }
    if let Some(template) = &character.default_chat_template_id {
        app.insert(
            "defaultChatTemplateId".into(),
            Value::String(template.clone()),
        );
    }
    let card = create_character_uec(
        payload,
        system_prompt_is_id,
        None,
        Some(Value::Object(app)),
        Some(lettuce_meta(created_at, updated_at)),
        Some(Value::Object(Map::new())),
    );
    stringify_v2_uec(&card)
}

fn lettuce_meta(created_at: i64, updated_at: i64) -> Value {
    let mut meta = Map::new();
    meta.insert("createdAt".into(), Value::from(created_at));
    meta.insert("updatedAt".into(), Value::from(updated_at));
    meta.insert("source".into(), Value::String("lettuceai".to_owned()));
    Value::Object(meta)
}

/// The pretty-printed v2 persona UEC of a persona package, as a conversion
/// writes it: no nickname, an empty description and an unset default omitted.
pub fn build_persona_package_uec(
    package: &PersonaPackage,
    persona_id: &str,
    created_at: Option<i64>,
    updated_at: Option<i64>,
) -> Result<String, EntityPackageError> {
    let mut payload = Map::new();
    payload.insert("id".into(), Value::String(persona_id.to_owned()));
    payload.insert("title".into(), Value::String(package.persona.title.clone()));
    if !package.persona.description.is_empty() {
        payload.insert(
            "description".into(),
            Value::String(package.persona.description.clone()),
        );
    }
    if let Some(avatar) = &package.avatar_data {
        payload.insert("avatar".into(), Value::String(avatar.clone()));
    }
    if let Some(is_default) = package.persona.is_default {
        payload.insert("isDefault".into(), Value::Bool(is_default));
    }
    let created_at = created_at.unwrap_or(package.exported_at);
    let updated_at = updated_at.unwrap_or(package.exported_at);
    payload.insert("createdAt".into(), Value::from(created_at));
    payload.insert("updatedAt".into(), Value::from(updated_at));
    let mut app = Map::new();
    if let Some(crop) = &package.persona.avatar_crop {
        app.insert("avatarCrop".into(), crop_value(crop));
    }
    if !package.persona.active_lorebook_ids.is_empty() {
        app.insert(
            "activeLorebookIds".into(),
            serde_json::json!(package.persona.active_lorebook_ids),
        );
    }
    let card = create_persona_uec(
        payload,
        None,
        Some(Value::Object(app)),
        Some(lettuce_meta(created_at, updated_at)),
        Some(Value::Object(Map::new())),
    );
    stringify_v2_uec(&card)
}

/// What a persona UEC export reads from a persona.
#[derive(Debug, Clone, PartialEq)]
pub struct PersonaUecSource {
    pub id: String,
    pub title: String,
    pub description: String,
    pub nickname: Option<String>,
    pub is_default: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub avatar: Option<String>,
    pub avatar_crop: Option<PackageCrop>,
    pub active_lorebook_ids: Vec<String>,
}

/// The pretty-printed v2 persona UEC.
pub fn build_persona_uec(persona: &PersonaUecSource) -> Result<String, EntityPackageError> {
    let mut payload = Map::new();
    payload.insert("id".into(), Value::String(persona.id.clone()));
    payload.insert("title".into(), Value::String(persona.title.clone()));
    payload.insert(
        "description".into(),
        Value::String(persona.description.clone()),
    );
    if let Some(nickname) = &persona.nickname {
        payload.insert("nickname".into(), Value::String(nickname.clone()));
    }
    payload.insert("isDefault".into(), Value::Bool(persona.is_default));
    payload.insert("createdAt".into(), Value::from(persona.created_at));
    payload.insert("updatedAt".into(), Value::from(persona.updated_at));
    if let Some(avatar) = &persona.avatar {
        payload.insert("avatar".into(), Value::String(avatar.clone()));
    }
    let mut app = Map::new();
    if let Some(crop) = &persona.avatar_crop {
        app.insert("avatarCrop".into(), crop_value(crop));
    }
    if !persona.active_lorebook_ids.is_empty() {
        app.insert(
            "activeLorebookIds".into(),
            serde_json::json!(persona.active_lorebook_ids),
        );
    }
    let card = create_persona_uec(
        payload,
        None,
        Some(Value::Object(app)),
        Some(lettuce_meta(persona.created_at, persona.updated_at)),
        Some(Value::Object(Map::new())),
    );
    stringify_v2_uec(&card)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn v1_card(scenes: Value, extra: &[(&str, Value)]) -> Value {
        let mut payload = Map::new();
        payload.insert("id".into(), json!("char-1"));
        payload.insert("name".into(), json!("Aster Vale"));
        payload.insert("scenes".into(), scenes);
        payload.insert("defaultSceneId".into(), json!("scene-1"));
        payload.insert("createdAt".into(), json!(1));
        payload.insert("updatedAt".into(), json!(2));
        for (key, value) in extra {
            payload.insert((*key).into(), value.clone());
        }
        create_character_uec(
            payload,
            false,
            None,
            None,
            Some(json!({ "createdAt": 1, "updatedAt": 2, "source": "lettuceai" })),
            None,
        )
    }

    fn v2(card: &Value) -> Value {
        serde_json::from_str(&stringify_v2_uec(card).expect("v2 json")).expect("json")
    }

    #[test]
    fn v1_and_v2_cards_are_readable() {
        let v1 = json!({
            "schema": { "name": "UEC", "version": SCHEMA_VERSION },
            "kind": "character",
            "payload": { "id": "char-v1", "name": "Aster Vale" }
        });
        assert_eq!(
            normalize_uec_for_read(&v1).expect("v1").schema.version,
            SCHEMA_VERSION
        );
        let v2 = json!({
            "schema": { "name": "UEC", "version": SCHEMA_VERSION_V2 },
            "kind": "character",
            "payload": {
                "id": "char-v2",
                "name": "Aster Vale",
                "scene": { "id": "scene-1", "content": "Hello there", "selectedVariant": 0, "variants": [] }
            },
            "meta": { "originalCreatedAt": 1, "originalUpdatedAt": 2 }
        });
        let read = normalize_uec_for_read(&v2).expect("v2");
        assert_eq!(read.schema.version, SCHEMA_VERSION);
        assert!(read.payload.get("scenes").is_some() && read.payload.get("scene").is_none());
    }

    #[test]
    fn stringify_upgrades_to_v2_with_asset_locators() {
        let value = v2(&v1_card(
            json!([{ "id": "scene-1", "content": "Hello there", "selectedVariantId": null, "variants": [] }]),
            &[
                ("avatar", json!("data:image/webp;base64,QUJD")),
                ("chatBackground", json!("https://example.com/bg.png")),
            ],
        ));
        assert_eq!(value["schema"]["version"], SCHEMA_VERSION_V2);
        assert!(value["payload"].get("scenes").is_none());
        assert_eq!(
            value["payload"]["avatar"],
            json!({"type": "inline_base64", "mimeType": "image/webp", "data": "QUJD"})
        );
        assert_eq!(value["payload"]["chatBackground"]["type"], "remote_url");
    }

    #[test]
    fn stringify_keeps_the_selected_variant_and_flattens_other_scenes() {
        let value = v2(&v1_card(
            json!([
                { "id": "scene-1", "content": "Hello there", "selectedVariantId": "variant-2", "variants": [
                    { "id": "variant-1", "content": "Variant one", "createdAt": 10 },
                    { "id": "variant-2", "content": "Variant two", "direction": "Second", "createdAt": 20 }
                ] },
                { "id": "scene-2", "content": "Second scene", "direction": "alt", "createdAt": 20, "selectedVariantId": null, "variants": [] }
            ]),
            &[],
        ));
        let scene = &value["payload"]["scene"];
        assert_eq!(scene["selectedVariant"], "variant-2");
        let ids = scene["variants"]
            .as_array()
            .expect("variants")
            .iter()
            .map(|variant| variant["id"].as_str().expect("id"))
            .collect::<Vec<_>>();
        assert_eq!(ids, ["variant-1", "variant-2", "scene-2"]);
        assert_eq!(scene["variants"][2]["direction"], "alt");
    }

    #[test]
    fn v2_characters_read_assets_scenes_and_the_fields_a_downgrade_drops() {
        let card = json!({
            "schema": { "name": "UEC", "version": SCHEMA_VERSION_V2 },
            "kind": "character",
            "payload": {
                "id": "char-v2",
                "name": "Aster Vale",
                "nickname": "Aster",
                "creator": "Ada",
                "creatorNotes": "notes",
                "source": ["hub"],
                "characterBook": { "entries": [] },
                "avatar": { "type": "inline_base64", "mimeType": "image/webp", "data": "QUJD" },
                "chatBackground": { "type": "remote_url", "url": "https://example.com/bg.png" },
                "scene": {
                    "id": "scene-1",
                    "content": "Primary scene",
                    "selectedVariant": "scene-3",
                    "variants": [
                        { "id": "scene-2", "content": "Second scene", "direction": "Alt two", "createdAt": 20 },
                        { "id": "scene-3", "content": "Third scene", "createdAt": 30 }
                    ]
                }
            },
            "meta": { "createdAt": 1, "updatedAt": 2, "originalCreatedAt": 1, "originalUpdatedAt": 2 }
        });
        let package = parse_uec_character(&card, 5).expect("v2 character");
        assert_eq!(
            package.avatar_data.as_deref(),
            Some("data:image/webp;base64,QUJD")
        );
        assert_eq!(
            package.background_image_data.as_deref(),
            Some("https://example.com/bg.png")
        );
        let ids = package
            .character
            .scenes
            .iter()
            .map(|scene| scene.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids, ["scene-1", "scene-2", "scene-3"]);
        assert_eq!(
            package.character.default_scene_id.as_deref(),
            Some("scene-3")
        );
        assert_eq!(package.character.nickname.as_deref(), Some("Aster"));
        assert_eq!(package.character.creator.as_deref(), Some("Ada"));
        assert_eq!(package.character.creator_notes.as_deref(), Some("notes"));
        assert_eq!(package.character.source, Some(vec!["hub".to_owned()]));
        assert_eq!(
            package.character.character_book,
            Some(json!({"entries": []}))
        );
        assert_eq!(package.character.mode.as_deref(), Some("roleplay"));
    }

    #[test]
    fn a_built_character_uec_reads_back() {
        let (package, format) = parse_character_import(
            &json!({"spec": "chara_card_v2", "data": {"name": "Ada", "first_mes": "Hi", "alternate_greetings": ["Yo", " "]}}),
            7,
            {
                let mut next = 0;
                move || {
                    next += 1;
                    format!("scene-{next}")
                }
            },
        )
        .expect("card");
        assert_eq!(format, CharacterFileFormat::CharaCardV2);
        assert_eq!(
            package.character.default_scene_id.as_deref(),
            Some("scene-1")
        );
        assert_eq!(package.character.scenes.len(), 2);
        let mut package = package;
        package.character.mode = Some("companion".into());
        package.character.prompt_template_id = Some("template".into());
        let uec: Value = serde_json::from_str(
            &build_character_uec(&package, "char-9", Some(3), None).expect("uec"),
        )
        .expect("json");
        assert_eq!(uec["payload"]["promptTemplateId"], "template");
        assert_eq!(
            uec["app_specific_settings"]["interaction_mode"],
            "companion"
        );
        assert_eq!(uec["meta"]["originalSource"], "lettuceai");
        let read = parse_uec_character(&uec, 8).expect("read back");
        assert_eq!(read.character.name, "Ada");
        assert_eq!(
            read.character.prompt_template_id.as_deref(),
            Some("template")
        );
        assert_eq!(read.character.mode.as_deref(), Some("companion"));
        assert_eq!(read.character.scenes.len(), 2);
        assert!(matches!(
            parse_character_import(&json!({"version": 1}), 0, String::new),
            Err(EntityPackageError::InvalidImport(_))
        ));
        assert_eq!(detect_character_format(&json!({"version": 1})), None);
    }

    #[test]
    fn a_built_persona_uec_reads_back_with_its_nickname() {
        let uec: Value = serde_json::from_str(
            &build_persona_uec(&PersonaUecSource {
                id: "persona-1".into(),
                title: "Traveller".into(),
                description: "Curious".into(),
                nickname: Some("T".into()),
                is_default: true,
                created_at: 1,
                updated_at: 2,
                avatar: None,
                avatar_crop: Some(PackageCrop {
                    x: 0.5,
                    y: 0.5,
                    scale: 1.0,
                }),
                active_lorebook_ids: vec!["book".into()],
            })
            .expect("persona"),
        )
        .expect("json");
        assert_eq!(uec["schema"]["version"], SCHEMA_VERSION_V2);
        let read = parse_persona_import(&uec, 3).expect("read back");
        assert_eq!(read.persona.title, "Traveller");
        assert_eq!(read.persona.nickname.as_deref(), Some("T"));
        assert_eq!(read.persona.is_default, Some(true));
        assert_eq!(read.persona.active_lorebook_ids, vec!["book".to_owned()]);
        let converted: Value = serde_json::from_str(
            &build_persona_package_uec(&read, "persona-2", None, None).expect("convert"),
        )
        .expect("json");
        assert!(converted["payload"].get("nickname").is_none());
        assert_eq!(converted["payload"]["createdAt"], 3);
        assert!(matches!(
            parse_persona_import(
                &json!({"version": 2, "persona": {"title": "x", "description": "", "nickname": null, "isDefault": null, "avatarCrop": null}, "avatarData": null}),
                0
            ),
            Err(EntityPackageError::UnsupportedVersion(2))
        ));
    }
}
