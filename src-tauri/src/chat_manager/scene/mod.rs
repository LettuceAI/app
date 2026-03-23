use serde_json::{json, Value};
use tauri::AppHandle;

use crate::api::{api_request, ApiRequest};
use crate::chat_manager::attachments::{cleanup_attachments, persist_attachments};
use crate::chat_manager::execution::{find_model_with_credential, prepare_sampling_request};
use crate::chat_manager::messages::build_multimodal_content;
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
    ChatGenerateScenePromptArgs, ImageAttachment, Model, Persona, PromptEntryPosition,
    ProviderCredential, Session, Settings, StoredMessage, SystemPromptEntry,
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

fn build_scene_reference_images(
    app: &AppHandle,
    character: &Character,
    persona: Option<&Persona>,
) -> SceneReferenceImages {
    let character_design_images =
        resolve_design_reference_images(app, &character.design_reference_image_ids);
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

fn build_scene_prompt_content_with_images(
    content: &str,
    reference_images: &SceneReferenceImages,
) -> Option<Value> {
    const CHARACTER_TOKEN: &str = "{{image[character]}}";
    const PERSONA_TOKEN: &str = "{{image[persona]}}";

    if !content.contains(CHARACTER_TOKEN) && !content.contains(PERSONA_TOKEN) {
        return None;
    }

    let mut parts: Vec<Value> = Vec::new();
    if content.contains(CHARACTER_TOKEN) {
        parts.extend(build_scene_prompt_image_parts(
            &reference_images.character_images,
        ));
    }
    if content.contains(PERSONA_TOKEN) {
        parts.extend(build_scene_prompt_image_parts(
            &reference_images.persona_images,
        ));
    }

    if parts.is_empty() {
        None
    } else {
        Some(Value::Array(parts))
    }
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
        persona_images,
        persona_reference_count,
        persona_reference_source,
    } = reference_images;
    let mut prompt_sections = Vec::new();
    let mut input_images = character_images.clone();
    input_images.extend(persona_images.clone());
    let has_character_reference = character_reference_count > 0;
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

    if has_character_reference || has_persona_reference {
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
        input_images: if input_images.is_empty() {
            None
        } else {
            Some(input_images)
        },
        size: Some("1024x1024".to_string()),
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

    condense_prompt_whitespace(prompt)
}

fn condense_scene_generation_entries(entries: Vec<SystemPromptEntry>) -> Vec<SystemPromptEntry> {
    let merged = entries
        .into_iter()
        .filter_map(|entry| {
            let trimmed = entry.content.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    if merged.trim().is_empty() {
        return Vec::new();
    }

    vec![SystemPromptEntry {
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
    }]
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
    character: &Character,
    persona: Option<&Persona>,
    recent_messages_text: &str,
) -> Vec<SystemPromptEntry> {
    let (template_entries, condense_prompt_entries) = load_scene_generation_prompt_entries(app);
    let mut rendered_entries = Vec::new();

    for entry in template_entries {
        if !entry.enabled && !entry.system_prompt {
            continue;
        }
        let rendered = render_scene_generation_prompt_content(
            &entry.content,
            character,
            persona,
            recent_messages_text,
        );
        if rendered.trim().is_empty() {
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
    if let Some(content) = build_scene_prompt_content_with_images(&entry.content, reference_images)
    {
        return Some(json!({ "role": "user", "content": content }));
    }

    let role = match entry.role {
        super::types::PromptEntryRole::System => system_role,
        super::types::PromptEntryRole::User => "user",
        super::types::PromptEntryRole::Assistant => "assistant",
    };

    let content = if role == system_role {
        Value::String(content_with_scene_image_hints(
            &entry.content,
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
    content: &str,
    reference_images: &SceneReferenceImages,
    character: &Character,
    persona: Option<&Persona>,
) -> String {
    content
        .replace(
            "{{image[character]}}",
            &build_scene_prompt_reference_hint(
                character.name.as_str(),
                reference_images.character_reference_count,
                reference_images.character_reference_source,
            ),
        )
        .replace(
            "{{image[persona]}}",
            &build_scene_prompt_reference_hint(
                &persona_scene_name(persona),
                reference_images.persona_reference_count,
                reference_images.persona_reference_source,
            ),
        )
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
    let prompt_entries =
        render_scene_generation_prompt_entries(&app, &character, persona, &recent_messages_text);
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
        extra_body_fields,
    );

    let api_request_payload = ApiRequest {
        url: built.url,
        method: Some("POST".into()),
        headers: Some(built.headers),
        query: None,
        body: Some(built.body),
        timeout_ms: Some(60_000),
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

    let mut image_inputs = Vec::new();
    if let Some(image) = avatar_image
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        image_inputs.push(image.to_string());
    }
    image_inputs.extend(
        reference_images
            .into_iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
    );

    if image_inputs.is_empty() {
        return Err("At least one avatar or reference image is required".to_string());
    }

    let system_role = adapter_for(credential).system_role().into_owned();
    let streaming_enabled = stream.unwrap_or(true) && adapter_for(credential).supports_stream();
    let system_prompt = [
        "Write one concise canonical visual reference description for future scene generation.",
        "Focus only on stable appearance: face visibility, hair, build, age presentation, outfit silhouette, materials, colors, accessories, and art direction.",
        "Do not write backstory, personality, scene action, camera directions, or poetic prose.",
        "If the subject's face is hidden or obscured, say so clearly.",
        "If the references disagree, keep only features supported by the majority of images.",
        "Output only the final description.",
    ]
    .join("\n");

    let mut request_sections = vec![
        format!("Subject: {}", subject_name),
        "Use the attached avatar and reference images to write a stable design note for future image prompts."
            .to_string(),
    ];

    if let Some(description) = subject_description {
        request_sections.push(format!("Character context:\n{}", description));
    }
    if let Some(existing) = current_description {
        request_sections.push(format!(
            "Current notes to refine if they match the images:\n{}",
            existing
        ));
    }

    let image_attachments = image_inputs
        .iter()
        .enumerate()
        .map(|(index, image)| ImageAttachment {
            id: format!("design-ref-{}", index),
            data: image.clone(),
            mime_type: "image/*".to_string(),
            filename: None,
            width: None,
            height: None,
            storage_path: None,
        })
        .collect::<Vec<_>>();

    let messages_for_api = vec![
        json!({ "role": system_role, "content": system_prompt }),
        json!({
            "role": "user",
            "content": build_multimodal_content(&request_sections.join("\n\n"), &image_attachments),
        }),
    ];

    let preview_session = Session {
        id: "scene-writer-preview".to_string(),
        character_id: String::new(),
        title: "Scene writer preview".to_string(),
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
        messages: Vec::new(),
        archived: false,
        created_at: 0,
        updated_at: 0,
    };

    let (request_settings, extra_body_fields) = prepare_sampling_request(
        &credential.provider_id,
        &preview_session,
        model,
        settings,
        768,
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
        extra_body_fields,
    );

    let api_request_payload = ApiRequest {
        url: built.url,
        method: Some("POST".into()),
        headers: Some(built.headers),
        query: None,
        body: Some(built.body),
        timeout_ms: Some(60_000),
        stream: Some(streaming_enabled),
        request_id: request_id.clone(),
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
