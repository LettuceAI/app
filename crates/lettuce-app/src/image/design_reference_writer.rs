//! Design reference notes from a subject's images (legacy
//! `chat_generate_design_reference_description`): the scene writer model
//! looks at the avatar and reference images and writes the design text the
//! character or persona editor offers as a draft.

use std::collections::{BTreeMap, HashSet};
use std::time::Duration;

use lettuce_context::{
    PromptConditionContext, PromptDocument, PromptEntryChatMode, PromptEntryImageSlot,
    PromptEntryPosition, PromptEntryRole, PromptRenderContext, PromptRenderValues,
    PromptRepository, PromptVariable as Variable,
};
use lettuce_conversations::{
    GenerationOperation, InferenceOutcome, InferencePort, InferenceRequest, MediaAssetRole,
    MessagePart, MessageRole, OutputPolicy, PortError, ProviderContextPart, ProviderFailureKind,
    ProviderNeutralContext, ProviderNeutralMessage, ProviderReplayArtifactPort,
    ResolvedInferenceProfile, SafetyContext, ToolPolicy,
};
use lettuce_jobs::{
    JobError, JobErrorCode, JobSnapshot, JobStore, ResourceAvailability, StoreError, SubjectKind,
    WorkerId, handle::JobHandle,
};
use lettuce_models::{
    CapabilityStatus, ChatRequirements, ExpectedModelIdentity, ModelCatalog, ReasoningMode,
};
use lettuce_settings::{GlobalSettings, GlobalSettingsStore};
use lettuce_types::{AssetId, GenerationAttemptId, GenerationTurnId, RequestId, TimestampMillis};
use lettuce_usage::JobUsageLedger;

use crate::generation::feature_prompt_entries::{
    FeatureEntry, condense, message_role, render_feature_entries, strip_tokens,
};
use crate::generation::runtime_text::RuntimeText;
use crate::jobs::job_inference_usage::{JobInferenceError, run_job_inference};
use crate::{BuiltInPromptId, ImageFeatureModelError, cleanup_outcome_replays};

const STAGE: &str = "design-reference";
const AVATAR_TOKEN: &str = "{{image[avatar]}}";
const REFERENCES_TOKEN: &str = "{{image[references]}}";
const DESIGN_TOKENS: [&str; 2] = [AVATAR_TOKEN, REFERENCES_TOKEN];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesignReferenceRequest {
    pub request_id: RequestId,
    pub subject_name: Option<String>,
    pub subject_description: Option<String>,
    pub current_description: Option<String>,
    pub avatar: Option<AssetId>,
    pub references: Vec<AssetId>,
    /// Streams the draft to `request_id` when the model can stream.
    pub stream: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesignReferenceReply {
    pub text: String,
    pub job: JobSnapshot,
}

#[derive(Debug, thiserror::Error)]
pub enum DesignReferenceError {
    #[error(transparent)]
    Model(#[from] ImageFeatureModelError),
    #[error("At least one avatar or reference image is required")]
    NoImages,
    #[error("Design reference template rendered no prompt content")]
    NoContent,
    #[error("Design reference writer model is invalid: {0}")]
    InvalidModel(lettuce_models::ChatProfileResolutionError),
    #[error("Design reference prompt is unavailable")]
    MissingPrompt,
    #[error("Design reference prompt could not be rendered")]
    InvalidPrompt,
    #[error("design reference storage failed")]
    Storage,
    #[error("design reference job storage failed: {0}")]
    Jobs(#[from] StoreError),
    #[error("design reference request was already settled")]
    AlreadySettled,
    #[error("design reference job could not be claimed")]
    NotClaimed,
    #[error("design reference inference failed: {0}")]
    Inference(PortError),
    #[error("design reference usage evidence could not be recorded")]
    Evidence,
    #[error("Failed to extract text from response")]
    NoText,
    #[error("Design reference generation returned an empty result")]
    Empty,
    #[error("design reference request was cancelled")]
    Cancelled,
    #[error("design reference replay cleanup failed")]
    ReplayCleanup,
}

impl From<crate::jobs::one_shot_job::OneShotJobError> for DesignReferenceError {
    fn from(error: crate::jobs::one_shot_job::OneShotJobError) -> Self {
        match error {
            crate::jobs::one_shot_job::OneShotJobError::Jobs(error) => Self::Jobs(error),
            crate::jobs::one_shot_job::OneShotJobError::AlreadySettled => Self::AlreadySettled,
            crate::jobs::one_shot_job::OneShotJobError::NotClaimed => Self::NotClaimed,
        }
    }
}

impl crate::jobs::one_shot_job::OneShotFailure for DesignReferenceError {
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
                "design-reference-unavailable",
            ),
            Self::Inference(PortError::Provider(failure))
                if failure.kind == ProviderFailureKind::Unavailable =>
            {
                (
                    JobErrorCode::ResourceUnavailable,
                    true,
                    "design-reference-unavailable",
                )
            }
            Self::Inference(_) | Self::NoText | Self::Empty => (
                JobErrorCode::WorkerFailed,
                false,
                "design-reference-inference-failed",
            ),
            Self::Jobs(_) | Self::Storage | Self::Model(ImageFeatureModelError::Storage) => (
                JobErrorCode::StorageFailure,
                true,
                "design-reference-storage-failed",
            ),
            Self::Model(_)
            | Self::NoImages
            | Self::NoContent
            | Self::InvalidModel(_)
            | Self::MissingPrompt
            | Self::InvalidPrompt
            | Self::Cancelled => (
                JobErrorCode::InvalidInput,
                false,
                "design-reference-invalid-input",
            ),
        };
        JobError::new(code, retryable, message).expect("constant job error is valid")
    }
}

/// Every port the design reference writer reads; the composition root's
/// database is one.
pub trait DesignReferenceSources:
    GlobalSettingsStore
    + ModelCatalog
    + lettuce_models::GlobalModelSettingsRepository
    + PromptRepository
    + JobStore
    + JobUsageLedger
    + ProviderReplayArtifactPort
{
}

impl<T> DesignReferenceSources for T where
    T: GlobalSettingsStore
        + ModelCatalog
        + lettuce_models::GlobalModelSettingsRepository
        + PromptRepository
        + JobStore
        + JobUsageLedger
        + ProviderReplayArtifactPort
{
}

#[derive(Debug)]
pub struct DesignReferenceWriter<'a, R: ?Sized, I: ?Sized> {
    repository: &'a R,
    inference: &'a I,
}

impl<'a, R: ?Sized, I: ?Sized> DesignReferenceWriter<'a, R, I> {
    #[must_use]
    pub const fn new(repository: &'a R, inference: &'a I) -> Self {
        Self {
            repository,
            inference,
        }
    }
}

impl<R, I> DesignReferenceWriter<'_, R, I>
where
    R: DesignReferenceSources + ?Sized,
    I: InferencePort + ?Sized,
{
    pub async fn generate(
        &self,
        request: &DesignReferenceRequest,
        worker_id: WorkerId,
        now: TimestampMillis,
        lease_for: Duration,
        allowed: &ResourceAvailability,
    ) -> Result<DesignReferenceReply, DesignReferenceError> {
        let settings = GlobalSettingsStore::load(self.repository)
            .map_err(|_| DesignReferenceError::Storage)?
            .settings;
        let writer = crate::scene_writer_model(self.repository, &settings, true)?;
        if request.avatar.is_none() && request.references.is_empty() {
            return Err(DesignReferenceError::NoImages);
        }
        let (settings, writer) = (&settings, &writer);
        let (text, job) = crate::jobs::one_shot_job::run_one_shot_job(
            self.repository,
            crate::jobs::one_shot_job::OneShotJob {
                name: "design-reference",
                stage: STAGE,
                subject_kind: SubjectKind::ModelProfile,
                subject: &writer.profile.id.to_string(),
                request_id: request.request_id,
            },
            crate::jobs::one_shot_job::OneShotLease {
                worker_id,
                now,
                lease_for,
                allowed,
            },
            |handle| async move { self.run(settings, writer, request, &handle, now).await },
        )
        .await?;
        Ok(DesignReferenceReply { text, job })
    }

    async fn run(
        &self,
        settings: &GlobalSettings,
        writer: &crate::FeatureModel,
        request: &DesignReferenceRequest,
        handle: &JobHandle,
        now: TimestampMillis,
    ) -> Result<String, DesignReferenceError> {
        let inference_request = self.prepare(settings, writer, request, handle)?;
        if handle.cancellation_token().is_cancelled() {
            return Err(DesignReferenceError::Cancelled);
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
                return Err(DesignReferenceError::Cancelled);
            }
            Err(JobInferenceError::Provider(error)) => {
                return Err(DesignReferenceError::Inference(error));
            }
            Err(JobInferenceError::Evidence) => return Err(DesignReferenceError::Evidence),
        };
        let text = generated_text(&outcome);
        cleanup_outcome_replays(self.repository, &outcome)
            .map_err(|_| DesignReferenceError::ReplayCleanup)?;
        let cleaned = clean_design_reference(&text.ok_or(DesignReferenceError::NoText)?);
        if cleaned.is_empty() {
            return Err(DesignReferenceError::Empty);
        }
        Ok(cleaned)
    }

    fn prepare(
        &self,
        settings: &GlobalSettings,
        writer: &crate::FeatureModel,
        request: &DesignReferenceRequest,
        handle: &JobHandle,
    ) -> Result<InferenceRequest, DesignReferenceError> {
        let text = RuntimeText::load(self.repository, BuiltInPromptId::ChatRuntime)
            .map_err(|_| DesignReferenceError::MissingPrompt)?;
        let present = |value: &Option<String>| {
            value
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
        };
        let subject_name = match present(&request.subject_name) {
            Some(name) => name,
            None => text
                .render_with("design_reference_unnamed_subject", [])
                .map_err(|_| DesignReferenceError::MissingPrompt)?,
        };
        let subject_description = present(&request.subject_description);
        let current_description = present(&request.current_description);
        let capabilities = &writer.profile.config.capabilities;
        let conditions = PromptConditionContext {
            chat_mode: PromptEntryChatMode::Direct,
            scene_generation_enabled: settings.image_generation.scene_enabled,
            avatar_generation_enabled: settings.image_generation.avatar_enabled,
            participant_count: 1,
            recent_text: [
                Some(subject_name.as_str()),
                subject_description.as_deref(),
                current_description.as_deref(),
            ]
            .into_iter()
            .flatten()
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
            has_subject_description: subject_description.is_some(),
            has_current_description: current_description.is_some(),
            input_scopes: crate::generation::conversation_generation_input::modality_scopes(
                capabilities.input_modalities,
            ),
            output_scopes: crate::generation::conversation_generation_input::modality_scopes(
                capabilities.output_modalities,
            ),
            provider_id: Some(writer.account.provider_kind.clone()),
            reasoning_enabled: writer.profile.config.chat_parameters.reasoning_mode
                == Some(ReasoningMode::Enabled),
            vision_enabled: capabilities.input_modalities.image == CapabilityStatus::Supported,
            ..Default::default()
        };
        let values = PromptRenderValues {
            purpose_values: BTreeMap::from([
                (Variable::SubjectName, subject_name),
                (
                    Variable::SubjectDescription,
                    subject_description.unwrap_or_default(),
                ),
                (
                    Variable::CurrentDescription,
                    current_description.unwrap_or_default(),
                ),
                (Variable::AvatarImage, AVATAR_TOKEN.to_owned()),
                (Variable::ReferenceImages, REFERENCES_TOKEN.to_owned()),
            ]),
            ..Default::default()
        };
        let document = crate::generation::built_in_prompts::active_built_in_prompt(
            self.repository,
            BuiltInPromptId::DesignReference,
        )
        .map_err(|_| DesignReferenceError::MissingPrompt)?
        .ok_or(DesignReferenceError::MissingPrompt)?;
        let messages = design_reference_messages(
            &document,
            &PromptRenderContext { conditions, values },
            request.avatar,
            &request.references,
        )?;
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
                .map_err(|_| DesignReferenceError::Storage)?
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
                crate::SCENE_DESIGN_REFERENCE_DEFAULTS,
                crate::FeatureRequestFields::SamplingAndPromptCache,
                account.protocol,
                &global,
            ),
            &ChatRequirements::default(),
        )
        .map_err(DesignReferenceError::InvalidModel)?;
        let streaming = request.stream
            && chat_profile.streaming_enabled
            && capabilities.streaming != CapabilityStatus::Unsupported;
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
            stream_sink: streaming.then_some(request.request_id),
            media_grants,
            tools: None,
            prompt_cache_key: None,
        };
        inference_request
            .validate()
            .map_err(|_| DesignReferenceError::InvalidPrompt)?;
        Ok(inference_request)
    }
}

/// Legacy `design_reference_prompt_entry_to_message`. An image entry with
/// images becomes a user message with its text first; one without images is
/// dropped. Tokens left in other entries are removed, where legacy sent them
/// as text.
fn entry_message(
    entry: &FeatureEntry,
    avatar: Option<AssetId>,
    references: &[AssetId],
) -> Option<ProviderNeutralMessage> {
    let has_tokens = DESIGN_TOKENS
        .iter()
        .any(|token| entry.content.contains(token));
    if entry.slot.is_some() || has_tokens {
        let images = match entry.slot {
            Some(PromptEntryImageSlot::Avatar) => avatar.into_iter().collect(),
            Some(PromptEntryImageSlot::References) => references.to_vec(),
            Some(_) => Vec::new(),
            None => {
                let mut images = Vec::new();
                if entry.content.contains(AVATAR_TOKEN) {
                    images.extend(avatar);
                }
                if entry.content.contains(REFERENCES_TOKEN) {
                    images.extend_from_slice(references);
                }
                images
            }
        };
        if !images.is_empty() {
            let text = condense(&strip_tokens(&entry.content, &DESIGN_TOKENS));
            return Some(ProviderNeutralMessage {
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
            });
        }
    }
    if entry.slot.is_some() || (has_tokens && entry.role == PromptEntryRole::User) {
        return None;
    }
    let text = strip_tokens(&entry.content, &DESIGN_TOKENS)
        .trim()
        .to_owned();
    (!text.is_empty()).then(|| ProviderNeutralMessage {
        role: message_role(entry.role),
        parts: vec![ProviderContextPart::Text { text }],
    })
}

/// Legacy `render_design_reference_prompt_entries` and its message list: the
/// relative entries, then every other entry, each in template order.
fn design_reference_messages(
    document: &PromptDocument,
    context: &PromptRenderContext,
    avatar: Option<AssetId>,
    references: &[AssetId],
) -> Result<Vec<ProviderNeutralMessage>, DesignReferenceError> {
    let entries = render_feature_entries(document, context, &DESIGN_TOKENS)
        .map_err(|_| DesignReferenceError::InvalidPrompt)?;
    let (relative, others): (Vec<_>, Vec<_>) = entries
        .iter()
        .partition(|entry| entry.position == PromptEntryPosition::Relative);
    let messages = relative
        .into_iter()
        .chain(others)
        .filter_map(|entry| entry_message(entry, avatar, references))
        .collect::<Vec<_>>();
    if messages.is_empty() {
        return Err(DesignReferenceError::NoContent);
    }
    Ok(messages)
}

fn generated_text(outcome: &InferenceOutcome) -> Option<String> {
    let candidate = outcome.candidates.first()?;
    let texts = candidate
        .parts
        .iter()
        .filter_map(|part| match part {
            MessagePart::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    (!texts.is_empty()).then(|| texts.join("\n"))
}

/// Legacy's cleanup of the writer's answer.
fn clean_design_reference(text: &str) -> String {
    condense(
        text.trim()
            .trim_matches('"')
            .trim()
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn context(description: Option<&str>) -> PromptRenderContext {
        PromptRenderContext {
            conditions: PromptConditionContext {
                has_subject_description: description.is_some(),
                ..Default::default()
            },
            values: PromptRenderValues {
                purpose_values: BTreeMap::from([
                    (Variable::SubjectName, "Mira".to_owned()),
                    (
                        Variable::SubjectDescription,
                        description.unwrap_or_default().to_owned(),
                    ),
                    (Variable::CurrentDescription, String::new()),
                    (Variable::AvatarImage, AVATAR_TOKEN.to_owned()),
                    (Variable::ReferenceImages, REFERENCES_TOKEN.to_owned()),
                ]),
                ..Default::default()
            },
        }
    }

    #[test]
    fn design_reference_messages_follow_legacy_order() {
        let document =
            crate::generation::built_in_prompts::seed_document(BuiltInPromptId::DesignReference);
        let (avatar, reference) = (AssetId::new(), AssetId::new());
        let messages = design_reference_messages(
            &document,
            &context(Some("A sailor.")),
            Some(avatar),
            &[reference],
        )
        .expect("messages");
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
                MessageRole::User,
                MessageRole::User
            ]
        );
        assert!(texts(&messages[0])[0].starts_with("You are a character design documentarian."));
        assert_eq!(
            texts(&messages[3]),
            ["# Subject\nMira\n\n# Subject Context\nA sailor.\n\n# Current Notes To Refine"]
        );
        assert_eq!(images(&messages[4]), [avatar]);
        assert_eq!(images(&messages[5]), [reference]);

        let messages =
            design_reference_messages(&document, &context(None), None, &[reference, avatar])
                .expect("messages");
        assert_eq!(messages.len(), 4);
        assert_eq!(images(&messages[3]), [reference, avatar]);

        let mut condensed = document.clone();
        condensed.condense = true;
        let messages =
            design_reference_messages(&condensed, &context(Some("A sailor.")), Some(avatar), &[])
                .expect("messages");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, MessageRole::System);
        assert!(texts(&messages[0])[0].contains("# Subject\nMira"));
        assert_eq!(images(&messages[1]), [avatar]);
    }

    #[test]
    fn token_entries_carry_their_images_or_are_dropped() {
        let entry = |role, content: &str| FeatureEntry {
            role,
            content: content.to_owned(),
            slot: None,
            position: PromptEntryPosition::Relative,
            depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
        };
        let avatar = AssetId::new();
        let message = entry_message(
            &entry(PromptEntryRole::System, "Look {{image[avatar]}}"),
            Some(avatar),
            &[],
        )
        .expect("image message");
        assert_eq!(message.role, MessageRole::User);
        assert_eq!(texts(&message), ["Look"]);
        assert_eq!(images(&message), [avatar]);
        assert!(
            entry_message(
                &entry(PromptEntryRole::User, "Look {{image[references]}}"),
                Some(avatar),
                &[]
            )
            .is_none()
        );
        let message = entry_message(
            &entry(PromptEntryRole::System, "Rules {{image[references]}}"),
            Some(avatar),
            &[],
        )
        .expect("text message");
        assert_eq!(texts(&message), ["Rules"]);
    }

    #[test]
    fn answers_are_cleaned_like_legacy() {
        assert_eq!(
            clean_design_reference(" \"```Tall, calm.\n\n\n\nRed coat.```\" "),
            "Tall, calm.\n\nRed coat."
        );
    }
}
