//! Scene image prompts for a direct chat message (legacy
//! `chat_generate_scene_prompt`): a writer model turns the messages around
//! the chosen one into one image prompt. It sees the character, persona and
//! chat background images when the scene image model is remote, and the
//! subjects' LoRA keywords when it runs locally.

use std::collections::{BTreeMap, HashSet};
use std::time::Duration;

use lettuce_characters::{CharacterRepository, PersonaRepository};
use lettuce_context::{
    PromptConditionContext, PromptDocument, PromptEntryChatMode, PromptEntryImageSlot,
    PromptEntryPayload, PromptEntryPosition, PromptEntryRole, PromptRenderContext,
    PromptRenderValues, PromptRepository, PromptVariable as Variable, SceneImageProtocolKind,
    render_prompt,
};
use lettuce_conversations::{
    ConversationKind, ConversationReader, ConversationRepositoryError, GenerationOperation,
    InferenceOutcome, InferencePort, InferenceRequest, MediaAssetRole, MessagePart,
    MessageRenderSource, MessageRole, OutputPolicy, PortError, ProviderContextPart,
    ProviderFailureKind, ProviderNeutralContext, ProviderNeutralMessage,
    ProviderReplayArtifactPort, ResolvedInferenceProfile, SafetyContext, TimelineItem, ToolPolicy,
    effective_persona,
};
use lettuce_image_generation::ImageMedia;
use lettuce_image_generation::sd_runtime::lora_library::LoraLibraryRepository;
use lettuce_jobs::{
    JobError, JobErrorCode, JobSnapshot, JobStore, ResourceAvailability, StoreError, SubjectKind,
    WorkerId, handle::JobHandle,
};
use lettuce_models::{
    CapabilityStatus, ChatRequirements, ExpectedModelIdentity, ModelCatalog, ReasoningMode,
};
use lettuce_settings::{GlobalSettings, GlobalSettingsStore};
use lettuce_types::{
    AssetId, ConversationId, GenerationAttemptId, GenerationTurnId, MessageId, PageLimit,
    PageRequest, RequestId, TimestampMillis,
};
use lettuce_usage::JobUsageLedger;

use crate::job_inference_usage::{JobInferenceError, run_job_inference};
use crate::runtime_text::RuntimeText;
use crate::scene_image::{ReferenceSource, SceneReferences, SceneSubject, StoredSceneReferences};
use crate::{BuiltInPromptId, ImageFeature, ImageFeatureModelError, cleanup_outcome_replays};

const STAGE: &str = "scene-prompt";
const SESSION_WINDOW: usize = 120;
const CHARACTER_TOKEN: &str = "{{image[character]}}";
const PERSONA_TOKEN: &str = "{{image[persona]}}";
const BACKGROUND_TOKEN: &str = "{{image[chatBackground]}}";
const BACKGROUND_ALIAS_TOKEN: &str = "{{image[chat_background]}}";
const SCENE_TOKENS: [&str; 4] = [
    CHARACTER_TOKEN,
    PERSONA_TOKEN,
    BACKGROUND_TOKEN,
    BACKGROUND_ALIAS_TOKEN,
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScenePromptRequest {
    pub conversation_id: ConversationId,
    pub message_id: MessageId,
    pub request_id: RequestId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScenePromptReply {
    pub text: String,
    pub job: JobSnapshot,
}

#[derive(Debug, thiserror::Error)]
pub enum ScenePromptError {
    #[error(transparent)]
    Model(#[from] ImageFeatureModelError),
    #[error("Session not found")]
    ConversationNotFound,
    #[error("Scene images are generated for direct chats only")]
    NotDirect,
    #[error("Character not found")]
    CharacterNotFound,
    #[error("Message not found in loaded session window")]
    MessageNotFound,
    #[error("No conversation context available for scene prompt generation")]
    NoContext,
    #[error("Scene generation prompt template rendered no usable entries")]
    NoEntries,
    #[error("Scene prompt writer model is invalid: {0}")]
    InvalidModel(lettuce_models::ChatProfileResolutionError),
    #[error("Scene prompt writer prompt is unavailable")]
    MissingPrompt,
    #[error("Scene prompt writer prompt could not be rendered")]
    InvalidPrompt,
    #[error("scene prompt storage failed")]
    Storage,
    #[error("scene prompt job storage failed: {0}")]
    Jobs(#[from] StoreError),
    #[error("scene prompt request was already settled")]
    AlreadySettled,
    #[error("scene prompt job could not be claimed")]
    NotClaimed,
    #[error("scene prompt inference failed: {0}")]
    Inference(PortError),
    #[error("scene prompt usage evidence could not be recorded")]
    Evidence,
    #[error("Failed to extract text from response")]
    NoText,
    #[error("Scene prompt generation returned an empty result")]
    Empty,
    #[error("scene prompt request was cancelled")]
    Cancelled,
    #[error("scene prompt replay cleanup failed")]
    ReplayCleanup,
}

impl From<crate::one_shot_job::OneShotJobError> for ScenePromptError {
    fn from(error: crate::one_shot_job::OneShotJobError) -> Self {
        match error {
            crate::one_shot_job::OneShotJobError::Jobs(error) => Self::Jobs(error),
            crate::one_shot_job::OneShotJobError::AlreadySettled => Self::AlreadySettled,
            crate::one_shot_job::OneShotJobError::NotClaimed => Self::NotClaimed,
        }
    }
}

impl crate::one_shot_job::OneShotFailure for ScenePromptError {
    fn is_cancelled(&self) -> bool {
        matches!(self, Self::Cancelled)
    }

    fn job_error(&self) -> JobError {
        let (code, retryable, message) = match self {
            Self::Inference(PortError::Unavailable)
            | Self::Evidence
            | Self::ReplayCleanup
            | Self::NotClaimed
            | Self::AlreadySettled => (
                JobErrorCode::ResourceUnavailable,
                true,
                "scene-prompt-unavailable",
            ),
            Self::Inference(PortError::Provider(failure))
                if failure.kind == ProviderFailureKind::Unavailable =>
            {
                (
                    JobErrorCode::ResourceUnavailable,
                    true,
                    "scene-prompt-unavailable",
                )
            }
            Self::Inference(_) | Self::NoText | Self::Empty => (
                JobErrorCode::WorkerFailed,
                false,
                "scene-prompt-inference-failed",
            ),
            Self::Jobs(_) | Self::Storage | Self::Model(ImageFeatureModelError::Storage) => (
                JobErrorCode::StorageFailure,
                true,
                "scene-prompt-storage-failed",
            ),
            Self::Model(_)
            | Self::ConversationNotFound
            | Self::NotDirect
            | Self::CharacterNotFound
            | Self::MessageNotFound
            | Self::NoContext
            | Self::NoEntries
            | Self::InvalidModel(_)
            | Self::MissingPrompt
            | Self::InvalidPrompt
            | Self::Cancelled => (
                JobErrorCode::InvalidInput,
                false,
                "scene-prompt-invalid-input",
            ),
        };
        JobError::new(code, retryable, message).expect("constant job error is valid")
    }
}

/// Every port the scene prompt writer reads; the composition root's database
/// is one.
pub trait ScenePromptSources:
    GlobalSettingsStore
    + ConversationReader
    + CharacterRepository
    + PersonaRepository
    + ModelCatalog
    + lettuce_models::GlobalModelSettingsRepository
    + PromptRepository
    + LoraLibraryRepository
    + JobStore
    + JobUsageLedger
    + ProviderReplayArtifactPort
{
}

impl<T> ScenePromptSources for T where
    T: GlobalSettingsStore
        + ConversationReader
        + CharacterRepository
        + PersonaRepository
        + ModelCatalog
        + lettuce_models::GlobalModelSettingsRepository
        + PromptRepository
        + LoraLibraryRepository
        + JobStore
        + JobUsageLedger
        + ProviderReplayArtifactPort
{
}

#[derive(Debug)]
pub struct ScenePromptWriter<'a, R: ?Sized, D: ?Sized, I: ?Sized> {
    repository: &'a R,
    media: &'a D,
    inference: &'a I,
}

impl<'a, R: ?Sized, D: ?Sized, I: ?Sized> ScenePromptWriter<'a, R, D, I> {
    #[must_use]
    pub const fn new(repository: &'a R, media: &'a D, inference: &'a I) -> Self {
        Self {
            repository,
            media,
            inference,
        }
    }
}

/// A rendered writer entry in template order, with the placement legacy gave
/// it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct WriterEntry {
    role: PromptEntryRole,
    content: String,
    slot: Option<PromptEntryImageSlot>,
    position: PromptEntryPosition,
    depth: u32,
    conditional_min_messages: Option<u32>,
    interval_turns: Option<u32>,
}

impl WriterEntry {
    fn image_bound(&self) -> bool {
        self.slot.is_some()
            || SCENE_TOKENS
                .iter()
                .any(|token| self.content.contains(token))
    }
}

impl<R, D, I> ScenePromptWriter<'_, R, D, I>
where
    R: ScenePromptSources + ?Sized,
    D: ImageMedia + ?Sized,
    I: InferencePort + ?Sized,
{
    pub async fn generate(
        &self,
        request: &ScenePromptRequest,
        worker_id: WorkerId,
        now: TimestampMillis,
        lease_for: Duration,
        allowed: &ResourceAvailability,
    ) -> Result<ScenePromptReply, ScenePromptError> {
        let settings = GlobalSettingsStore::load(self.repository)
            .map_err(|_| ScenePromptError::Storage)?
            .settings;
        if !settings.image_generation.scene_enabled {
            return Err(ImageFeatureModelError::SceneDisabled.into());
        }
        let settings = &settings;
        let (text, job) = crate::one_shot_job::run_one_shot_job(
            self.repository,
            crate::one_shot_job::OneShotJob {
                name: "scene-prompt",
                stage: STAGE,
                subject_kind: SubjectKind::Conversation,
                subject: &request.conversation_id.to_string(),
                request_id: request.request_id,
            },
            crate::one_shot_job::OneShotLease {
                worker_id,
                now,
                lease_for,
                allowed,
            },
            |handle| async move { self.run(settings, request, &handle, now).await },
        )
        .await?;
        Ok(ScenePromptReply { text, job })
    }

    async fn run(
        &self,
        settings: &GlobalSettings,
        request: &ScenePromptRequest,
        handle: &JobHandle,
        now: TimestampMillis,
    ) -> Result<String, ScenePromptError> {
        let inference_request = self.prepare(settings, request, handle)?;
        if handle.cancellation_token().is_cancelled() {
            return Err(ScenePromptError::Cancelled);
        }
        let outcome = match run_job_inference(
            self.repository,
            self.inference,
            handle.id(),
            inference_request,
            now,
        )
        .await
        {
            Ok(outcome) => outcome,
            Err(JobInferenceError::Provider(PortError::Cancelled)) => {
                return Err(ScenePromptError::Cancelled);
            }
            Err(JobInferenceError::Provider(error)) => {
                return Err(ScenePromptError::Inference(error));
            }
            Err(JobInferenceError::Evidence) => return Err(ScenePromptError::Evidence),
        };
        let text = generated_text(&outcome);
        cleanup_outcome_replays(self.repository, &outcome)
            .map_err(|_| ScenePromptError::ReplayCleanup)?;
        let cleaned = clean_scene_prompt(&text.ok_or(ScenePromptError::NoText)?);
        if cleaned.is_empty() {
            return Err(ScenePromptError::Empty);
        }
        Ok(cleaned)
    }

    fn prepare(
        &self,
        settings: &GlobalSettings,
        request: &ScenePromptRequest,
        handle: &JobHandle,
    ) -> Result<InferenceRequest, ScenePromptError> {
        let aggregate =
            ConversationReader::get(self.repository, request.conversation_id).map_err(|error| {
                match error {
                    ConversationRepositoryError::NotFound => ScenePromptError::ConversationNotFound,
                    _ => ScenePromptError::Storage,
                }
            })?;
        let conversation = &aggregate.conversation;
        let ConversationKind::Direct(details) = &conversation.kind else {
            return Err(ScenePromptError::NotDirect);
        };
        let window = self.session_window(conversation)?;
        let character = CharacterRepository::get(self.repository, details.character.source_id)
            .map_err(|_| ScenePromptError::Storage)?
            .ok_or(ScenePromptError::CharacterNotFound)?;
        let persona = effective_persona(conversation)
            .map(|persona| PersonaRepository::get(self.repository, persona.source_id))
            .transpose()
            .map_err(|_| ScenePromptError::Storage)?
            .flatten();
        let image_model =
            crate::image_feature_model(self.repository, settings, ImageFeature::Scene)?;
        let local = image_model.is_local_diffusion();
        let writer = crate::scene_writer_model(self.repository, settings, !local)?;
        let text = RuntimeText::load(self.repository, BuiltInPromptId::ChatRuntime)
            .map_err(|_| ScenePromptError::MissingPrompt)?;
        let recent = recent_messages(&text, &window, request.message_id)?;
        let stored = StoredSceneReferences::new(conversation, &character, persona.as_ref());
        let references = if local {
            stored.resolve::<D>(None)
        } else {
            stored.resolve(Some(self.media))
        };
        let (character_lora, persona_lora) = if local {
            let (character_lora, persona_lora) = crate::scene_loras::subject_loras(
                self.repository,
                details.character.source_id,
                persona.as_ref().map(|persona| persona.id),
            );
            let binding = |lora: Option<&lettuce_models::StableDiffusionLora>, filler: &str| {
                match crate::scene_loras::subject_binding(lora) {
                    lettuce_conversations::SceneLoraBinding::Keywords(keywords) => keywords,
                    lettuce_conversations::SceneLoraBinding::NoLora => {
                        text.render_with(filler, []).unwrap_or_default()
                    }
                }
            };
            (
                binding(character_lora.as_ref(), "scene_lora_primary_subject"),
                persona_lora
                    .map(|lora| binding(lora.as_ref(), "scene_lora_secondary_subject"))
                    .unwrap_or_default(),
            )
        } else {
            (String::new(), String::new())
        };
        let values = render_values(
            &text,
            WriterInputs {
                character: &character.character.profile,
                persona: persona.as_ref().map(|persona| WriterPersona {
                    title: &persona.title,
                    description: &persona.description,
                    design_description: persona.design_description.as_deref(),
                }),
                references: &references,
                recent: recent.clone(),
                character_lora,
                persona_lora,
                image_model_instructions: image_model
                    .profile
                    .config
                    .stable_diffusion
                    .prompt_writer_instructions
                    .as_deref()
                    .map(str::trim)
                    .unwrap_or_default()
                    .to_owned(),
            },
        )?;
        let effective = lettuce_conversations::resolve_effective_settings(conversation, None)
            .map_err(|_| ScenePromptError::Storage)?;
        let scene_id = effective
            .scene
            .as_ref()
            .map(|scene| scene.source_id)
            .or(character.character.defaults.default_scene_id);
        let capabilities = &writer.profile.config.capabilities;
        let conditions = PromptConditionContext {
            chat_mode: PromptEntryChatMode::Direct,
            scene_generation_enabled: true,
            avatar_generation_enabled: settings.image_generation.avatar_enabled,
            is_local_image_generation_model: local,
            is_scene_generation_local_image_model: local,
            scene_image_protocol: Some(if local {
                SceneImageProtocolKind::Local
            } else {
                SceneImageProtocolKind::Remote
            }),
            has_scene: scene_id.is_some(),
            has_scene_direction: scene_id
                .and_then(|id| character.scenes.iter().find(|scene| scene.id == id))
                .and_then(|scene| scene.direction.as_deref())
                .is_some_and(|direction| !direction.trim().is_empty()),
            has_persona: persona.is_some(),
            message_count: window.len(),
            participant_count: 2,
            recent_text: recent,
            does_author_note_exists: effective
                .author_note
                .as_deref()
                .is_some_and(|note| !note.trim().is_empty()),
            has_character_reference_images: !references.character.references.is_empty(),
            has_chat_background: references.background.is_some(),
            has_persona_reference_images: references
                .persona
                .as_ref()
                .is_some_and(|persona| !persona.references.is_empty()),
            has_character_reference_text: has_reference_text(&references.character),
            has_persona_reference_text: references.persona.as_ref().is_some_and(has_reference_text),
            input_scopes: crate::conversation_generation_input::modality_scopes(
                capabilities.input_modalities,
            ),
            output_scopes: crate::conversation_generation_input::modality_scopes(
                capabilities.output_modalities,
            ),
            provider_id: Some(writer.account.provider_kind.clone()),
            reasoning_enabled: writer.profile.config.chat_parameters.reasoning_mode
                == Some(ReasoningMode::Enabled),
            vision_enabled: capabilities.input_modalities.image == CapabilityStatus::Supported,
            ..Default::default()
        };
        let document = crate::built_in_prompts::active_built_in_prompt(
            self.repository,
            BuiltInPromptId::ScenePromptWriter,
        )
        .map_err(|_| ScenePromptError::MissingPrompt)?
        .ok_or(ScenePromptError::MissingPrompt)?;
        let entries = render_entries(&document, PromptRenderContext { conditions, values })?;
        if entries.is_empty() {
            return Err(ScenePromptError::NoEntries);
        }
        let messages = writer_messages(&entries, &references);
        let mut seen = HashSet::new();
        let media_grants = messages
            .iter()
            .flat_map(|message| &message.parts)
            .filter_map(|part| match part {
                ProviderContextPart::MediaAsset { asset_id, .. } if seen.insert(*asset_id) => {
                    Some(*asset_id)
                }
                _ => None,
            })
            .collect();
        let global =
            lettuce_models::GlobalModelSettingsRepository::global_model_settings(self.repository)
                .map_err(|_| ScenePromptError::Storage)?
                .0;
        let (model, account) = (&writer.profile, &writer.account);
        let chat_profile = lettuce_models::resolve_chat_profile(
            &ExpectedModelIdentity {
                model_profile_id: model.id,
                model_revision: model.revision,
                provider_account_id: account.id,
                provider_account_revision: account.revision,
                external_model_id: model.external_model_id.clone(),
                display_name: model.display_name.clone(),
                provider_protocol: account.protocol,
                model_kind: model.kind,
            },
            model,
            account,
            &crate::feature_parameter_input(
                &model.config.feature_parameters.scene_writer,
                crate::SCENE_WRITER_DEFAULTS,
                crate::FeatureRequestFields::SamplingAndPromptCache,
                account.protocol,
                &global,
            ),
            &ChatRequirements::default(),
        )
        .map_err(ScenePromptError::InvalidModel)?;
        let inference_request = InferenceRequest {
            turn_id: GenerationTurnId::from_uuid(request.request_id.as_uuid()),
            attempt_id: GenerationAttemptId::from_uuid(uuid::Uuid::new_v5(
                &handle.id().as_uuid(),
                STAGE.as_bytes(),
            )),
            operation: GenerationOperation::Send,
            profile: ResolvedInferenceProfile {
                chat_profile,
                tool_policy: ToolPolicy::Disabled,
                output_policy: OutputPolicy::Plain,
                safety_policy: SafetyContext::Standard,
                correlation_id: None,
            },
            context: ProviderNeutralContext {
                messages,
                attributions: Default::default(),
                budget: Default::default(),
            },
            cancellation: Some(handle.id()),
            stream_sink: None,
            media_grants,
            tools: None,
            prompt_cache_key: None,
        };
        inference_request
            .validate()
            .map_err(|_| ScenePromptError::InvalidPrompt)?;
        Ok(inference_request)
    }

    /// Legacy's loaded session: the latest messages of the active branch plus
    /// every older pinned one, oldest first, whatever their visibility.
    fn session_window(
        &self,
        conversation: &lettuce_conversations::Conversation,
    ) -> Result<Vec<TimelineItem>, ScenePromptError> {
        let mut window = Vec::new();
        let mut seen = 0usize;
        let mut cursor = None;
        loop {
            let page = self
                .repository
                .timeline_page(
                    conversation.id,
                    conversation.active_branch_id,
                    &PageRequest {
                        cursor,
                        limit: PageLimit::new(200),
                    },
                )
                .map_err(|_| ScenePromptError::Storage)?;
            for item in page.items {
                if seen < SESSION_WINDOW || item.message.pinned {
                    window.push(item);
                }
                seen += 1;
            }
            let Some(next) = page.next_cursor else {
                break;
            };
            cursor = Some(next);
        }
        window.reverse();
        Ok(window)
    }
}

struct WriterPersona<'a> {
    title: &'a str,
    description: &'a str,
    design_description: Option<&'a str>,
}

struct WriterInputs<'a> {
    character: &'a lettuce_characters::CharacterProfile,
    persona: Option<WriterPersona<'a>>,
    references: &'a SceneReferences,
    recent: String,
    character_lora: String,
    persona_lora: String,
    image_model_instructions: String,
}

/// Legacy `render_scene_generation_prompt_content`'s placeholder values.
fn render_values(
    text: &RuntimeText,
    inputs: WriterInputs<'_>,
) -> Result<PromptRenderValues, ScenePromptError> {
    let WriterInputs {
        character: profile,
        persona,
        references,
        recent,
        character_lora,
        persona_lora,
        image_model_instructions,
    } = inputs;
    let persona_name = persona.as_ref().map_or_else(
        || {
            text.render_with("scene_writer_default_persona_name", [])
                .unwrap_or_default()
        },
        |persona| persona.title.to_owned(),
    );
    let design_notes = |notes: Option<&str>| {
        notes
            .map(str::trim)
            .filter(|notes| !notes.is_empty())
            .map(|notes| {
                text.render_with(
                    "scene_writer_design_notes",
                    [(Variable::SubjectDescription, notes.to_owned())],
                )
                .unwrap_or_default()
            })
    };
    let character_description = [
        profile
            .definition
            .as_deref()
            .or(profile.description.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned),
        design_notes(profile.design_description.as_deref()),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join("\n\n");
    let persona_description = persona
        .as_ref()
        .map(|persona| {
            [
                Some(persona.description.trim())
                    .filter(|value| !value.is_empty())
                    .map(str::to_owned),
                design_notes(persona.design_description),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join("\n\n")
        })
        .unwrap_or_default();
    let scene_request = match &persona {
        Some(persona) => text.render_with(
            "scene_writer_request_with_persona",
            [
                (Variable::SubjectName, profile.name.clone()),
                (Variable::OtherSubjectName, persona.title.to_owned()),
            ],
        ),
        None => text.render_with(
            "scene_writer_request",
            [(Variable::SubjectName, profile.name.clone())],
        ),
    }
    .map_err(|_| ScenePromptError::MissingPrompt)?;
    let mut purpose_values = BTreeMap::from([
        (Variable::RecentMessages, recent),
        (Variable::SceneRequest, scene_request),
        (
            Variable::CharacterReferenceText,
            reference_text(text, &references.character),
        ),
        (
            Variable::PersonaReferenceText,
            references
                .persona
                .as_ref()
                .map(|persona| reference_text(text, persona))
                .unwrap_or_default(),
        ),
        (Variable::CharacterLoraKeywords, character_lora),
        (Variable::PersonaLoraKeywords, persona_lora),
        (Variable::ImageModelInstructions, image_model_instructions),
        (Variable::CharacterImage, CHARACTER_TOKEN.to_owned()),
        (Variable::PersonaImage, PERSONA_TOKEN.to_owned()),
        (Variable::ChatBackgroundImage, BACKGROUND_TOKEN.to_owned()),
    ]);
    if references.background.is_some() {
        purpose_values.insert(
            Variable::ChatBackgroundReferenceText,
            text.render_with("scene_writer_background_notes", [])
                .map_err(|_| ScenePromptError::MissingPrompt)?,
        );
    }
    Ok(PromptRenderValues {
        character_name: profile.name.clone(),
        character_description,
        persona_name: persona_name.clone(),
        persona_description,
        user_name: persona_name,
        purpose_values,
        ..Default::default()
    })
}

/// What a message shows now, without legacy inline image tokens.
fn shown_text(item: &TimelineItem) -> String {
    let parts = match item.message.active_render_source {
        MessageRenderSource::Candidate(_) => item
            .active_candidate
            .as_ref()
            .map(|candidate| candidate.parts.as_slice()),
        MessageRenderSource::Revision(_) => item
            .active_revision
            .as_ref()
            .map(|revision| revision.parts.as_slice()),
    }
    .unwrap_or_default();
    strip_inline_image_tokens(&message_text(parts))
}

/// Legacy `strip_inline_image_tokens`: drops every `{{image:…}}` span; an
/// unclosed span and what follows it stay.
fn strip_inline_image_tokens(text: &str) -> String {
    const PREFIX: &str = "{{image:";
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find(PREFIX) {
        let Some(end) = rest[start..].find("}}") else {
            break;
        };
        out.push_str(&rest[..start]);
        rest = &rest[start + end + 2..];
    }
    out.push_str(rest);
    out
}

/// Legacy `build_scene_prompt_context_messages`: the message and the two
/// positions before it in the loaded window, as "Role: text" lines.
fn recent_messages(
    text: &RuntimeText,
    window: &[TimelineItem],
    message_id: MessageId,
) -> Result<String, ScenePromptError> {
    let target = window
        .iter()
        .position(|item| item.message.id == message_id)
        .ok_or(ScenePromptError::MessageNotFound)?;
    let mut lines = Vec::new();
    for item in &window[target.saturating_sub(2)..=target] {
        let label = match item.message.role {
            MessageRole::User => "scene_writer_role_user",
            MessageRole::Assistant => "scene_writer_role_assistant",
            MessageRole::Scene => "scene_writer_role_scene",
            MessageRole::System => continue,
        };
        let content = shown_text(item);
        let content = content.trim();
        if content.is_empty() {
            continue;
        }
        let label = text
            .render_with(label, [])
            .map_err(|_| ScenePromptError::MissingPrompt)?;
        lines.push(
            text.render_with(
                "scene_writer_message_line",
                [
                    (Variable::SpeakerName, label),
                    (Variable::MessageText, content.to_owned()),
                ],
            )
            .map_err(|_| ScenePromptError::MissingPrompt)?,
        );
    }
    let context = lines.join("\n\n");
    if context.trim().is_empty() {
        return Err(ScenePromptError::NoContext);
    }
    Ok(context)
}

/// Legacy `build_scene_prompt_reference_hint`, from what the subject stores.
fn reference_hint(text: &RuntimeText, subject: &SceneSubject) -> String {
    let name = (Variable::SubjectName, subject.name.clone());
    match subject.stored_source {
        Some(ReferenceSource::Design) if subject.stored_design_count > 0 => text.render_with(
            if subject.stored_design_count == 1 {
                "scene_writer_design_hint_one"
            } else {
                "scene_writer_design_hint_many"
            },
            [
                (
                    Variable::ItemNumber,
                    subject.stored_design_count.to_string(),
                ),
                name,
            ],
        ),
        Some(ReferenceSource::Avatar) => text.render_with("scene_writer_avatar_hint", [name]),
        _ => Ok(String::new()),
    }
    .unwrap_or_default()
}

/// Legacy `build_scene_prompt_reference_text`: the subject's design notes
/// and the hint about its reference images.
fn reference_text(text: &RuntimeText, subject: &SceneSubject) -> String {
    let notes = subject.design_notes.as_ref().map(|notes| {
        text.render_with(
            "scene_writer_reference_notes",
            [
                (Variable::SubjectName, subject.name.clone()),
                (Variable::SubjectDescription, notes.clone()),
            ],
        )
        .unwrap_or_default()
    });
    notes
        .into_iter()
        .chain(Some(reference_hint(text, subject)))
        .filter(|section| !section.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Whether legacy's reference text for the images actually sent is non-empty.
fn has_reference_text(subject: &SceneSubject) -> bool {
    subject.design_notes.is_some() || subject.source.is_some()
}

/// Legacy `condense_prompt_whitespace`.
fn condense(input: &str) -> String {
    let mut output = input.to_owned();
    while output.contains("\n\n\n") {
        output = output.replace("\n\n\n", "\n\n");
    }
    output.trim().to_owned()
}

/// Legacy `render_scene_generation_prompt_entries`: the active entries in
/// template order, each condensed, and merged into one system entry (image
/// entries kept) when the template condenses.
fn render_entries(
    document: &PromptDocument,
    context: PromptRenderContext,
) -> Result<Vec<WriterEntry>, ScenePromptError> {
    let mut placed = document.clone();
    for entry in &mut placed.entries {
        if matches!(
            entry.injection_position,
            PromptEntryPosition::Conditional | PromptEntryPosition::Interval
        ) {
            entry.injection_position = PromptEntryPosition::InChat;
        }
    }
    let rendered = render_prompt(&placed, &context).map_err(|_| ScenePromptError::InvalidPrompt)?;
    let mut entries = rendered
        .relative
        .into_iter()
        .chain(rendered.in_chat)
        .filter_map(|message| {
            let index = document
                .entries
                .iter()
                .position(|entry| entry.id == message.entry_id)?;
            let entry = &document.entries[index];
            let content = condense(&message.content);
            if content.is_empty() && message.payload.is_none() {
                return None;
            }
            Some((
                index,
                WriterEntry {
                    role: message.role,
                    content,
                    slot: message.payload.map(|payload| match payload {
                        PromptEntryPayload::ImageSlot { slot } => slot,
                    }),
                    position: entry.injection_position,
                    depth: entry.depth,
                    conditional_min_messages: entry.conditional_min_messages,
                    interval_turns: entry.interval_turns,
                },
            ))
        })
        .collect::<Vec<_>>();
    entries.sort_by_key(|(index, _)| *index);
    let entries = entries.into_iter().map(|(_, entry)| entry);
    if !document.condense {
        return Ok(entries.collect());
    }
    let (images, sections): (Vec<_>, Vec<_>) = entries.partition(WriterEntry::image_bound);
    let merged = sections
        .iter()
        .map(|entry| entry.content.trim())
        .filter(|content| !content.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    Ok((!merged.trim().is_empty())
        .then_some(WriterEntry {
            role: PromptEntryRole::System,
            content: merged,
            slot: None,
            position: PromptEntryPosition::Relative,
            depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
        })
        .into_iter()
        .chain(images)
        .collect())
}

/// The images an entry carries: its payload slot's, else those its legacy
/// tokens name.
fn entry_images(entry: &WriterEntry, references: &SceneReferences) -> Vec<AssetId> {
    let persona = || {
        references
            .persona
            .as_ref()
            .map(|persona| persona.references.clone())
            .unwrap_or_default()
    };
    match entry.slot {
        Some(PromptEntryImageSlot::Character) => references.character.references.clone(),
        Some(PromptEntryImageSlot::Persona) => persona(),
        Some(PromptEntryImageSlot::ChatBackground) => references.background.into_iter().collect(),
        Some(PromptEntryImageSlot::Avatar | PromptEntryImageSlot::References) => Vec::new(),
        None => {
            let mut images = Vec::new();
            if entry.content.contains(CHARACTER_TOKEN) {
                images.extend(references.character.references.iter().copied());
            }
            if entry.content.contains(PERSONA_TOKEN) {
                images.extend(persona());
            }
            if entry.content.contains(BACKGROUND_TOKEN)
                || entry.content.contains(BACKGROUND_ALIAS_TOKEN)
            {
                images.extend(references.background);
            }
            images
        }
    }
}

fn strip_tokens(content: &str, tokens: &[&str]) -> String {
    tokens.iter().fold(content.to_owned(), |content, token| {
        content.replace(token, "")
    })
}

/// Legacy `scene_prompt_entry_to_message`. An entry with images becomes a
/// user message with its text first; an image entry without images is
/// dropped. Legacy replaced the tokens left in other entries with reference
/// hints that were always empty there, so they are removed. A blank message
/// is `Some(None)`: not sent, but still a position for in-chat placement.
fn entry_message(
    entry: &WriterEntry,
    references: &SceneReferences,
) -> Option<Option<ProviderNeutralMessage>> {
    let images = entry_images(entry, references);
    if !images.is_empty() {
        let text = condense(&strip_tokens(&entry.content, &SCENE_TOKENS));
        return Some(Some(ProviderNeutralMessage {
            role: MessageRole::User,
            parts: (!text.is_empty())
                .then_some(ProviderContextPart::Text { text })
                .into_iter()
                .chain(
                    images
                        .into_iter()
                        .map(|asset_id| ProviderContextPart::MediaAsset {
                            asset_id,
                            role: MediaAssetRole::Reference,
                        }),
                )
                .collect(),
        }));
    }
    let has_tokens = SCENE_TOKENS
        .iter()
        .any(|token| entry.content.contains(token));
    if entry.slot.is_some() || (has_tokens && entry.role == PromptEntryRole::User) {
        return None;
    }
    let text = strip_tokens(&entry.content, &SCENE_TOKENS);
    Some((!text.trim().is_empty()).then(|| ProviderNeutralMessage {
        role: match entry.role {
            PromptEntryRole::System => MessageRole::System,
            PromptEntryRole::User => MessageRole::User,
            PromptEntryRole::Assistant => MessageRole::Assistant,
        },
        parts: vec![ProviderContextPart::Text { text }],
    }))
}

/// Legacy `should_insert_in_chat_prompt_entry` against the number of
/// relative messages.
fn inserts_in_chat(entry: &WriterEntry, turn_count: usize) -> bool {
    match entry.position {
        PromptEntryPosition::InChat => true,
        PromptEntryPosition::Conditional => {
            turn_count >= entry.conditional_min_messages.unwrap_or(1) as usize
        }
        PromptEntryPosition::Interval => {
            let interval = entry.interval_turns.unwrap_or_default() as usize;
            interval > 0 && turn_count > 0 && turn_count % interval == 0
        }
        PromptEntryPosition::Relative => false,
    }
}

/// The relative entries as messages, then legacy
/// `insert_scene_in_chat_prompt_entries`: each in-chat entry lands `depth`
/// messages from the end, shifted by the entries placed before it (dropped
/// ones included).
fn writer_messages(
    entries: &[WriterEntry],
    references: &SceneReferences,
) -> Vec<ProviderNeutralMessage> {
    let mut messages = entries
        .iter()
        .filter(|entry| entry.position == PromptEntryPosition::Relative)
        .filter_map(|entry| entry_message(entry, references))
        .collect::<Vec<_>>();
    let base_len = messages.len();
    let mut inserts = entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| {
            entry.position != PromptEntryPosition::Relative && inserts_in_chat(entry, base_len)
        })
        .map(|(index, entry)| (base_len.saturating_sub(entry.depth as usize), index, entry))
        .collect::<Vec<_>>();
    inserts.sort_by_key(|(position, index, _)| (*position, *index));
    for (offset, (position, _, entry)) in inserts.into_iter().enumerate() {
        let at = (position + offset).min(messages.len());
        if let Some(message) = entry_message(entry, references) {
            messages.insert(at, message);
        }
    }
    messages.into_iter().flatten().collect()
}

fn message_text(parts: &[MessagePart]) -> String {
    parts
        .iter()
        .filter_map(|part| match part {
            MessagePart::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn generated_text(outcome: &InferenceOutcome) -> Option<String> {
    let candidate = outcome.candidates.first()?;
    candidate
        .parts
        .iter()
        .any(|part| matches!(part, MessagePart::Text { .. }))
        .then(|| message_text(&candidate.parts))
}

/// Legacy's cleanup of the writer's answer.
fn clean_scene_prompt(text: &str) -> String {
    condense(
        text.trim()
            .trim_matches('"')
            .trim()
            .trim_start_matches("<img>")
            .trim_end_matches("</img>")
            .trim_end_matches("[CONTINUE]")
            .trim_end_matches("[continue]")
            .trim_end_matches("[/continue]")
            .trim(),
    )
}

#[cfg(test)]
mod tests {
    use lettuce_conversations::{Message, MessageRevision, MessageVisibility};
    use lettuce_types::{ConversationBranchId, MessageRevisionId, Revision};

    use super::*;

    fn subject(
        name: &str,
        notes: Option<&str>,
        references: Vec<AssetId>,
        source: Option<ReferenceSource>,
    ) -> SceneSubject {
        SceneSubject {
            name: name.to_owned(),
            design_notes: notes.map(str::to_owned),
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

    fn profile() -> lettuce_characters::CharacterProfile {
        lettuce_characters::CharacterProfile {
            name: "Mira".into(),
            nickname: None,
            description: Some("Unused description".into()),
            definition: Some(" A sailor. ".into()),
            design_description: Some("red coat".into()),
            scenario: None,
            rules: Vec::new(),
        }
    }

    fn texts(message: &ProviderNeutralMessage) -> Vec<&str> {
        message
            .parts
            .iter()
            .filter_map(|part| match part {
                ProviderContextPart::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }

    fn images(message: &ProviderNeutralMessage) -> Vec<AssetId> {
        message
            .parts
            .iter()
            .filter_map(|part| match part {
                ProviderContextPart::MediaAsset { asset_id, .. } => Some(*asset_id),
                _ => None,
            })
            .collect()
    }

    fn conditions(local: bool, references: &SceneReferences) -> PromptConditionContext {
        PromptConditionContext {
            is_local_image_generation_model: local,
            is_scene_generation_local_image_model: local,
            has_persona: references.persona.is_some(),
            has_character_reference_images: !references.character.references.is_empty(),
            has_chat_background: references.background.is_some(),
            has_persona_reference_images: references
                .persona
                .as_ref()
                .is_some_and(|persona| !persona.references.is_empty()),
            has_character_reference_text: has_reference_text(&references.character),
            has_persona_reference_text: references.persona.as_ref().is_some_and(has_reference_text),
            ..Default::default()
        }
    }

    #[test]
    fn remote_writer_messages_follow_legacy_scene_prompt_order() {
        let text = RuntimeText::from_seed(BuiltInPromptId::ChatRuntime);
        let (first, second, avatar, background) = (
            AssetId::new(),
            AssetId::new(),
            AssetId::new(),
            AssetId::new(),
        );
        let references = SceneReferences {
            character: subject(
                "Mira",
                Some("red coat"),
                vec![first, second],
                Some(ReferenceSource::Design),
            ),
            persona: Some(subject(
                "Sol",
                None,
                vec![avatar],
                Some(ReferenceSource::Avatar),
            )),
            background: Some(background),
        };
        let profile = profile();
        let values = render_values(
            &text,
            WriterInputs {
                character: &profile,
                persona: Some(WriterPersona {
                    title: "Sol",
                    description: "A traveler.",
                    design_description: None,
                }),
                references: &references,
                recent: "User: Hello\n\nAssistant: Hi".into(),
                character_lora: String::new(),
                persona_lora: String::new(),
                image_model_instructions: "Use tags.".into(),
            },
        )
        .expect("values");
        let document = crate::built_in_prompts::seed_document(BuiltInPromptId::ScenePromptWriter);
        let entries = render_entries(
            &document,
            PromptRenderContext {
                conditions: conditions(false, &references),
                values,
            },
        )
        .expect("entries");
        let messages = writer_messages(&entries, &references);
        let roles = messages
            .iter()
            .map(|message| message.role)
            .collect::<Vec<_>>();
        assert_eq!(
            roles,
            [
                [MessageRole::System; 4].as_slice(),
                [MessageRole::User; 7].as_slice()
            ]
            .concat()
        );
        assert_eq!(texts(&messages[1]), ["Use tags."]);
        assert_eq!(
            texts(&messages[4]),
            [
                "# Scene Context\nCharacter: Mira\nA sailor.\n\nVisual design notes: red coat\n\n\
              Persona: Sol\nA traveler.\n\nRecent Messages:\nUser: Hello\n\nAssistant: Hi"
            ]
        );
        assert!(texts(&messages[5]).is_empty());
        assert_eq!(images(&messages[5]), [first, second]);
        assert_eq!(
            texts(&messages[6]),
            ["# Mira Reference Notes\nred coat\n\n\
              The image model will receive 2 saved design reference images for Mira."]
        );
        assert!(texts(&messages[7])[0].starts_with("# Chat Background Reference\n"));
        assert_eq!(images(&messages[7]), [background]);
        assert_eq!(images(&messages[8]), [avatar]);
        assert_eq!(
            texts(&messages[9]),
            ["The image model will receive Sol's base avatar as a visual reference."]
        );
        assert_eq!(
            texts(&messages[10]),
            [
                "# Scene Request\nCreate one polished scene image prompt for the visual moment \
              described by the recent messages. Focus on the currently active beat involving \
              Mira and Sol. Keep Mira and Sol visually distinct, and make the result \
              immediately usable for image generation."
            ]
        );

        let mut condensed = document.clone();
        condensed.condense = true;
        let values = render_values(
            &text,
            WriterInputs {
                character: &profile,
                persona: None,
                references: &references,
                recent: "User: Hello".into(),
                character_lora: String::new(),
                persona_lora: String::new(),
                image_model_instructions: String::new(),
            },
        )
        .expect("values");
        let entries = render_entries(
            &condensed,
            PromptRenderContext {
                conditions: conditions(false, &references),
                values,
            },
        )
        .expect("entries");
        let messages = writer_messages(&entries, &references);
        assert_eq!(messages.len(), 4);
        assert_eq!(messages[0].role, MessageRole::System);
        let merged = texts(&messages[0])[0];
        assert!(merged.contains("Persona: user\n\nRecent Messages:\nUser: Hello"));
        assert!(merged.contains("involving Mira. Make the result immediately usable"));
        assert!(merged.ends_with("do not wrap the result in quotes."));
        assert_eq!(images(&messages[1]), [first, second]);
        assert_eq!(images(&messages[2]), [background]);
        assert_eq!(images(&messages[3]), [avatar]);
    }

    #[test]
    fn local_writer_messages_carry_the_lora_bindings() {
        let text = RuntimeText::from_seed(BuiltInPromptId::ChatRuntime);
        let references = SceneReferences {
            character: subject("Mira", Some("red coat"), Vec::new(), None),
            persona: Some(subject("Sol", None, Vec::new(), None)),
            background: None,
        };
        let profile = profile();
        let values = render_values(
            &text,
            WriterInputs {
                character: &profile,
                persona: Some(WriterPersona {
                    title: "Sol",
                    description: "",
                    design_description: None,
                }),
                references: &references,
                recent: "User: Hello".into(),
                character_lora: "mira_v2".into(),
                persona_lora: "secondary subject".into(),
                image_model_instructions: String::new(),
            },
        )
        .expect("values");
        let entries = render_entries(
            &crate::built_in_prompts::seed_document(BuiltInPromptId::ScenePromptWriter),
            PromptRenderContext {
                conditions: conditions(true, &references),
                values,
            },
        )
        .expect("entries");
        let messages = writer_messages(&entries, &references);
        let roles = messages
            .iter()
            .map(|message| message.role)
            .collect::<Vec<_>>();
        assert_eq!(
            roles,
            [
                MessageRole::System,
                MessageRole::System,
                MessageRole::System,
                MessageRole::User,
                MessageRole::User
            ]
        );
        assert_eq!(
            texts(&messages[3]),
            [
                "Primary subject (Assistant) trigger keywords: mira_v2\n\nRecent Messages:\nUser: Hello"
            ]
        );
        assert_eq!(
            texts(&messages[4]),
            ["Secondary subject (User) trigger keywords: secondary subject"]
        );
    }

    fn entry(
        content: &str,
        position: PromptEntryPosition,
        depth: u32,
        slot: Option<PromptEntryImageSlot>,
    ) -> WriterEntry {
        WriterEntry {
            role: PromptEntryRole::System,
            content: content.to_owned(),
            slot,
            position,
            depth,
            conditional_min_messages: Some(3),
            interval_turns: Some(2),
        }
    }

    #[test]
    fn in_chat_entries_are_placed_like_legacy() {
        use PromptEntryPosition::{Conditional, InChat, Interval, Relative};
        let references = SceneReferences {
            character: subject("Mira", None, Vec::new(), None),
            persona: None,
            background: None,
        };
        let entries = [
            entry("A", Relative, 0, None),
            entry("B", Relative, 0, None),
            entry("X", InChat, 1, None),
            entry("", InChat, 0, Some(PromptEntryImageSlot::Persona)),
            entry("Y", InChat, 0, None),
            entry("Z", Conditional, 0, None),
            entry("W", Interval, 0, None),
            entry("T {{image[character]}}", InChat, 0, None),
        ];
        let messages = writer_messages(&entries, &references);
        let contents = messages
            .iter()
            .map(|message| texts(message)[0])
            .collect::<Vec<_>>();
        assert_eq!(contents, ["A", "X", "B", "Y", "W", "T "]);

        let blank = [
            entry("A", Relative, 0, None),
            entry("{{image[persona]}}", Relative, 0, None),
            entry("B", Relative, 0, None),
            entry("X", InChat, 2, None),
            entry("Z", Conditional, 0, None),
        ];
        let messages = writer_messages(&blank, &references);
        let contents = messages
            .iter()
            .map(|message| texts(message)[0])
            .collect::<Vec<_>>();
        assert_eq!(contents, ["A", "X", "B", "Z"]);
    }

    fn timeline_item(role: MessageRole, text: &str) -> TimelineItem {
        let id = MessageId::new();
        let revision = MessageRevisionId::new();
        TimelineItem {
            message: Message {
                id,
                conversation_id: ConversationId::new(),
                branch_id: ConversationBranchId::new(),
                parent_message_id: None,
                author_participant_id: None,
                role,
                logical_time: TimestampMillis::new(1),
                effective_time: TimestampMillis::new(1),
                visibility: MessageVisibility::Hidden,
                pinned: false,
                scene_edited: false,
                active_render_source: MessageRenderSource::Revision(revision),
                revision: Revision::INITIAL,
                created_at: TimestampMillis::new(1),
                updated_at: TimestampMillis::new(1),
            },
            active_revision: Some(MessageRevision {
                id: revision,
                message_id: id,
                sequence: Revision::INITIAL,
                parts: vec![MessagePart::Text { text: text.into() }],
                authored_at: TimestampMillis::new(1),
                source_turn_id: None,
                provider_replay: None,
            }),
            active_candidate: None,
            initial_origin: None,
        }
    }

    #[test]
    fn recent_messages_take_the_target_and_two_positions_before_it() {
        let text = RuntimeText::from_seed(BuiltInPromptId::ChatRuntime);
        let window = [
            timeline_item(MessageRole::User, "Too early"),
            timeline_item(MessageRole::System, "Rules"),
            timeline_item(MessageRole::Scene, " A harbor {{image:abc}}at dusk "),
            timeline_item(MessageRole::Assistant, "{{image:x}}"),
            timeline_item(MessageRole::User, "After"),
        ];
        assert_eq!(
            recent_messages(&text, &window, window[3].message.id).expect("context"),
            "Scene: A harbor at dusk"
        );
        assert!(matches!(
            recent_messages(&text, &window[1..4], window[3].message.id)
                .map(|context| context.len()),
            Ok(23)
        ));
        let silent = [
            timeline_item(MessageRole::System, "Rules"),
            timeline_item(MessageRole::Assistant, "{{image:x}}"),
        ];
        assert!(matches!(
            recent_messages(&text, &silent, silent[1].message.id),
            Err(ScenePromptError::NoContext)
        ));
        assert!(matches!(
            recent_messages(&text, &window, MessageId::new()),
            Err(ScenePromptError::MessageNotFound)
        ));
        assert_eq!(
            recent_messages(&text, &window, window[4].message.id).expect("context"),
            "Scene: A harbor at dusk\n\nUser: After"
        );
    }

    #[test]
    fn writer_answers_are_cleaned_like_legacy() {
        assert_eq!(
            clean_scene_prompt("  \"<img>harbor at dusk\n\n\n\nsoft light</img>\" "),
            "harbor at dusk\n\nsoft light"
        );
        assert_eq!(clean_scene_prompt("harbor[continue][CONTINUE]"), "harbor");
        assert_eq!(clean_scene_prompt("harbor</img>[CONTINUE]"), "harbor</img>");
        assert_eq!(clean_scene_prompt("\"\""), "");
        assert_eq!(
            strip_inline_image_tokens("a {{image:1}}b {{image:open"),
            "a b {{image:open"
        );
    }
}
