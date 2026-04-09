use serde_json::{json, Value};
use tauri::AppHandle;

use crate::api::{api_request, ApiRequest};
use crate::chat_manager::attachments::{cleanup_attachments, persist_attachments};
use crate::chat_manager::execution::{
    find_model_with_credential, prepare_default_sampling_request, prepare_sampling_request,
};
use crate::chat_manager::prompting::entry_conditions::{
    entry_is_active, PromptEntryConditionContext,
};
use crate::chat_manager::prompts;
use crate::chat_manager::provider_adapter::adapter_for;
use crate::chat_manager::request::extract_text;
use crate::chat_manager::service::{require_api_key, ChatContext};
use crate::chat_manager::storage::{
    get_base_prompt_entries, resolve_credential_for_model, PromptType,
};
use crate::chat_manager::turn_builder::{
    partition_prompt_entries, should_insert_in_chat_prompt_entry,
};
use crate::chat_manager::types::{
    Character, ChatGenerateDesignReferenceDescriptionArgs, ChatGenerateSceneImageArgs,
    ChatGenerateScenePromptArgs, ImageAttachment, Model, Persona, PromptEntryChatMode,
    PromptEntryImageSlot, PromptEntryPayload, PromptEntryPosition, ProviderCredential, Session,
    Settings, StoredMessage, SystemPromptEntry,
};
use crate::image_generator::types::ImageGenerationRequest;
use crate::storage_manager::media::{storage_load_avatar, storage_read_image_data};
use crate::storage_manager::sessions::{messages_upsert_batch_typed, session_upsert_meta_typed};
use crate::usage::tracking::UsageOperationType;
use crate::utils::now_millis;

fn resolve_persona_id<'a>(session: &'a Session, explicit: Option<&'a str>) -> Option<&'a str> {
    if explicit.is_some() {
        return explicit;
    }
    if session.persona_disabled {
        Some("")
    } else {
        session.persona_id.as_deref()
    }
}

fn resolve_image_generation_target<'a>(
    settings: &'a Settings,
    preferred_model_id: Option<&str>,
) -> Result<(&'a Model, &'a ProviderCredential), String> {
    if let Some(model_id) = preferred_model_id.filter(|id| !id.trim().is_empty()) {
        let (model, credential) = find_model_with_credential(settings, model_id)
            .ok_or_else(|| "Configured scene generation model could not be resolved".to_string())?;
        let supports_image_output = model
            .output_scopes
            .iter()
            .any(|scope| scope.eq_ignore_ascii_case("image"));
        if !supports_image_output {
            return Err(
                "Configured scene generation model does not support image output".to_string(),
            );
        }
        return Ok((model, credential));
    }

    settings
        .models
        .iter()
        .find_map(|model| {
            let supports_image_output = model
                .output_scopes
                .iter()
                .any(|scope| scope.eq_ignore_ascii_case("image"));
            if !supports_image_output {
                return None;
            }
            let credential = resolve_credential_for_model(settings, model)?;
            Some((model, credential))
        })
        .ok_or_else(|| "No image generation model is configured".to_string())
}

fn supports_scene_writer_model(model: &Model) -> bool {
    model
        .input_scopes
        .iter()
        .any(|scope| scope.eq_ignore_ascii_case("text"))
        && model
            .input_scopes
            .iter()
            .any(|scope| scope.eq_ignore_ascii_case("image"))
        && model
            .output_scopes
            .iter()
            .any(|scope| scope.eq_ignore_ascii_case("text"))
}

fn resolve_scene_writer_target<'a>(
    settings: &'a Settings,
    preferred_model_id: Option<&str>,
) -> Result<(&'a Model, &'a ProviderCredential), String> {
    if let Some(model_id) = preferred_model_id.filter(|id| !id.trim().is_empty()) {
        let (model, credential) = find_model_with_credential(settings, model_id)
            .ok_or_else(|| "Configured scene writer model could not be resolved".to_string())?;
        if !supports_scene_writer_model(model) {
            return Err(
                "Configured scene writer model must support text and image input with text output"
                    .to_string(),
            );
        }
        return Ok((model, credential));
    }

    settings
        .models
        .iter()
        .find_map(|model| {
            if !supports_scene_writer_model(model) {
                return None;
            }
            let credential = resolve_credential_for_model(settings, model)?;
            Some((model, credential))
        })
        .ok_or_else(|| {
            "No compatible scene writer model is configured. Add an image-text-to-text model in Settings > Image Generation."
                .to_string()
        })
}

fn scene_generation_enabled(settings: &Settings) -> bool {
    settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| advanced.scene_generation_enabled)
        .unwrap_or(true)
}

fn avatar_generation_enabled(settings: &Settings) -> bool {
    settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| advanced.avatar_generation_enabled)
        .unwrap_or(true)
}

fn model_supports_vision(model: &Model) -> bool {
    model.input_scopes.iter().any(|scope| {
        matches!(
            scope.trim().to_ascii_lowercase().as_str(),
            "image" | "vision"
        )
    })
}

fn resolve_avatar_reference_data(
    app: &AppHandle,
    entity_prefix: &str,
    entity_id: &str,
    avatar_path: Option<&str>,
) -> Option<String> {
    let prefixed_entity_id = format!("{}-{}", entity_prefix, entity_id);

    storage_load_avatar(
        app.clone(),
        prefixed_entity_id.clone(),
        "avatar_base.webp".to_string(),
    )
    .ok()
    .or_else(|| {
        let filename = avatar_path?.trim();
        if filename.is_empty() || filename.eq_ignore_ascii_case("avatar_base.webp") {
            return None;
        }
        storage_load_avatar(app.clone(), prefixed_entity_id, filename.to_string()).ok()
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SceneReferenceSource {
    None,
    AvatarFallback,
    DesignImages,
}

struct SceneReferenceImages {
    character_images: Vec<String>,
    character_reference_count: usize,
    character_reference_source: SceneReferenceSource,
    chat_background_images: Vec<String>,
    persona_images: Vec<String>,
    persona_reference_count: usize,
    persona_reference_source: SceneReferenceSource,
}

fn resolve_design_reference_images(app: &AppHandle, image_ids: &[String]) -> Vec<String> {
    image_ids
        .iter()
        .filter_map(|image_id| {
            let image_id = image_id.trim();
            if image_id.is_empty() {
                return None;
            }
            storage_read_image_data(app, image_id).ok()
        })
        .collect()
}

fn resolve_chat_background_image(
    app: &AppHandle,
    background_image_path: Option<&str>,
) -> Vec<String> {
    let Some(image_id) = background_image_path
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Vec::new();
    };

    storage_read_image_data(app, image_id)
        .map(|image| vec![image])
        .unwrap_or_default()
}

fn build_scene_reference_images(
    app: &AppHandle,
    character: &Character,
    persona: Option<&Persona>,
) -> SceneReferenceImages {
    let character_design_images =
        resolve_design_reference_images(app, &character.design_reference_image_ids);
    let chat_background_images =
        resolve_chat_background_image(app, character.background_image_path.as_deref());
    let (character_images, character_reference_count, character_reference_source) =
        if !character_design_images.is_empty() {
            let count = character_design_images.len();
            (
                character_design_images,
                count,
                SceneReferenceSource::DesignImages,
            )
        } else if let Some(character_image) = resolve_avatar_reference_data(
            app,
            "character",
            &character.id,
            character.avatar_path.as_deref(),
        ) {
            (
                vec![character_image],
                1,
                SceneReferenceSource::AvatarFallback,
            )
        } else {
            (Vec::new(), 0, SceneReferenceSource::None)
        };

    let mut persona_images = Vec::new();
    let mut persona_reference_count = 0;
    let mut persona_reference_source = SceneReferenceSource::None;
    if let Some(persona) = persona {
        let persona_design_images =
            resolve_design_reference_images(app, &persona.design_reference_image_ids);
        if !persona_design_images.is_empty() {
            persona_reference_count = persona_design_images.len();
            persona_reference_source = SceneReferenceSource::DesignImages;
            persona_images = persona_design_images;
        } else if let Some(persona_image) = resolve_avatar_reference_data(
            app,
            "persona",
            &persona.id,
            persona.avatar_path.as_deref(),
        ) {
            persona_images.push(persona_image);
            persona_reference_count = 1;
            persona_reference_source = SceneReferenceSource::AvatarFallback;
        }
    }

    SceneReferenceImages {
        character_images,
        character_reference_count,
        character_reference_source,
        chat_background_images,
        persona_images,
        persona_reference_count,
        persona_reference_source,
    }
}

fn format_scene_reference_range(start_index: usize, count: usize) -> String {
    if count <= 1 {
        format!("attached image {}", start_index)
    } else {
        format!(
            "attached images {}-{}",
            start_index,
            start_index + count - 1
        )
    }
}

fn persona_scene_name(persona: Option<&Persona>) -> String {
    persona
        .and_then(|value| value.nickname.as_deref())
        .filter(|value| !value.trim().is_empty())
        .or_else(|| persona.map(|value| value.title.as_str()))
        .unwrap_or("the persona")
        .to_string()
}

fn build_scene_prompt_reference_hint(
    entity_name: &str,
    reference_count: usize,
    reference_source: SceneReferenceSource,
) -> String {
    match reference_source {
        SceneReferenceSource::DesignImages if reference_count > 0 => format!(
            "The image model will receive {} saved design reference image{} for {}.",
            reference_count,
            if reference_count == 1 { "" } else { "s" },
            entity_name
        ),
        SceneReferenceSource::AvatarFallback => {
            format!(
                "The image model will receive {}'s base avatar as a visual reference.",
                entity_name
            )
        }
        SceneReferenceSource::None => String::new(),
        SceneReferenceSource::DesignImages => String::new(),
    }
}

fn build_scene_prompt_reference_text(
    entity_name: &str,
    design_description: Option<&str>,
    reference_count: usize,
    reference_source: SceneReferenceSource,
) -> String {
    let mut sections = Vec::new();

    if let Some(description) = design_description
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        sections.push(format!(
            "# {} Reference Notes\n{}",
            entity_name, description
        ));
    }

    let reference_hint =
        build_scene_prompt_reference_hint(entity_name, reference_count, reference_source);
    if !reference_hint.is_empty() {
        sections.push(reference_hint);
    }

    sections.join("\n\n")
}

fn build_chat_background_reference_hint() -> &'static str {
    "The image model will receive the chat background image as an environment reference."
}

fn build_chat_background_reference_text() -> String {
    [
        "# Chat Background Notes",
        "The attached chat background image represents the intended environment or backdrop for this chat.",
        "Use it as the scene's environmental anchor when it fits the current moment, preserving major location cues, palette, lighting mood, architecture, and large background features unless the recent messages clearly establish a different setting.",
    ]
    .join("\n")
}

fn build_scene_prompt_image_parts(images: &[String]) -> Vec<Value> {
    images
        .iter()
        .filter(|image| !image.trim().is_empty())
        .map(|image| {
            json!({
                "type": "image_url",
                "image_url": {
                    "url": image,
                    "detail": "auto"
                }
            })
        })
        .collect()
}

const SCENE_CHARACTER_TOKEN: &str = "{{image[character]}}";
const SCENE_PERSONA_TOKEN: &str = "{{image[persona]}}";
const SCENE_CHAT_BACKGROUND_TOKEN: &str = "{{image[chatBackground]}}";
const SCENE_CHAT_BACKGROUND_LEGACY_TOKEN: &str = "{{image[chat_background]}}";
const DESIGN_REFERENCE_AVATAR_TOKEN: &str = "{{image[avatar]}}";
const DESIGN_REFERENCE_REFERENCES_TOKEN: &str = "{{image[references]}}";

fn legacy_prompt_entry_image_slot(content: &str) -> Option<PromptEntryImageSlot> {
    if content.contains(SCENE_CHARACTER_TOKEN) {
        Some(PromptEntryImageSlot::Character)
    } else if content.contains(SCENE_PERSONA_TOKEN) {
        Some(PromptEntryImageSlot::Persona)
    } else if content.contains(SCENE_CHAT_BACKGROUND_TOKEN)
        || content.contains(SCENE_CHAT_BACKGROUND_LEGACY_TOKEN)
    {
        Some(PromptEntryImageSlot::ChatBackground)
    } else if content.contains(DESIGN_REFERENCE_AVATAR_TOKEN) {
        Some(PromptEntryImageSlot::Avatar)
    } else if content.contains(DESIGN_REFERENCE_REFERENCES_TOKEN) {
        Some(PromptEntryImageSlot::References)
    } else {
        None
    }
}

fn prompt_entry_image_slot(entry: &SystemPromptEntry) -> Option<PromptEntryImageSlot> {
    match &entry.prompt_entry_payload {
        Some(PromptEntryPayload::ImageSlot { slot }) => Some(slot.clone()),
        None => None,
    }
}

fn prompt_entry_has_image_binding(entry: &SystemPromptEntry) -> bool {
    entry.prompt_entry_payload.is_some() || legacy_prompt_entry_image_slot(&entry.content).is_some()
}

fn cleaned_image_entry_text(content: &str) -> Option<String> {
    let stripped = content
        .replace(SCENE_CHARACTER_TOKEN, "")
        .replace(SCENE_PERSONA_TOKEN, "")
        .replace(SCENE_CHAT_BACKGROUND_TOKEN, "")
        .replace(SCENE_CHAT_BACKGROUND_LEGACY_TOKEN, "")
        .replace(DESIGN_REFERENCE_AVATAR_TOKEN, "")
        .replace(DESIGN_REFERENCE_REFERENCES_TOKEN, "");
    let cleaned = condense_prompt_whitespace(stripped);
    if cleaned.trim().is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

fn build_multimodal_image_content(text: Option<String>, image_parts: Vec<Value>) -> Option<Value> {
    if image_parts.is_empty() {
        return None;
    }

    let mut parts = Vec::new();
    if let Some(text) = text.filter(|value| !value.trim().is_empty()) {
        parts.push(json!({
            "type": "text",
            "text": text,
        }));
    }
    parts.extend(image_parts);

    Some(Value::Array(parts))
}

fn build_scene_prompt_content_with_images(
    entry: &SystemPromptEntry,
    reference_images: &SceneReferenceImages,
) -> Option<Value> {
    if let Some(slot) = prompt_entry_image_slot(entry) {
        let parts = match slot {
            PromptEntryImageSlot::Character => {
                build_scene_prompt_image_parts(&reference_images.character_images)
            }
            PromptEntryImageSlot::ChatBackground => {
                build_scene_prompt_image_parts(&reference_images.chat_background_images)
            }
            PromptEntryImageSlot::Persona => {
                build_scene_prompt_image_parts(&reference_images.persona_images)
            }
            PromptEntryImageSlot::Avatar | PromptEntryImageSlot::References => Vec::new(),
        };
        return build_multimodal_image_content(cleaned_image_entry_text(&entry.content), parts);
    }

    let mut parts: Vec<Value> = Vec::new();
    if entry.content.contains(SCENE_CHARACTER_TOKEN) {
        parts.extend(build_scene_prompt_image_parts(
            &reference_images.character_images,
        ));
    }
    if entry.content.contains(SCENE_PERSONA_TOKEN) {
        parts.extend(build_scene_prompt_image_parts(
            &reference_images.persona_images,
        ));
    }
    if entry.content.contains(SCENE_CHAT_BACKGROUND_TOKEN)
        || entry.content.contains(SCENE_CHAT_BACKGROUND_LEGACY_TOKEN)
    {
        parts.extend(build_scene_prompt_image_parts(
            &reference_images.chat_background_images,
        ));
    }

    build_multimodal_image_content(cleaned_image_entry_text(&entry.content), parts)
}

fn build_scene_generation_request(
    scene_prompt: &str,
    model: &Model,
    credential: &ProviderCredential,
    character: &Character,
    persona: Option<&Persona>,
    reference_images: SceneReferenceImages,
) -> ImageGenerationRequest {
    let SceneReferenceImages {
        character_images,
        character_reference_count,
        character_reference_source,
        chat_background_images,
        persona_images,
        persona_reference_count,
        persona_reference_source,
    } = reference_images;
    let mut prompt_sections = Vec::new();
    let mut input_images = character_images.clone();
    input_images.extend(chat_background_images.clone());
    input_images.extend(persona_images.clone());
    let has_character_reference = character_reference_count > 0;
    let has_chat_background_reference = !chat_background_images.is_empty();
    let has_persona_reference = persona_reference_count > 0;

    if let Some(design_description) = character
        .design_description
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        prompt_sections.push(format!(
            "Character design notes for {}:\n{}",
            character.name, design_description
        ));
    }

    if let Some(persona) = persona {
        if let Some(design_description) = persona
            .design_description
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let persona_name = persona_scene_name(Some(persona));
            prompt_sections.push(format!(
                "Persona design notes for {}:\n{}",
                persona_name, design_description
            ));
        }
    }

    if has_character_reference || has_chat_background_reference || has_persona_reference {
        let mut reference_lines = Vec::new();
        let mut next_image_index = 1;
        if has_character_reference {
            let range_label =
                format_scene_reference_range(next_image_index, character_reference_count);
            let source_label = match character_reference_source {
                SceneReferenceSource::DesignImages => "saved character design reference",
                SceneReferenceSource::AvatarFallback => "base avatar reference",
                SceneReferenceSource::None => "character reference",
            };
            reference_lines.push(format!(
                "The {} is the {} for {}. Use it only for {}'s identity, face, body, outfit cues, and signature styling.",
                range_label, source_label, character.name, character.name
            ));
            next_image_index += character_reference_count;
        }
        if has_chat_background_reference {
            let range_label =
                format_scene_reference_range(next_image_index, chat_background_images.len());
            reference_lines.push(format!(
                "The {} is the chat background environment reference. Use it for setting, palette, lighting mood, architecture, and large backdrop elements. Do not treat it as a character identity reference.",
                range_label
            ));
            next_image_index += chat_background_images.len();
        }
        if has_persona_reference {
            let persona_name = persona_scene_name(persona);
            let range_label =
                format_scene_reference_range(next_image_index, persona_reference_count);
            let source_label = match persona_reference_source {
                SceneReferenceSource::DesignImages => "saved persona design reference",
                SceneReferenceSource::AvatarFallback => "base avatar reference",
                SceneReferenceSource::None => "persona reference",
            };
            reference_lines.push(format!(
                "The {} is the {} for {}. Use it only for {}'s identity, face, body, outfit cues, and signature styling.",
                range_label, source_label, persona_name, persona_name
            ));
        }
        reference_lines.push(
            "Do not swap, merge, or borrow identity-defining features between reference images."
                .to_string(),
        );
        match (has_character_reference, has_persona_reference) {
            (true, false) => reference_lines.push(format!(
                "Only {} has a reference image attached. Do not invent {} from {}'s appearance.",
                character.name,
                persona
                    .and_then(|value| value.nickname.as_deref())
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| persona
                        .map(|value| value.title.as_str())
                        .unwrap_or("the persona")),
                character.name
            )),
            (false, true) => reference_lines.push(format!(
                "Only {} has a reference image attached. Do not invent {} from {}'s appearance.",
                persona
                    .and_then(|value| value.nickname.as_deref())
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| persona
                        .map(|value| value.title.as_str())
                        .unwrap_or("the persona")),
                character.name,
                persona
                    .and_then(|value| value.nickname.as_deref())
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| persona
                        .map(|value| value.title.as_str())
                        .unwrap_or("the persona"))
            )),
            _ => {}
        }
        prompt_sections.push(reference_lines.join("\n"));
    }

    prompt_sections.push(scene_prompt.trim().to_string());

    ImageGenerationRequest {
        prompt: prompt_sections.join("\n\n"),
        model: model.name.clone(),
        provider_id: model.provider_id.clone(),
        credential_id: credential.id.clone(),
        advanced_model_settings: model.advanced_model_settings.clone(),
        input_images: if input_images.is_empty() {
            None
        } else {
            Some(input_images)
        },
        size: model
            .advanced_model_settings
            .as_ref()
            .and_then(|settings| settings.sd_size.clone())
            .or_else(|| Some("1024x1024".to_string())),
        quality: None,
        style: None,
        n: Some(1),
    }
}

async fn generate_scene_image_with_retry(
    app: &AppHandle,
    request: ImageGenerationRequest,
    max_attempts: usize,
) -> Result<crate::image_generator::types::ImageGenerationResponse, String> {
    let mut last_error: Option<String> = None;

    for attempt in 1..=max_attempts.max(1) {
        match crate::image_generator::commands::generate_image(app.clone(), request.clone()).await {
            Ok(response) if !response.images.is_empty() => return Ok(response),
            Ok(_) => {
                let error = "No images found in response".to_string();
                if attempt >= max_attempts {
                    return Err(error);
                }
                last_error = Some(error);
            }
            Err(error) => {
                if !error.to_ascii_lowercase().contains("no image") || attempt >= max_attempts {
                    return Err(error);
                }
                last_error = Some(error);
            }
        }
    }

    Err(last_error.unwrap_or_else(|| "No images found in response".to_string()))
}

fn build_scene_prompt_context_messages(
    session: &Session,
    message_id: &str,
) -> Result<String, String> {
    let target_index = session
        .messages
        .iter()
        .position(|message| message.id == message_id)
        .ok_or_else(|| "Message not found in loaded session window".to_string())?;

    let start_index = target_index.saturating_sub(2);
    let context_slice = &session.messages[start_index..=target_index];

    let context = context_slice
        .iter()
        .filter(|message| {
            matches!(message.role.as_str(), "user" | "assistant" | "scene")
                && !message.content.trim().is_empty()
        })
        .map(|message| {
            let role = match message.role.as_str() {
                "assistant" => "Assistant",
                "scene" => "Scene",
                _ => "User",
            };
            format!("{}: {}", role, message.content.trim())
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    if context.trim().is_empty() {
        return Err("No conversation context available for scene prompt generation".to_string());
    }

    Ok(context)
}

fn condense_prompt_whitespace(input: String) -> String {
    let mut output = input;
    while output.contains("\n\n\n") {
        output = output.replace("\n\n\n", "\n\n");
    }
    output.trim().to_string()
}

fn render_scene_generation_prompt_content(
    template_content: &str,
    character: &Character,
    persona: Option<&Persona>,
    recent_messages_text: &str,
    has_chat_background: bool,
) -> String {
    let mut prompt = template_content.to_string();
    let char_name = character.name.as_str();
    let mut char_desc_parts = Vec::new();
    if let Some(value) = character
        .definition
        .as_deref()
        .or(character.description.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        char_desc_parts.push(value.to_string());
    }
    if let Some(value) = character
        .design_description
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        char_desc_parts.push(format!("Visual design notes: {}", value));
    }
    let char_desc = char_desc_parts.join("\n\n");
    let persona_name = persona.map(|value| value.title.as_str()).unwrap_or("User");
    let mut persona_desc_parts = Vec::new();
    if let Some(value) = persona
        .map(|value| value.description.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        persona_desc_parts.push(value.to_string());
    }
    if let Some(value) = persona
        .and_then(|value| value.design_description.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        persona_desc_parts.push(format!("Visual design notes: {}", value));
    }
    let persona_desc = persona_desc_parts.join("\n\n");
    let character_reference_text = build_scene_prompt_reference_text(
        char_name,
        character.design_description.as_deref(),
        character.design_reference_image_ids.len(),
        if !character.design_reference_image_ids.is_empty() {
            SceneReferenceSource::DesignImages
        } else if character.avatar_path.is_some() {
            SceneReferenceSource::AvatarFallback
        } else {
            SceneReferenceSource::None
        },
    );
    let persona_reference_text = if let Some(persona) = persona {
        build_scene_prompt_reference_text(
            &persona_scene_name(Some(persona)),
            persona.design_description.as_deref(),
            persona.design_reference_image_ids.len(),
            if !persona.design_reference_image_ids.is_empty() {
                SceneReferenceSource::DesignImages
            } else if persona.avatar_path.is_some() {
                SceneReferenceSource::AvatarFallback
            } else {
                SceneReferenceSource::None
            },
        )
    } else {
        String::new()
    };
    let chat_background_reference_text = if has_chat_background {
        build_chat_background_reference_text()
    } else {
        String::new()
    };
    prompt = prompt.replace("{{char.name}}", char_name);
    prompt = prompt.replace("{{char}}", char_name);
    prompt = prompt.replace("{{user}}", persona_name);
    prompt = prompt.replace("{{persona}}", persona_name);
    prompt = prompt.replace("{{char.desc}}", &char_desc);
    prompt = prompt.replace("{{persona.name}}", persona_name);
    prompt = prompt.replace("{{persona.desc}}", &persona_desc);
    prompt = prompt.replace("{{recent_messages}}", recent_messages_text);
    let scene_request = if let Some(persona) = persona {
        format!(
            "Create one polished scene image prompt for the visual moment described by the recent messages. Focus on the currently active beat involving {} and {}. Keep {} and {} visually distinct, and make the result immediately usable for image generation.",
            character.name, persona.title, character.name, persona.title
        )
    } else {
        format!(
            "Create one polished scene image prompt for the visual moment described by the recent messages. Focus on the currently active beat involving {}. Make the result immediately usable for image generation.",
            character.name
        )
    };
    prompt = prompt.replace("{{scene_request}}", &scene_request);
    prompt = prompt.replace("{{reference[character]}}", &character_reference_text);
    prompt = prompt.replace("{{reference[persona]}}", &persona_reference_text);
    prompt = prompt.replace(
        "{{reference[chatBackground]}}",
        &chat_background_reference_text,
    );
    prompt = prompt.replace(
        "{{reference[chat_background]}}",
        &chat_background_reference_text,
    );

    condense_prompt_whitespace(prompt)
}

fn condense_scene_generation_entries(entries: Vec<SystemPromptEntry>) -> Vec<SystemPromptEntry> {
    let mut merged_sections = Vec::new();
    let mut image_entries = Vec::new();

    for entry in entries {
        if prompt_entry_has_image_binding(&entry) {
            image_entries.push(entry);
            continue;
        }

        let trimmed = entry.content.trim();
        if trimmed.is_empty() {
            continue;
        }

        merged_sections.push(trimmed.to_string());
    }

    let merged = merged_sections.join("\n\n");

    let mut condensed = Vec::new();
    if !merged.trim().is_empty() {
        condensed.push(SystemPromptEntry {
            id: "scene_gen_condensed_system".to_string(),
            name: "Condensed Scene Generation Prompt".to_string(),
            role: super::types::PromptEntryRole::System,
            content: merged,
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
            conditions: None,
            prompt_entry_payload: None,
        });
    }

    condensed.extend(image_entries);
    condensed
}

fn render_design_reference_prompt_content(
    template_content: &str,
    subject_name: &str,
    subject_description: Option<&str>,
    current_description: Option<&str>,
) -> String {
    let mut prompt = template_content.to_string();
    prompt = prompt.replace("{{subject_name}}", subject_name);
    prompt = prompt.replace("{{subject_description}}", subject_description.unwrap_or(""));
    prompt = prompt.replace("{{current_description}}", current_description.unwrap_or(""));
    condense_prompt_whitespace(prompt)
}

fn condense_design_reference_entries(entries: Vec<SystemPromptEntry>) -> Vec<SystemPromptEntry> {
    let mut merged_sections = Vec::new();
    let mut image_entries = Vec::new();

    for entry in entries {
        if prompt_entry_has_image_binding(&entry) {
            image_entries.push(entry);
            continue;
        }

        let trimmed = entry.content.trim();
        if trimmed.is_empty() {
            continue;
        }

        merged_sections.push(trimmed.to_string());
    }

    let mut condensed = Vec::new();
    if !merged_sections.is_empty() {
        condensed.push(SystemPromptEntry {
            id: "design_ref_condensed_system".to_string(),
            name: "Condensed Design Reference Prompt".to_string(),
            role: super::types::PromptEntryRole::System,
            content: merged_sections.join("\n\n"),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            injection_depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: true,
            conditions: None,
            prompt_entry_payload: None,
        });
    }
    condensed.extend(image_entries);
    condensed
}

fn load_design_reference_prompt_entries(app: &AppHandle) -> (Vec<SystemPromptEntry>, bool) {
    match prompts::get_template(app, prompts::APP_DESIGN_REFERENCE_TEMPLATE_ID) {
        Ok(Some(template)) => {
            if !template.entries.is_empty() {
                (template.entries, template.condense_prompt_entries)
            } else if !template.content.trim().is_empty() {
                (
                    vec![SystemPromptEntry {
                        id: "design_ref_single_entry".to_string(),
                        name: "Design Reference Prompt".to_string(),
                        role: super::types::PromptEntryRole::System,
                        content: template.content,
                        enabled: true,
                        injection_position: PromptEntryPosition::Relative,
                        injection_depth: 0,
                        conditional_min_messages: None,
                        interval_turns: None,
                        system_prompt: true,
                        conditions: None,
                        prompt_entry_payload: None,
                    }],
                    template.condense_prompt_entries,
                )
            } else {
                (
                    get_base_prompt_entries(PromptType::DesignReferencePrompt),
                    false,
                )
            }
        }
        _ => (
            get_base_prompt_entries(PromptType::DesignReferencePrompt),
            false,
        ),
    }
}

fn render_design_reference_prompt_entries(
    app: &AppHandle,
    settings: &Settings,
    model: &Model,
    subject_name: &str,
    subject_description: Option<&str>,
    current_description: Option<&str>,
) -> Vec<SystemPromptEntry> {
    let (template_entries, condense_prompt_entries) = load_design_reference_prompt_entries(app);
    let mut rendered_entries = Vec::new();
    let recent_text = [
        subject_name,
        subject_description.unwrap_or(""),
        current_description.unwrap_or(""),
    ]
    .into_iter()
    .filter(|value| !value.trim().is_empty())
    .collect::<Vec<_>>()
    .join("\n");
    let condition_context = PromptEntryConditionContext {
        chat_mode: PromptEntryChatMode::Direct,
        scene_generation_enabled: scene_generation_enabled(settings),
        avatar_generation_enabled: avatar_generation_enabled(settings),
        has_scene: false,
        has_scene_direction: false,
        has_persona: false,
        message_count: 0,
        participant_count: 1,
        recent_text: &recent_text,
        dynamic_memory_enabled: false,
        has_memory_summary: false,
        has_key_memories: false,
        has_lorebook_content: false,
        has_subject_description: subject_description.is_some(),
        has_current_description: current_description.is_some(),
        has_character_reference_images: false,
        has_chat_background: false,
        has_persona_reference_images: false,
        has_character_reference_text: false,
        has_persona_reference_text: false,
        input_scopes: &model.input_scopes,
        output_scopes: &model.output_scopes,
        provider_id: Some(model.provider_id.as_str()),
        reasoning_enabled: model
            .advanced_model_settings
            .as_ref()
            .and_then(|cfg| cfg.reasoning_enabled)
            .unwrap_or(false),
        vision_enabled: model_supports_vision(model),
    };

    for entry in template_entries {
        if !entry_is_active(&entry, &condition_context) {
            continue;
        }
        let rendered = render_design_reference_prompt_content(
            &entry.content,
            subject_name,
            subject_description,
            current_description,
        );
        if rendered.trim().is_empty() && entry.prompt_entry_payload.is_none() {
            continue;
        }
        let mut next_entry = entry.clone();
        next_entry.content = rendered;
        rendered_entries.push(next_entry);
    }

    if condense_prompt_entries {
        condense_design_reference_entries(rendered_entries)
    } else {
        rendered_entries
    }
}

fn build_design_reference_prompt_content_with_images(
    entry: &SystemPromptEntry,
    avatar_image: Option<&str>,
    reference_images: &[String],
) -> Option<Value> {
    if let Some(slot) = entry
        .prompt_entry_payload
        .as_ref()
        .and_then(|payload| match payload {
            PromptEntryPayload::ImageSlot { slot } => Some(slot.clone()),
        })
    {
        let parts = match slot {
            PromptEntryImageSlot::Avatar => avatar_image
                .filter(|value| !value.trim().is_empty())
                .map(|image| build_scene_prompt_image_parts(&[image.to_string()]))
                .unwrap_or_default(),
            PromptEntryImageSlot::References => build_scene_prompt_image_parts(reference_images),
            PromptEntryImageSlot::Character
            | PromptEntryImageSlot::Persona
            | PromptEntryImageSlot::ChatBackground => Vec::new(),
        };
        return build_multimodal_image_content(cleaned_image_entry_text(&entry.content), parts);
    }

    if !entry.content.contains(DESIGN_REFERENCE_AVATAR_TOKEN)
        && !entry.content.contains(DESIGN_REFERENCE_REFERENCES_TOKEN)
    {
        return None;
    }

    let mut parts: Vec<Value> = Vec::new();
    if entry.content.contains(DESIGN_REFERENCE_AVATAR_TOKEN) {
        if let Some(image) = avatar_image.filter(|value| !value.trim().is_empty()) {
            parts.extend(build_scene_prompt_image_parts(&[image.to_string()]));
        }
    }
    if entry.content.contains(DESIGN_REFERENCE_REFERENCES_TOKEN) && !reference_images.is_empty() {
        parts.extend(build_scene_prompt_image_parts(reference_images));
    }

    build_multimodal_image_content(cleaned_image_entry_text(&entry.content), parts)
}

fn design_reference_prompt_entry_to_message(
    entry: &SystemPromptEntry,
    system_role: &str,
    avatar_image: Option<&str>,
    reference_images: &[String],
) -> Option<Value> {
    if let Some(content) =
        build_design_reference_prompt_content_with_images(entry, avatar_image, reference_images)
    {
        return Some(json!({ "role": "user", "content": content }));
    }

    let contains_legacy_image_tokens = entry.content.contains(DESIGN_REFERENCE_AVATAR_TOKEN)
        || entry.content.contains(DESIGN_REFERENCE_REFERENCES_TOKEN);
    if entry.prompt_entry_payload.is_some()
        || (contains_legacy_image_tokens
            && matches!(entry.role, super::types::PromptEntryRole::User))
    {
        return None;
    }

    let role = match entry.role {
        super::types::PromptEntryRole::System => system_role,
        super::types::PromptEntryRole::User => "user",
        super::types::PromptEntryRole::Assistant => "assistant",
    };

    let trimmed = entry.content.trim();
    if trimmed.is_empty() {
        return None;
    }

    Some(json!({ "role": role, "content": trimmed }))
}

fn load_scene_generation_prompt_entries(app: &AppHandle) -> (Vec<SystemPromptEntry>, bool) {
    match prompts::get_template(app, prompts::APP_SCENE_GENERATION_TEMPLATE_ID) {
        Ok(Some(template)) => {
            if !template.entries.is_empty() {
                (template.entries, template.condense_prompt_entries)
            } else if !template.content.trim().is_empty() {
                (
                    vec![SystemPromptEntry {
                        id: "scene_gen_single_entry".to_string(),
                        name: "Scene Generation Prompt".to_string(),
                        role: super::types::PromptEntryRole::System,
                        content: template.content,
                        enabled: true,
                        injection_position: PromptEntryPosition::Relative,
                        injection_depth: 0,
                        conditional_min_messages: None,
                        interval_turns: None,
                        system_prompt: true,
                        conditions: None,
                        prompt_entry_payload: None,
                    }],
                    template.condense_prompt_entries,
                )
            } else {
                (
                    get_base_prompt_entries(PromptType::SceneGenerationPrompt),
                    false,
                )
            }
        }
        _ => (
            get_base_prompt_entries(PromptType::SceneGenerationPrompt),
            false,
        ),
    }
}

fn render_scene_generation_prompt_entries(
    app: &AppHandle,
    settings: &Settings,
    model: &Model,
    session: &Session,
    character: &Character,
    persona: Option<&Persona>,
    recent_messages_text: &str,
    reference_images: &SceneReferenceImages,
) -> Vec<SystemPromptEntry> {
    let (template_entries, condense_prompt_entries) = load_scene_generation_prompt_entries(app);
    let mut rendered_entries = Vec::new();
    let has_scene = session.selected_scene_id.is_some() || character.default_scene_id.is_some();
    let has_scene_direction = if let Some(scene_id) = session
        .selected_scene_id
        .as_ref()
        .or(character.default_scene_id.as_ref())
    {
        character
            .scenes
            .iter()
            .find(|scene| &scene.id == scene_id)
            .and_then(|scene| scene.direction.as_deref())
            .map(|direction| !direction.trim().is_empty())
            .unwrap_or(false)
    } else {
        false
    };
    let has_character_reference_text = !build_scene_prompt_reference_text(
        character.name.as_str(),
        character.design_description.as_deref(),
        reference_images.character_reference_count,
        reference_images.character_reference_source,
    )
    .trim()
    .is_empty();
    let has_persona_reference_text = if let Some(persona) = persona {
        !build_scene_prompt_reference_text(
            &persona_scene_name(Some(persona)),
            persona.design_description.as_deref(),
            reference_images.persona_reference_count,
            reference_images.persona_reference_source,
        )
        .trim()
        .is_empty()
    } else {
        false
    };
    let condition_context = PromptEntryConditionContext {
        chat_mode: PromptEntryChatMode::Direct,
        scene_generation_enabled: scene_generation_enabled(settings),
        avatar_generation_enabled: avatar_generation_enabled(settings),
        has_scene,
        has_scene_direction,
        has_persona: persona.is_some(),
        message_count: session.messages.len(),
        participant_count: 2,
        recent_text: recent_messages_text,
        dynamic_memory_enabled: false,
        has_memory_summary: false,
        has_key_memories: false,
        has_lorebook_content: false,
        has_subject_description: false,
        has_current_description: false,
        has_character_reference_images: !reference_images.character_images.is_empty(),
        has_chat_background: !reference_images.chat_background_images.is_empty(),
        has_persona_reference_images: !reference_images.persona_images.is_empty(),
        has_character_reference_text,
        has_persona_reference_text,
        input_scopes: &model.input_scopes,
        output_scopes: &model.output_scopes,
        provider_id: Some(model.provider_id.as_str()),
        reasoning_enabled: model
            .advanced_model_settings
            .as_ref()
            .and_then(|cfg| cfg.reasoning_enabled)
            .unwrap_or(false),
        vision_enabled: model_supports_vision(model),
    };

    for entry in template_entries {
        if !entry_is_active(&entry, &condition_context) {
            continue;
        }
        let rendered = render_scene_generation_prompt_content(
            &entry.content,
            character,
            persona,
            recent_messages_text,
            !reference_images.chat_background_images.is_empty(),
        );
        if rendered.trim().is_empty() && entry.prompt_entry_payload.is_none() {
            continue;
        }
        let mut next_entry = entry.clone();
        next_entry.content = rendered;
        rendered_entries.push(next_entry);
    }

    if condense_prompt_entries {
        condense_scene_generation_entries(rendered_entries)
    } else {
        rendered_entries
    }
}

fn scene_prompt_entry_to_message(
    entry: &SystemPromptEntry,
    system_role: &str,
    reference_images: &SceneReferenceImages,
    character: &Character,
    persona: Option<&Persona>,
) -> Option<Value> {
    if let Some(content) = build_scene_prompt_content_with_images(entry, reference_images) {
        return Some(json!({ "role": "user", "content": content }));
    }

    let contains_legacy_image_tokens = entry.content.contains(SCENE_CHARACTER_TOKEN)
        || entry.content.contains(SCENE_PERSONA_TOKEN)
        || entry.content.contains(SCENE_CHAT_BACKGROUND_TOKEN)
        || entry.content.contains(SCENE_CHAT_BACKGROUND_LEGACY_TOKEN);
    if entry.prompt_entry_payload.is_some()
        || (contains_legacy_image_tokens
            && matches!(entry.role, super::types::PromptEntryRole::User))
    {
        return None;
    }

    let role = match entry.role {
        super::types::PromptEntryRole::System => system_role,
        super::types::PromptEntryRole::User => "user",
        super::types::PromptEntryRole::Assistant => "assistant",
    };

    let content = if role == system_role {
        Value::String(content_with_scene_image_hints(
            entry,
            reference_images,
            character,
            persona,
        ))
    } else {
        Value::String(entry.content.clone())
    };

    Some(json!({ "role": role, "content": content }))
}

fn content_with_scene_image_hints(
    entry: &SystemPromptEntry,
    reference_images: &SceneReferenceImages,
    character: &Character,
    persona: Option<&Persona>,
) -> String {
    let character_hint = build_scene_prompt_reference_hint(
        character.name.as_str(),
        reference_images.character_reference_count,
        reference_images.character_reference_source,
    );
    let persona_hint = build_scene_prompt_reference_hint(
        &persona_scene_name(persona),
        reference_images.persona_reference_count,
        reference_images.persona_reference_source,
    );
    let chat_background_hint = if reference_images.chat_background_images.is_empty() {
        String::new()
    } else {
        build_chat_background_reference_hint().to_string()
    };

    match entry
        .prompt_entry_payload
        .as_ref()
        .and_then(|payload| match payload {
            PromptEntryPayload::ImageSlot { slot } => Some(slot.clone()),
        }) {
        Some(PromptEntryImageSlot::Character) => {
            let replaced = entry
                .content
                .replace(SCENE_CHARACTER_TOKEN, &character_hint);
            if replaced == entry.content && !character_hint.trim().is_empty() {
                if entry.content.trim().is_empty() {
                    character_hint
                } else {
                    format!("{}\n{}", entry.content.trim(), character_hint)
                }
            } else {
                replaced
            }
        }
        Some(PromptEntryImageSlot::Persona) => {
            let replaced = entry.content.replace(SCENE_PERSONA_TOKEN, &persona_hint);
            if replaced == entry.content && !persona_hint.trim().is_empty() {
                if entry.content.trim().is_empty() {
                    persona_hint
                } else {
                    format!("{}\n{}", entry.content.trim(), persona_hint)
                }
            } else {
                replaced
            }
        }
        Some(PromptEntryImageSlot::ChatBackground) => {
            let replaced = entry
                .content
                .replace(SCENE_CHAT_BACKGROUND_TOKEN, &chat_background_hint)
                .replace(SCENE_CHAT_BACKGROUND_LEGACY_TOKEN, &chat_background_hint);
            if replaced == entry.content && !chat_background_hint.trim().is_empty() {
                if entry.content.trim().is_empty() {
                    chat_background_hint
                } else {
                    format!("{}\n{}", entry.content.trim(), chat_background_hint)
                }
            } else {
                replaced
            }
        }
        _ => entry
            .content
            .replace(SCENE_CHARACTER_TOKEN, &character_hint)
            .replace(SCENE_PERSONA_TOKEN, &persona_hint)
            .replace(SCENE_CHAT_BACKGROUND_TOKEN, &chat_background_hint)
            .replace(SCENE_CHAT_BACKGROUND_LEGACY_TOKEN, &chat_background_hint),
    }
}

fn insert_scene_in_chat_prompt_entries(
    messages: &mut Vec<Value>,
    system_role: &str,
    entries: &[SystemPromptEntry],
    reference_images: &SceneReferenceImages,
    character: &Character,
    persona: Option<&Persona>,
) {
    if entries.is_empty() {
        return;
    }
    let base_len = messages.len();
    let turn_count = base_len;
    let mut inserts: Vec<(usize, usize, &SystemPromptEntry)> = entries
        .iter()
        .enumerate()
        .filter_map(|(idx, entry)| {
            if !should_insert_in_chat_prompt_entry(entry, turn_count) {
                return None;
            }
            let depth = entry.injection_depth as usize;
            let pos = base_len.saturating_sub(depth);
            Some((pos, idx, entry))
        })
        .collect();
    inserts.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    for (offset, (pos, _, entry)) in inserts.into_iter().enumerate() {
        let insert_at = (pos + offset).min(messages.len());
        if let Some(message) =
            scene_prompt_entry_to_message(entry, system_role, reference_images, character, persona)
        {
            messages.insert(insert_at, message);
        }
    }
}

pub async fn chat_generate_scene_image(
    app: AppHandle,
    args: ChatGenerateSceneImageArgs,
) -> Result<StoredMessage, String> {
    let ChatGenerateSceneImageArgs {
        session_id,
        message_id,
        attachment_id,
        scene_prompt,
    } = args;

    if scene_prompt.trim().is_empty() {
        return Err("scenePrompt cannot be empty".to_string());
    }

    let mut session = super::storage::load_session(&app, &session_id)?
        .ok_or_else(|| "Session not found".to_string())?;

    let target_index = session
        .messages
        .iter()
        .position(|message| message.id == message_id)
        .ok_or_else(|| "Message not found in loaded session window".to_string())?;

    let settings = super::storage::load_settings(&app)?;
    if !scene_generation_enabled(&settings) {
        return Err("Scene generation is disabled in settings".to_string());
    }
    let (model, credential) = resolve_image_generation_target(
        &settings,
        settings
            .advanced_settings
            .as_ref()
            .and_then(|advanced| advanced.scene_generation_model_id.as_deref()),
    )?;

    let characters = super::storage::load_characters(&app)?;
    let character = characters
        .iter()
        .find(|value| value.id == session.character_id)
        .ok_or_else(|| "Session character not found".to_string())?;

    let personas = super::storage::load_personas(&app)?;
    let persona = if session.persona_disabled {
        None
    } else {
        session
            .persona_id
            .as_deref()
            .and_then(|persona_id| personas.iter().find(|value| value.id == persona_id))
    };

    let reference_images = build_scene_reference_images(&app, character, persona);
    let request = build_scene_generation_request(
        &scene_prompt,
        model,
        credential,
        character,
        persona,
        reference_images,
    );
    let response = generate_scene_image_with_retry(&app, request, 3).await?;
    let generated = response
        .images
        .into_iter()
        .next()
        .ok_or_else(|| "No images found in response".to_string())?;
    let generated_data = storage_read_image_data(&app, &generated.asset_id)?;

    let persisted_attachments = persist_attachments(
        &app,
        &session.character_id,
        &session.id,
        &message_id,
        "assistant",
        vec![ImageAttachment {
            id: attachment_id.clone(),
            data: generated_data,
            mime_type: generated.mime_type,
            filename: Some(scene_prompt),
            width: generated.width,
            height: generated.height,
            storage_path: None,
        }],
    )?;

    let persisted_attachment = persisted_attachments
        .into_iter()
        .next()
        .ok_or_else(|| "Failed to persist generated scene attachment".to_string())?;
    let cleanup_attachment = persisted_attachment.clone();

    let updated_message = {
        let target = &mut session.messages[target_index];
        if let Some(existing) = target
            .attachments
            .iter_mut()
            .find(|attachment| attachment.id == attachment_id)
        {
            *existing = persisted_attachment;
        } else {
            target.attachments.push(persisted_attachment);
        }
        target.clone()
    };

    session.updated_at = now_millis()?;

    let mut meta = session.clone();
    meta.messages = Vec::new();
    if let Err(err) = session_upsert_meta_typed(&app, &meta) {
        cleanup_attachments(
            &app,
            std::slice::from_ref(&cleanup_attachment),
            "chat_generate_scene_image",
        );
        return Err(err);
    }

    if let Err(err) =
        messages_upsert_batch_typed(&app, &session_id, std::slice::from_ref(&updated_message))
    {
        cleanup_attachments(
            &app,
            std::slice::from_ref(&cleanup_attachment),
            "chat_generate_scene_image",
        );
        return Err(err);
    }

    Ok(updated_message)
}

pub async fn chat_generate_scene_prompt(
    app: AppHandle,
    args: ChatGenerateScenePromptArgs,
) -> Result<String, String> {
    let ChatGenerateScenePromptArgs {
        session_id,
        message_id,
    } = args;

    let context = ChatContext::initialize(app.clone())?;
    let settings = &context.settings;
    if !scene_generation_enabled(settings) {
        return Err("Scene generation is disabled in settings".to_string());
    }
    let session = context
        .load_session(&session_id)?
        .ok_or_else(|| "Session not found".to_string())?;
    let character = context.find_character(&session.character_id)?;
    let persona = context.choose_persona(resolve_persona_id(&session, None));
    let (model, credential) = resolve_scene_writer_target(
        settings,
        settings
            .advanced_settings
            .as_ref()
            .and_then(|advanced| advanced.scene_writer_model_id.as_deref()),
    )?;
    let api_key = require_api_key(&app, credential, "scene_prompt")?;

    let recent_messages_text = build_scene_prompt_context_messages(&session, &message_id)?;
    let reference_images = build_scene_reference_images(&app, &character, persona);
    let prompt_entries = render_scene_generation_prompt_entries(
        &app,
        settings,
        model,
        &session,
        &character,
        persona,
        &recent_messages_text,
        &reference_images,
    );
    if prompt_entries.is_empty() {
        return Err("Scene generation prompt template rendered no usable entries".to_string());
    }

    let (relative_entries, in_chat_entries) = partition_prompt_entries(prompt_entries);
    let mut messages_for_api: Vec<Value> = relative_entries
        .iter()
        .filter_map(|entry| {
            scene_prompt_entry_to_message(entry, "system", &reference_images, &character, persona)
        })
        .collect();
    insert_scene_in_chat_prompt_entries(
        &mut messages_for_api,
        "system",
        &in_chat_entries,
        &reference_images,
        &character,
        persona,
    );

    let (request_settings, extra_body_fields) = prepare_sampling_request(
        &credential.provider_id,
        &session,
        model,
        settings,
        1280,
        0.7,
        1.0,
        None,
        None,
        None,
    );

    let built = super::request_builder::build_chat_request(
        credential,
        &api_key,
        &model.name,
        &messages_for_api,
        None,
        request_settings.temperature,
        request_settings.top_p,
        request_settings.max_tokens,
        request_settings.context_length,
        false,
        None,
        None,
        None,
        None,
        None,
        request_settings.reasoning_enabled,
        request_settings.reasoning_effort.clone(),
        request_settings.reasoning_budget,
        request_settings.prompt_caching_enabled.unwrap_or(false),
        extra_body_fields,
    );

    let api_request_payload = ApiRequest {
        url: built.url,
        method: Some("POST".into()),
        headers: Some(built.headers),
        query: None,
        body: Some(built.body),
        timeout_ms: Some(crate::transport::DEFAULT_REQUEST_TIMEOUT_MS),
        stream: Some(false),
        request_id: None,
        provider_id: Some(credential.provider_id.clone()),
    };

    let api_response = api_request(app.clone(), api_request_payload).await?;
    if !api_response.ok {
        return Err(format!(
            "API request failed with status {}",
            api_response.status
        ));
    }

    let generated_text = extract_text(&api_response.data, Some(&credential.provider_id))
        .ok_or_else(|| "Failed to extract text from response".to_string())?;

    let cleaned = condense_prompt_whitespace(
        generated_text
            .trim()
            .trim_matches('"')
            .trim()
            .trim_start_matches("<img>")
            .trim_end_matches("</img>")
            .trim_end_matches("[CONTINUE]")
            .trim_end_matches("[continue]")
            .trim_end_matches("[/continue]")
            .trim()
            .to_string(),
    );

    let usage = super::sse::usage_from_value(&api_response.data);
    super::service::record_usage_if_available(
        &context,
        &usage,
        &session,
        &character,
        model,
        credential,
        &api_key,
        now_millis().unwrap_or(0),
        UsageOperationType::ReplyHelper,
        "scene_prompt",
    )
    .await;

    if cleaned.is_empty() {
        return Err("Scene prompt generation returned an empty result".to_string());
    }

    Ok(cleaned)
}

pub async fn chat_generate_design_reference_description(
    app: AppHandle,
    args: ChatGenerateDesignReferenceDescriptionArgs,
) -> Result<String, String> {
    let ChatGenerateDesignReferenceDescriptionArgs {
        subject_name,
        subject_description,
        current_description,
        avatar_image,
        reference_images,
        request_id,
        stream,
    } = args;

    let context = ChatContext::initialize(app.clone())?;
    let settings = &context.settings;
    let (model, credential) = resolve_scene_writer_target(
        settings,
        settings
            .advanced_settings
            .as_ref()
            .and_then(|advanced| advanced.scene_writer_model_id.as_deref()),
    )?;
    let api_key = require_api_key(&app, credential, "design_reference_writer")?;

    let subject_name = subject_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("Unnamed subject");
    let subject_description = subject_description
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let current_description = current_description
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let avatar_input = avatar_image
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
    let reference_inputs = reference_images
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();

    if avatar_input.is_none() && reference_inputs.is_empty() {
        return Err("At least one avatar or reference image is required".to_string());
    }

    let system_role = adapter_for(credential).system_role().into_owned();
    let streaming_enabled =
        super::request_builder::effective_streaming_enabled(credential, stream.unwrap_or(true));
    let prompt_entries = render_design_reference_prompt_entries(
        &app,
        settings,
        model,
        subject_name,
        subject_description,
        current_description,
    );
    let (relative_entries, in_chat_entries) = partition_prompt_entries(prompt_entries);
    let messages_for_api = relative_entries
        .into_iter()
        .chain(in_chat_entries.into_iter())
        .filter_map(|entry| {
            design_reference_prompt_entry_to_message(
                &entry,
                &system_role,
                avatar_input.as_deref(),
                &reference_inputs,
            )
        })
        .collect::<Vec<_>>();

    if messages_for_api.is_empty() {
        return Err("Design reference template rendered no prompt content".to_string());
    }

    let preview_session = Session {
        id: "scene-writer-preview".to_string(),
        character_id: String::new(),
        title: "Scene writer preview".to_string(),
        background_image_path: None,
        system_prompt: None,
        selected_scene_id: None,
        prompt_template_id: None,
        persona_id: None,
        persona_disabled: false,
        voice_autoplay: None,
        advanced_model_settings: None,
        memories: Vec::new(),
        memory_embeddings: Vec::new(),
        memory_summary: None,
        memory_summary_token_count: 0,
        memory_tool_events: Vec::new(),
        memory_status: None,
        memory_error: None,
        memory_progress_step: None,
        messages: Vec::new(),
        archived: false,
        created_at: 0,
        updated_at: 0,
    };

    let (request_settings, extra_body_fields) = prepare_default_sampling_request(
        &credential.provider_id,
        &preview_session,
        model,
        settings,
        0.4,
        1.0,
        None,
        None,
        None,
    );

    let built = super::request_builder::build_chat_request(
        credential,
        &api_key,
        &model.name,
        &messages_for_api,
        None,
        request_settings.temperature,
        request_settings.top_p,
        request_settings.max_tokens,
        request_settings.context_length,
        streaming_enabled,
        request_id.clone(),
        None,
        None,
        None,
        None,
        request_settings.reasoning_enabled,
        request_settings.reasoning_effort.clone(),
        request_settings.reasoning_budget,
        request_settings.prompt_caching_enabled.unwrap_or(false),
        extra_body_fields,
    );

    let api_request_payload = ApiRequest {
        url: built.url,
        method: Some("POST".into()),
        headers: Some(built.headers),
        query: None,
        body: Some(built.body),
        timeout_ms: Some(crate::transport::DEFAULT_REQUEST_TIMEOUT_MS),
        stream: Some(built.stream),
        request_id: built.request_id.clone(),
        provider_id: Some(credential.provider_id.clone()),
    };

    let api_response = api_request(app.clone(), api_request_payload).await?;
    if !api_response.ok {
        return Err(format!(
            "API request failed with status {}",
            api_response.status
        ));
    }

    let generated_text = extract_text(&api_response.data, Some(&credential.provider_id))
        .ok_or_else(|| "Failed to extract text from response".to_string())?;

    let cleaned = condense_prompt_whitespace(
        generated_text
            .trim()
            .trim_matches('"')
            .trim()
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim()
            .to_string(),
    );

    if cleaned.is_empty() {
        return Err("Design reference generation returned an empty result".to_string());
    }

    Ok(cleaned)
}
