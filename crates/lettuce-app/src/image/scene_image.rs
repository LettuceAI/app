//! Scene images for a chat message: the image request is built from the
//! cast's references and the chat background, and the generated image is
//! added to the message. The follow-up (`scene_follow_up.rs`) runs them as
//! image jobs.

use lettuce_context::{PromptConditionContext, PromptEntryChatMode, PromptRenderContext, PromptVariable, SceneImageProtocolKind};
use lettuce_conversations::{
    ConversationReader, ConversationRepository, EditMessageResult, MediaAssetRole, OperationToken,
};
use lettuce_image_generation::sd_runtime::lora_library::LoraLibraryRepository;
use lettuce_image_generation::{
    ImageAttribution, ImageGenerationRequest, ImageGenerationSource, ImageMedia, ImageOutputPolicy,
};
use lettuce_models::{ModelCatalog, StableDiffusionLora};
use lettuce_types::{
    AssetId, ContentHash, ConversationId, MessageId, RequestId, TimestampMillis,
};

use crate::generation::runtime_text::RuntimeText;
use crate::image::scene_cast::{
    CastSubject, SceneCast, SceneCastError, SceneCastSelection, SceneCastSources,
};
use crate::{BuiltInPromptId, ImageFeature, ImageFeatureModelError, image_feature_model};

const MAX_INPUT_IMAGES: usize = lettuce_image_generation::MAX_INPUT_IMAGES;
const DEFAULT_SIZE: &str = "1024x1024";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneImageRequest {
    pub conversation_id: ConversationId,
    pub message_id: MessageId,
    pub scene_prompt: String,
    /// Identifies this generation; each attempt's image request and the
    /// message edit derive their ids from it, so a replay repeats them.
    pub request_id: RequestId,
    /// The cast the user picked; `None` uses the message's own.
    pub cast: Option<SceneCastSelection>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SceneImageError {
    #[error("scenePrompt cannot be empty")]
    EmptyPrompt,
    #[error("Session not found")]
    ConversationNotFound,
    #[error("Message not found in loaded session window")]
    MessageNotFound,
    #[error("Session character not found")]
    CharacterNotFound,
    #[error("the cast names a character that is not in the conversation")]
    ForeignCastMember,
    #[error("the scene has nobody to depict")]
    EmptyCast,
    #[error(transparent)]
    Model(#[from] ImageFeatureModelError),
    #[error("{0}")]
    Generation(String),
    #[error("The message is generating or was changed, so the scene image could not be added")]
    MessageUnavailable,
    #[error("scene image storage failed: {reason}")]
    Storage { reason: String },
}

impl From<SceneCastError> for SceneImageError {
    fn from(error: SceneCastError) -> Self {
        match error {
            SceneCastError::CharacterNotFound => Self::CharacterNotFound,
            SceneCastError::ForeignCharacter => Self::ForeignCastMember,
            SceneCastError::Empty => Self::EmptyCast,
            SceneCastError::MessageNotFound => Self::MessageNotFound,
            SceneCastError::Storage { reason } => Self::Storage { reason },
        }
    }
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
    pub(crate) persona: bool,
    pub(crate) design_notes: Option<String>,
    pub(crate) references: Vec<AssetId>,
    pub(crate) source: Option<ReferenceSource>,
    /// What the stored media alone suggest, before unreadable images are
    /// skipped: the writer hints count the stored design ids and the avatar
    /// path.
    pub(crate) stored_design_count: usize,
    pub(crate) stored_source: Option<ReferenceSource>,
}

fn references_of(
    design: Vec<AssetId>,
    avatar: Option<AssetId>,
) -> (Vec<AssetId>, Option<ReferenceSource>) {
    if !design.is_empty() {
        (design, Some(ReferenceSource::Design))
    } else if let Some(avatar) = avatar {
        (vec![avatar], Some(ReferenceSource::Avatar))
    } else {
        (Vec::new(), None)
    }
}

impl SceneSubject {
    /// The subject with only the images `media` can read; without media (a
    /// local image model) no image is sent.
    fn resolve<D: ImageMedia + ?Sized>(subject: &CastSubject, media: Option<&D>) -> Self {
        let (stored, stored_source) =
            references_of(subject.design.clone(), subject.avatars.first().copied());
        let readable = |assets: &[AssetId]| {
            media.map_or_else(Vec::new, |media| {
                assets
                    .iter()
                    .copied()
                    .filter(|asset| media.load_input(*asset).is_ok())
                    .collect::<Vec<_>>()
            })
        };
        let (references, source) = references_of(
            readable(&subject.design),
            readable(&subject.avatars).first().copied(),
        );
        Self {
            name: subject.name.clone(),
            persona: subject.is_persona(),
            design_notes: subject.design_notes.clone(),
            references,
            source,
            stored_design_count: if stored_source == Some(ReferenceSource::Design) {
                stored.len()
            } else {
                0
            },
            stored_source,
        }
    }
}

/// The references of a remote scene image: each subject's readable design
/// references (else its readable avatar) and the readable chat background.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SceneReferences {
    pub(crate) subjects: Vec<SceneSubject>,
    pub(crate) background: Option<AssetId>,
}

impl SceneReferences {
    pub(crate) fn resolve<D: ImageMedia + ?Sized>(cast: &SceneCast, media: Option<&D>) -> Self {
        Self {
            subjects: cast
                .subjects
                .iter()
                .map(|subject| SceneSubject::resolve(subject, media))
                .collect(),
            background: media.and_then(|media| {
                cast.background
                    .filter(|asset| media.load_input(*asset).is_ok())
            }),
        }
    }

    pub(crate) fn characters(&self) -> impl Iterator<Item = &SceneSubject> {
        self.subjects.iter().filter(|subject| !subject.persona)
    }

    pub(crate) fn persona(&self) -> Option<&SceneSubject> {
        self.subjects.iter().find(|subject| subject.persona)
    }

    /// The references of the cast's primary character.
    pub(crate) fn primary(&self, cast: &SceneCast) -> Option<&SceneSubject> {
        let primary = cast.primary()?;
        cast.subjects
            .iter()
            .position(|subject| subject.kind == primary.kind)
            .and_then(|index| self.subjects.get(index))
    }
}

/// `names` as one phrase: "A", "A and B", "A, B and C".
pub(crate) fn subject_list(text: &RuntimeText, names: &[&str]) -> String {
    match names {
        [] => String::new(),
        [only] => (*only).to_owned(),
        [rest @ .., last] => text
            .render_with(
                "scene_subject_list",
                [
                    (PromptVariable::SubjectName, rest.join(", ")),
                    (PromptVariable::OtherSubjectName, (*last).to_owned()),
                ],
            )
            .unwrap_or_default(),
    }
}

/// Fits the subjects' references and the background into the input bound:
/// persona references go first, then the last subjects' extra references,
/// then their last ones.
fn bounded_references(subjects: &[SceneSubject], background: bool) -> Vec<Vec<AssetId>> {
    let mut references = subjects
        .iter()
        .map(|subject| subject.references.clone())
        .collect::<Vec<_>>();
    let mut total = references.iter().map(Vec::len).sum::<usize>() + usize::from(background);
    while total > MAX_INPUT_IMAGES {
        let index = subjects
            .iter()
            .rposition(|subject| subject.persona)
            .filter(|index| !references[*index].is_empty())
            .or_else(|| references.iter().rposition(|refs| refs.len() > 1))
            .or_else(|| references.iter().rposition(|refs| !refs.is_empty()));
        let Some(index) = index else {
            break;
        };
        references[index].pop();
        total -= 1;
    }
    references
}

/// The prompt and input images of a remote scene image, in this order: the
/// characters' references, the chat background, the persona's references.
fn remote_scene_prompt(
    text: &RuntimeText,
    scene_prompt: &str,
    references: &SceneReferences,
    persona_fallback_name: Option<&str>,
) -> (String, Vec<AssetId>) {
    let bounded = bounded_references(&references.subjects, references.background.is_some());
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
    let ordered = references
        .subjects
        .iter()
        .zip(&bounded)
        .filter(|(subject, _)| !subject.persona)
        .chain(
            references
                .subjects
                .iter()
                .zip(&bounded)
                .filter(|(subject, _)| subject.persona),
        )
        .collect::<Vec<_>>();
    let mut sections = Vec::new();
    for (subject, _) in &ordered {
        if let Some(notes) = &subject.design_notes {
            sections.push(render(
                if subject.persona {
                    "scene_image_persona_design_notes"
                } else {
                    "scene_image_character_design_notes"
                },
                vec![
                    (PromptVariable::SubjectName, subject.name.clone()),
                    (PromptVariable::SubjectDescription, notes.clone()),
                ],
            ));
        }
    }
    let any_reference = bounded.iter().any(|refs| !refs.is_empty());
    let mut inputs = Vec::new();
    if any_reference || references.background.is_some() {
        let mut lines = Vec::new();
        let mut next = 1;
        let mut background_placed = false;
        for (subject, refs) in &ordered {
            if subject.persona && !background_placed {
                if let Some(background) = references.background {
                    lines.push(render(
                        "scene_image_background_reference",
                        vec![(PromptVariable::ReferenceRange, range(next, 1))],
                    ));
                    inputs.push(background);
                    next += 1;
                }
                background_placed = true;
            }
            if refs.is_empty() {
                continue;
            }
            lines.push(render(
                "scene_image_subject_reference",
                vec![
                    (PromptVariable::ReferenceRange, range(next, refs.len())),
                    (
                        PromptVariable::ReferenceSource,
                        render(
                            match (subject.source, subject.persona) {
                                (Some(ReferenceSource::Avatar), _) => "scene_image_avatar_source",
                                (_, true) => "scene_image_persona_design_source",
                                (_, false) => "scene_image_character_design_source",
                            },
                            Vec::new(),
                        ),
                    ),
                    (PromptVariable::SubjectName, subject.name.clone()),
                ],
            ));
            inputs.extend(refs.iter().copied());
            next += refs.len();
        }
        if !background_placed && let Some(background) = references.background {
            lines.push(render(
                "scene_image_background_reference",
                vec![(PromptVariable::ReferenceRange, range(next, 1))],
            ));
            inputs.push(background);
        }
        lines.push(render("scene_image_no_swap", Vec::new()));
        let with = ordered
            .iter()
            .filter(|(_, refs)| !refs.is_empty())
            .map(|(subject, _)| subject.name.as_str())
            .collect::<Vec<_>>();
        let mut without = ordered
            .iter()
            .filter(|(_, refs)| refs.is_empty())
            .map(|(subject, _)| subject.name.as_str())
            .collect::<Vec<_>>();
        if let Some(fallback) = persona_fallback_name
            && references.persona().is_none()
        {
            without.push(fallback);
        }
        if !with.is_empty() && !without.is_empty() {
            lines.push(render(
                if with.len() == 1 {
                    "scene_image_only_reference"
                } else {
                    "scene_image_only_references"
                },
                vec![
                    (PromptVariable::SubjectName, subject_list(text, &with)),
                    (PromptVariable::OtherSubjectName, subject_list(text, &without)),
                ],
            ));
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
    (
        sections
            .into_iter()
            .filter(|section| !section.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n"),
        inputs,
    )
}

/// A LoRA without keywords always applies, otherwise one of its keywords
/// must appear in the prompt.
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

pub(crate) fn derived_id(root: RequestId, label: &str) -> uuid::Uuid {
    uuid::Uuid::new_v5(&root.as_uuid(), label.as_bytes())
}

/// The image request that draws `request.scene_prompt` for a chat message:
/// the scene model's request built from the cast's references and the chat
/// background (a local model gets the cast's LoRAs instead).
/// `request.request_id` is the id of the image request.
pub fn scene_generation_request<R, D>(
    repository: &R,
    media: &D,
    request: &SceneImageRequest,
    now: TimestampMillis,
) -> Result<ImageGenerationRequest, SceneImageError>
where
    R: SceneCastSources
        + ModelCatalog
        + lettuce_settings::GlobalSettingsStore
        + lettuce_context::PromptRepository
        + LoraLibraryRepository
        + ?Sized,
    D: ImageMedia + ?Sized,
{
    let scene_prompt = request.scene_prompt.trim();
    if scene_prompt.is_empty() {
        return Err(SceneImageError::EmptyPrompt);
    }
    let aggregate = ConversationReader::get(repository, request.conversation_id)
        .map_err(|_| SceneImageError::ConversationNotFound)?;
    let conversation = &aggregate.conversation;
    let settings = lettuce_settings::GlobalSettingsStore::load(repository)
        .map_err(|error| SceneImageError::Storage {
            reason: error.to_string(),
        })?
        .settings;
    let model = image_feature_model(repository, &settings, ImageFeature::Scene)?;
    let cast = crate::image::scene_cast::scene_cast(
        repository,
        conversation,
        request.message_id,
        request.cast.as_ref(),
    )?;
    let (prompt, input_images, loras) = if model.is_local_diffusion() {
        let loras = cast
            .subjects
            .iter()
            .filter_map(|subject| {
                crate::image::scene_loras::recommendation_lora(
                    repository,
                    subject.recommendation.as_ref(),
                )
            })
            .filter(|lora| applies_to_prompt(lora, scene_prompt))
            .collect();
        (scene_prompt.to_owned(), Vec::new(), loras)
    } else {
        let text =
            RuntimeText::load(repository, BuiltInPromptId::ChatRuntime).map_err(|error| {
                SceneImageError::Storage {
                    reason: error.to_string(),
                }
            })?;
        let references = SceneReferences::resolve(&cast, Some(media));
        let fallback_name = text
            .render_with("scene_image_persona_fallback_name", [])
            .unwrap_or_default();
        let (legend, inputs) = remote_scene_prompt(
            &text,
            "",
            &references,
            (!cast.group).then_some(fallback_name.as_str()),
        );
        let document = crate::image::prompt_selection::selected_prompt(
            repository, settings.image_generation.scene_prompt_id, BuiltInPromptId::SceneGeneration,
        ).map_err(|error| SceneImageError::Generation(error.to_string()))?
            .ok_or_else(|| SceneImageError::Generation("the scene generation prompt is unavailable".into()))?;
        let moment = crate::image::scene_cast::scene_moment(repository, conversation, request.message_id)?;
        let recent = moment.iter().map(crate::image::scene_cast::item_text).collect::<Vec<_>>().join("\n\n");
        let selected_speaker = moment.last().filter(|_| cast.group).and_then(|item| item.message.author_participant_id);
        let effective = lettuce_conversations::resolve_effective_settings(conversation, selected_speaker)
            .map_err(|error| SceneImageError::Generation(error.to_string()))?;
        let mut values = crate::image::scene_prompt_writer::render_values(&text,
            crate::image::scene_prompt_writer::WriterInputs {
                cast: &cast, references: &references, loras: &[],
                recent: recent.clone(),
                image_model_instructions: model.profile.config.stable_diffusion.prompt_writer_instructions.clone().unwrap_or_default(),
            },
        ).map_err(|error| SceneImageError::Generation(error.to_string()))?;
        values.purpose_values.insert(PromptVariable::SceneRequest, scene_prompt.to_owned());
        let capabilities = &model.profile.config.capabilities;
        let conditions = PromptConditionContext {
            chat_mode: if cast.group { PromptEntryChatMode::Group } else { PromptEntryChatMode::Direct },
            scene_generation_enabled: true,
            avatar_generation_enabled: settings.image_generation.avatar_enabled,
            scene_image_protocol: Some(SceneImageProtocolKind::Remote),
            message_count: moment.len(), participant_count: cast.subjects.len(),
            recent_text: recent,
            has_scene: effective.scene.is_some(),
            does_author_note_exists: effective.author_note.as_deref().is_some_and(|note| !note.trim().is_empty()),
            has_persona: cast.persona().is_some(),
            has_character_reference_images: references.primary(&cast).is_some_and(|subject| !subject.references.is_empty()),
            has_persona_reference_images: references.persona().is_some_and(|subject| !subject.references.is_empty()),
            has_character_reference_text: references.primary(&cast).is_some_and(|subject| subject.design_notes.is_some() || subject.source.is_some()),
            has_persona_reference_text: references.persona().is_some_and(|subject| subject.design_notes.is_some() || subject.source.is_some()),
            has_chat_background: references.background.is_some(),
            provider_id: Some(model.account.provider_kind.clone()),
            input_scopes: crate::generation::conversation_generation_input::modality_scopes(capabilities.input_modalities),
            output_scopes: crate::generation::conversation_generation_input::modality_scopes(capabilities.output_modalities),
            vision_enabled: capabilities.input_modalities.image == lettuce_models::CapabilityStatus::Supported,
            ..Default::default()
        };
        let entries = crate::generation::feature_prompt_entries::render_feature_entries(
            &document, &PromptRenderContext { conditions, values }, &crate::image::scene_prompt_writer::SCENE_TOKENS,
        ).map_err(|error| SceneImageError::Generation(error.to_string()))?;
        let template = entries.into_iter().map(|entry| {
            crate::generation::feature_prompt_entries::strip_tokens(&entry.content, &crate::image::scene_prompt_writer::SCENE_TOKENS)
        }).filter(|entry| !entry.trim().is_empty()).collect::<Vec<_>>().join("\n\n");
        let request_text = crate::generation::feature_prompt_entries::condense(scene_prompt);
        let include_request = !template.contains(&request_text);
        let prompt = [legend, template, if include_request { request_text } else { String::new() }]
            .into_iter().filter(|section| !section.is_empty()).collect::<Vec<_>>().join("\n\n");
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
    let character_id = cast.speaker.or_else(|| {
        cast.characters().find_map(|subject| match subject.kind {
            crate::image::scene_cast::CastSubjectKind::Character(id) => Some(id),
            crate::image::scene_cast::CastSubjectKind::Persona(_) => None,
        })
    });
    Ok(ImageGenerationRequest {
        id: request.request_id,
        model_profile_id: model.profile.id,
        prompt,
        settings: Default::default(),
        input_images,
        mask_image: None,
        loras,
        size,
        quality: None,
        style: None,
        count: 1,
        source: ImageGenerationSource::Scene,
        attribution: ImageAttribution {
            conversation_id: Some(request.conversation_id),
            character_id,
        },
        output_policy: ImageOutputPolicy::Retained,
        created_at: now,
    })
}

/// Appends the image to the reply variant it was asked for, wherever the
/// message stands now: the shown render when it is that variant, otherwise
/// the variant's own latest edit. `seed` names the edit, so repeating the
/// call adds the image once.
pub fn attach_scene_image<R: ConversationRepository + ?Sized>(
    repository: &R,
    conversation_id: ConversationId,
    message_id: MessageId,
    target: lettuce_conversations::SceneFollowUpTarget,
    seed: RequestId,
    asset: AssetId,
    now: TimestampMillis,
) -> Result<EditMessageResult, SceneImageError> {
    attach_scene_image_phase(
        repository,
        conversation_id,
        message_id,
        target,
        SceneAttachmentPhase::Initial(seed),
        asset,
        now,
    )
}

pub(crate) enum SceneAttachmentPhase {
    Initial(RequestId),
    Deferred(RequestId),
}

pub(crate) fn attach_scene_image_phase<R: ConversationRepository + ?Sized>(
    repository: &R,
    conversation_id: ConversationId,
    message_id: MessageId,
    target: lettuce_conversations::SceneFollowUpTarget,
    phase: SceneAttachmentPhase,
    asset: AssetId,
    now: TimestampMillis,
) -> Result<EditMessageResult, SceneImageError> {
    let (seed, complete_deferred) = match phase {
        SceneAttachmentPhase::Initial(seed) => (seed, false),
        SceneAttachmentPhase::Deferred(seed) => (seed, true),
    };
    let key = derived_id(
        seed,
        if complete_deferred {
            "complete-attachment"
        } else {
            "attach"
        },
    );
    repository
        .attach_scene_media(
            &lettuce_conversations::AttachSceneMedia {
                complete_deferred,
                conversation_id,
                message_id,
                target,
                operation: OperationToken {
                    key: lettuce_jobs::IdempotencyKey::new(format!("scene-image-{key}")).map_err(
                        |error| SceneImageError::Storage {
                            reason: error.to_string(),
                        },
                    )?,
                    request_digest: ContentHash::parse(
                        blake3::hash(
                            format!("{}:{}:{asset}", target.kind(), target.id()).as_bytes(),
                        )
                        .to_hex()
                        .to_string(),
                    )
                    .map_err(|error| SceneImageError::Storage {
                        reason: error.to_string(),
                    })?,
                },
                asset_id: asset,
                role: MediaAssetRole::Attachment,
            },
            now,
        )
        .map_err(|error| match error {
            lettuce_conversations::ConversationRepositoryError::Conflict
            | lettuce_conversations::ConversationRepositoryError::NotFound => {
                SceneImageError::MessageUnavailable
            }
            error => SceneImageError::Storage {
                reason: error.to_string(),
            },
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
        let (references, source) = references_of(design, avatar);
        SceneSubject {
            name: name.to_owned(),
            persona: false,
            design_notes: notes
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

    fn persona(subject: SceneSubject) -> SceneSubject {
        SceneSubject {
            persona: true,
            ..subject
        }
    }

    fn refs(subjects: Vec<SceneSubject>, background: Option<AssetId>) -> SceneReferences {
        SceneReferences {
            subjects,
            background,
        }
    }

    #[test]
    fn remote_scene_prompts_match_legacy_build_scene_generation_request() {
        let text = RuntimeText::from_seed(BuiltInPromptId::ChatRuntime);
        let (first, second, background) = (AssetId::new(), AssetId::new(), AssetId::new());
        let mira = subject("Mira", Some(" red coat "), vec![first, second], None);
        let sol = persona(subject("Sol", None, Vec::new(), None));
        let (prompt, inputs) = remote_scene_prompt(
            &text,
            " A harbor at dusk ",
            &refs(vec![mira.clone(), sol], Some(background)),
            Some("the persona"),
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
        let sol = persona(subject("Sol", Some("tall"), Vec::new(), Some(avatar)));
        let (prompt, inputs) = remote_scene_prompt(
            &text,
            "Rain",
            &refs(vec![bare.clone(), sol], None),
            Some("the persona"),
        );
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
        let (prompt, _) =
            remote_scene_prompt(&text, "Rain", &refs(vec![lone], None), Some("the persona"));
        assert!(prompt.contains("Do not invent the persona from Mira's appearance."));
        let (prompt, inputs) =
            remote_scene_prompt(&text, "Rain", &refs(vec![bare], None), Some("the persona"));
        assert_eq!((prompt.as_str(), inputs.len()), ("Rain", 0));

        let many = subject(
            "Mira",
            None,
            (0..MAX_INPUT_IMAGES / 2).map(|_| AssetId::new()).collect(),
            None,
        );
        let crowd = persona(subject(
            "Sol",
            None,
            (0..MAX_INPUT_IMAGES / 2).map(|_| AssetId::new()).collect(),
            None,
        ));
        let (prompt, inputs) = remote_scene_prompt(
            &text,
            "Rain",
            &refs(vec![many, crowd], Some(background)),
            Some("the persona"),
        );
        assert_eq!(inputs.len(), MAX_INPUT_IMAGES);
        assert!(prompt.contains(&format!("attached images {}-{} is the saved persona design reference", MAX_INPUT_IMAGES / 2 + 2, MAX_INPUT_IMAGES)));
    }

    #[test]
    fn group_scene_prompts_number_every_subject_and_trim_from_the_end() {
        let text = RuntimeText::from_seed(BuiltInPromptId::ChatRuntime);
        let background = AssetId::new();
        let mira = subject("Mira", None, (0..8).map(|_| AssetId::new()).collect(), None);
        let tom = subject("Tom", None, (0..MAX_INPUT_IMAGES).map(|_| AssetId::new()).collect(), None);
        let ada = subject("Ada", None, Vec::new(), None);
        let sol = persona(subject("Sol", None, vec![AssetId::new()], None));
        let (prompt, inputs) = remote_scene_prompt(
            &text,
            "Dinner",
            &refs(vec![mira.clone(), tom, ada, sol], Some(background)),
            None,
        );
        assert_eq!(inputs.len(), MAX_INPUT_IMAGES);
        assert_eq!(&inputs[..8], &mira.references[..]);
        assert_eq!(inputs[MAX_INPUT_IMAGES - 1], background);
        assert!(prompt.contains("attached images 1-8 is the saved character design reference for Mira"));
        assert!(prompt.contains(&format!("attached images 9-{} is the saved character design reference for Tom", MAX_INPUT_IMAGES - 1)));
        assert!(prompt.contains(&format!("attached image {} is the chat background", MAX_INPUT_IMAGES)));
        assert!(prompt.contains(
            "Only Mira and Tom have reference images attached. Do not invent Ada and Sol from their appearance."
        ));
    }

    #[test]
    fn subject_lists_read_naturally() {
        let text = RuntimeText::from_seed(BuiltInPromptId::ChatRuntime);
        assert_eq!(subject_list(&text, &[]), "");
        assert_eq!(subject_list(&text, &["Mira"]), "Mira");
        assert_eq!(subject_list(&text, &["Mira", "Tom"]), "Mira and Tom");
        assert_eq!(subject_list(&text, &["Mira", "Tom", "Ada"]), "Mira, Tom and Ada");
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
