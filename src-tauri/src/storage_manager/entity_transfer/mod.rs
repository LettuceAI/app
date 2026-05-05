use base64::{engine::general_purpose, Engine as _};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value as JsonValue};
use std::fs;
use std::io::Read;
use unified_entity_card::{
    assert_uec, convert_uec_v1_to_v2, create_character_uec, create_persona_uec, downgrade_uec, Uec,
    UecKind, SCHEMA_VERSION, SCHEMA_VERSION_V2,
};

#[cfg(not(target_os = "android"))]
use tauri::Manager;

use super::db::{now_ms, open_db};
use super::legacy::storage_root;
use super::lorebook::{get_lorebook, get_lorebook_entries, Lorebook, LorebookEntry};
use super::media::storage_write_image_bytes;
use crate::storage_manager::internal_read_settings;
use crate::utils::log_info;

mod engine;

pub use engine::{CharacterFileFormat, CharacterFormatInfo};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterExportPackage {
    pub version: u32,
    #[serde(default)]
    pub exported_at: i64,
    pub character: CharacterExportData,
    pub avatar_data: Option<String>,           // base64 data URL
    pub background_image_data: Option<String>, // base64 data URL
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AvatarCrop {
    pub x: f64,
    pub y: f64,
    pub scale: f64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterExportData {
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
    pub creator_notes_multilingual: Option<JsonValue>,
    #[serde(default)]
    pub source: Option<Vec<String>>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub character_book: Option<JsonValue>,
    pub rules: Vec<String>,
    pub scenes: Vec<SceneExport>,
    pub default_scene_id: Option<String>,
    pub default_model_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub companion: Option<JsonValue>,
    #[serde(default)]
    pub memory_type: Option<String>,
    #[serde(default)]
    pub active_lorebook_ids: Vec<String>,
    #[serde(default)]
    pub lorebooks: Vec<LorebookExportData>,
    pub prompt_template_id: Option<String>,
    pub system_prompt: Option<String>,
    pub voice_config: Option<JsonValue>,
    pub voice_autoplay: Option<bool>,
    pub disable_avatar_gradient: bool,
    pub avatar_crop: Option<AvatarCrop>,
    pub custom_gradient_enabled: Option<bool>,
    pub custom_gradient_colors: Option<Vec<String>>,
    pub custom_text_color: Option<String>,
    pub custom_text_secondary: Option<String>,
    #[serde(default)]
    pub chat_templates: Vec<ChatTemplateExport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_chat_template_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LorebookExportData {
    pub lorebook: Lorebook,
    #[serde(default)]
    pub entries: Vec<LorebookEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneExport {
    pub id: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background_image_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<i64>,
    pub selected_variant_id: Option<String>,
    pub variants: Vec<SceneVariantExport>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneVariantExport {
    pub id: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatTemplateExport {
    pub id: String,
    pub name: String,
    pub messages: Vec<ChatTemplateMessageExport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatTemplateMessageExport {
    pub id: String,
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonaExportPackage {
    pub version: u32,
    #[serde(default)]
    pub exported_at: i64,
    pub persona: PersonaExportData,
    pub avatar_data: Option<String>, // base64 data URL
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonaExportData {
    pub title: String,
    pub description: String,
    pub nickname: Option<String>,
    pub is_default: Option<bool>,
    pub avatar_crop: Option<AvatarCrop>,
    #[serde(default)]
    pub active_lorebook_ids: Vec<String>,
}

fn number_to_i64(value: &JsonValue) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().map(|v| v as i64))
        .or_else(|| value.as_f64().map(|v| v as i64))
}

fn parse_system_prompt_fields(
    payload: &JsonMap<String, JsonValue>,
) -> (Option<String>, Option<String>) {
    match payload.get("systemPrompt").and_then(|value| value.as_str()) {
        Some(value) if value.starts_with("_ID:") => {
            (Some(value.trim_start_matches("_ID:").to_string()), None)
        }
        Some(value) => (None, Some(value.to_string())),
        None => (None, None),
    }
}

fn parse_avatar_crop(value: Option<&JsonValue>) -> Option<AvatarCrop> {
    value.and_then(|crop| crop.as_object()).and_then(|crop| {
        Some(AvatarCrop {
            x: crop.get("x")?.as_f64()?,
            y: crop.get("y")?.as_f64()?,
            scale: crop.get("scale")?.as_f64()?,
        })
    })
}

fn asset_string_to_v2_locator(value: &str) -> JsonValue {
    if let Some(rest) = value.strip_prefix("data:") {
        let (mime_type, data) = rest.split_once(";base64,").unwrap_or(("", rest));
        let mut locator = JsonMap::new();
        locator.insert(
            "type".into(),
            JsonValue::String("inline_base64".to_string()),
        );
        if !mime_type.is_empty() {
            locator.insert("mimeType".into(), JsonValue::String(mime_type.to_string()));
        }
        locator.insert("data".into(), JsonValue::String(data.to_string()));
        return JsonValue::Object(locator);
    }

    if value.starts_with("http://") || value.starts_with("https://") {
        let mut locator = JsonMap::new();
        locator.insert("type".into(), JsonValue::String("remote_url".to_string()));
        locator.insert("url".into(), JsonValue::String(value.to_string()));
        return JsonValue::Object(locator);
    }

    JsonValue::String(value.to_string())
}

fn asset_locator_to_string(value: Option<&JsonValue>) -> Option<String> {
    let value = value?;
    match value {
        JsonValue::String(content) => Some(content.clone()),
        JsonValue::Object(map) => match map.get("type").and_then(|item| item.as_str()) {
            Some("inline_base64") => {
                let data = map.get("data").and_then(|item| item.as_str())?;
                let mime_type = map
                    .get("mimeType")
                    .and_then(|item| item.as_str())
                    .unwrap_or("application/octet-stream");
                Some(format!("data:{};base64,{}", mime_type, data))
            }
            Some("remote_url") => map
                .get("url")
                .and_then(|item| item.as_str())
                .map(|url| url.to_string()),
            Some("asset_ref") => None,
            _ => None,
        },
        _ => None,
    }
}

fn normalize_v2_asset_fields(card: &mut JsonValue) {
    let Some(payload) = card
        .get_mut("payload")
        .and_then(|payload| payload.as_object_mut())
    else {
        return;
    };

    for key in ["avatar", "chatBackground"] {
        let Some(current) = payload.get(key).cloned() else {
            continue;
        };
        if let JsonValue::String(text) = current {
            payload.insert(key.to_string(), asset_string_to_v2_locator(&text));
        }
    }
}

fn normalize_legacy_asset_fields(card: &mut JsonValue) {
    let Some(payload) = card
        .get_mut("payload")
        .and_then(|payload| payload.as_object_mut())
    else {
        return;
    };

    for key in ["avatar", "chatBackground"] {
        let Some(current) = payload.get(key).cloned() else {
            continue;
        };
        if let Some(text) = asset_locator_to_string(Some(&current)) {
            payload.insert(key.to_string(), JsonValue::String(text));
        }
    }
}

fn resolve_v1_scene_for_v2(card: &JsonValue) -> Option<JsonValue> {
    let payload = card.get("payload")?.as_object()?;
    let scenes = payload.get("scenes")?.as_array()?;
    if scenes.is_empty() {
        return None;
    }

    let default_scene_id = payload.get("defaultSceneId").and_then(JsonValue::as_str);
    let picked = default_scene_id
        .and_then(|id| {
            scenes.iter().find(|scene| {
                scene
                    .get("id")
                    .and_then(JsonValue::as_str)
                    .is_some_and(|scene_id| scene_id == id)
            })
        })
        .or_else(|| scenes.first())?;

    let selected_scene_id = picked.get("id").and_then(JsonValue::as_str)?.to_string();
    let mut scene = picked.as_object()?.clone();
    let selected_variant_id = scene
        .remove("selectedVariantId")
        .and_then(|value| value.as_str().map(|id| id.to_string()));

    let mut merged_variants = scene
        .get("variants")
        .and_then(JsonValue::as_array)
        .cloned()
        .unwrap_or_default();

    for alt_scene in scenes.iter().filter(|scene| {
        scene
            .get("id")
            .and_then(JsonValue::as_str)
            .is_some_and(|scene_id| scene_id != selected_scene_id)
    }) {
        let Some(alt_map) = alt_scene.as_object() else {
            continue;
        };

        let mut scene_variant = JsonMap::new();
        if let Some(id) = alt_map.get("id").cloned() {
            scene_variant.insert("id".to_string(), id);
        }
        if let Some(content) = alt_map.get("content").cloned() {
            scene_variant.insert("content".to_string(), content);
        }
        if let Some(direction) = alt_map.get("direction").cloned() {
            scene_variant.insert("direction".to_string(), direction);
        }
        if let Some(created_at) = alt_map
            .get("createdAt")
            .or_else(|| alt_map.get("created_at"))
            .cloned()
        {
            scene_variant.insert("createdAt".to_string(), created_at);
        }
        if scene_variant.contains_key("id") && scene_variant.contains_key("content") {
            merged_variants.push(JsonValue::Object(scene_variant));
        }

        if let Some(extra_variants) = alt_map.get("variants").and_then(JsonValue::as_array) {
            merged_variants.extend(extra_variants.iter().cloned());
        }
    }

    if !merged_variants.is_empty() {
        scene.insert(
            "variants".to_string(),
            JsonValue::Array(merged_variants.clone()),
        );
    }

    let selected_variant = selected_variant_id
        .filter(|selected_id| {
            merged_variants.iter().any(|variant| {
                variant
                    .get("id")
                    .and_then(JsonValue::as_str)
                    .is_some_and(|variant_id| variant_id == selected_id)
            })
        })
        .map(JsonValue::String)
        .unwrap_or_else(|| JsonValue::from(0));

    scene.insert("selectedVariant".to_string(), selected_variant);

    Some(JsonValue::Object(scene))
}

fn extract_v2_scene_variants_as_scenes(
    value: &JsonValue,
) -> Option<(Vec<SceneExport>, Option<String>)> {
    let schema_version = value
        .get("schema")
        .and_then(|schema| schema.get("version"))
        .and_then(JsonValue::as_str);
    if schema_version != Some(SCHEMA_VERSION_V2) {
        return None;
    }

    let scene = value.get("payload")?.get("scene")?.as_object()?;
    let base_id = scene.get("id")?.as_str()?.to_string();
    let base_content = scene.get("content")?.as_str()?.to_string();
    let base_direction = scene
        .get("direction")
        .and_then(JsonValue::as_str)
        .map(|value| value.to_string());
    let base_created_at = scene.get("createdAt").and_then(number_to_i64);

    let mut scenes = vec![SceneExport {
        id: base_id.clone(),
        content: base_content,
        direction: base_direction,
        background_image_path: None,
        created_at: base_created_at,
        selected_variant_id: None,
        variants: Vec::new(),
    }];

    if let Some(variants) = scene.get("variants").and_then(JsonValue::as_array) {
        for variant in variants {
            let Some(variant_map) = variant.as_object() else {
                continue;
            };
            let Some(id) = variant_map.get("id").and_then(JsonValue::as_str) else {
                continue;
            };
            let Some(content) = variant_map.get("content").and_then(JsonValue::as_str) else {
                continue;
            };

            scenes.push(SceneExport {
                id: id.to_string(),
                content: content.to_string(),
                direction: variant_map
                    .get("direction")
                    .and_then(JsonValue::as_str)
                    .map(|value| value.to_string()),
                background_image_path: None,
                created_at: variant_map.get("createdAt").and_then(number_to_i64),
                selected_variant_id: None,
                variants: Vec::new(),
            });
        }
    }

    let default_scene_id = match scene.get("selectedVariant") {
        Some(JsonValue::String(selected_id)) => Some(selected_id.clone()),
        _ => Some(base_id),
    };

    Some((scenes, default_scene_id))
}

fn normalize_uec_for_read(value: &JsonValue, strict: bool) -> Result<Uec, String> {
    let uec = assert_uec(value, strict)?;
    if uec.schema.version == SCHEMA_VERSION_V2 {
        let mut downgraded = downgrade_uec(value, SCHEMA_VERSION, false).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to downgrade UEC v2 for legacy parser: {}", e),
            )
        })?;
        normalize_legacy_asset_fields(&mut downgraded.card);
        return assert_uec(&downgraded.card, strict).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Invalid downgraded UEC payload: {}", e),
            )
        });
    }

    Ok(uec)
}

fn stringify_v2_uec(card: &JsonValue) -> Result<String, String> {
    let resolved_scene = resolve_v1_scene_for_v2(card);
    let mut upgraded = convert_uec_v1_to_v2(card).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to upgrade UEC v1 payload to v2: {}", e),
        )
    })?;

    if let Some(scene) = resolved_scene {
        if let Some(payload) = upgraded
            .get_mut("payload")
            .and_then(JsonValue::as_object_mut)
        {
            payload.insert("scene".to_string(), scene);
        }
    }

    normalize_v2_asset_fields(&mut upgraded);

    serde_json::to_string_pretty(&upgraded).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to serialize export: {}", e),
        )
    })
}

fn parse_uec_character(value: &JsonValue) -> Result<CharacterExportPackage, String> {
    let uec = normalize_uec_for_read(value, false)?;
    if uec.kind != UecKind::Character {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "Invalid import: This is not a character UEC",
        ));
    }

    let payload = uec
        .payload
        .as_object()
        .ok_or_else(|| "Invalid UEC payload: expected object".to_string())?;

    let name = payload
        .get("name")
        .and_then(|value| value.as_str())
        .ok_or_else(|| "Invalid UEC payload: missing name".to_string())?
        .to_string();
    let description = payload
        .get("description")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string());
    let definition = payload
        .get("definitions")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
        .or_else(|| description.clone());
    let scenario = payload
        .get("scenario")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string());
    let nickname = payload
        .get("nickname")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string());
    let creator = payload
        .get("creator")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string());
    let creator_notes = payload
        .get("creatorNotes")
        .or_else(|| payload.get("creator_notes"))
        .and_then(|value| value.as_str())
        .map(|value| value.to_string());
    let creator_notes_multilingual = payload
        .get("creatorNotesMultilingual")
        .or_else(|| payload.get("creator_notes_multilingual"))
        .cloned();
    let source = payload
        .get("source")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(|s| s.to_string()))
                .collect::<Vec<String>>()
        });
    let tags = payload
        .get("tags")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(|s| s.to_string()))
                .collect::<Vec<String>>()
        });
    let character_book = payload
        .get("characterBook")
        .or_else(|| payload.get("character_book"))
        .cloned();

    let rules = payload
        .get("rules")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(|s| s.to_string()))
                .collect::<Vec<String>>()
        })
        .unwrap_or_default();

    let mut scenes = payload
        .get("scenes")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|scene| {
                    let map = scene.as_object()?;
                    let id = map.get("id")?.as_str()?.to_string();
                    let content = map.get("content")?.as_str()?.to_string();
                    let direction = map
                        .get("direction")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    let created_at = map.get("createdAt").and_then(number_to_i64);
                    let selected_variant_id = map
                        .get("selectedVariantId")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    let variants = map
                        .get("variants")
                        .and_then(|v| v.as_array())
                        .map(|variant_items| {
                            variant_items
                                .iter()
                                .filter_map(|variant| {
                                    let vmap = variant.as_object()?;
                                    let vid = vmap.get("id")?.as_str()?.to_string();
                                    let vcontent = vmap.get("content")?.as_str()?.to_string();
                                    let vdirection = vmap
                                        .get("direction")
                                        .and_then(|v| v.as_str())
                                        .map(|s| s.to_string());
                                    let vcreated = vmap.get("createdAt").and_then(number_to_i64);
                                    Some(SceneVariantExport {
                                        id: vid,
                                        content: vcontent,
                                        direction: vdirection,
                                        created_at: vcreated,
                                    })
                                })
                                .collect::<Vec<SceneVariantExport>>()
                        })
                        .unwrap_or_default();

                    Some(SceneExport {
                        id,
                        content,
                        direction,
                        background_image_path: map
                            .get("backgroundImagePath")
                            .and_then(|value| value.as_str())
                            .map(|value| value.to_string()),
                        created_at,
                        selected_variant_id,
                        variants,
                    })
                })
                .collect::<Vec<SceneExport>>()
        })
        .unwrap_or_default();

    let mut default_scene_id = payload
        .get("defaultSceneId")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string());
    let default_model_id = payload
        .get("defaultModelId")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string());

    let (prompt_template_id, system_prompt) = parse_system_prompt_fields(payload);

    let voice_config = payload.get("voiceConfig").cloned();
    let voice_autoplay = payload.get("voiceAutoplay").and_then(|v| v.as_bool());

    let app_specific = uec
        .app_specific_settings
        .as_ref()
        .and_then(|value| value.as_object());

    let memory_type = app_specific
        .and_then(|map| map.get("memoryType").and_then(|v| v.as_str()))
        .map(|value| value.to_string());
    let mode = app_specific
        .and_then(|map| map.get("mode").and_then(|v| v.as_str()))
        .or_else(|| payload.get("mode").and_then(|v| v.as_str()))
        .map(|value| value.to_string());
    let companion = app_specific
        .and_then(|map| map.get("companion"))
        .or_else(|| payload.get("companion"))
        .cloned();
    let active_lorebook_ids = app_specific
        .and_then(|map| map.get("activeLorebookIds"))
        .or_else(|| payload.get("activeLorebookIds"))
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(|id| id.to_string()))
                .collect::<Vec<String>>()
        })
        .unwrap_or_default();
    let lorebooks = app_specific
        .and_then(|map| map.get("lorebooks"))
        .or_else(|| payload.get("lorebooks"))
        .and_then(|value| serde_json::from_value::<Vec<LorebookExportData>>(value.clone()).ok())
        .unwrap_or_default();
    let disable_avatar_gradient = app_specific
        .and_then(|map| map.get("disableAvatarGradient").and_then(|v| v.as_bool()))
        .unwrap_or(false);
    let custom_gradient_enabled =
        app_specific.and_then(|map| map.get("customGradientEnabled").and_then(|v| v.as_bool()));
    let custom_gradient_colors = app_specific
        .and_then(|map| map.get("customGradientColors").and_then(|v| v.as_array()))
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(|s| s.to_string()))
                .collect::<Vec<String>>()
        });
    let custom_text_color = app_specific
        .and_then(|map| map.get("customTextColor").and_then(|v| v.as_str()))
        .map(|value| value.to_string());
    let custom_text_secondary = app_specific
        .and_then(|map| map.get("customTextSecondary").and_then(|v| v.as_str()))
        .map(|value| value.to_string());
    let avatar_crop = parse_avatar_crop(app_specific.and_then(|map| map.get("avatarCrop")));
    let chat_templates: Vec<ChatTemplateExport> = app_specific
        .and_then(|map| map.get("chatTemplates"))
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    let default_chat_template_id = app_specific
        .and_then(|map| map.get("defaultChatTemplateId").and_then(|v| v.as_str()))
        .map(|value| value.to_string());

    let avatar_data = asset_locator_to_string(payload.get("avatar"));
    let background_image_data = asset_locator_to_string(payload.get("chatBackground"));

    if let Some((v2_scenes, v2_default_scene_id)) = extract_v2_scene_variants_as_scenes(value) {
        scenes = v2_scenes;
        default_scene_id = v2_default_scene_id;
    }

    Ok(CharacterExportPackage {
        version: 1,
        exported_at: now_ms() as i64,
        character: CharacterExportData {
            name,
            description,
            definition,
            scenario,
            nickname,
            creator,
            creator_notes,
            creator_notes_multilingual,
            source,
            tags,
            character_book,
            rules,
            scenes,
            default_scene_id,
            default_model_id,
            mode,
            companion,
            memory_type,
            active_lorebook_ids,
            lorebooks,
            prompt_template_id,
            system_prompt,
            voice_config,
            voice_autoplay,
            disable_avatar_gradient,
            avatar_crop,
            custom_gradient_enabled,
            custom_gradient_colors,
            custom_text_color,
            custom_text_secondary,
            chat_templates,
            default_chat_template_id,
        },
        avatar_data,
        background_image_data,
    })
}

fn parse_uec_persona(value: &JsonValue) -> Result<PersonaExportPackage, String> {
    let uec = normalize_uec_for_read(value, false)?;
    if uec.kind != UecKind::Persona {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "Invalid import: This is not a persona UEC",
        ));
    }

    let payload = uec
        .payload
        .as_object()
        .ok_or_else(|| "Invalid UEC payload: expected object".to_string())?;

    let title = payload
        .get("title")
        .and_then(|value| value.as_str())
        .ok_or_else(|| "Invalid UEC payload: missing title".to_string())?
        .to_string();
    let description = payload
        .get("description")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    let nickname = payload
        .get("nickname")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let is_default = payload.get("isDefault").and_then(|v| v.as_bool());
    let avatar_data = asset_locator_to_string(payload.get("avatar"));
    let avatar_crop = parse_avatar_crop(
        uec.app_specific_settings
            .as_ref()
            .and_then(|v| v.get("avatarCrop")),
    );
    let active_lorebook_ids = uec
        .app_specific_settings
        .as_ref()
        .and_then(|value| value.get("activeLorebookIds"))
        .or_else(|| payload.get("activeLorebookIds"))
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(|id| id.to_string()))
                .collect::<Vec<String>>()
        })
        .unwrap_or_default();

    Ok(PersonaExportPackage {
        version: 1,
        exported_at: now_ms() as i64,
        persona: PersonaExportData {
            title,
            description,
            nickname,
            is_default,
            avatar_crop,
            active_lorebook_ids,
        },
        avatar_data,
    })
}

fn looks_like_uec(value: &JsonValue) -> bool {
    value
        .get("schema")
        .and_then(|schema| schema.get("name"))
        .and_then(|name| name.as_str())
        == Some("UEC")
        || value.get("kind").and_then(|kind| kind.as_str()).is_some()
}

fn decode_base64_json_candidate(candidate: &str) -> Option<String> {
    let engines = [
        &general_purpose::STANDARD,
        &general_purpose::STANDARD_NO_PAD,
        &general_purpose::URL_SAFE,
        &general_purpose::URL_SAFE_NO_PAD,
    ];

    for engine in engines {
        let decoded = match engine.decode(candidate) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let text = match String::from_utf8(decoded) {
            Ok(value) => value,
            Err(_) => continue,
        };
        if serde_json::from_str::<JsonValue>(&text).is_ok() {
            return Some(text);
        }
    }

    None
}

fn try_parse_character_json(candidate: &str) -> Option<String> {
    let trimmed = candidate.trim();
    if trimmed.is_empty() {
        return None;
    }

    if serde_json::from_str::<JsonValue>(trimmed).is_ok() {
        return Some(trimmed.to_string());
    }

    decode_base64_json_candidate(trimmed)
}

fn decode_png_text_chunk(chunk_type: &str, chunk: &[u8]) -> Option<(String, String)> {
    match chunk_type {
        "tEXt" => {
            let separator_index = chunk.iter().position(|byte| *byte == 0)?;
            if separator_index == 0 {
                return None;
            }

            let keyword = String::from_utf8(chunk[..separator_index].to_vec()).ok()?;
            let text = String::from_utf8(chunk[separator_index + 1..].to_vec()).ok()?;
            Some((keyword, text))
        }
        "zTXt" => {
            let separator_index = chunk.iter().position(|byte| *byte == 0)?;
            if separator_index == 0 || separator_index + 2 > chunk.len() {
                return None;
            }

            let keyword = String::from_utf8(chunk[..separator_index].to_vec()).ok()?;
            let compression_method = chunk[separator_index + 1];
            if compression_method != 0 {
                return None;
            }

            let mut decoder = flate2::read::ZlibDecoder::new(&chunk[separator_index + 2..]);
            let mut text = String::new();
            decoder.read_to_string(&mut text).ok()?;
            Some((keyword, text))
        }
        "iTXt" => {
            let mut cursor = 0usize;
            let next_null = |cursor: &mut usize| -> Option<usize> {
                let relative = chunk.get(*cursor..)?.iter().position(|byte| *byte == 0)?;
                let index = *cursor + relative;
                *cursor = index + 1;
                Some(index)
            };

            let keyword_end = next_null(&mut cursor)?;
            if keyword_end == 0 || cursor + 1 >= chunk.len() {
                return None;
            }

            let keyword = String::from_utf8(chunk[..keyword_end].to_vec()).ok()?;
            let compression_flag = chunk[cursor];
            let compression_method = chunk[cursor + 1];
            cursor += 2;

            next_null(&mut cursor)?;
            next_null(&mut cursor)?;

            let text_bytes = chunk.get(cursor..)?;
            if compression_flag == 1 {
                if compression_method != 0 {
                    return None;
                }

                let mut decoder = flate2::read::ZlibDecoder::new(text_bytes);
                let mut text = String::new();
                decoder.read_to_string(&mut text).ok()?;
                return Some((keyword, text));
            }

            let text = String::from_utf8(text_bytes.to_vec()).ok()?;
            Some((keyword, text))
        }
        _ => None,
    }
}

fn extract_character_json_from_png_bytes(data: &[u8]) -> Result<String, String> {
    const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if data.len() < PNG_SIGNATURE.len() || &data[..PNG_SIGNATURE.len()] != PNG_SIGNATURE {
        return Err("Invalid PNG file".to_string());
    }

    let mut candidates: Vec<(String, String)> = Vec::new();
    let mut offset = PNG_SIGNATURE.len();

    while offset + 12 <= data.len() {
        let length = u32::from_be_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]) as usize;
        offset += 4;

        if offset + 8 > data.len() || offset + length + 8 > data.len() {
            return Err("Corrupted PNG metadata".to_string());
        }

        let chunk_type = std::str::from_utf8(&data[offset..offset + 4])
            .map_err(|_| "Corrupted PNG metadata".to_string())?;
        offset += 4;

        let chunk = &data[offset..offset + length];
        offset += length;
        offset += 4; // Skip CRC.

        if chunk_type == "IEND" {
            break;
        }

        if matches!(chunk_type, "tEXt" | "zTXt" | "iTXt") {
            if let Some((keyword, text)) = decode_png_text_chunk(chunk_type, chunk) {
                candidates.push((keyword, text));
            }
        }
    }

    for preferred in ["ccv3", "chara", "ccv2"] {
        for (keyword, text) in &candidates {
            if keyword.eq_ignore_ascii_case(preferred) {
                if let Some(parsed) = try_parse_character_json(text) {
                    return Ok(parsed);
                }
            }
        }
    }

    for (_, text) in &candidates {
        if let Some(parsed) = try_parse_character_json(text) {
            return Ok(parsed);
        }
    }

    Err("PNG does not contain a supported character card payload".to_string())
}

fn decode_character_import_file_bytes(filename: &str, data: &[u8]) -> Result<String, String> {
    if filename.to_ascii_lowercase().ends_with(".png") {
        return extract_character_json_from_png_bytes(data);
    }

    String::from_utf8(data.to_vec()).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Invalid UTF-8 import file: {}", e),
        )
    })
}

fn detect_character_format(value: &JsonValue) -> Option<CharacterFileFormat> {
    if looks_like_uec(value) {
        return Some(CharacterFileFormat::Uec);
    }
    if let Some(format) = engine::guess_chara_card_format(value) {
        return Some(format);
    }
    if serde_json::from_value::<CharacterExportPackage>(value.clone()).is_ok() {
        return Some(CharacterFileFormat::LegacyJson);
    }
    None
}

fn parse_character_import_payload(
    raw_value: &JsonValue,
) -> Result<(CharacterExportPackage, CharacterFileFormat), String> {
    if looks_like_uec(raw_value) {
        return Ok((parse_uec_character(raw_value)?, CharacterFileFormat::Uec));
    }
    if engine::looks_like_chara_card_v3(raw_value) {
        return Ok((
            engine::parse_chara_card_v3(raw_value)?,
            CharacterFileFormat::CharaCardV3,
        ));
    }
    if engine::looks_like_chara_card_v2(raw_value) {
        return Ok((
            engine::parse_chara_card_v2(raw_value)?,
            CharacterFileFormat::CharaCardV2,
        ));
    }
    if engine::looks_like_chara_card_v1(raw_value) {
        return Ok((
            engine::parse_chara_card_v1(raw_value)?,
            CharacterFileFormat::CharaCardV1,
        ));
    }
    let legacy =
        serde_json::from_value::<CharacterExportPackage>(raw_value.clone()).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Invalid import data: {}", e),
            )
        })?;
    Ok((legacy, CharacterFileFormat::LegacyJson))
}

fn is_http_url(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://")
}

fn should_auto_download_character_card_avatars(app: &tauri::AppHandle) -> bool {
    if let Ok(Some(raw)) = internal_read_settings(app) {
        if let Ok(json) = serde_json::from_str::<JsonValue>(&raw) {
            if let Some(app_state) = json.get("appState").and_then(|v| v.as_object()) {
                if let Some(enabled) = app_state
                    .get("autoDownloadCharacterCardAvatars")
                    .and_then(|v| v.as_bool())
                {
                    return enabled;
                }
                // Backward-compat fallback for older in-flight setting key.
                if let Some(enabled) = app_state
                    .get("autoDownloadDiscoveryAvatars")
                    .and_then(|v| v.as_bool())
                {
                    return enabled;
                }
            }
        }
    }
    true
}

fn legacy_entity_id(raw_value: &JsonValue, key: &str) -> Option<String> {
    raw_value
        .get(key)
        .and_then(|value| value.as_object())
        .and_then(|map| {
            map.get("originalId")
                .or_else(|| map.get("id"))
                .and_then(|value| value.as_str())
        })
        .map(|value| value.to_string())
}

struct CharacterExportSnapshot {
    character_id: String,
    created_at: i64,
    updated_at: i64,
    package: CharacterExportPackage,
}

fn load_character_export_snapshot(
    app: &tauri::AppHandle,
    character_id: &str,
) -> Result<CharacterExportSnapshot, String> {
    let conn = open_db(app)?;

    let (
        name,
        avatar_path,
        bg_path,
        description,
        definition,
        nickname,
        scenario,
        creator_notes,
        creator,
        creator_notes_multilingual,
        source,
        tags,
        default_scene_id,
        default_model_id,
        mode,
        companion_raw,
        prompt_template_id,
        system_prompt,
        voice_config_raw,
        voice_autoplay_raw,
        memory_type,
        active_lorebook_ids_json,
        disable_avatar_gradient,
        custom_gradient_enabled,
        custom_gradient_colors,
        custom_text_color,
        custom_text_secondary,
        avatar_crop_x,
        avatar_crop_y,
        avatar_crop_scale,
        default_chat_template_id,
        created_at,
        updated_at,
    ): (
        String,         // name
        Option<String>, // avatar_path
        Option<String>, // background_image_path
        Option<String>, // description
        Option<String>, // definition
        Option<String>, // nickname
        Option<String>, // scenario
        Option<String>, // creator_notes
        Option<String>, // creator
        Option<String>, // creator_notes_multilingual
        Option<String>, // source
        Option<String>, // tags
        Option<String>, // default_scene_id
        Option<String>, // default_model_id
        Option<String>, // mode
        Option<String>, // companion
        Option<String>, // prompt_template_id
        Option<String>, // system_prompt
        Option<String>, // voice_config
        Option<i64>,    // voice_autoplay
        Option<String>, // memory_type
        Option<String>, // active_lorebook_ids
        i64,            // disable_avatar_gradient
        i64,            // custom_gradient_enabled
        Option<String>, // custom_gradient_colors
        Option<String>, // custom_text_color
        Option<String>, // custom_text_secondary
        Option<f64>,    // avatar_crop_x
        Option<f64>,    // avatar_crop_y
        Option<f64>,    // avatar_crop_scale
        Option<String>, // default_chat_template_id
        i64,            // created_at
        i64,            // updated_at
    ) = conn
        .query_row(
            "SELECT name, avatar_path, background_image_path, description, definition, nickname, scenario, creator_notes, creator, creator_notes_multilingual, source, tags, default_scene_id, default_model_id, COALESCE(mode, 'roleplay'), companion, prompt_template_id, system_prompt, voice_config, voice_autoplay, memory_type, active_lorebook_ids, disable_avatar_gradient, custom_gradient_enabled, custom_gradient_colors, custom_text_color, custom_text_secondary, avatar_crop_x, avatar_crop_y, avatar_crop_scale, default_chat_template_id, created_at, updated_at FROM characters WHERE id = ?",
            params![character_id],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                    r.get(8)?,
                    r.get(9)?,
                    r.get(10)?,
                    r.get(11)?,
                    r.get(12)?,
                    r.get(13)?,
                    r.get(14)?,
                    r.get(15)?,
                    r.get(16)?,
                    r.get(17)?,
                    r.get(18)?,
                    r.get(19)?,
                    r.get(20)?,
                    r.get(21)?,
                    r.get::<_, i64>(22)?,
                    r.get::<_, i64>(23)?,
                    r.get(24)?,
                    r.get(25)?,
                    r.get(26)?,
                    r.get(27)?,
                    r.get(28)?,
                    r.get(29)?,
                    r.get(30)?,
                    r.get(31)?,
                    r.get(32)?,
                ))
            },
        )
        .map_err(|e| crate::utils::err_msg(module_path!(), line!(), format!("Character not found: {}", e)))?;

    let mut rules: Vec<String> = Vec::new();
    let mut stmt = conn
        .prepare("SELECT rule FROM character_rules WHERE character_id = ? ORDER BY idx ASC")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rule_rows = stmt
        .query_map(params![character_id], |r| r.get::<_, String>(0))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    for rule in rule_rows {
        rules.push(rule.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?);
    }

    let mut scenes: Vec<SceneExport> = Vec::new();
    let mut scenes_stmt = conn
        .prepare("SELECT id, content, direction, background_image_path, created_at, selected_variant_id FROM scenes WHERE character_id = ? ORDER BY created_at ASC")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let scene_rows = scenes_stmt
        .query_map(params![character_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, Option<String>>(5)?,
            ))
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    for row in scene_rows {
        let (
            scene_id,
            content,
            direction,
            background_image_path,
            scene_created_at,
            selected_variant_id,
        ) = row.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        let mut variants: Vec<SceneVariantExport> = Vec::new();
        let mut var_stmt = conn
            .prepare("SELECT id, content, direction, created_at FROM scene_variants WHERE scene_id = ? ORDER BY created_at ASC")
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let var_rows = var_stmt
            .query_map(params![&scene_id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, i64>(3)?,
                ))
            })
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        for v in var_rows {
            let (vid, vcontent, vdirection, vcreated) =
                v.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            variants.push(SceneVariantExport {
                id: vid,
                content: vcontent,
                direction: vdirection,
                created_at: Some(vcreated),
            });
        }

        scenes.push(SceneExport {
            id: scene_id,
            content,
            direction,
            background_image_path,
            created_at: Some(scene_created_at),
            selected_variant_id,
            variants,
        });
    }

    // Chat templates
    let mut chat_templates: Vec<ChatTemplateExport> = Vec::new();
    let mut tmpl_stmt = conn
        .prepare("SELECT id, name, created_at FROM chat_templates WHERE character_id = ? ORDER BY created_at ASC")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let tmpl_rows = tmpl_stmt
        .query_map(params![character_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    for row in tmpl_rows {
        let (tmpl_id, tmpl_name, tmpl_created_at) =
            row.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let mut msg_stmt = conn
            .prepare("SELECT id, role, content FROM chat_template_messages WHERE template_id = ? ORDER BY idx ASC")
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let msg_rows = msg_stmt
            .query_map(params![&tmpl_id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let mut messages: Vec<ChatTemplateMessageExport> = Vec::new();
        for msg in msg_rows {
            let (msg_id, role, content) =
                msg.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            messages.push(ChatTemplateMessageExport {
                id: msg_id,
                role,
                content,
            });
        }
        chat_templates.push(ChatTemplateExport {
            id: tmpl_id,
            name: tmpl_name,
            messages,
            created_at: Some(tmpl_created_at),
        });
    }

    let avatar_data = if let Some(ref avatar_filename) = avatar_path {
        read_avatar_as_base64(app, &format!("character-{}", character_id), avatar_filename).ok()
    } else {
        None
    };

    let background_image_data = if let Some(ref bg_id) = bg_path {
        read_background_image_as_base64(app, bg_id).ok()
    } else {
        None
    };

    let resolved_definition = definition.clone().or_else(|| description.clone());
    let memory_value = memory_type.unwrap_or_else(|| "manual".to_string());
    let voice_config = voice_config_raw
        .and_then(|vc| serde_json::from_str::<JsonValue>(&vc).ok())
        .filter(|v| !v.is_null());
    let voice_autoplay = voice_autoplay_raw.map(|v| v != 0);
    let companion = companion_raw
        .as_ref()
        .and_then(|value| serde_json::from_str::<JsonValue>(value).ok())
        .filter(|value| !value.is_null());
    let creator_notes_multilingual = creator_notes_multilingual
        .as_ref()
        .and_then(|value| serde_json::from_str::<JsonValue>(value).ok())
        .filter(|value| value.is_object());
    let source = source
        .as_ref()
        .and_then(|value| serde_json::from_str::<Vec<String>>(value).ok());
    let tags = tags
        .as_ref()
        .and_then(|value| serde_json::from_str::<Vec<String>>(value).ok());
    let active_lorebook_ids = active_lorebook_ids_json
        .as_ref()
        .and_then(|value| serde_json::from_str::<Vec<String>>(value).ok())
        .unwrap_or_default();
    let mut lorebooks = Vec::new();
    for lorebook_id in &active_lorebook_ids {
        if let Some(lorebook) = get_lorebook(&conn, lorebook_id)? {
            let entries = get_lorebook_entries(&conn, lorebook_id)?;
            lorebooks.push(LorebookExportData { lorebook, entries });
        }
    }

    let custom_gradient_colors = custom_gradient_colors
        .as_ref()
        .and_then(|colors_json| serde_json::from_str::<Vec<String>>(colors_json).ok());
    let avatar_crop = match (avatar_crop_x, avatar_crop_y, avatar_crop_scale) {
        (Some(x), Some(y), Some(scale)) => Some(AvatarCrop { x, y, scale }),
        _ => None,
    };

    let package = CharacterExportPackage {
        version: 1,
        exported_at: now_ms() as i64,
        character: CharacterExportData {
            name,
            description: description.clone(),
            definition: resolved_definition,
            scenario,
            nickname,
            creator,
            creator_notes,
            creator_notes_multilingual,
            source,
            tags,
            character_book: None,
            rules,
            scenes,
            default_scene_id,
            default_model_id,
            mode,
            companion,
            memory_type: Some(memory_value),
            active_lorebook_ids,
            lorebooks,
            prompt_template_id,
            system_prompt,
            voice_config,
            voice_autoplay,
            disable_avatar_gradient: disable_avatar_gradient != 0,
            avatar_crop,
            custom_gradient_enabled: Some(custom_gradient_enabled != 0),
            custom_gradient_colors,
            custom_text_color,
            custom_text_secondary,
            chat_templates,
            default_chat_template_id,
        },
        avatar_data,
        background_image_data,
    };

    Ok(CharacterExportSnapshot {
        character_id: character_id.to_string(),
        created_at,
        updated_at,
        package,
    })
}

fn build_uec_from_package(
    package: &CharacterExportPackage,
    character_id: &str,
    created_at: Option<i64>,
    updated_at: Option<i64>,
) -> Result<String, String> {
    let resolved_definition = package
        .character
        .definition
        .clone()
        .or_else(|| package.character.description.clone());
    let memory_value = package
        .character
        .memory_type
        .clone()
        .unwrap_or_else(|| "manual".to_string());

    let mut payload = JsonMap::new();
    payload.insert("id".into(), JsonValue::String(character_id.to_string()));
    payload.insert(
        "name".into(),
        JsonValue::String(package.character.name.clone()),
    );
    if let Some(desc) = package.character.description.clone() {
        payload.insert("description".into(), JsonValue::String(desc));
    }
    if let Some(def) = resolved_definition {
        payload.insert("definitions".into(), JsonValue::String(def));
    }
    if let Some(data) = package.avatar_data.clone() {
        payload.insert("avatar".into(), JsonValue::String(data));
    }
    if let Some(data) = package.background_image_data.clone() {
        payload.insert("chatBackground".into(), JsonValue::String(data));
    }
    payload.insert(
        "rules".into(),
        JsonValue::Array(
            package
                .character
                .rules
                .iter()
                .map(|rule| JsonValue::String(rule.clone()))
                .collect(),
        ),
    );
    let scenes = serde_json::to_value(&package.character.scenes)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    payload.insert("scenes".into(), scenes);
    if let Some(ds) = package.character.default_scene_id.clone() {
        payload.insert("defaultSceneId".into(), JsonValue::String(ds));
    }
    if let Some(dm) = package.character.default_model_id.clone() {
        payload.insert("defaultModelId".into(), JsonValue::String(dm));
    }
    if let Some(value) = package.character.scenario.clone() {
        payload.insert("scenario".into(), JsonValue::String(value));
    }
    if let Some(value) = package.character.nickname.clone() {
        payload.insert("nickname".into(), JsonValue::String(value));
    }
    if let Some(value) = package.character.creator.clone() {
        payload.insert("creator".into(), JsonValue::String(value));
    }
    if let Some(value) = package.character.creator_notes.clone() {
        payload.insert("creatorNotes".into(), JsonValue::String(value));
    }
    if let Some(value) = package.character.creator_notes_multilingual.clone() {
        payload.insert("creatorNotesMultilingual".into(), value);
    }
    if let Some(value) = package.character.source.clone() {
        payload.insert("source".into(), serde_json::json!(value));
    }
    if let Some(value) = package.character.tags.clone() {
        payload.insert("tags".into(), serde_json::json!(value));
    }
    if let Some(value) = package.character.character_book.clone() {
        payload.insert("characterBook".into(), value);
    }

    let mut system_prompt_is_id = false;
    if let Some(pt) = package.character.prompt_template_id.clone() {
        payload.insert("systemPrompt".into(), JsonValue::String(pt));
        system_prompt_is_id = true;
    } else if let Some(sp) = package.character.system_prompt.clone() {
        payload.insert("systemPrompt".into(), JsonValue::String(sp));
    }

    if let Some(vc) = package.character.voice_config.clone() {
        if !vc.is_null() {
            payload.insert("voiceConfig".into(), vc);
        }
    }

    payload.insert(
        "voiceAutoplay".into(),
        JsonValue::Bool(package.character.voice_autoplay.unwrap_or(false)),
    );

    let created_at = created_at.unwrap_or(package.exported_at);
    let updated_at = updated_at.unwrap_or(package.exported_at);

    payload.insert("createdAt".into(), JsonValue::from(created_at));
    payload.insert("updatedAt".into(), JsonValue::from(updated_at));

    let mut app_specific = JsonMap::new();
    app_specific.insert(
        "disableAvatarGradient".into(),
        JsonValue::Bool(package.character.disable_avatar_gradient),
    );
    if let Some(mode) = package.character.mode.clone() {
        app_specific.insert("mode".into(), JsonValue::String(mode));
    }
    if let Some(companion) = package.character.companion.clone() {
        app_specific.insert("companion".into(), companion);
    }
    app_specific.insert("memoryType".into(), JsonValue::String(memory_value));
    if !package.character.active_lorebook_ids.is_empty() {
        app_specific.insert(
            "activeLorebookIds".into(),
            serde_json::json!(package.character.active_lorebook_ids),
        );
    }
    if !package.character.lorebooks.is_empty() {
        app_specific.insert(
            "lorebooks".into(),
            serde_json::to_value(&package.character.lorebooks)
                .unwrap_or(JsonValue::Array(Vec::new())),
        );
    }
    app_specific.insert(
        "customGradientEnabled".into(),
        JsonValue::Bool(package.character.custom_gradient_enabled.unwrap_or(false)),
    );
    if let Some(colors) = package.character.custom_gradient_colors.clone() {
        app_specific.insert("customGradientColors".into(), serde_json::json!(colors));
    }
    if let Some(color) = package.character.custom_text_color.clone() {
        app_specific.insert("customTextColor".into(), JsonValue::String(color));
    }
    if let Some(color) = package.character.custom_text_secondary.clone() {
        app_specific.insert("customTextSecondary".into(), JsonValue::String(color));
    }
    if let Some(crop) = package.character.avatar_crop.clone() {
        app_specific.insert(
            "avatarCrop".into(),
            serde_json::json!({
                "x": crop.x,
                "y": crop.y,
                "scale": crop.scale,
            }),
        );
    }
    if !package.character.chat_templates.is_empty() {
        let chat_templates = serde_json::to_value(&package.character.chat_templates)
            .unwrap_or(JsonValue::Array(Vec::new()));
        app_specific.insert("chatTemplates".into(), chat_templates);
    }
    if let Some(dct) = package.character.default_chat_template_id.clone() {
        app_specific.insert("defaultChatTemplateId".into(), JsonValue::String(dct));
    }

    let mut meta = JsonMap::new();
    meta.insert("createdAt".into(), JsonValue::from(created_at));
    meta.insert("updatedAt".into(), JsonValue::from(updated_at));
    meta.insert("source".into(), JsonValue::String("lettuceai".to_string()));

    let export_card = create_character_uec(
        payload,
        system_prompt_is_id,
        None,
        Some(JsonValue::Object(app_specific)),
        Some(JsonValue::Object(meta)),
        Some(JsonValue::Object(JsonMap::new())),
    );

    stringify_v2_uec(&export_card)
}

fn build_uec_from_persona_package(
    package: &PersonaExportPackage,
    persona_id: &str,
    created_at: Option<i64>,
    updated_at: Option<i64>,
) -> Result<String, String> {
    let mut payload = JsonMap::new();
    payload.insert("id".into(), JsonValue::String(persona_id.to_string()));
    payload.insert(
        "title".into(),
        JsonValue::String(package.persona.title.clone()),
    );
    if !package.persona.description.is_empty() {
        payload.insert(
            "description".into(),
            JsonValue::String(package.persona.description.clone()),
        );
    }
    if let Some(data) = package.avatar_data.clone() {
        payload.insert("avatar".into(), JsonValue::String(data));
    }
    if let Some(is_default) = package.persona.is_default {
        payload.insert("isDefault".into(), JsonValue::Bool(is_default));
    }

    let created_at = created_at.unwrap_or(package.exported_at);
    let updated_at = updated_at.unwrap_or(package.exported_at);

    payload.insert("createdAt".into(), JsonValue::from(created_at));
    payload.insert("updatedAt".into(), JsonValue::from(updated_at));

    let mut meta = JsonMap::new();
    meta.insert("createdAt".into(), JsonValue::from(created_at));
    meta.insert("updatedAt".into(), JsonValue::from(updated_at));
    meta.insert("source".into(), JsonValue::String("lettuceai".to_string()));

    let mut app_specific = JsonMap::new();
    if let Some(crop) = package.persona.avatar_crop.clone() {
        app_specific.insert(
            "avatarCrop".into(),
            serde_json::json!({
                "x": crop.x,
                "y": crop.y,
                "scale": crop.scale,
            }),
        );
    }
    if !package.persona.active_lorebook_ids.is_empty() {
        app_specific.insert(
            "activeLorebookIds".into(),
            serde_json::json!(package.persona.active_lorebook_ids),
        );
    }

    let uec = create_persona_uec(
        payload,
        None,
        Some(JsonValue::Object(app_specific)),
        Some(JsonValue::Object(meta)),
        Some(JsonValue::Object(JsonMap::new())),
    );

    stringify_v2_uec(&uec)
}

#[tauri::command]
pub fn character_export(app: tauri::AppHandle, character_id: String) -> Result<String, String> {
    character_export_with_format(app, character_id, CharacterFileFormat::Uec)
}

#[tauri::command]
pub fn character_export_with_format(
    app: tauri::AppHandle,
    character_id: String,
    format: CharacterFileFormat,
) -> Result<String, String> {
    log_info(
        &app,
        "character_export",
        format!("Exporting character {} as {:?}", character_id, format),
    );

    let snapshot = load_character_export_snapshot(&app, &character_id)?;

    let json = match format {
        CharacterFileFormat::Uec => build_uec_from_package(
            &snapshot.package,
            &snapshot.character_id,
            Some(snapshot.created_at),
            Some(snapshot.updated_at),
        )?,
        CharacterFileFormat::CharaCardV3 => {
            let card = engine::export_chara_card_v3(
                &snapshot.package,
                Some(snapshot.created_at),
                Some(snapshot.updated_at),
            );
            serde_json::to_string_pretty(&card).map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to serialize export: {}", e),
                )
            })?
        }
        CharacterFileFormat::CharaCardV2 => {
            let card = engine::export_chara_card_v2(&snapshot.package);
            serde_json::to_string_pretty(&card).map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to serialize export: {}", e),
                )
            })?
        }
        CharacterFileFormat::CharaCardV1 => {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                "Character Card V1 export is read-only",
            ));
        }
        CharacterFileFormat::LegacyJson => {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                "Legacy JSON export is not supported",
            ));
        }
    };

    log_info(
        &app,
        "character_export",
        format!("Successfully exported character: {}", character_id),
    );

    Ok(json)
}

#[tauri::command]
pub fn character_list_formats() -> Result<Vec<CharacterFormatInfo>, String> {
    Ok(engine::all_character_formats())
}

#[tauri::command]
pub fn character_detect_format(import_json: String) -> Result<CharacterFormatInfo, String> {
    let raw_value: JsonValue = serde_json::from_str(&import_json).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Invalid import data: {}", e),
        )
    })?;
    let format = detect_character_format(&raw_value)
        .ok_or_else(|| "Unsupported character file format".to_string())?;
    Ok(engine::character_format_info(format))
}

fn import_lorebooks_for_character_package(
    tx: &rusqlite::Transaction<'_>,
    package: &CharacterExportPackage,
    now: i64,
) -> Result<Vec<String>, String> {
    let mut lorebook_id_map = std::collections::HashMap::new();

    for bundled in &package.character.lorebooks {
        let new_lorebook_id = uuid::Uuid::new_v4().to_string();
        lorebook_id_map.insert(bundled.lorebook.id.clone(), new_lorebook_id.clone());
        tx.execute(
            "INSERT INTO lorebooks (id, name, avatar_path, keyword_detection_mode, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                &new_lorebook_id,
                &bundled.lorebook.name,
                &bundled.lorebook.avatar_path,
                bundled.lorebook.keyword_detection_mode.as_db_value(),
                now,
                now,
            ],
        )
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to import bundled lorebook: {}", e),
            )
        })?;

        for entry in &bundled.entries {
            let keywords_json = serde_json::to_string(&entry.keywords).map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to serialize bundled lorebook entry keywords: {}", e),
                )
            })?;
            tx.execute(
                r#"
                INSERT INTO lorebook_entries (
                    id, lorebook_id, title, enabled, always_active, keywords,
                    case_sensitive, content, priority, display_order, created_at, updated_at
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
                "#,
                params![
                    uuid::Uuid::new_v4().to_string(),
                    &new_lorebook_id,
                    &entry.title,
                    entry.enabled as i64,
                    entry.always_active as i64,
                    keywords_json,
                    entry.case_sensitive as i64,
                    &entry.content,
                    entry.priority,
                    entry.display_order,
                    now,
                    now,
                ],
            )
            .map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to import bundled lorebook entry: {}", e),
                )
            })?;
        }
    }

    Ok(package
        .character
        .active_lorebook_ids
        .iter()
        .map(|id| {
            lorebook_id_map
                .get(id)
                .cloned()
                .unwrap_or_else(|| id.clone())
        })
        .collect())
}

/// Import a character from a JSON package
#[tauri::command]
pub fn character_import(app: tauri::AppHandle, import_json: String) -> Result<String, String> {
    log_info(&app, "character_import", "Starting character import");

    let raw_value: JsonValue = serde_json::from_str(&import_json).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Invalid import data: {}", e),
        )
    })?;
    let (package, _format) = parse_character_import_payload(&raw_value)?;

    // Validate version
    if package.version > 1 {
        return Err(format!(
            "Unsupported export version: {}. Please update your app.",
            package.version
        ));
    }

    let mut conn = open_db(&app)?;
    let tx = conn
        .transaction()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    // Generate new ID for imported character
    let new_character_id = uuid::Uuid::new_v4().to_string();
    let now = now_ms() as i64;

    log_info(
        &app,
        "character_import",
        format!("Importing as new character: {}", new_character_id),
    );

    let auto_download_character_card_avatars = should_auto_download_character_card_avatars(&app);

    // Save avatar if provided
    let avatar_path = if let Some(ref avatar_base64) = package.avatar_data {
        if is_http_url(avatar_base64) {
            if auto_download_character_card_avatars {
                match save_avatar_from_url(
                    &app,
                    &format!("character-{}", new_character_id),
                    avatar_base64,
                ) {
                    Ok(filename) => Some(filename),
                    Err(e) => {
                        log_info(
                            &app,
                            "character_import",
                            format!("Warning: Failed to import remote avatar URL: {}", e),
                        );
                        None
                    }
                }
            } else {
                log_info(
                    &app,
                    "character_import",
                    "Skipping remote avatar URL import because auto-download is disabled",
                );
                None
            }
        } else {
            match save_avatar_from_base64(
                &app,
                &format!("character-{}", new_character_id),
                avatar_base64,
            ) {
                Ok(filename) => Some(filename),
                Err(e) => {
                    log_info(
                        &app,
                        "character_import",
                        format!("Warning: Failed to import avatar: {}", e),
                    );
                    None
                }
            }
        }
    } else {
        None
    };

    // Save background image if provided
    let background_image_path = if let Some(ref bg_base64) = package.background_image_data {
        if is_http_url(bg_base64) {
            log_info(
                &app,
                "character_import",
                "Skipping remote background URL during import",
            );
            None
        } else {
            match save_background_image_from_base64(&app, bg_base64) {
                Ok(image_id) => Some(image_id),
                Err(e) => {
                    log_info(
                        &app,
                        "character_import",
                        format!("Warning: Failed to import background image: {}", e),
                    );
                    None
                }
            }
        }
    } else {
        None
    };

    let memory_type = match package.character.memory_type.as_deref() {
        Some("dynamic") => "dynamic".to_string(),
        _ => "manual".to_string(),
    };
    let custom_gradient_enabled = package.character.custom_gradient_enabled.unwrap_or(false) as i64;
    let custom_gradient_colors = package
        .character
        .custom_gradient_colors
        .as_ref()
        .and_then(|colors| serde_json::to_string(colors).ok());
    let custom_text_color = package.character.custom_text_color.clone();
    let custom_text_secondary = package.character.custom_text_secondary.clone();
    let creator_notes_multilingual = package
        .character
        .creator_notes_multilingual
        .as_ref()
        .and_then(|value| serde_json::to_string(value).ok());
    let source = Some("[\"lettuceai\"]".to_string());
    let tags = package
        .character
        .tags
        .as_ref()
        .and_then(|value| serde_json::to_string(value).ok());
    let (avatar_crop_x, avatar_crop_y, avatar_crop_scale) = package
        .character
        .avatar_crop
        .as_ref()
        .map(|crop| (Some(crop.x), Some(crop.y), Some(crop.scale)))
        .unwrap_or((None, None, None));

    let voice_config = package.character.voice_config.as_ref().and_then(|v| {
        if v.is_null() {
            None
        } else {
            serde_json::to_string(v).ok()
        }
    });
    let voice_autoplay = package.character.voice_autoplay.unwrap_or(false) as i64;
    let mode = package
        .character
        .mode
        .as_deref()
        .filter(|value| *value == "companion")
        .unwrap_or("roleplay");
    let companion = package
        .character
        .companion
        .as_ref()
        .and_then(|value| serde_json::to_string(value).ok());

    // Insert character
    tx.execute(
        r#"INSERT INTO characters (id, name, avatar_path, avatar_crop_x, avatar_crop_y, avatar_crop_scale, background_image_path, description, definition, nickname, scenario, creator_notes, creator, creator_notes_multilingual, source, tags, default_scene_id, default_model_id, mode, companion, prompt_template_id, system_prompt, voice_config, voice_autoplay, memory_type, disable_avatar_gradient, custom_gradient_enabled, custom_gradient_colors, custom_text_color, custom_text_secondary, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        params![
            &new_character_id,
            &package.character.name,
            avatar_path,
            avatar_crop_x,
            avatar_crop_y,
            avatar_crop_scale,
            background_image_path,
            package.character.description,
            package
                .character
                .definition
                .clone()
                .or(package.character.description.clone()),
            package.character.nickname,
            package.character.scenario,
            package.character.creator_notes,
            package.character.creator,
            creator_notes_multilingual,
            source,
            tags,
            package.character.default_model_id,
            mode,
            companion,
            package.character.prompt_template_id,
            package.character.system_prompt,
            voice_config,
            voice_autoplay,
            memory_type,
            package.character.disable_avatar_gradient as i64,
            custom_gradient_enabled,
            custom_gradient_colors,
            custom_text_color,
            custom_text_secondary,
            now,
            now
        ],
    )
    .map_err(|e| crate::utils::err_msg(module_path!(), line!(), format!("Failed to insert character: {}", e)))?;

    let active_lorebook_ids = import_lorebooks_for_character_package(&tx, &package, now)?;
    if !active_lorebook_ids.is_empty() {
        let active_lorebook_ids_json =
            serde_json::to_string(&active_lorebook_ids).map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to serialize imported character lorebook ids: {}", e),
                )
            })?;
        tx.execute(
            "UPDATE characters SET active_lorebook_ids = ?1 WHERE id = ?2",
            params![active_lorebook_ids_json, &new_character_id],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    // Insert rules
    for (idx, rule) in package.character.rules.iter().enumerate() {
        tx.execute(
            "INSERT INTO character_rules (character_id, idx, rule) VALUES (?, ?, ?)",
            params![&new_character_id, idx as i64, rule],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    }

    // Map old scene IDs to new ones
    let mut scene_id_map: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    let mut new_default_scene_id: Option<String> = None;

    // Insert scenes
    for (i, scene) in package.character.scenes.iter().enumerate() {
        let new_scene_id = uuid::Uuid::new_v4().to_string();
        scene_id_map.insert(scene.id.clone(), new_scene_id.clone());

        // Map old variant IDs to new ones
        let mut variant_id_map: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();

        // Insert scene variants first
        for variant in &scene.variants {
            let new_variant_id = uuid::Uuid::new_v4().to_string();
            variant_id_map.insert(variant.id.clone(), new_variant_id.clone());
            let variant_created_at = variant.created_at.unwrap_or(now);

            tx.execute(
                "INSERT INTO scene_variants (id, scene_id, content, direction, created_at) VALUES (?, ?, ?, ?, ?)",
                params![
                    new_variant_id,
                    &new_scene_id,
                    &variant.content,
                    &variant.direction,
                    variant_created_at
                ],
            )
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        }

        // Map selected variant ID
        let new_selected_variant_id = scene
            .selected_variant_id
            .as_ref()
            .and_then(|old_id| variant_id_map.get(old_id).cloned());

        let scene_created_at = scene.created_at.unwrap_or(now);
        tx.execute(
            "INSERT INTO scenes (id, character_id, content, direction, background_image_path, created_at, selected_variant_id) VALUES (?, ?, ?, ?, ?, ?, ?)",
            params![
                &new_scene_id,
                &new_character_id,
                &scene.content,
                &scene.direction,
                &scene.background_image_path,
                scene_created_at,
                new_selected_variant_id
            ],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        // Set first scene as default if no default was specified
        if i == 0
            && (package.character.default_scene_id.is_none() || new_default_scene_id.is_none())
        {
            new_default_scene_id = Some(new_scene_id.clone());
        }

        // Map the original default scene ID
        if let Some(ref old_default) = package.character.default_scene_id {
            if old_default == &scene.id {
                new_default_scene_id = Some(new_scene_id.clone());
            }
        }
    }

    // Update default scene
    tx.execute(
        "UPDATE characters SET default_scene_id = ? WHERE id = ?",
        params![new_default_scene_id, &new_character_id],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    // Import chat templates
    let mut tmpl_id_map: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    for template in &package.character.chat_templates {
        let new_tmpl_id = uuid::Uuid::new_v4().to_string();
        tmpl_id_map.insert(template.id.clone(), new_tmpl_id.clone());
        let tmpl_created = template.created_at.unwrap_or(now);
        tx.execute(
            "INSERT INTO chat_templates (id, character_id, name, created_at) VALUES (?, ?, ?, ?)",
            params![
                &new_tmpl_id,
                &new_character_id,
                &template.name,
                tmpl_created
            ],
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        for (idx, msg) in template.messages.iter().enumerate() {
            let new_msg_id = uuid::Uuid::new_v4().to_string();
            tx.execute(
                "INSERT INTO chat_template_messages (id, template_id, idx, role, content) VALUES (?, ?, ?, ?, ?)",
                params![&new_msg_id, &new_tmpl_id, idx as i64, &msg.role, &msg.content],
            )
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        }
    }
    let new_default_chat_template_id = package
        .character
        .default_chat_template_id
        .as_ref()
        .and_then(|old_id| tmpl_id_map.get(old_id).cloned());
    tx.execute(
        "UPDATE characters SET default_chat_template_id = ? WHERE id = ?",
        params![new_default_chat_template_id, &new_character_id],
    )
    .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    tx.commit()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    log_info(
        &app,
        "character_import",
        format!("Successfully imported character: {}", new_character_id),
    );

    let conn2 = open_db(&app)?;
    read_imported_character(&conn2, &new_character_id)
}

/// Preview a character import without saving it
#[tauri::command]
pub fn character_import_preview(import_json: String) -> Result<String, String> {
    let raw_value: JsonValue = serde_json::from_str(&import_json).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Invalid import data: {}", e),
        )
    })?;
    let (package, format) = parse_character_import_payload(&raw_value)?;

    build_character_import_preview(package, format)
}

fn build_character_import_preview(
    package: CharacterExportPackage,
    format: CharacterFileFormat,
) -> Result<String, String> {
    if package.version > 1 {
        return Err(format!(
            "Unsupported export version: {}. Please update your app.",
            package.version
        ));
    }

    let description = package.character.description.clone().unwrap_or_default();
    let definition = package
        .character
        .definition
        .clone()
        .or(package.character.description.clone())
        .unwrap_or_default();
    let memory_type = match package.character.memory_type.as_deref() {
        Some("dynamic") => "dynamic".to_string(),
        _ => "manual".to_string(),
    };

    let scenes = serde_json::to_value(&package.character.scenes)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let chat_templates = serde_json::to_value(&package.character.chat_templates)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let preview = serde_json::json!({
        "name": package.character.name,
        "description": description,
        "definition": definition,
        "scenario": package.character.scenario,
        "nickname": package.character.nickname,
        "creator": package.character.creator,
        "creatorNotes": package.character.creator_notes,
        "creatorNotesMultilingual": package.character.creator_notes_multilingual,
        "source": package.character.source,
        "tags": package.character.tags,
        "characterBook": package.character.character_book,
        "scenes": scenes,
        "chatTemplates": chat_templates,
        "defaultSceneId": package.character.default_scene_id,
        "defaultChatTemplateId": package.character.default_chat_template_id,
        "mode": package.character.mode,
        "companion": package.character.companion,
        "promptTemplateId": package.character.prompt_template_id,
        "activeLorebookIds": package.character.active_lorebook_ids,
        "lorebooks": package.character.lorebooks,
        "memoryType": memory_type,
        "disableAvatarGradient": package.character.disable_avatar_gradient,
        "fileFormat": format,
        "avatarData": package.avatar_data,
        "avatarCrop": package.character.avatar_crop,
        "backgroundImageData": package.background_image_data
    });

    serde_json::to_string(&preview)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

#[tauri::command]
pub fn character_import_preview_from_bytes(
    app: tauri::AppHandle,
    filename: String,
    data: Vec<u8>,
) -> Result<String, String> {
    let import_json = decode_character_import_file_bytes(&filename, &data)?;
    let raw_value: JsonValue = serde_json::from_str(&import_json).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Invalid import data: {}", e),
        )
    })?;
    let (mut package, format) = parse_character_import_payload(&raw_value)?;

    if filename.to_ascii_lowercase().ends_with(".png") {
        let image_id = format!("import-preview-{}", uuid::Uuid::new_v4());
        storage_write_image_bytes(&app, &image_id, &data)?;
        package.avatar_data = Some(image_id);
    }

    build_character_import_preview(package, format)
}

/// Convert a legacy export package to a UEC file (no import performed)
#[tauri::command]
pub fn convert_export_to_uec(import_json: String) -> Result<String, String> {
    let raw_value: JsonValue = serde_json::from_str(&import_json).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Invalid import data: {}", e),
        )
    })?;

    if looks_like_uec(&raw_value) {
        let uec = assert_uec(&raw_value, false).map_err(|e| {
            crate::utils::err_msg(module_path!(), line!(), format!("Invalid UEC: {}", e))
        })?;
        return if uec.schema.version == SCHEMA_VERSION_V2 {
            serde_json::to_string_pretty(&raw_value)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
        } else {
            stringify_v2_uec(&raw_value)
        };
    }

    if engine::guess_chara_card_format(&raw_value).is_some() {
        let (package, _) = parse_character_import_payload(&raw_value)?;
        let character_id = uuid::Uuid::new_v4().to_string();
        return build_uec_from_package(&package, &character_id, None, None);
    }

    if let Ok(package) = serde_json::from_value::<CharacterExportPackage>(raw_value.clone()) {
        let character_id = uuid::Uuid::new_v4().to_string();
        return build_uec_from_package(&package, &character_id, None, None);
    }

    if let Ok(package) = serde_json::from_value::<PersonaExportPackage>(raw_value.clone()) {
        let persona_id = uuid::Uuid::new_v4().to_string();
        return build_uec_from_persona_package(&package, &persona_id, None, None);
    }

    let kind = raw_value
        .get("type")
        .and_then(|value| value.as_str())
        .ok_or_else(|| "Invalid import: missing type".to_string())?;

    match kind {
        "character" => {
            let package: CharacterExportPackage = serde_json::from_value(raw_value.clone())
                .map_err(|e| {
                    crate::utils::err_msg(
                        module_path!(),
                        line!(),
                        format!("Invalid import data: {}", e),
                    )
                })?;
            let legacy_id = legacy_entity_id(&raw_value, "character")
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            let legacy_created_at = raw_value
                .get("character")
                .and_then(|value| value.get("createdAt"))
                .and_then(number_to_i64);
            let legacy_updated_at = raw_value
                .get("character")
                .and_then(|value| value.get("updatedAt"))
                .and_then(number_to_i64);

            let mut payload = JsonMap::new();
            payload.insert("id".into(), JsonValue::String(legacy_id));
            payload.insert(
                "name".into(),
                JsonValue::String(package.character.name.clone()),
            );
            if let Some(desc) = package.character.description.clone() {
                payload.insert("description".into(), JsonValue::String(desc.clone()));
            }
            if let Some(def) = package
                .character
                .definition
                .clone()
                .or(package.character.description.clone())
            {
                payload.insert("definitions".into(), JsonValue::String(def));
            }
            if let Some(data) = package.avatar_data.clone() {
                payload.insert("avatar".into(), JsonValue::String(data));
            }
            if let Some(data) = package.background_image_data.clone() {
                payload.insert("chatBackground".into(), JsonValue::String(data));
            }
            payload.insert(
                "rules".into(),
                JsonValue::Array(
                    package
                        .character
                        .rules
                        .iter()
                        .map(|rule| JsonValue::String(rule.clone()))
                        .collect(),
                ),
            );
            let scenes = serde_json::to_value(&package.character.scenes)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            payload.insert("scenes".into(), scenes);
            if let Some(ds) = package.character.default_scene_id.clone() {
                payload.insert("defaultSceneId".into(), JsonValue::String(ds));
            }
            if let Some(dm) = package.character.default_model_id.clone() {
                payload.insert("defaultModelId".into(), JsonValue::String(dm));
            }

            let mut system_prompt_is_id = false;
            if let Some(pt) = package.character.prompt_template_id.clone() {
                payload.insert("systemPrompt".into(), JsonValue::String(pt));
                system_prompt_is_id = true;
            } else if let Some(sp) = package.character.system_prompt.clone() {
                payload.insert("systemPrompt".into(), JsonValue::String(sp));
            }

            if let Some(vc) = package.character.voice_config.clone() {
                if !vc.is_null() {
                    payload.insert("voiceConfig".into(), vc);
                }
            }
            if let Some(autoplay) = package.character.voice_autoplay {
                payload.insert("voiceAutoplay".into(), JsonValue::Bool(autoplay));
            }
            if let Some(created_at) = legacy_created_at {
                payload.insert("createdAt".into(), JsonValue::from(created_at));
            }
            if let Some(updated_at) = legacy_updated_at {
                payload.insert("updatedAt".into(), JsonValue::from(updated_at));
            }

            let mut app_specific = JsonMap::new();
            app_specific.insert(
                "disableAvatarGradient".into(),
                JsonValue::Bool(package.character.disable_avatar_gradient),
            );
            if let Some(mode) = package.character.mode.clone() {
                app_specific.insert("mode".into(), JsonValue::String(mode));
            }
            if let Some(companion) = package.character.companion.clone() {
                app_specific.insert("companion".into(), companion);
            }
            let memory_type = package
                .character
                .memory_type
                .clone()
                .unwrap_or_else(|| "manual".to_string());
            app_specific.insert("memoryType".into(), JsonValue::String(memory_type));
            if !package.character.active_lorebook_ids.is_empty() {
                app_specific.insert(
                    "activeLorebookIds".into(),
                    serde_json::json!(package.character.active_lorebook_ids),
                );
            }
            if !package.character.lorebooks.is_empty() {
                app_specific.insert(
                    "lorebooks".into(),
                    serde_json::to_value(&package.character.lorebooks)
                        .unwrap_or(JsonValue::Array(Vec::new())),
                );
            }
            if let Some(enabled) = package.character.custom_gradient_enabled {
                app_specific.insert("customGradientEnabled".into(), JsonValue::Bool(enabled));
            }
            if let Some(colors) = package.character.custom_gradient_colors.clone() {
                app_specific.insert("customGradientColors".into(), serde_json::json!(colors));
            }
            if let Some(color) = package.character.custom_text_color.clone() {
                app_specific.insert("customTextColor".into(), JsonValue::String(color));
            }
            if let Some(color) = package.character.custom_text_secondary.clone() {
                app_specific.insert("customTextSecondary".into(), JsonValue::String(color));
            }
            if let Some(crop) = package.character.avatar_crop.clone() {
                app_specific.insert(
                    "avatarCrop".into(),
                    serde_json::json!({
                        "x": crop.x,
                        "y": crop.y,
                        "scale": crop.scale,
                    }),
                );
            }

            let fallback_ts = package.exported_at;
            let mut meta = JsonMap::new();
            meta.insert(
                "createdAt".into(),
                JsonValue::from(legacy_created_at.unwrap_or(fallback_ts)),
            );
            meta.insert(
                "updatedAt".into(),
                JsonValue::from(legacy_updated_at.unwrap_or(fallback_ts)),
            );
            meta.insert("source".into(), JsonValue::String("lettuceai".to_string()));

            let uec = create_character_uec(
                payload,
                system_prompt_is_id,
                None,
                Some(JsonValue::Object(app_specific)),
                Some(JsonValue::Object(meta)),
                Some(JsonValue::Object(JsonMap::new())),
            );
            stringify_v2_uec(&uec)
        }
        "persona" => {
            let package: PersonaExportPackage =
                serde_json::from_value(raw_value.clone()).map_err(|e| {
                    crate::utils::err_msg(
                        module_path!(),
                        line!(),
                        format!("Invalid import data: {}", e),
                    )
                })?;
            let legacy_id = legacy_entity_id(&raw_value, "persona")
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            let legacy_created_at = raw_value
                .get("persona")
                .and_then(|value| value.get("createdAt"))
                .and_then(number_to_i64);
            let legacy_updated_at = raw_value
                .get("persona")
                .and_then(|value| value.get("updatedAt"))
                .and_then(number_to_i64);

            let mut payload = JsonMap::new();
            payload.insert("id".into(), JsonValue::String(legacy_id));
            payload.insert(
                "title".into(),
                JsonValue::String(package.persona.title.clone()),
            );
            if !package.persona.description.is_empty() {
                payload.insert(
                    "description".into(),
                    JsonValue::String(package.persona.description.clone()),
                );
            }
            if let Some(data) = package.avatar_data.clone() {
                payload.insert("avatar".into(), JsonValue::String(data));
            }
            if let Some(is_default) = package.persona.is_default {
                payload.insert("isDefault".into(), JsonValue::Bool(is_default));
            }
            if let Some(created_at) = legacy_created_at {
                payload.insert("createdAt".into(), JsonValue::from(created_at));
            }
            if let Some(updated_at) = legacy_updated_at {
                payload.insert("updatedAt".into(), JsonValue::from(updated_at));
            }

            let fallback_ts = package.exported_at;
            let mut meta = JsonMap::new();
            meta.insert(
                "createdAt".into(),
                JsonValue::from(legacy_created_at.unwrap_or(fallback_ts)),
            );
            meta.insert(
                "updatedAt".into(),
                JsonValue::from(legacy_updated_at.unwrap_or(fallback_ts)),
            );
            meta.insert("source".into(), JsonValue::String("lettuceai".to_string()));

            let mut app_specific = JsonMap::new();
            if let Some(crop) = package.persona.avatar_crop.clone() {
                app_specific.insert(
                    "avatarCrop".into(),
                    serde_json::json!({
                        "x": crop.x,
                        "y": crop.y,
                        "scale": crop.scale,
                    }),
                );
            }
            if !package.persona.active_lorebook_ids.is_empty() {
                app_specific.insert(
                    "activeLorebookIds".into(),
                    serde_json::json!(package.persona.active_lorebook_ids),
                );
            }

            let uec = create_persona_uec(
                payload,
                None,
                Some(JsonValue::Object(app_specific)),
                Some(JsonValue::Object(meta)),
                Some(JsonValue::Object(JsonMap::new())),
            );
            stringify_v2_uec(&uec)
        }
        _ => Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "Unsupported import type",
        )),
    }
}

/// Convert a character export between supported formats without importing.
#[tauri::command]
pub fn convert_export_to_format(
    import_json: String,
    target_format: CharacterFileFormat,
) -> Result<String, String> {
    if target_format == CharacterFileFormat::Uec {
        return convert_export_to_uec(import_json);
    }

    let raw_value: JsonValue = serde_json::from_str(&import_json).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Invalid import data: {}", e),
        )
    })?;

    if looks_like_uec(&raw_value) {
        let kind = raw_value
            .get("kind")
            .and_then(|value| value.as_str())
            .unwrap_or("character");
        if kind == "persona" {
            return Err("Persona conversions are only supported for UEC.".to_string());
        }
    }

    let package = if looks_like_uec(&raw_value) {
        parse_uec_character(&raw_value)?
    } else if engine::guess_chara_card_format(&raw_value).is_some() {
        parse_character_import_payload(&raw_value)?.0
    } else if let Ok(package) = serde_json::from_value::<CharacterExportPackage>(raw_value.clone())
    {
        package
    } else {
        return Err("Unsupported or invalid character format".to_string());
    };

    let json = match target_format {
        CharacterFileFormat::CharaCardV2 => {
            let card = engine::export_chara_card_v2(&package);
            serde_json::to_string_pretty(&card).map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to serialize export: {}", e),
                )
            })?
        }
        CharacterFileFormat::CharaCardV3 => {
            let card = engine::export_chara_card_v3(&package, None, None);
            serde_json::to_string_pretty(&card).map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to serialize export: {}", e),
                )
            })?
        }
        CharacterFileFormat::Uec => unreachable!(),
        CharacterFileFormat::LegacyJson | CharacterFileFormat::CharaCardV1 => {
            return Err("Target format is not supported for conversion.".to_string());
        }
    };

    Ok(json)
}

/// Helper: Read avatar as base64 data URL
fn read_avatar_as_base64(
    app: &tauri::AppHandle,
    entity_id: &str,
    filename: &str,
) -> Result<String, String> {
    let avatar_path = storage_root(app)?
        .join("avatars")
        .join(entity_id)
        .join(filename);

    if !avatar_path.exists() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Avatar not found: {}", filename),
        ));
    }

    let bytes = fs::read(&avatar_path)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    // Determine MIME type
    let mime_type = if filename.ends_with(".webp") {
        "image/webp"
    } else if filename.ends_with(".png") {
        "image/png"
    } else if filename.ends_with(".jpg") || filename.ends_with(".jpeg") {
        "image/jpeg"
    } else {
        "image/webp"
    };

    let base64_data = general_purpose::STANDARD.encode(&bytes);
    Ok(format!("data:{};base64,{}", mime_type, base64_data))
}

/// Helper: Read background image as base64 data URL
fn read_background_image_as_base64(
    app: &tauri::AppHandle,
    image_id: &str,
) -> Result<String, String> {
    let images_dir = storage_root(app)?.join("images");

    for ext in &["jpg", "jpeg", "png", "gif", "webp"] {
        let image_path = images_dir.join(format!("{}.{}", image_id, ext));
        if image_path.exists() {
            let bytes = fs::read(&image_path)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            let mime_type = match *ext {
                "jpg" | "jpeg" => "image/jpeg",
                "png" => "image/png",
                "gif" => "image/gif",
                "webp" => "image/webp",
                _ => "image/png",
            };
            let base64_data = general_purpose::STANDARD.encode(&bytes);
            return Ok(format!("data:{};base64,{}", mime_type, base64_data));
        }
    }

    Err(crate::utils::err_msg(
        module_path!(),
        line!(),
        format!("Background image not found: {}", image_id),
    ))
}

/// Helper: Save avatar from base64 data URL
/// entity_id should be "character-{id}" or "persona-{id}"
fn save_avatar_from_base64(
    app: &tauri::AppHandle,
    entity_id: &str,
    base64_data: &str,
) -> Result<String, String> {
    // Strip data URL prefix if present
    let data = if let Some(comma_idx) = base64_data.find(',') {
        &base64_data[comma_idx + 1..]
    } else {
        base64_data
    };

    let bytes = general_purpose::STANDARD.decode(data).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to decode base64: {}", e),
        )
    })?;

    save_avatar_from_bytes(app, entity_id, &bytes)
}

fn save_avatar_from_url(
    app: &tauri::AppHandle,
    entity_id: &str,
    url: &str,
) -> Result<String, String> {
    let rt = tokio::runtime::Runtime::new().map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to create runtime: {}", e),
        )
    })?;

    let bytes = rt.block_on(async {
        let response = reqwest::get(url).await.map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to download avatar image: {}", e),
            )
        })?;

        if !response.status().is_success() {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                format!(
                    "Failed to download avatar image: HTTP {}",
                    response.status()
                ),
            ));
        }

        response.bytes().await.map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to read avatar image bytes: {}", e),
            )
        })
    })?;

    save_avatar_from_bytes(app, entity_id, &bytes)
}

fn save_avatar_from_bytes(
    app: &tauri::AppHandle,
    entity_id: &str,
    bytes: &[u8],
) -> Result<String, String> {
    let avatars_dir = storage_root(app)?.join("avatars").join(entity_id);

    crate::utils::log_debug(
        app,
        "entity_transfer",
        format!("Creating avatar directory: {:?}", avatars_dir),
    );
    fs::create_dir_all(&avatars_dir).map_err(|e| {
        crate::utils::log_error(
            app,
            "entity_transfer",
            format!("Failed to create avatar directory: {:?}", e),
        );
        e.to_string()
    })?;

    // Convert to WebP
    let webp_bytes = match image::load_from_memory(bytes) {
        Ok(img) => {
            let mut webp_data: Vec<u8> = Vec::new();
            let encoder = image::codecs::webp::WebPEncoder::new_lossless(&mut webp_data);
            img.write_with_encoder(encoder).map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to encode WebP: {}", e),
                )
            })?;
            webp_data
        }
        Err(_) => bytes.to_vec(),
    };

    let base_filename = "avatar_base.webp";
    let base_path = avatars_dir.join(base_filename);
    fs::write(&base_path, &webp_bytes)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let legacy_path = avatars_dir.join("avatar.webp");
    fs::write(&legacy_path, &webp_bytes)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let round_path = avatars_dir.join("avatar_round.webp");
    fs::write(&round_path, &webp_bytes)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    Ok(base_filename.to_string())
}

/// Helper: Save background image from base64 data URL
fn save_background_image_from_base64(
    app: &tauri::AppHandle,
    base64_data: &str,
) -> Result<String, String> {
    // Strip data URL prefix if present
    let data = if let Some(comma_idx) = base64_data.find(',') {
        &base64_data[comma_idx + 1..]
    } else {
        base64_data
    };

    let bytes = general_purpose::STANDARD.decode(data).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to decode base64: {}", e),
        )
    })?;

    let images_dir = storage_root(app)?.join("images");
    fs::create_dir_all(&images_dir)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    // Detect image format
    let extension = if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "jpg"
    } else if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        "png"
    } else if bytes.starts_with(&[0x47, 0x49, 0x46]) {
        "gif"
    } else if bytes.len() > 12 && &bytes[8..12] == b"WEBP" {
        "webp"
    } else {
        "png"
    };

    let image_id = uuid::Uuid::new_v4().to_string();
    let image_path = images_dir.join(format!("{}.{}", image_id, extension));
    fs::write(&image_path, bytes)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    Ok(image_id)
}

/// Helper: Read imported character and return as JSON
fn read_imported_character(
    conn: &rusqlite::Connection,
    character_id: &str,
) -> Result<String, String> {
    let (
        name,
        avatar_path,
        avatar_crop_x,
        avatar_crop_y,
        avatar_crop_scale,
        bg_path,
        description,
        definition,
        nickname,
        scenario,
        creator_notes,
        creator,
        creator_notes_multilingual,
        source,
        tags,
        default_scene_id,
        default_model_id,
        mode,
        companion_raw,
        prompt_template_id,
        active_lorebook_ids,
        system_prompt,
        voice_config,
        voice_autoplay,
        memory_type,
        disable_avatar_gradient,
        custom_gradient_enabled,
        custom_gradient_colors,
        custom_text_color,
        custom_text_secondary,
        created_at,
        updated_at,
    ): (
        String,         // name
        Option<String>, // avatar_path
        Option<f64>,    // avatar_crop_x
        Option<f64>,    // avatar_crop_y
        Option<f64>,    // avatar_crop_scale
        Option<String>, // background_image_path
        Option<String>, // description
        Option<String>, // definition
        Option<String>, // nickname
        Option<String>, // scenario
        Option<String>, // creator_notes
        Option<String>, // creator
        Option<String>, // creator_notes_multilingual
        Option<String>, // source
        Option<String>, // tags
        Option<String>, // default_scene_id
        Option<String>, // default_model_id
        Option<String>, // mode
        Option<String>, // companion
        Option<String>, // prompt_template_id
        Option<String>, // active_lorebook_ids
        Option<String>, // system_prompt
        Option<String>, // voice_config
        Option<i64>,    // voice_autoplay
        Option<String>, // memory_type
        i64,            // disable_avatar_gradient
        i64,            // custom_gradient_enabled
        Option<String>, // custom_gradient_colors
        Option<String>, // custom_text_color
        Option<String>, // custom_text_secondary
        i64,            // created_at
        i64,            // updated_at
    ) = conn
        .query_row(
            "SELECT name, avatar_path, avatar_crop_x, avatar_crop_y, avatar_crop_scale, background_image_path, description, definition, nickname, scenario, creator_notes, creator, creator_notes_multilingual, source, tags, default_scene_id, default_model_id, COALESCE(mode, 'roleplay'), companion, prompt_template_id, active_lorebook_ids, system_prompt, voice_config, voice_autoplay, memory_type, disable_avatar_gradient, custom_gradient_enabled, custom_gradient_colors, custom_text_color, custom_text_secondary, created_at, updated_at FROM characters WHERE id = ?",
            params![character_id],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                    r.get(8)?,
                    r.get(9)?,
                    r.get(10)?,
                    r.get(11)?,
                    r.get(12)?,
                    r.get(13)?,
                    r.get(14)?,
                    r.get(15)?,
                    r.get(16)?,
                    r.get(17)?,
                    r.get(18)?,
                    r.get(19)?,
                    r.get(20)?,
                    r.get(21)?,
                    r.get(22)?,
                    r.get(23)?,
                    r.get(24)?,
                    r.get::<_, i64>(25)?,
                    r.get::<_, i64>(26)?,
                    r.get(27)?,
                    r.get(28)?,
                    r.get(29)?,
                    r.get(30)?,
                    r.get(31)?,
                ))
            },
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    // Read rules
    let mut rules: Vec<JsonValue> = Vec::new();
    let mut stmt = conn
        .prepare("SELECT rule FROM character_rules WHERE character_id = ? ORDER BY idx ASC")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let rule_rows = stmt
        .query_map(params![character_id], |r| r.get::<_, String>(0))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    for rule in rule_rows {
        rules.push(JsonValue::String(rule.map_err(|e| {
            crate::utils::err_to_string(module_path!(), line!(), e)
        })?));
    }

    // Read scenes
    let mut scenes: Vec<JsonValue> = Vec::new();
    let mut scenes_stmt = conn
        .prepare("SELECT id, content, direction, background_image_path, created_at, selected_variant_id FROM scenes WHERE character_id = ? ORDER BY created_at ASC")
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    let scenes_rows = scenes_stmt
        .query_map(params![character_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, Option<String>>(5)?,
            ))
        })
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    for row in scenes_rows {
        let (
            scene_id,
            content,
            direction,
            background_image_path,
            _scene_created_at,
            selected_variant_id,
        ) = row.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        // Read scene variants
        let mut variants: Vec<JsonValue> = Vec::new();
        let mut var_stmt = conn
            .prepare("SELECT id, content, direction, created_at FROM scene_variants WHERE scene_id = ? ORDER BY created_at ASC")
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
        let var_rows = var_stmt
            .query_map(params![&scene_id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, i64>(3)?,
                ))
            })
            .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

        for v in var_rows {
            let (vid, vcontent, vdirection, vcreated) =
                v.map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            let mut variant_obj =
                serde_json::json!({"id": vid, "content": vcontent, "createdAt": vcreated});
            if let Some(dir) = vdirection {
                variant_obj["direction"] = serde_json::json!(dir);
            }
            variants.push(variant_obj);
        }

        let mut scene_obj = JsonMap::new();
        scene_obj.insert("id".into(), JsonValue::String(scene_id));
        scene_obj.insert("content".into(), JsonValue::String(content));
        if let Some(dir) = direction {
            scene_obj.insert("direction".into(), JsonValue::String(dir));
        }
        if let Some(path) = background_image_path {
            scene_obj.insert("backgroundImagePath".into(), JsonValue::String(path));
        }
        scene_obj.insert("createdAt".into(), JsonValue::from(_scene_created_at));
        if !variants.is_empty() {
            scene_obj.insert("variants".into(), JsonValue::Array(variants));
        }
        if let Some(sel) = selected_variant_id {
            scene_obj.insert("selectedVariantId".into(), JsonValue::String(sel));
        }
        scenes.push(JsonValue::Object(scene_obj));
    }

    let mut root = JsonMap::new();
    root.insert("id".into(), JsonValue::String(character_id.to_string()));
    root.insert("name".into(), JsonValue::String(name));
    if let Some(a) = avatar_path {
        root.insert("avatarPath".into(), JsonValue::String(a));
    }
    if let (Some(x), Some(y), Some(scale)) = (avatar_crop_x, avatar_crop_y, avatar_crop_scale) {
        root.insert(
            "avatarCrop".into(),
            serde_json::json!({ "x": x, "y": y, "scale": scale }),
        );
    }
    if let Some(b) = bg_path {
        root.insert("backgroundImagePath".into(), JsonValue::String(b));
    }
    let resolved_definition = definition.or_else(|| description.clone());
    if let Some(def) = resolved_definition {
        root.insert("definition".into(), JsonValue::String(def));
    }
    if let Some(d) = description {
        root.insert("description".into(), JsonValue::String(d));
    }
    if let Some(value) = nickname {
        root.insert("nickname".into(), JsonValue::String(value));
    }
    if let Some(value) = scenario {
        root.insert("scenario".into(), JsonValue::String(value));
    }
    if let Some(value) = creator_notes {
        root.insert("creatorNotes".into(), JsonValue::String(value));
    }
    if let Some(value) = creator {
        root.insert("creator".into(), JsonValue::String(value));
    }
    if let Some(value) = creator_notes_multilingual {
        if let Ok(parsed) = serde_json::from_str::<JsonValue>(&value) {
            if parsed.is_object() {
                root.insert("creatorNotesMultilingual".into(), parsed);
            }
        }
    }
    if let Some(value) = source {
        if let Ok(parsed) = serde_json::from_str::<Vec<String>>(&value) {
            root.insert("source".into(), serde_json::json!(parsed));
        }
    }
    if let Some(value) = tags {
        if let Ok(parsed) = serde_json::from_str::<Vec<String>>(&value) {
            root.insert("tags".into(), serde_json::json!(parsed));
        }
    }
    root.insert("rules".into(), JsonValue::Array(rules));
    root.insert("scenes".into(), JsonValue::Array(scenes));
    if let Some(ds) = default_scene_id {
        root.insert("defaultSceneId".into(), JsonValue::String(ds));
    }
    if let Some(dm) = default_model_id {
        root.insert("defaultModelId".into(), JsonValue::String(dm));
    }
    root.insert(
        "mode".into(),
        JsonValue::String(mode.unwrap_or_else(|| "roleplay".to_string())),
    );
    if let Some(value) = companion_raw {
        if let Ok(parsed) = serde_json::from_str::<JsonValue>(&value) {
            if !parsed.is_null() {
                root.insert("companion".into(), parsed);
            }
        }
    }
    if let Some(value) = active_lorebook_ids {
        if let Ok(parsed) = serde_json::from_str::<Vec<String>>(&value) {
            root.insert("activeLorebookIds".into(), serde_json::json!(parsed));
        }
    }
    let memory_value = memory_type.unwrap_or_else(|| "manual".to_string());
    root.insert("memoryType".into(), JsonValue::String(memory_value));
    if let Some(pt) = prompt_template_id {
        root.insert("promptTemplateId".into(), JsonValue::String(pt));
    }
    if let Some(sp) = system_prompt {
        root.insert("systemPrompt".into(), JsonValue::String(sp));
    }
    if let Some(vc) = voice_config {
        if let Ok(value) = serde_json::from_str::<JsonValue>(&vc) {
            if !value.is_null() {
                root.insert("voiceConfig".into(), value);
            }
        }
    }
    root.insert(
        "voiceAutoplay".into(),
        JsonValue::Bool(voice_autoplay.unwrap_or(0) != 0),
    );
    root.insert(
        "disableAvatarGradient".into(),
        JsonValue::Bool(disable_avatar_gradient != 0),
    );
    root.insert(
        "customGradientEnabled".into(),
        JsonValue::Bool(custom_gradient_enabled != 0),
    );
    if let Some(colors_json) = custom_gradient_colors {
        if let Ok(colors) = serde_json::from_str::<Vec<String>>(&colors_json) {
            root.insert("customGradientColors".into(), serde_json::json!(colors));
        }
    }
    if let Some(tc) = custom_text_color {
        root.insert("customTextColor".into(), JsonValue::String(tc));
    }
    if let Some(ts) = custom_text_secondary {
        root.insert("customTextSecondary".into(), JsonValue::String(ts));
    }
    root.insert("createdAt".into(), JsonValue::from(created_at));
    root.insert("updatedAt".into(), JsonValue::from(updated_at));

    serde_json::to_string(&JsonValue::Object(root))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

#[tauri::command]
pub fn persona_export(app: tauri::AppHandle, persona_id: String) -> Result<String, String> {
    log_info(
        &app,
        "persona_export",
        format!("Exporting persona: {}", persona_id),
    );

    let conn = open_db(&app)?;

    // Read persona data
    let (title, description, nickname, avatar_path, avatar_crop_x, avatar_crop_y, avatar_crop_scale, active_lorebook_ids, is_default, created_at, updated_at): (
        String,
        String,
        Option<String>,
        Option<String>,
        Option<f64>,
        Option<f64>,
        Option<f64>,
        String,
        i64,
        i64,
        i64,
    ) = conn
        .query_row(
            "SELECT title, description, nickname, avatar_path, avatar_crop_x, avatar_crop_y, avatar_crop_scale, COALESCE(active_lorebook_ids, '[]'), is_default, created_at, updated_at FROM personas WHERE id = ?",
            params![&persona_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?, r.get(10)?)),
        )
        .map_err(|e| crate::utils::err_msg(module_path!(), line!(), format!("Persona not found: {}", e)))?;

    // Read avatar image if exists
    let avatar_data = if let Some(ref avatar_filename) = avatar_path {
        read_avatar_as_base64(&app, &format!("persona-{}", persona_id), avatar_filename).ok()
    } else {
        None
    };

    let mut payload = JsonMap::new();
    payload.insert("id".into(), JsonValue::String(persona_id.clone()));
    payload.insert("title".into(), JsonValue::String(title));
    payload.insert("description".into(), JsonValue::String(description));
    if let Some(n) = nickname {
        payload.insert("nickname".into(), JsonValue::String(n));
    }
    payload.insert("isDefault".into(), JsonValue::Bool(is_default != 0));
    payload.insert("createdAt".into(), JsonValue::from(created_at));
    payload.insert("updatedAt".into(), JsonValue::from(updated_at));
    if let Some(data) = avatar_data {
        payload.insert("avatar".into(), JsonValue::String(data));
    }

    let mut meta = JsonMap::new();
    meta.insert("createdAt".into(), JsonValue::from(created_at));
    meta.insert("updatedAt".into(), JsonValue::from(updated_at));
    meta.insert("source".into(), JsonValue::String("lettuceai".to_string()));

    let mut app_specific = JsonMap::new();
    if let (Some(x), Some(y), Some(scale)) = (avatar_crop_x, avatar_crop_y, avatar_crop_scale) {
        app_specific.insert(
            "avatarCrop".into(),
            serde_json::json!({ "x": x, "y": y, "scale": scale }),
        );
    }
    if let Ok(parsed) = serde_json::from_str::<Vec<String>>(&active_lorebook_ids) {
        if !parsed.is_empty() {
            app_specific.insert("activeLorebookIds".into(), serde_json::json!(parsed));
        }
    }

    let export_card = create_persona_uec(
        payload,
        None,
        Some(JsonValue::Object(app_specific)),
        Some(JsonValue::Object(meta)),
        Some(JsonValue::Object(JsonMap::new())),
    );

    let json = stringify_v2_uec(&export_card)?;

    log_info(
        &app,
        "persona_export",
        format!("Successfully exported persona: {}", persona_id),
    );

    Ok(json)
}

/// Import a persona from a JSON package
#[tauri::command]
pub fn persona_import(app: tauri::AppHandle, import_json: String) -> Result<String, String> {
    log_info(&app, "persona_import", "Starting persona import");

    let raw_value: JsonValue = serde_json::from_str(&import_json).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Invalid import data: {}", e),
        )
    })?;
    let package = if looks_like_uec(&raw_value) {
        parse_uec_persona(&raw_value)?
    } else {
        serde_json::from_value::<PersonaExportPackage>(raw_value).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Invalid import data: {}", e),
            )
        })?
    };

    // Validate version
    if package.version > 1 {
        return Err(format!(
            "Unsupported export version: {}. Please update your app.",
            package.version
        ));
    }

    let mut conn = open_db(&app)?;
    let tx = conn
        .transaction()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    // Generate new ID for imported persona
    let new_persona_id = uuid::Uuid::new_v4().to_string();
    let now = now_ms() as i64;

    log_info(
        &app,
        "persona_import",
        format!("Importing as new persona: {}", new_persona_id),
    );

    // Save avatar if provided
    let avatar_path = if let Some(ref avatar_base64) = package.avatar_data {
        if is_http_url(avatar_base64) {
            match save_avatar_from_url(&app, &format!("persona-{}", new_persona_id), avatar_base64)
            {
                Ok(filename) => Some(filename),
                Err(e) => {
                    log_info(
                        &app,
                        "persona_import",
                        format!("Warning: Failed to import remote avatar URL: {}", e),
                    );
                    None
                }
            }
        } else {
            match save_avatar_from_base64(
                &app,
                &format!("persona-{}", new_persona_id),
                avatar_base64,
            ) {
                Ok(filename) => Some(filename),
                Err(e) => {
                    log_info(
                        &app,
                        "persona_import",
                        format!("Warning: Failed to import avatar: {}", e),
                    );
                    None
                }
            }
        }
    } else {
        None
    };

    let is_default = package.persona.is_default.unwrap_or(false);
    let (avatar_crop_x, avatar_crop_y, avatar_crop_scale) = package
        .persona
        .avatar_crop
        .as_ref()
        .map(|crop| (Some(crop.x), Some(crop.y), Some(crop.scale)))
        .unwrap_or((None, None, None));
    let active_lorebook_ids = serde_json::to_string(&package.persona.active_lorebook_ids)
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
    if is_default {
        tx.execute("UPDATE personas SET is_default = 0", [])
            .map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to clear default persona: {}", e),
                )
            })?;
    }

    tx.execute(
        r#"INSERT INTO personas (id, title, description, nickname, avatar_path, avatar_crop_x, avatar_crop_y, avatar_crop_scale, active_lorebook_ids, is_default, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        params![
            &new_persona_id,
            &package.persona.title,
            &package.persona.description,
            &package.persona.nickname,
            avatar_path,
            avatar_crop_x,
            avatar_crop_y,
            avatar_crop_scale,
            active_lorebook_ids,
            is_default as i64,
            now,
            now
        ],
    )
    .map_err(|e| crate::utils::err_msg(module_path!(), line!(), format!("Failed to insert persona: {}", e)))?;

    tx.commit()
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    log_info(
        &app,
        "persona_import",
        format!("Successfully imported persona: {}", new_persona_id),
    );

    // Return the new persona as JSON
    let conn2 = open_db(&app)?;
    read_imported_persona(&conn2, &new_persona_id)
}

/// Helper: Read imported persona and return as JSON
fn read_imported_persona(conn: &rusqlite::Connection, persona_id: &str) -> Result<String, String> {
    let (title, description, avatar_path, avatar_crop_x, avatar_crop_y, avatar_crop_scale, active_lorebook_ids, is_default, created_at, updated_at):
        (String, String, Option<String>, Option<f64>, Option<f64>, Option<f64>, String, i64, i64, i64) =
        conn.query_row(
            "SELECT title, description, avatar_path, avatar_crop_x, avatar_crop_y, avatar_crop_scale, COALESCE(active_lorebook_ids, '[]'), is_default, created_at, updated_at FROM personas WHERE id = ?",
            params![persona_id],
            |r| Ok((
                r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get::<_, i64>(7)?, r.get(8)?, r.get(9)?
            )),
        )
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;

    let mut root = JsonMap::new();
    root.insert("id".into(), JsonValue::String(persona_id.to_string()));
    root.insert("title".into(), JsonValue::String(title));
    root.insert("description".into(), JsonValue::String(description));
    if let Some(a) = avatar_path {
        root.insert("avatarPath".into(), JsonValue::String(a));
    }
    if let (Some(x), Some(y), Some(scale)) = (avatar_crop_x, avatar_crop_y, avatar_crop_scale) {
        root.insert(
            "avatarCrop".into(),
            serde_json::json!({ "x": x, "y": y, "scale": scale }),
        );
    }
    if let Ok(parsed) = serde_json::from_str::<Vec<String>>(&active_lorebook_ids) {
        root.insert("activeLorebookIds".into(), serde_json::json!(parsed));
    }
    root.insert("isDefault".into(), JsonValue::Bool(is_default != 0));
    root.insert("createdAt".into(), JsonValue::from(created_at));
    root.insert("updatedAt".into(), JsonValue::from(updated_at));

    serde_json::to_string(&JsonValue::Object(root))
        .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
}

/// Generic import that auto-detects whether it's a character or persona export
/// Returns a JSON object with "importType" field indicating what was imported
#[tauri::command]
pub fn import_package(app: tauri::AppHandle, import_json: String) -> Result<String, String> {
    log_info(&app, "import_package", "Auto-detecting import type");

    let raw_value: JsonValue = serde_json::from_str(&import_json).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Invalid import data: {}", e),
        )
    })?;

    let import_kind = if looks_like_uec(&raw_value) {
        raw_value
            .get("kind")
            .and_then(|value| value.as_str())
            .ok_or_else(|| "Invalid UEC: missing kind".to_string())?
            .to_string()
    } else if let Some(kind) = raw_value.get("type").and_then(|value| value.as_str()) {
        kind.to_string()
    } else if engine::guess_chara_card_format(&raw_value).is_some() {
        "character".to_string()
    } else {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "Invalid import: missing type",
        ));
    };

    match import_kind.as_str() {
        "character" => {
            log_info(&app, "import_package", "Detected character export");
            let result = character_import(app, import_json)?;

            let mut result_obj = serde_json::from_str::<JsonMap<String, JsonValue>>(&result)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            result_obj.insert(
                "importType".into(),
                JsonValue::String("character".to_string()),
            );
            serde_json::to_string(&JsonValue::Object(result_obj))
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
        }
        "persona" => {
            log_info(&app, "import_package", "Detected persona export");
            let result = persona_import(app, import_json)?;

            let mut result_obj = serde_json::from_str::<JsonMap<String, JsonValue>>(&result)
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))?;
            result_obj.insert(
                "importType".into(),
                JsonValue::String("persona".to_string()),
            );
            serde_json::to_string(&JsonValue::Object(result_obj))
                .map_err(|e| crate::utils::err_to_string(module_path!(), line!(), e))
        }
        _ => Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "Unsupported import type",
        )),
    }
}

#[tauri::command]
pub fn save_json_to_downloads(
    app: tauri::AppHandle,
    filename: String,
    json_content: String,
) -> Result<String, String> {
    log_info(
        &app,
        "save_json_to_downloads",
        format!("Saving file to downloads: {}", filename),
    );

    #[cfg(target_os = "android")]
    let download_dir = {
        use std::path::PathBuf;
        PathBuf::from("/storage/emulated/0/Download")
    };

    #[cfg(not(target_os = "android"))]
    let download_dir = app.path().download_dir().map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to get downloads directory: {}", e),
        )
    })?;

    if !download_dir.exists() {
        fs::create_dir_all(&download_dir).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to create downloads directory: {}", e),
            )
        })?;
    }

    let file_path = download_dir.join(&filename);

    fs::write(&file_path, json_content.as_bytes()).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to write file: {}", e),
        )
    })?;

    let path_str = file_path
        .to_str()
        .ok_or_else(|| "Invalid path".to_string())?
        .to_string();

    log_info(
        &app,
        "save_json_to_downloads",
        format!("Successfully saved file to: {}", path_str),
    );

    Ok(path_str)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn normalize_uec_for_read_accepts_v1_schema() {
        let card = json!({
            "schema": { "name": "UEC", "version": SCHEMA_VERSION },
            "kind": "character",
            "payload": {
                "id": "char-v1",
                "name": "Aster Vale"
            }
        });

        let parsed = normalize_uec_for_read(&card, false).expect("v1 UEC should be readable");
        assert_eq!(parsed.kind, UecKind::Character);
        assert_eq!(parsed.schema.version, SCHEMA_VERSION);
    }

    #[test]
    fn normalize_uec_for_read_downgrades_v2_schema_for_legacy_parser() {
        let card = json!({
            "schema": {
                "name": "UEC",
                "version": SCHEMA_VERSION_V2
            },
            "kind": "character",
            "payload": {
                "id": "char-v2",
                "name": "Aster Vale",
                "scene": {
                    "id": "scene-1",
                    "content": "Hello there",
                    "selectedVariant": 0,
                    "variants": []
                }
            },
            "meta": {
                "originalCreatedAt": 1,
                "originalUpdatedAt": 2
            }
        });

        let parsed = normalize_uec_for_read(&card, false).expect("v2 UEC should be readable");
        assert_eq!(parsed.kind, UecKind::Character);
        assert_eq!(parsed.schema.version, SCHEMA_VERSION);
        let payload = parsed.payload.as_object().expect("payload object");
        assert!(payload.get("scenes").is_some());
        assert!(payload.get("scene").is_none());
    }

    #[test]
    fn stringify_v2_uec_upgrades_v1_schema_to_v2() {
        let mut payload = JsonMap::new();
        payload.insert("id".into(), JsonValue::String("char-1".to_string()));
        payload.insert("name".into(), JsonValue::String("Aster Vale".to_string()));
        payload.insert(
            "avatar".into(),
            JsonValue::String("data:image/webp;base64,QUJD".to_string()),
        );
        payload.insert(
            "chatBackground".into(),
            JsonValue::String("https://example.com/bg.png".to_string()),
        );
        payload.insert(
            "scenes".into(),
            JsonValue::Array(vec![json!({
                "id": "scene-1",
                "content": "Hello there",
                "selectedVariantId": null,
                "variants": []
            })]),
        );
        payload.insert(
            "defaultSceneId".into(),
            JsonValue::String("scene-1".to_string()),
        );
        payload.insert("createdAt".into(), JsonValue::from(1));
        payload.insert("updatedAt".into(), JsonValue::from(2));

        let v1 = create_character_uec(
            payload,
            false,
            None,
            None,
            Some(json!({ "createdAt": 1, "updatedAt": 2, "source": "lettuceai" })),
            None,
        );
        let value: JsonValue =
            serde_json::from_str(&stringify_v2_uec(&v1).expect("v2 json")).expect("valid json");
        let schema = value
            .get("schema")
            .and_then(|schema| schema.as_object())
            .expect("schema object");

        assert_eq!(
            schema.get("version").and_then(|value| value.as_str()),
            Some(SCHEMA_VERSION_V2)
        );
        let payload = value
            .get("payload")
            .and_then(|payload| payload.as_object())
            .expect("payload object");
        assert!(payload.get("scene").is_some());
        assert!(payload.get("scenes").is_none());
        assert_eq!(
            payload
                .get("avatar")
                .and_then(|avatar| avatar.get("type"))
                .and_then(|value| value.as_str()),
            Some("inline_base64")
        );
        assert_eq!(
            payload
                .get("chatBackground")
                .and_then(|background| background.get("type"))
                .and_then(|value| value.as_str()),
            Some("remote_url")
        );
    }

    #[test]
    fn stringify_v2_uec_preserves_scene_variants_and_selected_id() {
        let mut payload = JsonMap::new();
        payload.insert("id".into(), JsonValue::String("char-1".to_string()));
        payload.insert("name".into(), JsonValue::String("Aster Vale".to_string()));
        payload.insert(
            "scenes".into(),
            JsonValue::Array(vec![json!({
                "id": "scene-1",
                "content": "Hello there",
                "selectedVariantId": "variant-2",
                "variants": [
                    {
                        "id": "variant-1",
                        "content": "Variant one",
                        "createdAt": 10
                    },
                    {
                        "id": "variant-2",
                        "content": "Variant two",
                        "direction": "Second",
                        "createdAt": 20
                    }
                ]
            })]),
        );
        payload.insert(
            "defaultSceneId".into(),
            JsonValue::String("scene-1".to_string()),
        );
        payload.insert("createdAt".into(), JsonValue::from(1));
        payload.insert("updatedAt".into(), JsonValue::from(2));

        let v1 = create_character_uec(
            payload,
            false,
            None,
            None,
            Some(json!({ "createdAt": 1, "updatedAt": 2, "source": "lettuceai" })),
            None,
        );

        let value: JsonValue =
            serde_json::from_str(&stringify_v2_uec(&v1).expect("v2 json")).expect("valid json");
        let scene = value
            .get("payload")
            .and_then(|payload| payload.get("scene"))
            .and_then(JsonValue::as_object)
            .expect("scene object");

        assert_eq!(
            scene.get("selectedVariant").and_then(JsonValue::as_str),
            Some("variant-2")
        );
        let variants = scene
            .get("variants")
            .and_then(JsonValue::as_array)
            .expect("variants array");
        assert_eq!(variants.len(), 2);
        assert_eq!(
            variants[1].get("id").and_then(JsonValue::as_str),
            Some("variant-2")
        );
        assert_eq!(
            variants[1].get("direction").and_then(JsonValue::as_str),
            Some("Second")
        );
    }

    #[test]
    fn stringify_v2_uec_flattens_additional_scenes_into_variants() {
        let mut payload = JsonMap::new();
        payload.insert("id".into(), JsonValue::String("char-1".to_string()));
        payload.insert("name".into(), JsonValue::String("Aster Vale".to_string()));
        payload.insert(
            "scenes".into(),
            JsonValue::Array(vec![
                json!({
                    "id": "scene-1",
                    "content": "Primary scene",
                    "selectedVariantId": null,
                    "variants": []
                }),
                json!({
                    "id": "scene-2",
                    "content": "Second scene",
                    "direction": "alt",
                    "createdAt": 20,
                    "selectedVariantId": null,
                    "variants": []
                }),
                json!({
                    "id": "scene-3",
                    "content": "Third scene",
                    "createdAt": 30,
                    "selectedVariantId": null,
                    "variants": []
                }),
            ]),
        );
        payload.insert(
            "defaultSceneId".into(),
            JsonValue::String("scene-1".to_string()),
        );
        payload.insert("createdAt".into(), JsonValue::from(1));
        payload.insert("updatedAt".into(), JsonValue::from(2));

        let v1 = create_character_uec(
            payload,
            false,
            None,
            None,
            Some(json!({ "createdAt": 1, "updatedAt": 2, "source": "lettuceai" })),
            None,
        );

        let value: JsonValue =
            serde_json::from_str(&stringify_v2_uec(&v1).expect("v2 json")).expect("valid json");
        let scene = value
            .get("payload")
            .and_then(|payload| payload.get("scene"))
            .and_then(JsonValue::as_object)
            .expect("scene object");
        let variants = scene
            .get("variants")
            .and_then(JsonValue::as_array)
            .expect("variants array");

        assert_eq!(variants.len(), 2);
        assert_eq!(
            variants[0].get("id").and_then(JsonValue::as_str),
            Some("scene-2")
        );
        assert_eq!(
            variants[1].get("id").and_then(JsonValue::as_str),
            Some("scene-3")
        );
    }

    #[test]
    fn parse_uec_character_reads_v2_asset_locators() {
        let card = json!({
            "schema": { "name": "UEC", "version": SCHEMA_VERSION_V2 },
            "kind": "character",
            "payload": {
                "id": "char-v2",
                "name": "Aster Vale",
                "avatar": {
                    "type": "inline_base64",
                    "mimeType": "image/webp",
                    "data": "QUJD"
                },
                "chatBackground": {
                    "type": "remote_url",
                    "url": "https://example.com/bg.png"
                },
                "scene": {
                    "id": "scene-1",
                    "content": "Hello there",
                    "selectedVariant": 0,
                    "variants": []
                }
            },
            "meta": {
                "createdAt": 1,
                "updatedAt": 2,
                "originalCreatedAt": 1,
                "originalUpdatedAt": 2
            }
        });

        let package = parse_uec_character(&card).expect("v2 character should parse");
        assert_eq!(
            package.avatar_data.as_deref(),
            Some("data:image/webp;base64,QUJD")
        );
        assert_eq!(
            package.background_image_data.as_deref(),
            Some("https://example.com/bg.png")
        );
    }

    #[test]
    fn parse_uec_character_expands_v2_scene_variants_into_scenes() {
        let card = json!({
            "schema": { "name": "UEC", "version": SCHEMA_VERSION_V2 },
            "kind": "character",
            "payload": {
                "id": "char-v2",
                "name": "Aster Vale",
                "scene": {
                    "id": "scene-1",
                    "content": "Primary scene",
                    "selectedVariant": "scene-3",
                    "variants": [
                        {
                            "id": "scene-2",
                            "content": "Second scene",
                            "direction": "Alt two",
                            "createdAt": 20
                        },
                        {
                            "id": "scene-3",
                            "content": "Third scene",
                            "createdAt": 30
                        }
                    ]
                }
            },
            "meta": {
                "createdAt": 1,
                "updatedAt": 2,
                "originalCreatedAt": 1,
                "originalUpdatedAt": 2
            }
        });

        let package = parse_uec_character(&card).expect("v2 character should parse");
        assert_eq!(package.character.scenes.len(), 3);
        assert_eq!(package.character.scenes[0].id, "scene-1");
        assert_eq!(package.character.scenes[1].id, "scene-2");
        assert_eq!(package.character.scenes[2].id, "scene-3");
        assert_eq!(
            package.character.default_scene_id.as_deref(),
            Some("scene-3")
        );
    }
}
