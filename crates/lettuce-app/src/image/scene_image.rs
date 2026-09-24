//! Scene images for a direct chat message (legacy `chat_generate_scene_image`
//! and `build_scene_generation_request`): the request is built from the
//! character, persona and background references, run as an image job with
//! legacy's three attempts on a missing image, and the first image is added to
//! the message.

use std::time::Duration;

use lettuce_characters::{
    CharacterMediaSlot, CharacterRepository, PersonaMediaSlot, PersonaRepository, SceneAssetSlot,
};
use lettuce_context::PromptVariable;
use lettuce_conversations::{
    ConversationBackground, ConversationKind, ConversationReader, ConversationRepository,
    EditMessage, EditMessageResult, MediaAssetRole, MessageEditDraft, MessagePart,
    MessageRenderSource, OperationToken, TimelineItem,
};
use lettuce_image_generation::sd_runtime::lora_library::LoraLibraryRepository;
use lettuce_image_generation::{
    ImageAttribution, ImageGenerationRepository, ImageGenerationRequest, ImageGenerationSource,
    ImageGenerationState, ImageMedia, ImageOutputPolicy, ImageProviderPort,
};
use lettuce_jobs::{CancellationReason, JobStore, ResourceAvailability, WorkerId};
use lettuce_models::{
    ModelCatalog, ModelProfileRepository, ProviderAccountRepository, StableDiffusionLora,
};
use lettuce_types::{
    AssetId, ContentHash, ConversationId, MessageId, PageLimit, PageRequest, RequestId,
    TimestampMillis,
};
use lettuce_usage::JobUsageLedger;

use crate::generation::runtime_text::RuntimeText;
use crate::{BuiltInPromptId, ImageFeature, ImageFeatureModelError, image_feature_model};

const MAX_ATTEMPTS: u32 = 3;
const MAX_INPUT_IMAGES: usize = 16;
const DEFAULT_SIZE: &str = "1024x1024";
const NO_IMAGES: &str = "No images found in response";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneImageRequest {
    pub conversation_id: ConversationId,
    pub message_id: MessageId,
    pub scene_prompt: String,
    /// Identifies this generation; each attempt's image request and the
    /// message edit derive their ids from it, so a replay repeats them.
    pub request_id: RequestId,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SceneImageError {
    #[error("scenePrompt cannot be empty")]
    EmptyPrompt,
    #[error("Session not found")]
    ConversationNotFound,
    #[error("Scene images are generated for direct chats only")]
    NotDirect,
    #[error("Message not found in loaded session window")]
    MessageNotFound,
    #[error("Session character not found")]
    CharacterNotFound,
    #[error(transparent)]
    Model(#[from] ImageFeatureModelError),
    #[error("{0}")]
    Generation(String),
    #[error("The message is generating or was changed, so the scene image could not be added")]
    MessageUnavailable,
    #[error("scene image storage failed")]
    Storage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReferenceSource {
    Design,
    Avatar,
}

/// A depicted subject of a remote scene image: its name, design notes and
/// reference images (design references, else its avatar).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SceneSubject {
    pub(crate) name: String,
    pub(crate) design_notes: Option<String>,
    pub(crate) references: Vec<AssetId>,
    pub(crate) source: Option<ReferenceSource>,
    /// What the stored media alone suggest, before unreadable images are
    /// skipped: legacy's writer hints counted the stored design ids and the
    /// avatar path.
    pub(crate) stored_design_count: usize,
    pub(crate) stored_source: Option<ReferenceSource>,
}

impl SceneSubject {
    fn new(
        name: String,
        design_notes: Option<&str>,
        design: Vec<AssetId>,
        avatar: Option<AssetId>,
    ) -> Self {
        let (references, source) = if !design.is_empty() {
            (design, Some(ReferenceSource::Design))
        } else if let Some(avatar) = avatar {
            (vec![avatar], Some(ReferenceSource::Avatar))
        } else {
            (Vec::new(), None)
        };
        Self {
            name,
            design_notes: design_notes
                .map(str::trim)
                .filter(|notes| !notes.is_empty())
                .map(str::to_owned),
            stored_design_count: if source == Some(ReferenceSource::Design) {
                references.len()
            } else {
                0
            },
            stored_source: source,
            references,
            source,
        }
    }
}

/// The prompt and input images of a remote scene image, in legacy's order:
/// character references, the chat background, persona references. Past the
/// request's input bound, persona references are dropped first.
fn remote_scene_prompt(
    text: &RuntimeText,
    scene_prompt: &str,
    character: &SceneSubject,
    background: Option<AssetId>,
    persona: Option<&SceneSubject>,
    persona_fallback_name: &str,
) -> (String, Vec<AssetId>) {
    let mut character_refs = character.references.clone();
    let mut persona_refs = persona
        .map(|persona| persona.references.clone())
        .unwrap_or_default();
    let background_count = usize::from(background.is_some());
    while character_refs.len() + background_count + persona_refs.len() > MAX_INPUT_IMAGES {
        if persona_refs.pop().is_none() {
            character_refs.pop();
        }
    }
    let render = |key: &str, variables: Vec<(PromptVariable, String)>| {
        text.render_with(key, variables).unwrap_or_default()
    };
    let range = |start: usize, count: usize| {
        if count <= 1 {
            render(
                "scene_image_reference_single",
                vec![(PromptVariable::ItemNumber, start.to_string())],
            )
        } else {
            render(
                "scene_image_reference_range",
                vec![
                    (PromptVariable::ItemNumber, start.to_string()),
                    (PromptVariable::RangeEnd, (start + count - 1).to_string()),
                ],
            )
        }
    };
    let persona_name = persona.map_or(persona_fallback_name, |persona| persona.name.as_str());
    let mut sections = Vec::new();
    if let Some(notes) = &character.design_notes {
        sections.push(render(
            "scene_image_character_design_notes",
            vec![
                (PromptVariable::SubjectName, character.name.clone()),
                (PromptVariable::SubjectDescription, notes.clone()),
            ],
        ));
    }
    if let Some((persona, notes)) =
        persona.and_then(|persona| persona.design_notes.as_ref().map(|notes| (persona, notes)))
    {
        sections.push(render(
            "scene_image_persona_design_notes",
            vec![
                (PromptVariable::SubjectName, persona.name.clone()),
                (PromptVariable::SubjectDescription, notes.clone()),
            ],
        ));
    }
    let has_character = !character_refs.is_empty();
    let has_persona = !persona_refs.is_empty();
    if has_character || background.is_some() || has_persona {
        let mut lines = Vec::new();
        let mut next = 1;
        let subject_line = |start: usize,
                            count: usize,
                            source: Option<ReferenceSource>,
                            design_key: &str,
                            name: &str| {
            render(
                "scene_image_subject_reference",
                vec![
                    (PromptVariable::ReferenceRange, range(start, count)),
                    (
                        PromptVariable::ReferenceSource,
                        render(
                            match source {
                                Some(ReferenceSource::Avatar) => "scene_image_avatar_source",
                                _ => design_key,
                            },
                            Vec::new(),
                        ),
                    ),
                    (PromptVariable::SubjectName, name.to_owned()),
                ],
            )
        };
        if has_character {
            lines.push(subject_line(
                next,
                character_refs.len(),
                character.source,
                "scene_image_character_design_source",
                &character.name,
            ));
            next += character_refs.len();
        }
        if background.is_some() {
            lines.push(render(
                "scene_image_background_reference",
                vec![(PromptVariable::ReferenceRange, range(next, 1))],
            ));
            next += 1;
        }
        if has_persona {
            lines.push(subject_line(
                next,
                persona_refs.len(),
                persona.and_then(|persona| persona.source),
                "scene_image_persona_design_source",
                persona_name,
            ));
        }
        lines.push(render("scene_image_no_swap", Vec::new()));
        let only = |subject: &str, other: &str| {
            render(
                "scene_image_only_reference",
                vec![
                    (PromptVariable::SubjectName, subject.to_owned()),
                    (PromptVariable::OtherSubjectName, other.to_owned()),
                ],
            )
        };
        match (has_character, has_persona) {
            (true, false) => lines.push(only(&character.name, persona_name)),
            (false, true) => lines.push(only(persona_name, &character.name)),
            _ => {}
        }
        sections.push(
            lines
                .into_iter()
                .filter(|line| !line.is_empty())
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    sections.push(scene_prompt.trim().to_owned());
    let inputs = character_refs
        .into_iter()
        .chain(background)
        .chain(persona_refs)
        .collect();
    (
        sections
            .into_iter()
            .filter(|section| !section.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n"),
        inputs,
    )
}

/// Legacy `lora_applies_to_scene_prompt`: a LoRA without keywords always
/// applies, otherwise one of its keywords must appear in the prompt.
fn applies_to_prompt(lora: &StableDiffusionLora, prompt: &str) -> bool {
    let prompt = prompt.to_lowercase();
    lora.keywords.is_empty()
        || lora
            .keywords
            .iter()
            .map(|keyword| keyword.trim())
            .filter(|keyword| !keyword.is_empty())
            .any(|keyword| prompt.contains(&keyword.to_lowercase()))
}

/// Legacy `persona_scene_name`: the nickname, else the title.
fn persona_scene_name(persona: &lettuce_characters::Persona) -> String {
    persona
        .nickname
        .as_deref()
        .filter(|nickname| !nickname.trim().is_empty())
        .unwrap_or(&persona.title)
        .to_owned()
}

fn derived_id(root: RequestId, label: &str) -> uuid::Uuid {
    uuid::Uuid::new_v5(&root.as_uuid(), label.as_bytes())
}

/// Generates a scene image for `request.message_id` and adds it to the
/// message as an attachment. The message keeps its visible content: its
/// rendered revision or candidate is revised with the image appended.
pub async fn generate_scene_image<R, D, P>(
    repository: &R,
    media: &D,
    provider: &P,
    request: &SceneImageRequest,
    allowed: &ResourceAvailability,
    now: TimestampMillis,
) -> Result<EditMessageResult, SceneImageError>
where
    R: ConversationRepository
        + CharacterRepository
        + PersonaRepository
        + ModelCatalog
        + ModelProfileRepository
        + ProviderAccountRepository
        + lettuce_settings::GlobalSettingsStore
        + lettuce_context::PromptRepository
        + ImageGenerationRepository
        + JobUsageLedger
        + LoraLibraryRepository
        + JobStore
        + ?Sized,
    D: ImageMedia + ?Sized,
    P: ImageProviderPort + ?Sized,
{
    let scene_prompt = request.scene_prompt.trim();
    if scene_prompt.is_empty() {
        return Err(SceneImageError::EmptyPrompt);
    }
    let aggregate = ConversationReader::get(repository, request.conversation_id)
        .map_err(|_| SceneImageError::ConversationNotFound)?;
    let ConversationKind::Direct(details) = &aggregate.conversation.kind else {
        return Err(SceneImageError::NotDirect);
    };
    find_message(repository, &aggregate.conversation, request.message_id)?;
    let settings = lettuce_settings::GlobalSettingsStore::load(repository)
        .map_err(|_| SceneImageError::Storage)?
        .settings;
    let model = image_feature_model(repository, &settings, ImageFeature::Scene)?;
    let character = CharacterRepository::get(repository, details.character.source_id)
        .map_err(|_| SceneImageError::Storage)?
        .ok_or(SceneImageError::CharacterNotFound)?;
    let persona = lettuce_conversations::effective_persona(&aggregate.conversation)
        .map(|persona| PersonaRepository::get(repository, persona.source_id))
        .transpose()
        .map_err(|_| SceneImageError::Storage)?
        .flatten();
    let (prompt, input_images, loras) = if model.is_local_diffusion() {
        let (character_lora, persona_lora) = crate::image::scene_loras::subject_loras(
            repository,
            details.character.source_id,
            persona.as_ref().map(|persona| persona.id),
        );
        let loras = character_lora
            .into_iter()
            .chain(persona_lora.flatten())
            .filter(|lora| applies_to_prompt(lora, scene_prompt))
            .collect();
        (scene_prompt.to_owned(), Vec::new(), loras)
    } else {
        let text = RuntimeText::load(repository, BuiltInPromptId::ChatRuntime)
            .map_err(|_| SceneImageError::Storage)?;
        let references =
            StoredSceneReferences::new(&aggregate.conversation, &character, persona.as_ref())
                .resolve(Some(media));
        let fallback_name = text
            .render_with("scene_image_persona_fallback_name", [])
            .unwrap_or_default();
        let (prompt, inputs) = remote_scene_prompt(
            &text,
            scene_prompt,
            &references.character,
            references.background,
            references.persona.as_ref(),
            &fallback_name,
        );
        (prompt, inputs, Vec::new())
    };
    let size = model
        .profile
        .config
        .stable_diffusion
        .size
        .clone()
        .or_else(|| settings.image_generation.scene_default_size.clone())
        .or_else(|| Some(DEFAULT_SIZE.to_owned()));
    let coordinator = crate::ImageGenerationCoordinator::new(repository, repository);
    let mut asset = None;
    let mut last_error = NO_IMAGES.to_owned();
    for attempt in 1..=MAX_ATTEMPTS {
        let generation = ImageGenerationRequest {
            id: RequestId::from_uuid(derived_id(
                request.request_id,
                &format!("attempt-{attempt}"),
            )),
            model_profile_id: model.profile.id,
            prompt: prompt.clone(),
            settings: Default::default(),
            input_images: input_images.clone(),
            mask_image: None,
            loras: loras.clone(),
            size: size.clone(),
            quality: None,
            style: None,
            count: 1,
            source: ImageGenerationSource::Scene,
            attribution: ImageAttribution {
                conversation_id: Some(request.conversation_id),
                character_id: Some(details.character.source_id),
            },
            output_policy: ImageOutputPolicy::Retained,
            created_at: now,
        };
        let admitted = coordinator
            .admit(generation, repository)
            .map_err(|error| SceneImageError::Generation(error.to_string()))?;
        let record = match coordinator
            .claim(
                admitted.job.id,
                WorkerId::new(),
                now,
                Duration::from_secs(60 * 60),
                allowed,
            )
            .map_err(|error| SceneImageError::Generation(error.to_string()))?
        {
            Some(work) => match coordinator
                .run(
                    work,
                    repository,
                    media,
                    provider,
                    CancellationReason::User,
                    now,
                )
                .await
                .map_err(|error| SceneImageError::Generation(error.to_string()))?
            {
                crate::ImageGenerationRunResult::Succeeded { record, .. }
                | crate::ImageGenerationRunResult::Failed { record, .. }
                | crate::ImageGenerationRunResult::Cancelled { record, .. } => record,
            },
            None => ImageGenerationRepository::get(repository, admitted.job.id)
                .map_err(|_| SceneImageError::Storage)?,
        };
        match record.state {
            ImageGenerationState::Succeeded { result } if !result.images.is_empty() => {
                asset = Some(result.images[0].asset_id);
                break;
            }
            ImageGenerationState::Succeeded { .. } => last_error = NO_IMAGES.to_owned(),
            ImageGenerationState::Failed { message, .. }
                if message.to_ascii_lowercase().contains("no image") =>
            {
                last_error = message;
            }
            ImageGenerationState::Failed { message, .. } => {
                return Err(SceneImageError::Generation(message));
            }
            ImageGenerationState::Cancelled { .. } | ImageGenerationState::Pending => {
                return Err(SceneImageError::Generation(
                    "Image generation was interrupted.".to_owned(),
                ));
            }
        }
    }
    let asset = asset.ok_or(SceneImageError::Generation(last_error))?;
    attach_image(repository, request, asset, now)
}

/// The references of a remote scene image: each subject's readable design
/// references (else its readable avatar) and the readable chat background.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SceneReferences {
    pub(crate) character: SceneSubject,
    pub(crate) persona: Option<SceneSubject>,
    pub(crate) background: Option<AssetId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StoredSubject {
    name: String,
    notes: Option<String>,
    design: Vec<AssetId>,
    avatars: Vec<AssetId>,
}

impl StoredSubject {
    fn resolve<D: ImageMedia + ?Sized>(&self, media: Option<&D>) -> SceneSubject {
        let stored = SceneSubject::new(
            self.name.clone(),
            self.notes.as_deref(),
            self.design.clone(),
            self.avatars.first().copied(),
        );
        let readable = |assets: &[AssetId]| {
            media.map_or_else(Vec::new, |media| {
                assets
                    .iter()
                    .copied()
                    .filter(|asset| media.load_input(*asset).is_ok())
                    .collect::<Vec<_>>()
            })
        };
        SceneSubject {
            stored_design_count: stored.stored_design_count,
            stored_source: stored.stored_source,
            ..SceneSubject::new(
                self.name.clone(),
                self.notes.as_deref(),
                readable(&self.design),
                readable(&self.avatars).first().copied(),
            )
        }
    }
}

/// What a scene's subjects and chat background store, before any image is
/// read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StoredSceneReferences {
    character: StoredSubject,
    persona: Option<StoredSubject>,
    background: Option<AssetId>,
}

impl StoredSceneReferences {
    pub(crate) fn new(
        conversation: &lettuce_conversations::Conversation,
        character: &lettuce_characters::CharacterDetails,
        persona: Option<&lettuce_characters::Persona>,
    ) -> Self {
        let media_of = |slot: CharacterMediaSlot| {
            let mut links = character
                .character
                .media
                .links
                .iter()
                .filter(|link| link.slot == slot)
                .collect::<Vec<_>>();
            links.sort_by_key(|link| link.ordinal);
            links
                .into_iter()
                .map(|link| link.asset_id)
                .collect::<Vec<_>>()
        };
        let notes = |notes: Option<&str>| notes.map(str::to_owned);
        let persona = persona.map(|persona| {
            let mut design = persona
                .media
                .links
                .iter()
                .filter(|link| link.slot == PersonaMediaSlot::DesignReference)
                .collect::<Vec<_>>();
            design.sort_by_key(|link| link.ordinal);
            StoredSubject {
                name: persona_scene_name(persona),
                notes: notes(persona.design_description.as_deref()),
                design: design.into_iter().map(|link| link.asset_id).collect(),
                avatars: persona
                    .media
                    .links
                    .iter()
                    .filter(|link| link.slot == PersonaMediaSlot::Avatar)
                    .map(|link| link.asset_id)
                    .collect(),
            }
        });
        Self {
            character: StoredSubject {
                name: character.character.profile.name.clone(),
                notes: notes(character.character.profile.design_description.as_deref()),
                design: media_of(CharacterMediaSlot::DesignReference),
                avatars: media_of(CharacterMediaSlot::AvatarOriginal),
            },
            persona,
            background: conversation_background(conversation, character, &media_of),
        }
    }

    /// The references with only the images `media` can read; without media
    /// (a local image model) no image is sent, as legacy's local scenes had
    /// none.
    pub(crate) fn resolve<D: ImageMedia + ?Sized>(&self, media: Option<&D>) -> SceneReferences {
        SceneReferences {
            character: self.character.resolve(media),
            persona: self.persona.as_ref().map(|persona| persona.resolve(media)),
            background: media.and_then(|media| {
                self.background
                    .filter(|asset| media.load_input(*asset).is_ok())
            }),
        }
    }
}

/// Legacy's effective chat background: the conversation's own (none when it
/// hides it), else the selected or default scene's, else the character's.
fn conversation_background(
    conversation: &lettuce_conversations::Conversation,
    character: &lettuce_characters::CharacterDetails,
    media_of: &dyn Fn(CharacterMediaSlot) -> Vec<AssetId>,
) -> Option<AssetId> {
    match conversation
        .current_settings
        .as_ref()
        .and_then(|settings| settings.background)
    {
        Some(ConversationBackground::Image { asset_id }) => return Some(asset_id),
        Some(ConversationBackground::Hidden) => return None,
        None => {}
    }
    let scene_id = lettuce_conversations::resolve_effective_settings(conversation, None)
        .ok()
        .and_then(|settings| settings.scene.map(|scene| scene.source_id))
        .or(character.character.defaults.default_scene_id);
    scene_id
        .and_then(|scene_id| character.scenes.iter().find(|scene| scene.id == scene_id))
        .and_then(|scene| {
            scene
                .assets
                .iter()
                .find(|link| link.slot == SceneAssetSlot::Background)
                .map(|link| link.asset_id)
        })
        .or_else(|| media_of(CharacterMediaSlot::Background).first().copied())
}

fn find_message<R: ConversationReader + ?Sized>(
    repository: &R,
    conversation: &lettuce_conversations::Conversation,
    message_id: MessageId,
) -> Result<TimelineItem, SceneImageError> {
    let mut page = PageRequest {
        cursor: None,
        limit: PageLimit::new(200),
    };
    loop {
        let timeline = repository
            .timeline_page(conversation.id, conversation.active_branch_id, &page)
            .map_err(|_| SceneImageError::Storage)?;
        if let Some(item) = timeline
            .items
            .into_iter()
            .find(|item| item.message.id == message_id)
        {
            return Ok(item);
        }
        match timeline.next_cursor {
            Some(cursor) => page.cursor = Some(cursor),
            None => return Err(SceneImageError::MessageNotFound),
        }
    }
}

/// Appends the image to what the message shows now, read again after the
/// generation so an edit or candidate switch made meanwhile is kept.
fn attach_image<R: ConversationRepository + ?Sized>(
    repository: &R,
    request: &SceneImageRequest,
    asset: AssetId,
    now: TimestampMillis,
) -> Result<EditMessageResult, SceneImageError> {
    let conversation = ConversationReader::get(repository, request.conversation_id)
        .map_err(|_| SceneImageError::ConversationNotFound)?
        .conversation;
    let item = find_message(repository, &conversation, request.message_id)?;
    let parts = match item.message.active_render_source {
        MessageRenderSource::Candidate(_) => item
            .active_candidate
            .as_ref()
            .map(|candidate| candidate.parts.clone()),
        MessageRenderSource::Revision(_) => item
            .active_revision
            .as_ref()
            .map(|revision| revision.parts.clone()),
    }
    .ok_or(SceneImageError::MessageNotFound)?;
    let key = derived_id(request.request_id, "attach");
    repository
        .edit_message(
            &EditMessage {
                conversation_id: request.conversation_id,
                message_id: request.message_id,
                expected_revision: conversation.revision,
                operation: OperationToken {
                    key: lettuce_jobs::IdempotencyKey::new(format!("scene-image-{key}"))
                        .map_err(|_| SceneImageError::Storage)?,
                    request_digest: ContentHash::parse(
                        blake3::hash(format!("{}:{asset}", request.message_id).as_bytes())
                            .to_hex()
                            .to_string(),
                    )
                    .map_err(|_| SceneImageError::Storage)?,
                },
                draft: MessageEditDraft {
                    parts: parts
                        .into_iter()
                        .chain([MessagePart::MediaAsset {
                            asset_id: asset,
                            role: MediaAssetRole::Attachment,
                        }])
                        .collect(),
                    visibility: item.message.visibility,
                    pinned: item.message.pinned,
                    scene_edited: item.message.scene_edited,
                },
            },
            now,
        )
        .map_err(|error| match error {
            lettuce_conversations::ConversationRepositoryError::Conflict => {
                SceneImageError::MessageUnavailable
            }
            _ => SceneImageError::Storage,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn subject(
        name: &str,
        notes: Option<&str>,
        design: Vec<AssetId>,
        avatar: Option<AssetId>,
    ) -> SceneSubject {
        SceneSubject::new(name.to_owned(), notes, design, avatar)
    }

    #[test]
    fn remote_scene_prompts_match_legacy_build_scene_generation_request() {
        let text = RuntimeText::from_seed(BuiltInPromptId::ChatRuntime);
        let (first, second, background) = (AssetId::new(), AssetId::new(), AssetId::new());
        let mira = subject("Mira", Some(" red coat "), vec![first, second], None);
        let sol = subject("Sol", None, Vec::new(), None);
        let (prompt, inputs) = remote_scene_prompt(
            &text,
            " A harbor at dusk ",
            &mira,
            Some(background),
            Some(&sol),
            "the persona",
        );
        assert_eq!(
            prompt,
            "Character design notes for Mira:\nred coat\n\n\
             The attached images 1-2 is the saved character design reference for Mira. Use it only for Mira's identity, face, body, outfit cues, and signature styling.\n\
             The attached image 3 is the chat background environment reference. Use it for setting, palette, lighting mood, architecture, and large backdrop elements. Do not treat it as a character identity reference.\n\
             Do not swap, merge, or borrow identity-defining features between reference images.\n\
             Only Mira has a reference image attached. Do not invent Sol from Mira's appearance.\n\n\
             A harbor at dusk"
        );
        assert_eq!(inputs, vec![first, second, background]);

        let avatar = AssetId::new();
        let bare = subject("Mira", None, Vec::new(), None);
        let sol = subject("Sol", Some("tall"), Vec::new(), Some(avatar));
        let (prompt, inputs) =
            remote_scene_prompt(&text, "Rain", &bare, None, Some(&sol), "the persona");
        assert_eq!(
            prompt,
            "Persona design notes for Sol:\ntall\n\n\
             The attached image 1 is the base avatar reference for Sol. Use it only for Sol's identity, face, body, outfit cues, and signature styling.\n\
             Do not swap, merge, or borrow identity-defining features between reference images.\n\
             Only Sol has a reference image attached. Do not invent Mira from Sol's appearance.\n\n\
             Rain"
        );
        assert_eq!(inputs, vec![avatar]);

        let lone = subject("Mira", None, vec![first], None);
        let (prompt, _) = remote_scene_prompt(&text, "Rain", &lone, None, None, "the persona");
        assert!(prompt.contains("Do not invent the persona from Mira's appearance."));
        let (prompt, inputs) = remote_scene_prompt(&text, "Rain", &bare, None, None, "the persona");
        assert_eq!((prompt.as_str(), inputs.len()), ("Rain", 0));

        let many = subject(
            "Mira",
            None,
            (0..12).map(|_| AssetId::new()).collect(),
            None,
        );
        let crowd = subject("Sol", None, (0..12).map(|_| AssetId::new()).collect(), None);
        let (prompt, inputs) = remote_scene_prompt(
            &text,
            "Rain",
            &many,
            Some(background),
            Some(&crowd),
            "the persona",
        );
        assert_eq!(inputs.len(), MAX_INPUT_IMAGES);
        assert!(prompt.contains("attached images 14-16 is the saved persona design reference"));
    }

    #[test]
    fn local_scene_loras_apply_like_legacy() {
        let lora = |keywords: &[&str]| StableDiffusionLora {
            path: "mira.safetensors".into(),
            multiplier: 0.8,
            is_high_noise: false,
            keywords: keywords
                .iter()
                .map(|keyword| (*keyword).to_owned())
                .collect(),
        };
        assert!(applies_to_prompt(&lora(&[]), "a quiet harbor"));
        assert!(applies_to_prompt(
            &lora(&[" Mira "]),
            "MIRA walks the harbor"
        ));
        assert!(!applies_to_prompt(&lora(&["mira", " "]), "a quiet harbor"));
    }
}
