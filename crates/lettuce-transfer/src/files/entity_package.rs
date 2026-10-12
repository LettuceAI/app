//! Compatibility readers for UEC v1/v2, character cards and package JSON.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use unified_entity_card::{SCHEMA_VERSION, SCHEMA_VERSION_V2, UecKind, assert_uec, downgrade_uec};

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
    pub disabled_lorebook_ids: Vec<lettuce_types::LorebookId>,
    pub make_default: bool,
}

pub trait PersonaFileRepository: Send + Sync {
    /// Writes the whole import in one transaction, so a failure leaves nothing
    /// behind.
    fn import_persona_file(
        &self,
        import: &PersonaFileImport,
    ) -> Result<lettuce_characters::Persona, lettuce_characters::RepositoryError>;

    /// Deletes an asset stored for an import that did not happen, unless
    /// something references it, and answers its blob when the asset went.
    fn discard_unlinked_asset(
        &self,
        asset_id: lettuce_types::AssetId,
    ) -> Result<Option<lettuce_types::MediaBlobId>, lettuce_characters::RepositoryError>;

    /// Marks a blob no asset uses as missing so its bytes can be deleted;
    /// `false` when the blob is still in use.
    fn release_unused_blob(
        &self,
        blob_id: lettuce_types::MediaBlobId,
        now: lettuce_types::TimestampMillis,
    ) -> Result<bool, lettuce_characters::RepositoryError>;
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

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
    fn v2_reader_keeps_asset_locators() {
        let value = json!({
            "schema": { "name": "UEC", "version": SCHEMA_VERSION_V2 },
            "kind": "character",
            "payload": {
                "id": "char-1",
                "name": "Aster Vale",
                "scene": { "id": "scene-1", "content": "Hello there", "selectedVariant": 0, "variants": [] },
                "avatar": { "type": "inline_base64", "mimeType": "image/webp", "data": "QUJD" },
                "chatBackground": { "type": "remote_url", "url": "https://example.com/bg.png" }
            }
        });
        let read = parse_uec_character(&value, 3).expect("v2 character");
        assert_eq!(
            read.avatar_data.as_deref(),
            Some("data:image/webp;base64,QUJD")
        );
        assert_eq!(
            read.background_image_data.as_deref(),
            Some("https://example.com/bg.png")
        );
        assert_eq!(read.character.scenes.len(), 1);
        assert_eq!(read.character.scenes[0].content, "Hello there");
    }

    #[test]
    fn v2_reader_keeps_the_selected_variant_and_flattened_scenes() {
        let value = json!({
            "schema": { "name": "UEC", "version": SCHEMA_VERSION_V2 },
            "kind": "character",
            "payload": {
                "id": "char-1",
                "name": "Aster Vale",
                "scene": {
                    "id": "scene-1",
                    "content": "Hello there",
                    "selectedVariant": "variant-2",
                    "variants": [
                        { "id": "variant-1", "content": "Variant one", "createdAt": 10 },
                        { "id": "variant-2", "content": "Variant two", "direction": "Second", "createdAt": 20 },
                        { "id": "scene-2", "content": "Second scene", "direction": "alt", "createdAt": 20 }
                    ]
                }
            }
        });
        let read = parse_uec_character(&value, 3).expect("v2 character");
        let ids = read
            .character
            .scenes
            .iter()
            .map(|scene| scene.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids, ["scene-1", "variant-1", "variant-2", "scene-2"]);
        assert_eq!(
            read.character.default_scene_id.as_deref(),
            Some("variant-2")
        );
        assert_eq!(
            read.character.scenes[2].direction.as_deref(),
            Some("Second")
        );
        assert_eq!(read.character.scenes[3].direction.as_deref(), Some("alt"));
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
    fn a_captured_uec2_character_still_reads() {
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
        let uec: Value =
            serde_json::from_str(include_str!("../../tests/fixtures/uec2-character.json"))
                .expect("fixture");
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
    fn a_captured_uec2_persona_still_reads_with_its_nickname() {
        let uec: Value =
            serde_json::from_str(include_str!("../../tests/fixtures/uec2-persona.json"))
                .expect("fixture");
        let read = parse_persona_import(&uec, 3).expect("read back");
        assert_eq!(read.persona.title, "Traveller");
        assert_eq!(read.persona.nickname.as_deref(), Some("T"));
        assert_eq!(read.persona.is_default, Some(true));
        assert_eq!(read.persona.active_lorebook_ids, vec!["book".to_owned()]);
        assert!(matches!(
            parse_persona_import(
                &json!({"version": 2, "persona": {"title": "x", "description": "", "nickname": null, "isDefault": null, "avatarCrop": null}, "avatarData": null}),
                0
            ),
            Err(EntityPackageError::UnsupportedVersion(2))
        ));
    }
}
