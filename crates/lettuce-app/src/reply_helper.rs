use std::time::Duration;

use lettuce_characters::{CharacterRepository, PersonaRepository};
use lettuce_context::{
    LifecycleStatus, PromptConditionContext, PromptDocument, PromptEntryChatMode, PromptEntryRole,
    PromptRenderContext, PromptRenderValues, PromptRepository, PromptVariable as Variable,
    render_prompt,
};
use lettuce_conversations::{
    ConversationKind, ConversationReader, ConversationRepositoryError, GenerationOperation,
    InferenceOutcome, InferencePort, InferenceRequest, MessagePart, MessageRole, MessageVisibility,
    OutputPolicy, PortError, ProviderContextPart, ProviderFailureKind, ProviderNeutralContext,
    ProviderNeutralMessage, ProviderReplayArtifactPort, ResolvedInferenceProfile, SafetyContext,
    ToolPolicy, resolve_effective_settings,
};
use lettuce_jobs::{
    CancellationPolicy, CancellationReason, FiniteFraction, IdempotencyKey, JobError, JobErrorCode,
    JobKind, JobMutation, JobOutcome, JobPriority, JobSnapshot, JobSpec, JobStore, JobSubject,
    OutcomeRef, ProgressSnapshot, RecoveryPolicy, ResourceAvailability, ResourceClass,
    StageSnapshot, StoreError, SubjectKind, WorkerId, handle::JobHandle,
};
use lettuce_models::{
    ChatParameterOverrides, ChatParameterResolutionInput, ChatRequirements, ExpectedModelIdentity,
    ModelProfileRepository, ParameterOverride, ProviderAccountRepository,
};
use lettuce_settings::{GlobalSettingsStore, HelpMeReplyStyle};
use lettuce_types::{
    ConversationId, GenerationAttemptId, GenerationTurnId, PageLimit, PageRequest, RequestId,
    TimestampMillis,
};
use lettuce_usage::JobUsageLedger;

use crate::job_inference_usage::{JobInferenceError, run_job_inference};
use crate::runtime_text::{RuntimeText, RuntimeTextError};
use crate::{BuiltInPromptId, cleanup_outcome_replays};

/// Legacy `HELP_ME_REPLY_DEFAULTS`: temperature 0.8 and top_p 1.0 for the
/// request, applied only where the model declares the parameter, plus the
/// settings' output cap.
const HELP_ME_REPLY_TEMPERATURE: f64 = 0.8;
const HELP_ME_REPLY_TOP_P: f64 = 1.0;
const HELP_ME_REPLY_STAGE: &str = "reply-helper";

fn help_me_reply_parameters(
    support: lettuce_models::ParameterSupport,
    max_output_tokens: u32,
) -> ChatParameterResolutionInput {
    let supported = |status, value| {
        if status == lettuce_models::CapabilityStatus::Supported {
            ParameterOverride::Set(value)
        } else {
            ParameterOverride::Inherit
        }
    };
    ChatParameterResolutionInput {
        operation: ChatParameterOverrides {
            temperature: supported(support.temperature, HELP_ME_REPLY_TEMPERATURE),
            top_p: supported(support.top_p, HELP_ME_REPLY_TOP_P),
            max_output_tokens: ParameterOverride::Set(max_output_tokens),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// One "help me reply" request, legacy `chat_generate_user_reply`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplyHelperRequest {
    pub conversation_id: ConversationId,
    pub request_id: RequestId,
    pub current_draft: Option<String>,
    pub swap_places: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplyHelperReply {
    pub text: String,
    pub job: JobSnapshot,
}

#[derive(Debug, thiserror::Error)]
pub enum ReplyHelperError {
    #[error("Help Me Reply is disabled in settings")]
    Disabled,
    #[error("reply helper settings are unavailable: {0}")]
    Settings(lettuce_settings::GlobalSettingsStoreError),
    #[error("reply helper conversation lookup failed: {0}")]
    Conversation(ConversationRepositoryError),
    #[error("reply helper character lookup failed: {0:?}")]
    Character(lettuce_characters::RepositoryError),
    #[error("reply helper supports direct conversations only")]
    UnsupportedConversation,
    #[error("No conversation history to base reply on")]
    NoHistory,
    #[error("No model configured for Help Me Reply")]
    MissingModel,
    #[error("Help Me Reply model is invalid: {0}")]
    InvalidModel(lettuce_models::ChatProfileResolutionError),
    #[error("Help Me Reply model could not be read")]
    ModelUnavailable,
    #[error("Help Me Reply prompt is unavailable")]
    MissingPrompt,
    #[error("Help Me Reply prompt could not be rendered")]
    InvalidPrompt,
    #[error("reply helper job storage failed: {0}")]
    Jobs(#[from] StoreError),
    #[error("reply helper job could not be claimed")]
    NotClaimed,
    #[error("reply helper inference failed: {0}")]
    Inference(PortError),
    #[error("reply helper usage evidence could not be recorded")]
    Evidence,
    #[error("Failed to extract text from response")]
    EmptyResponse,
    #[error("reply helper request was cancelled")]
    Cancelled,
    #[error("reply helper replay cleanup failed")]
    ReplayCleanup,
}

/// Every port the reply helper reads; the composition root's database is one.
pub trait ReplyHelperSources:
    GlobalSettingsStore
    + ConversationReader
    + CharacterRepository
    + PersonaRepository
    + ModelProfileRepository
    + ProviderAccountRepository
    + PromptRepository
    + JobStore
    + JobUsageLedger
    + ProviderReplayArtifactPort
{
}

impl<T> ReplyHelperSources for T where
    T: GlobalSettingsStore
        + ConversationReader
        + CharacterRepository
        + PersonaRepository
        + ModelProfileRepository
        + ProviderAccountRepository
        + PromptRepository
        + JobStore
        + JobUsageLedger
        + ProviderReplayArtifactPort
{
}

/// Legacy `chat_generate_user_reply`: a one-shot completion, driven by the
/// live settings and sources, that drafts the user's next message. It runs as
/// a job so its usage evidence and cancellation follow every other feature
/// request.
#[derive(Debug)]
pub struct ReplyHelperCoordinator<'a, R: ?Sized, I: ?Sized> {
    repository: &'a R,
    inference: &'a I,
}

impl<'a, R: ?Sized, I: ?Sized> ReplyHelperCoordinator<'a, R, I> {
    #[must_use]
    pub const fn new(repository: &'a R, inference: &'a I) -> Self {
        Self {
            repository,
            inference,
        }
    }
}

struct PreparedReply {
    request: InferenceRequest,
    user_name: String,
}

struct Speakers {
    character_name: String,
    character_description: String,
    persona_name: String,
    persona_description: String,
}

impl<R, I> ReplyHelperCoordinator<'_, R, I>
where
    R: ReplyHelperSources + ?Sized,
    I: InferencePort + ?Sized,
{
    pub async fn generate(
        &self,
        request: &ReplyHelperRequest,
        worker_id: WorkerId,
        now: TimestampMillis,
        lease_for: Duration,
        allowed: &ResourceAvailability,
    ) -> Result<ReplyHelperReply, ReplyHelperError> {
        let stored =
            GlobalSettingsStore::load(self.repository).map_err(ReplyHelperError::Settings)?;
        if !stored.settings.help_me_reply.enabled {
            return Err(ReplyHelperError::Disabled);
        }
        let job = self.admit(request)?;
        let at = now.max(job.updated_at);
        let Some(claim) = self
            .repository
            .claim(job.id, worker_id, at, lease_for, allowed)?
        else {
            return Err(ReplyHelperError::NotClaimed);
        };
        let handle = JobHandle::new(job.id);
        self.repository.append_and_transition(JobMutation::Start {
            claim: claim.claim.clone(),
            at,
        })?;
        self.repository
            .append_and_transition(JobMutation::StageChanged {
                claim: claim.claim.clone(),
                stage: StageSnapshot::new(HELP_ME_REPLY_STAGE, false)
                    .expect("constant job stage is valid"),
                at,
            })?;
        let result = self.run(&stored, request, &handle, now).await;
        self.settle(claim.claim, request.request_id, result, at)
    }

    fn admit(&self, request: &ReplyHelperRequest) -> Result<JobSnapshot, ReplyHelperError> {
        let key = IdempotencyKey::new(format!("reply-helper-{}", request.request_id))
            .expect("request ids are safe idempotency keys");
        let spec = JobSpec::new(
            JobKind::CreationRun,
            JobSubject::new(
                SubjectKind::Conversation,
                request.conversation_id.to_string(),
            )
            .expect("conversation ids are safe job subjects"),
            OutcomeRef::Request(request.request_id),
        )
        .with_idempotency_key(key)
        .with_resources(vec![ResourceClass::Network])
        .with_priority(JobPriority::Interactive)
        .with_policies(RecoveryPolicy::Restart, CancellationPolicy::Cooperative);
        let admitted = self.repository.create_or_get(spec)?;
        Ok(admitted.job)
    }

    async fn run(
        &self,
        stored: &lettuce_settings::StoredGlobalSettings,
        request: &ReplyHelperRequest,
        handle: &JobHandle,
        now: TimestampMillis,
    ) -> Result<String, ReplyHelperError> {
        let prepared = self.prepare(stored, request, handle)?;
        if handle.cancellation_token().is_cancelled() {
            return Err(ReplyHelperError::Cancelled);
        }
        let outcome = match run_job_inference(
            self.repository,
            self.inference,
            handle.id(),
            prepared.request,
            now,
        )
        .await
        {
            Ok(outcome) => outcome,
            Err(JobInferenceError::Provider(PortError::Cancelled)) => {
                return Err(ReplyHelperError::Cancelled);
            }
            Err(JobInferenceError::Provider(error)) => {
                return Err(ReplyHelperError::Inference(error));
            }
            Err(JobInferenceError::Evidence) => return Err(ReplyHelperError::Evidence),
        };
        let text = generated_text(&outcome);
        cleanup_outcome_replays(self.repository, &outcome)
            .map_err(|_| ReplyHelperError::ReplayCleanup)?;
        let text = text.ok_or(ReplyHelperError::EmptyResponse)?;
        Ok(clean_reply(&text, &prepared.user_name))
    }

    fn prepare(
        &self,
        stored: &lettuce_settings::StoredGlobalSettings,
        request: &ReplyHelperRequest,
        handle: &JobHandle,
    ) -> Result<PreparedReply, ReplyHelperError> {
        let settings = &stored.settings.help_me_reply;
        let aggregate = ConversationReader::get(self.repository, request.conversation_id)
            .map_err(ReplyHelperError::Conversation)?;
        let ConversationKind::Direct(details) = &aggregate.conversation.kind else {
            return Err(ReplyHelperError::UnsupportedConversation);
        };
        let effective =
            resolve_effective_settings(&aggregate.conversation, None).map_err(|error| {
                ReplyHelperError::Conversation(ConversationRepositoryError::Invalid(error))
            })?;
        let character = CharacterRepository::get(self.repository, details.character.source_id)
            .map_err(ReplyHelperError::Character)?
            .ok_or(ReplyHelperError::UnsupportedConversation)?
            .character;
        let persona = effective
            .persona
            .as_ref()
            .map(|persona| PersonaRepository::get(self.repository, persona.source_id))
            .transpose()
            .map_err(ReplyHelperError::Character)?
            .flatten();
        let speakers = speakers(
            &character.profile.name,
            character
                .profile
                .definition
                .as_deref()
                .or(character.profile.description.as_deref())
                .unwrap_or_default(),
            persona
                .as_ref()
                .map(|persona| (persona.title.as_str(), persona.description.as_str())),
            request.swap_places,
        );
        let history = self.recent_dialogue(&aggregate.conversation, settings.history_count())?;
        if history.is_empty() {
            return Err(ReplyHelperError::NoHistory);
        }
        let model_id = settings
            .model_profile_id
            .or(stored.default_model_profile_id)
            .ok_or(ReplyHelperError::MissingModel)?;
        let model = ModelProfileRepository::get(self.repository, model_id)
            .map_err(|_| ReplyHelperError::ModelUnavailable)?
            .ok_or(ReplyHelperError::MissingModel)?;
        let account = ProviderAccountRepository::get(self.repository, model.provider_account_id)
            .map_err(|_| ReplyHelperError::ModelUnavailable)?
            .ok_or(ReplyHelperError::MissingModel)?;
        let streaming = settings.streaming;
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
            &model,
            &account,
            &help_me_reply_parameters(
                model.config.capabilities.parameter_support,
                settings.max_output_tokens,
            ),
            &ChatRequirements {
                require_streaming: streaming,
                ..Default::default()
            },
        )
        .map_err(ReplyHelperError::InvalidModel)?;
        let (override_id, built_in) = match settings.style {
            HelpMeReplyStyle::Roleplay => {
                (settings.roleplay_prompt_id, BuiltInPromptId::ReplyHelper)
            }
            HelpMeReplyStyle::Conversational => (
                settings.conversational_prompt_id,
                BuiltInPromptId::ReplyHelperConversational,
            ),
        };
        let document = match override_id
            .map(|id| PromptRepository::get(self.repository, id))
            .transpose()
            .map_err(|_| ReplyHelperError::MissingPrompt)?
            .flatten()
            .filter(|document| {
                document.status == LifecycleStatus::Active && document.purpose == built_in.purpose()
            }) {
            Some(document) => document,
            None => crate::built_in_prompts::active_built_in_prompt(self.repository, built_in)
                .map_err(|_| ReplyHelperError::MissingPrompt)?
                .ok_or(ReplyHelperError::MissingPrompt)?,
        };
        let text = RuntimeText::load(self.repository, BuiltInPromptId::ChatRuntime)
            .map_err(|_| ReplyHelperError::MissingPrompt)?;
        let mut messages = render_entries(&document, &speakers, request.current_draft.as_deref())?;
        messages.push(ProviderNeutralMessage {
            role: MessageRole::User,
            parts: vec![ProviderContextPart::Text {
                text: reply_input(&text, &history, &speakers, request.swap_places)?,
            }],
        });
        let context = ProviderNeutralContext {
            messages,
            attributions: Default::default(),
            budget: Default::default(),
        };
        let request = InferenceRequest {
            turn_id: GenerationTurnId::from_uuid(request.request_id.as_uuid()),
            attempt_id: GenerationAttemptId::from_uuid(uuid::Uuid::new_v5(
                &handle.id().as_uuid(),
                b"reply-helper",
            )),
            operation: GenerationOperation::Send,
            profile: ResolvedInferenceProfile {
                chat_profile,
                tool_policy: ToolPolicy::Disabled,
                output_policy: OutputPolicy::Plain,
                safety_policy: SafetyContext::Standard,
                correlation_id: None,
            },
            context,
            cancellation: Some(handle.id()),
            stream_sink: streaming.then_some(request.request_id),
            media_grants: Vec::new(),
            tools: None,
        };
        request
            .validate()
            .map_err(|_| ReplyHelperError::InvalidPrompt)?;
        Ok(PreparedReply {
            request,
            user_name: speakers.persona_name,
        })
    }

    /// The last `limit` visible user and assistant messages on the active
    /// branch, oldest first, with their current text.
    fn recent_dialogue(
        &self,
        conversation: &lettuce_conversations::Conversation,
        limit: usize,
    ) -> Result<Vec<(MessageRole, String)>, ReplyHelperError> {
        let mut recent = Vec::new();
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
                .map_err(ReplyHelperError::Conversation)?;
            for item in &page.items {
                if recent.len() >= limit {
                    break;
                }
                if item.message.visibility != MessageVisibility::Visible
                    || !matches!(
                        item.message.role,
                        MessageRole::User | MessageRole::Assistant
                    )
                {
                    continue;
                }
                let parts = item
                    .active_candidate
                    .as_ref()
                    .map(|candidate| candidate.parts.as_slice())
                    .or_else(|| {
                        item.active_revision
                            .as_ref()
                            .map(|revision| revision.parts.as_slice())
                    })
                    .unwrap_or_default();
                recent.push((item.message.role, message_text(parts)));
            }
            if recent.len() >= limit {
                break;
            }
            let Some(next) = page.next_cursor else {
                break;
            };
            cursor = Some(next);
        }
        recent.reverse();
        Ok(recent)
    }

    fn settle(
        &self,
        claim: lettuce_jobs::ClaimRef,
        request_id: RequestId,
        result: Result<String, ReplyHelperError>,
        at: TimestampMillis,
    ) -> Result<ReplyHelperReply, ReplyHelperError> {
        match result {
            Ok(text) => {
                self.repository
                    .append_and_transition(JobMutation::Progress {
                        claim: claim.clone(),
                        progress: ProgressSnapshot {
                            fraction: Some(
                                FiniteFraction::new(1.0).expect("constant job progress is valid"),
                            ),
                            ..ProgressSnapshot::default()
                        },
                        at,
                    })?;
                let job = self
                    .repository
                    .append_and_transition(JobMutation::Succeed {
                        outcome: JobOutcome::Success {
                            result_ref: OutcomeRef::Request(request_id),
                        },
                        claim,
                        at,
                    })?;
                Ok(ReplyHelperReply { text, job })
            }
            Err(ReplyHelperError::Cancelled) => {
                self.repository
                    .append_and_transition(JobMutation::RequestCancellation {
                        id: claim.job_id,
                        reason: CancellationReason::User,
                        at,
                    })?;
                self.repository
                    .append_and_transition(JobMutation::RequestCleanup {
                        claim: claim.clone(),
                        at,
                    })?;
                self.repository
                    .append_and_transition(JobMutation::FinishCancellation { claim, at })?;
                Err(ReplyHelperError::Cancelled)
            }
            Err(error) => {
                self.repository.append_and_transition(JobMutation::Fail {
                    claim,
                    error: job_error(&error),
                    at,
                })?;
                Err(error)
            }
        }
    }
}

/// Legacy `swapped_prompt_entities`: with `swap_places` and a persona, the
/// character speaks as the persona and the persona as the character; without
/// a persona the names stay ("user" with no description).
fn speakers(
    character_name: &str,
    character_description: &str,
    persona: Option<(&str, &str)>,
    swap_places: bool,
) -> Speakers {
    let (persona_name, persona_description) = persona.unwrap_or(("user", ""));
    if swap_places && persona.is_some() {
        Speakers {
            character_name: persona_name.to_owned(),
            character_description: persona_description.to_owned(),
            persona_name: character_name.to_owned(),
            persona_description: character_description.to_owned(),
        }
    } else {
        Speakers {
            character_name: character_name.to_owned(),
            character_description: character_description.to_owned(),
            persona_name: persona_name.to_owned(),
            persona_description: persona_description.to_owned(),
        }
    }
}

fn render_entries(
    document: &PromptDocument,
    speakers: &Speakers,
    current_draft: Option<&str>,
) -> Result<Vec<ProviderNeutralMessage>, ReplyHelperError> {
    let mut values = PromptRenderValues {
        character_name: speakers.character_name.clone(),
        character_description: speakers.character_description.clone(),
        persona_name: speakers.persona_name.clone(),
        persona_description: speakers.persona_description.clone(),
        user_name: speakers.persona_name.clone(),
        user_description: speakers.persona_description.clone(),
        ai_name: speakers.character_name.clone(),
        ai_description: speakers.character_description.clone(),
        ..Default::default()
    };
    values.purpose_values.insert(
        Variable::CurrentDraft,
        current_draft
            .filter(|draft| !draft.trim().is_empty())
            .unwrap_or_default()
            .to_owned(),
    );
    let rendered = render_prompt(
        document,
        &PromptRenderContext {
            conditions: PromptConditionContext {
                chat_mode: PromptEntryChatMode::Direct,
                ..Default::default()
            },
            values,
        },
    )
    .map_err(|_| ReplyHelperError::InvalidPrompt)?;
    Ok(rendered
        .relative
        .into_iter()
        .chain(rendered.in_chat)
        .filter(|message| !message.content.trim().is_empty())
        .map(|message| ProviderNeutralMessage {
            role: match message.role {
                PromptEntryRole::System => MessageRole::System,
                PromptEntryRole::User => MessageRole::User,
                PromptEntryRole::Assistant => MessageRole::Assistant,
            },
            parts: vec![ProviderContextPart::Text {
                text: message.content,
            }],
        })
        .collect())
}

/// Legacy's runtime user entry: every recent message as "{name}: {text}"
/// (roles swapped with the speakers when `swap_places`), then the request to
/// draft the persona's next line.
fn reply_input(
    text: &RuntimeText,
    history: &[(MessageRole, String)],
    speakers: &Speakers,
    swap_places: bool,
) -> Result<String, ReplyHelperError> {
    let line = |name: &str, content: &str| {
        text.render_with(
            "runtime_reply_helper_line",
            [
                (Variable::SpeakerName, name.to_owned()),
                (Variable::MessageText, content.to_owned()),
            ],
        )
    };
    let lines = history
        .iter()
        .map(|(role, content)| {
            let user_spoke = (*role == MessageRole::User) != swap_places;
            line(
                if user_spoke {
                    &speakers.persona_name
                } else {
                    &speakers.character_name
                },
                content,
            )
        })
        .collect::<Result<Vec<_>, RuntimeTextError>>()
        .map_err(|_| ReplyHelperError::MissingPrompt)?
        .join("\n\n");
    text.render_with(
        "runtime_reply_helper_input",
        [
            (Variable::SelectedMessages, lines),
            (Variable::SpeakerName, speakers.persona_name.clone()),
        ],
    )
    .map_err(|_| ReplyHelperError::MissingPrompt)
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
    let text = outcome
        .candidates
        .first()
        .map(|candidate| message_text(&candidate.parts))
        .unwrap_or_default();
    (!text.trim().is_empty()).then_some(text)
}

/// Legacy cleaned the completion by trimming, dropping surrounding quotes,
/// stripping a leading "{user}:" and trimming again.
fn clean_reply(text: &str, user_name: &str) -> String {
    text.trim()
        .trim_matches('"')
        .trim_start_matches(&format!("{user_name}:"))
        .trim()
        .to_owned()
}

fn job_error(error: &ReplyHelperError) -> JobError {
    let (code, retryable, message) = match error {
        ReplyHelperError::Inference(PortError::Unavailable)
        | ReplyHelperError::Evidence
        | ReplyHelperError::ReplayCleanup
        | ReplyHelperError::NotClaimed => (
            JobErrorCode::ResourceUnavailable,
            true,
            "reply-helper-unavailable",
        ),
        ReplyHelperError::Inference(PortError::Provider(failure))
            if failure.kind == ProviderFailureKind::Unavailable =>
        {
            (
                JobErrorCode::ResourceUnavailable,
                true,
                "reply-helper-unavailable",
            )
        }
        ReplyHelperError::Inference(_) | ReplyHelperError::EmptyResponse => (
            JobErrorCode::WorkerFailed,
            false,
            "reply-helper-inference-failed",
        ),
        ReplyHelperError::Jobs(_)
        | ReplyHelperError::Settings(_)
        | ReplyHelperError::Conversation(_)
        | ReplyHelperError::Character(_) => (
            JobErrorCode::StorageFailure,
            true,
            "reply-helper-storage-failed",
        ),
        ReplyHelperError::Disabled
        | ReplyHelperError::UnsupportedConversation
        | ReplyHelperError::NoHistory
        | ReplyHelperError::MissingModel
        | ReplyHelperError::InvalidModel(_)
        | ReplyHelperError::ModelUnavailable
        | ReplyHelperError::MissingPrompt
        | ReplyHelperError::InvalidPrompt
        | ReplyHelperError::Cancelled => (
            JobErrorCode::InvalidInput,
            false,
            "reply-helper-invalid-input",
        ),
    };
    JobError::new(code, retryable, message).expect("constant job error is valid")
}

#[cfg(test)]
mod tests {
    use lettuce_models::{CapabilityStatus, ParameterOverride, ParameterSupport};

    use super::{clean_reply, help_me_reply_parameters, speakers};

    #[test]
    fn feature_defaults_apply_only_to_declared_parameters() {
        let all = ParameterSupport {
            temperature: CapabilityStatus::Supported,
            top_p: CapabilityStatus::Unknown,
            ..Default::default()
        };
        let input = help_me_reply_parameters(all, 150);
        assert_eq!(input.operation.temperature, ParameterOverride::Set(0.8));
        assert_eq!(input.operation.top_p, ParameterOverride::Inherit);
        assert_eq!(
            input.operation.max_output_tokens,
            ParameterOverride::Set(150)
        );
    }

    #[test]
    fn replies_are_cleaned_like_legacy() {
        assert_eq!(clean_reply("  \"Mira: Let's go.\"  ", "Mira"), "Let's go.");
        assert_eq!(clean_reply("user: fine", "user"), "fine");
        assert_eq!(clean_reply("Nobody: stays", "Mira"), "Nobody: stays");
    }

    #[test]
    fn swapping_places_exchanges_the_speakers_only_with_a_persona() {
        let swapped = speakers("Ada", "An engineer", Some(("Mira", "A sailor")), true);
        assert_eq!(swapped.character_name, "Mira");
        assert_eq!(swapped.character_description, "A sailor");
        assert_eq!(swapped.persona_name, "Ada");
        assert_eq!(swapped.persona_description, "An engineer");
        let plain = speakers("Ada", "An engineer", None, true);
        assert_eq!(plain.character_name, "Ada");
        assert_eq!(plain.persona_name, "user");
        assert_eq!(plain.persona_description, "");
    }
}
