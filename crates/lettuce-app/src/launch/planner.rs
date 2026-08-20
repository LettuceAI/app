use lettuce_characters::{
    CharacterRepository, ConversationStarter, GroupDetails, GroupProfile, GroupRepository, Persona,
    PersonaRepository, Scene, SceneOwner, StarterRole,
};
use lettuce_context::{
    CharacterLorebookBindingRepository, GroupLorebookBindingRepository, LifecycleFilter,
    LorebookBinding, LorebookDetails, LorebookRepository, PersonaLorebookBindingRepository,
    PromptDocument, PromptLibraryQuery, PromptLookupResult, PromptProvenance, PromptPurpose,
    PromptRepository,
};
use lettuce_conversations::{
    CharacterLaunchSnapshot, ConversationCreator, ConversationKind, ConversationParticipantDraft,
    ConversationReader, ConversationRepositoryError, CreateConversationPlan,
    CreateConversationResult, DirectConversationDetails, GroupChatModeSnapshot,
    GroupConversationDetails, GroupLaunchSnapshot, GroupMemberLaunchSnapshot,
    GroupParticipantPolicyDocument, GroupParticipantPolicySnapshot, IdempotencyKey,
    InitialMessageDraft, InitialMessageOrigin, InitialTimelineDraft, LorebookLaunchSnapshot,
    MemorySettingsSnapshot, MessagePart, MessageRole, ModelSelectionSnapshot, OperationToken,
    ParticipantRole, ParticipantSource, PersonaLaunchSnapshot, PreparedConversationLaunch,
    PromptLaunchSnapshot, PromptPurposeSnapshot, SceneLaunchSnapshot, SnapshotArtifactDraft,
    SnapshotSelection, StarterLaunchSnapshot,
};
use lettuce_models::{
    ModelKind, ModelProfile, ModelProfileRepository, ProviderAccount, ProviderAccountRepository,
};
use lettuce_settings::GlobalSettingsStore;
use lettuce_types::{
    GroupId, LorebookId, ModelProfileId, PageRequest, PromptDocumentId, Revision, TimestampMillis,
};
use std::collections::HashSet;

use super::digest::{
    GroupLaunchIntent, GroupMemberIntent, direct_request_digest, group_request_digest,
};
use super::documents;
use super::error::{ConversationLaunchError, LaunchSourceError};
use super::identity::{ArtifactSlot, LaunchIdentities, launch_conversation_id};
use super::policy::{self, LorebookRegistry, ModelRegistry, Selected};
use super::request::{
    DIRECT_LAUNCH_REQUEST_FORMAT_V1, DirectConversationLaunchRequest,
    GROUP_LAUNCH_REQUEST_FORMAT_V1, GroupConversationLaunchRequest, LaunchSelection,
};
use crate::BuiltInPromptId;

const MAX_DISPLAY_BYTES: usize = 1024;

/// Every authored source a conversation launch reads. One bound keeps the
/// planner usable with any composition root that owns all of them.
pub trait DirectLaunchSources:
    CharacterRepository
    + PersonaRepository
    + PromptRepository
    + LorebookRepository
    + CharacterLorebookBindingRepository
    + PersonaLorebookBindingRepository
    + ModelProfileRepository
    + ProviderAccountRepository
    + GlobalSettingsStore
{
}

impl<T> DirectLaunchSources for T where
    T: CharacterRepository
        + PersonaRepository
        + PromptRepository
        + LorebookRepository
        + CharacterLorebookBindingRepository
        + PersonaLorebookBindingRepository
        + ModelProfileRepository
        + ProviderAccountRepository
        + GlobalSettingsStore
{
}

/// Everything a direct launch reads plus the group graph and its bindings.
pub trait GroupLaunchSources:
    DirectLaunchSources + GroupRepository + GroupLorebookBindingRepository
{
}

impl<T> GroupLaunchSources for T where
    T: DirectLaunchSources + GroupRepository + GroupLorebookBindingRepository
{
}

#[derive(Debug)]
pub struct ConversationLaunchPlanner<'a, S> {
    sources: &'a S,
}

impl<'a, S> ConversationLaunchPlanner<'a, S>
where
    S: DirectLaunchSources + ConversationCreator,
{
    #[must_use]
    pub const fn new(sources: &'a S) -> Self {
        Self { sources }
    }

    pub fn launch_direct(
        &self,
        request: &DirectConversationLaunchRequest,
        now: TimestampMillis,
    ) -> Result<CreateConversationResult, ConversationLaunchError> {
        let launch = match self.prepare_direct(request) {
            Ok(launch) => launch,
            Err(error) => {
                return Err(self.already_launched_or(&request.operation_key, error));
            }
        };
        ConversationCreator::create(self.sources, launch, now).map_err(|error| match error {
            ConversationRepositoryError::Conflict => ConversationLaunchError::CreateConflict,
            other => LaunchSourceError::Conversation(other).into(),
        })
    }

    /// A source archived or deleted after a committed launch must not look
    /// like a fresh failure; the caller is told to open what already exists.
    fn already_launched_or(
        &self,
        operation_key: &IdempotencyKey,
        error: ConversationLaunchError,
    ) -> ConversationLaunchError {
        let conversation_id = launch_conversation_id(operation_key);
        if ConversationReader::get(self.sources, conversation_id).is_ok() {
            return ConversationLaunchError::AlreadyLaunched { conversation_id };
        }
        error
    }

    pub fn prepare_direct(
        &self,
        request: &DirectConversationLaunchRequest,
    ) -> Result<PreparedConversationLaunch, ConversationLaunchError> {
        if request.format_version != DIRECT_LAUNCH_REQUEST_FORMAT_V1 {
            return Err(ConversationLaunchError::InvalidRequest {
                field: "format_version",
            });
        }
        check_display("title", &request.title)?;
        check_display("user.display_name", &request.user.display_name)?;

        let conversation_id = launch_conversation_id(&request.operation_key);
        let identities = LaunchIdentities::new(conversation_id);

        let character = CharacterRepository::get(self.sources, request.character_id)
            .map_err(LaunchSourceError::Character)?
            .ok_or(ConversationLaunchError::CharacterNotFound {
                character_id: request.character_id,
            })?;
        if character.character.status == lettuce_characters::LifecycleStatus::Archived {
            return Err(ConversationLaunchError::CharacterArchived {
                character_id: request.character_id,
            });
        }
        if policy::is_companion(&character.character.defaults) {
            return Err(ConversationLaunchError::CompanionUnsupported {
                character_id: request.character_id,
            });
        }
        let defaults = character.character.defaults.clone();

        let starter = self.resolve_starter(request, &character)?;
        let scene = self.resolve_scene(request, &character, starter)?;
        let scene_text = scene
            .value()
            .and_then(|value| policy::resolve_scene_text(value, &character.variants));
        let planned_entries = usize::from(scene_text.is_some())
            + starter.value().map_or(0, |value| value.messages.len());
        if policy::timeline_bound_exceeded(planned_entries) {
            return Err(ConversationLaunchError::TooManyInitialMessages {
                max: policy::MAX_LAUNCH_TIMELINE_ENTRIES,
            });
        }
        let persona = self.resolve_persona(request.persona)?;
        let prompt = self.resolve_prompt(&defaults, starter)?;

        let character_bindings = CharacterLorebookBindingRepository::list_character_bindings(
            self.sources,
            request.character_id,
        )
        .map_err(LaunchSourceError::Binding)?;
        let persona_bindings = match persona.value() {
            Some(value) => {
                PersonaLorebookBindingRepository::list_persona_bindings(self.sources, value.id)
                    .map_err(LaunchSourceError::Binding)?
            }
            None => Vec::new(),
        };
        self.check_for_drift(request, &character, persona.value())?;

        let conversation_lorebooks = policy::lorebook_choice(
            starter.map(|value| &value.lorebooks),
            &character_bindings,
            &persona_bindings,
        );
        let persona_lorebooks = persona.with(policy::enabled_lorebooks(&persona_bindings));

        let mut requested = LorebookRegistry::default();
        for id in conversation_lorebooks.value().into_iter().flatten() {
            requested.register(*id);
        }
        for id in persona_lorebooks.value().into_iter().flatten() {
            requested.register(*id);
        }
        if policy::lorebook_bound_exceeded(requested.ordered().len()) {
            return Err(ConversationLaunchError::TooManyLorebooks {
                max: policy::MAX_LAUNCH_LOREBOOKS,
            });
        }
        let authored: HashSet<LorebookId> = match &conversation_lorebooks {
            Selected::Explicit(ids) => ids.iter().copied().collect(),
            _ => HashSet::new(),
        };
        let books = self.load_lorebooks(requested.ordered(), &authored)?;
        let kept: Vec<LorebookId> = books.iter().map(|book| book.book.id).collect();
        let conversation_lorebooks = conversation_lorebooks.map(|ids| retain(ids, &kept));
        let persona_lorebooks = persona_lorebooks.map(|ids| retain(ids, &kept));
        let model = self.resolve_model(&defaults)?;

        let mut character_drafts = Vec::new();
        let character_draft = documents::draft(
            identities.artifact(ArtifactSlot::Character),
            character.character.revision,
            documents::character_body(&character.character),
        )?;
        let character_snapshot = character_snapshot(&character.character, &character_draft);
        character_drafts.push(character_draft);

        let mut persona_draft = None;
        if let Some(value) = persona.value() {
            persona_draft = Some(documents::draft(
                identities.artifact(ArtifactSlot::Persona),
                value.revision,
                documents::persona_body(value),
            )?);
        }
        let mut scene_draft = None;
        if let Some(value) = scene.value() {
            scene_draft = Some(documents::draft(
                identities.artifact(ArtifactSlot::Scene),
                value.revision,
                documents::scene_body(value, &character.variants),
            )?);
        }
        let mut starter_draft = None;
        if let Some(value) = starter.value() {
            starter_draft = Some(documents::draft(
                identities.artifact(ArtifactSlot::Starter),
                value.revision,
                documents::starter_body(value),
            )?);
        }
        let mut prompt_draft = None;
        if let Some(value) = prompt.value() {
            prompt_draft = Some(documents::draft(
                identities.artifact(ArtifactSlot::Prompt),
                value.revision,
                documents::prompt_body(value),
            )?);
        }
        let mut lorebook_drafts = Vec::with_capacity(books.len());
        let mut lorebook_snapshots: Vec<(LorebookId, LorebookLaunchSnapshot)> =
            Vec::with_capacity(books.len());
        for (ordinal, book) in books.iter().enumerate() {
            let draft = documents::draft(
                identities.artifact(ArtifactSlot::Lorebook(ordinal)),
                book.book.revision,
                documents::lorebook_body(book),
            )?;
            lorebook_snapshots.push((
                book.book.id,
                LorebookLaunchSnapshot {
                    snapshot_ref: draft.reference(),
                    source_id: book.book.id,
                    source_revision: book.book.revision,
                    name: book.book.name.clone(),
                },
            ));
            lorebook_drafts.push(draft);
        }
        let mut model_draft = None;
        if let Some((profile, account)) = model.value() {
            model_draft = Some(documents::draft(
                identities.artifact(ArtifactSlot::Model),
                profile.revision,
                documents::model_body(profile, account),
            )?);
        }

        let scene_snapshot = match scene_draft.as_ref() {
            None => SnapshotSelection::Disabled,
            Some(draft) => scene
                .map(|value| SceneLaunchSnapshot {
                    snapshot_ref: draft.reference(),
                    source_id: value.id,
                    source_revision: value.revision,
                    title: policy::scene_title(scene_text.as_deref(), value.ordinal),
                })
                .into_snapshot(),
        };
        let starter_snapshot = match starter_draft.as_ref() {
            None => SnapshotSelection::Disabled,
            Some(draft) => starter
                .map(|value| StarterLaunchSnapshot {
                    snapshot_ref: draft.reference(),
                    source_id: value.id,
                    source_revision: value.revision,
                    title: value.name.clone(),
                })
                .into_snapshot(),
        };
        let prompt_snapshot = match prompt_draft.as_ref() {
            None => SnapshotSelection::Disabled,
            Some(draft) => prompt
                .as_ref()
                .map(|value| PromptLaunchSnapshot {
                    snapshot_ref: draft.reference(),
                    source_id: value.id,
                    source_revision: value.revision,
                    title: value.name.clone(),
                    purpose: PromptPurposeSnapshot::Direct,
                })
                .into_snapshot(),
        };
        let model_snapshot = match model_draft.as_ref() {
            None => SnapshotSelection::Disabled,
            Some(draft) => model
                .as_ref()
                .map(|(profile, account)| model_snapshot(profile, account, draft))
                .into_snapshot(),
        };
        let persona_snapshot = match persona_draft.as_ref() {
            None => SnapshotSelection::Disabled,
            Some(draft) => persona
                .as_ref()
                .map(|value| PersonaLaunchSnapshot {
                    snapshot_ref: draft.reference(),
                    source_id: value.id,
                    source_revision: value.revision,
                    title: value.title.clone(),
                    nickname: value.nickname.clone(),
                    lorebooks: persona_lorebooks
                        .as_ref()
                        .map(|ids| collect_lorebooks(ids, &lorebook_snapshots))
                        .into_snapshot(),
                })
                .into_snapshot(),
        };

        let user_participant = ConversationParticipantDraft {
            id: identities.user_participant(),
            role: ParticipantRole::User,
            ordinal: 0,
            source: ParticipantSource::User,
            enabled: true,
            muted: false,
            display_name: request.user.display_name.clone(),
            authored_description: request.user.authored_description.clone(),
            model_selection: SnapshotSelection::Disabled,
        };
        let character_participant = ConversationParticipantDraft {
            id: identities.character_participant(),
            role: ParticipantRole::Character,
            ordinal: 1,
            source: ParticipantSource::Character(character.character.id),
            enabled: true,
            muted: false,
            display_name: policy::character_display_name(&character.character),
            authored_description: None,
            model_selection: model_snapshot.clone(),
        };

        let mut entries = Vec::new();
        if let (Some(draft), Some(text)) = (scene_draft.as_ref(), scene_text.clone()) {
            entries.push(InitialMessageDraft {
                message_id: identities.scene_message(),
                revision_id: identities.scene_revision(),
                origin: InitialMessageOrigin::SelectedScene {
                    snapshot_ref: draft.reference(),
                },
                role: MessageRole::Scene,
                author_participant_id: None,
                parts: vec![MessagePart::Text { text }],
            });
        }
        if let (Some(value), Some(draft)) = (starter.value(), starter_draft.as_ref()) {
            for (ordinal, message) in value.messages.iter().enumerate() {
                let (role, author) = match message.role {
                    StarterRole::User => (MessageRole::User, user_participant.id),
                    StarterRole::Assistant => (MessageRole::Assistant, character_participant.id),
                };
                entries.push(InitialMessageDraft {
                    message_id: identities.starter_message(ordinal),
                    revision_id: identities.starter_revision(ordinal),
                    origin: InitialMessageOrigin::StarterMessage {
                        snapshot_ref: draft.reference(),
                        starter_message_id: message.id,
                    },
                    role,
                    author_participant_id: Some(author),
                    parts: vec![MessagePart::Text {
                        text: message.content.clone(),
                    }],
                });
            }
        }
        let initial_timeline = InitialTimelineDraft {
            format_version: 1,
            entries,
        };

        let mut drafts: Vec<SnapshotArtifactDraft> = character_drafts;
        drafts.extend(persona_draft);
        drafts.extend(scene_draft);
        drafts.extend(starter_draft);
        drafts.extend(prompt_draft);
        drafts.extend(lorebook_drafts);
        drafts.extend(model_draft);

        let ordered: Vec<&SnapshotArtifactDraft> = drafts.iter().collect();
        let request_digest = direct_request_digest(request, &ordered, &initial_timeline).ok_or(
            ConversationLaunchError::InvalidRequest {
                field: "request_digest",
            },
        )?;

        let details = DirectConversationDetails {
            format_version: 1,
            character: character_snapshot,
            persona: persona_snapshot,
            scene: scene_snapshot,
            starter: starter_snapshot,
            prompt: prompt_snapshot,
            lorebooks: conversation_lorebooks
                .as_ref()
                .map(|ids| collect_lorebooks(ids, &lorebook_snapshots))
                .into_snapshot(),
            model: model_snapshot,
            memory: SnapshotSelection::Inherited(MemorySettingsSnapshot {
                policy_ref: None,
                mode: policy::memory_mode(&defaults),
                selected_revision_ids: Vec::new(),
            }),
            voice: SnapshotSelection::Disabled,
        };

        let plan = CreateConversationPlan {
            conversation_id,
            title: request.title.clone(),
            kind: ConversationKind::Direct(details),
            participants: vec![user_participant, character_participant],
            initial_timeline,
            operation: OperationToken {
                key: request.operation_key.clone(),
                request_digest,
            },
        };
        Ok(PreparedConversationLaunch::new(plan, drafts)?)
    }

    fn resolve_starter<'d>(
        &self,
        request: &DirectConversationLaunchRequest,
        character: &'d lettuce_characters::CharacterDetails,
    ) -> Result<Selected<&'d ConversationStarter>, ConversationLaunchError> {
        Ok(match policy::starter_choice(request.starter) {
            Selected::Explicit(id) => {
                Selected::Explicit(policy::find_starter(&character.starters, id).ok_or(
                    ConversationLaunchError::StarterNotOwned {
                        starter_id: id,
                        character_id: request.character_id,
                    },
                )?)
            }
            Selected::Inherited(id) => Selected::Inherited(
                policy::find_starter(&character.starters, id)
                    .ok_or(ConversationLaunchError::StarterNotFound { starter_id: id })?,
            ),
            Selected::Disabled => Selected::Disabled,
        })
    }

    fn resolve_scene<'d>(
        &self,
        request: &DirectConversationLaunchRequest,
        character: &'d lettuce_characters::CharacterDetails,
        starter: Selected<&ConversationStarter>,
    ) -> Result<Selected<&'d Scene>, ConversationLaunchError> {
        match policy::scene_choice(request.scene, starter.map(|value| value.scene_id)) {
            policy::SceneChoice::Explicit(id) => Ok(Selected::Explicit(
                policy::find_scene(&character.scenes, id).ok_or(
                    ConversationLaunchError::SceneNotOwned {
                        scene_id: id,
                        character_id: request.character_id,
                    },
                )?,
            )),
            policy::SceneChoice::Inherited(id) => {
                match policy::find_any_scene(&character.scenes, id) {
                    None => Err(ConversationLaunchError::SceneNotFound { scene_id: id }),
                    Some(scene) if scene.status == lettuce_characters::LifecycleStatus::Active => {
                        Ok(Selected::Inherited(scene))
                    }
                    Some(_) => Ok(Selected::Disabled),
                }
            }
            policy::SceneChoice::Inherit => match policy::inherited_scene(
                &character.scenes,
                character.character.defaults.default_scene_id,
            ) {
                policy::InheritedScene::Resolved(scene) => Ok(Selected::Inherited(scene)),
                policy::InheritedScene::Dangling(scene_id) => {
                    Err(ConversationLaunchError::SceneNotFound { scene_id })
                }
                policy::InheritedScene::None => Ok(Selected::Disabled),
            },
            policy::SceneChoice::None => Ok(Selected::Disabled),
        }
    }

    fn check_for_drift(
        &self,
        request: &DirectConversationLaunchRequest,
        character: &lettuce_characters::CharacterDetails,
        persona: Option<&Persona>,
    ) -> Result<(), ConversationLaunchError> {
        let reread = CharacterRepository::get(self.sources, request.character_id)
            .map_err(LaunchSourceError::Character)?
            .ok_or(ConversationLaunchError::CharacterNotFound {
                character_id: request.character_id,
            })?;
        let persona_revisions = match persona {
            Some(value) => {
                let after = PersonaRepository::get(self.sources, value.id)
                    .map_err(LaunchSourceError::Persona)?
                    .ok_or(ConversationLaunchError::PersonaNotFound {
                        persona_id: value.id,
                    })?;
                Some((value.id, value.revision, after.revision))
            }
            None => None,
        };
        match policy::detect_source_drift(
            (
                request.character_id,
                character.character.revision,
                reread.character.revision,
            ),
            persona_revisions,
        ) {
            Some(changed) => Err(ConversationLaunchError::SourceChanged { changed }),
            None => Ok(()),
        }
    }

    fn resolve_persona(
        &self,
        selection: LaunchSelection<lettuce_types::PersonaId>,
    ) -> Result<Selected<Persona>, ConversationLaunchError> {
        match selection {
            LaunchSelection::Explicit(persona_id) => {
                let persona = PersonaRepository::get(self.sources, persona_id)
                    .map_err(LaunchSourceError::Persona)?
                    .ok_or(ConversationLaunchError::PersonaNotFound { persona_id })?;
                if persona.status == lettuce_characters::LifecycleStatus::Archived {
                    return Err(ConversationLaunchError::PersonaInactive { persona_id });
                }
                Ok(Selected::Explicit(persona))
            }
            LaunchSelection::Inherit => {
                let snapshot = PersonaRepository::get_default_snapshot(self.sources)
                    .map_err(LaunchSourceError::Persona)?;
                Ok(match snapshot.persona {
                    Some(persona)
                        if persona.status == lettuce_characters::LifecycleStatus::Active =>
                    {
                        Selected::Inherited(persona)
                    }
                    _ => Selected::Disabled,
                })
            }
            LaunchSelection::Disabled => Ok(Selected::Disabled),
        }
    }

    fn resolve_prompt(
        &self,
        defaults: &lettuce_characters::CharacterDefaults,
        starter: Selected<&ConversationStarter>,
    ) -> Result<Selected<PromptDocument>, ConversationLaunchError> {
        let choice: Selected<PromptDocumentId> =
            match starter.value().and_then(|value| value.prompt_id) {
                Some(id) => Selected::Explicit(id),
                None => match defaults.direct_prompt_id {
                    Some(id) => Selected::Inherited(id),
                    None => Selected::Disabled,
                },
            };
        let Some(prompt_id) = choice.value().copied() else {
            return Ok(Selected::Disabled);
        };
        let authored = matches!(choice, Selected::Explicit(_));
        let document = match PromptRepository::lookup_exact(
            self.sources,
            prompt_id,
            PromptPurpose::DirectChat,
        )
        .map_err(LaunchSourceError::Prompt)?
        {
            PromptLookupResult::Missing => {
                return Err(ConversationLaunchError::PromptNotFound { prompt_id });
            }
            PromptLookupResult::Archived { .. } if authored => {
                return Err(ConversationLaunchError::PromptArchived { prompt_id });
            }
            PromptLookupResult::Archived { .. } => return Ok(Selected::Disabled),
            PromptLookupResult::PurposeMismatch { .. } => {
                return Err(ConversationLaunchError::PromptWrongPurpose { prompt_id });
            }
            PromptLookupResult::Available { document } => document,
        };
        Ok(choice.with(document))
    }

    /// Books reached through bindings mirror context resolution and drop out
    /// when archived; a book a starter names explicitly does not.
    fn load_lorebooks(
        &self,
        ids: &[LorebookId],
        authored: &HashSet<LorebookId>,
    ) -> Result<Vec<LorebookDetails>, ConversationLaunchError> {
        let mut books = Vec::with_capacity(ids.len());
        for lorebook_id in ids {
            let details = LorebookRepository::get(self.sources, *lorebook_id)
                .map_err(LaunchSourceError::Lorebook)?
                .ok_or(ConversationLaunchError::LorebookNotFound {
                    lorebook_id: *lorebook_id,
                })?;
            if details.book.status == lettuce_context::LifecycleStatus::Archived {
                if authored.contains(lorebook_id) {
                    return Err(ConversationLaunchError::LorebookArchived {
                        lorebook_id: *lorebook_id,
                    });
                }
                continue;
            }
            books.push(details);
        }
        Ok(books)
    }

    fn resolve_model(
        &self,
        defaults: &lettuce_characters::CharacterDefaults,
    ) -> Result<Selected<(ModelProfile, ProviderAccount)>, ConversationLaunchError> {
        let settings =
            GlobalSettingsStore::load(self.sources).map_err(LaunchSourceError::Settings)?;
        let choice = policy::model_choice(defaults, settings.default_model_profile_id);
        let Some(model_profile_id) = choice.value().copied() else {
            return Ok(Selected::Disabled);
        };
        Ok(choice.with(self.load_model(model_profile_id)?))
    }

    /// Every launch model has to be a chat model behind an enabled provider,
    /// whether it was chosen by a character, a group member, or the app.
    fn load_model(
        &self,
        model_profile_id: ModelProfileId,
    ) -> Result<(ModelProfile, ProviderAccount), ConversationLaunchError> {
        let profile = ModelProfileRepository::get(self.sources, model_profile_id)
            .map_err(LaunchSourceError::Model)?
            .ok_or(ConversationLaunchError::ModelNotFound { model_profile_id })?;
        if profile.kind != ModelKind::Chat {
            return Err(ConversationLaunchError::NonChatModel { model_profile_id });
        }
        let provider_account_id = profile.provider_account_id;
        let account = ProviderAccountRepository::get(self.sources, provider_account_id)
            .map_err(LaunchSourceError::Model)?
            .ok_or(ConversationLaunchError::ProviderNotFound {
                provider_account_id,
            })?;
        if !account.enabled {
            return Err(ConversationLaunchError::ProviderDisabled {
                provider_account_id,
            });
        }
        Ok((profile, account))
    }
}

impl<'a, S> ConversationLaunchPlanner<'a, S>
where
    S: GroupLaunchSources + ConversationCreator,
{
    pub fn launch_group(
        &self,
        request: &GroupConversationLaunchRequest,
        now: TimestampMillis,
    ) -> Result<CreateConversationResult, ConversationLaunchError> {
        let launch = match self.prepare_group(request, now) {
            Ok(launch) => launch,
            Err(error) => {
                return Err(self.already_launched_or(&request.operation_key, error));
            }
        };
        ConversationCreator::create(self.sources, launch, now).map_err(|error| match error {
            ConversationRepositoryError::Conflict => ConversationLaunchError::CreateConflict,
            other => LaunchSourceError::Conversation(other).into(),
        })
    }

    /// `now` only stamps the initial participant policy document; it never
    /// reaches the request digest, so a retry minutes later still replays.
    pub fn prepare_group(
        &self,
        request: &GroupConversationLaunchRequest,
        now: TimestampMillis,
    ) -> Result<PreparedConversationLaunch, ConversationLaunchError> {
        if request.format_version != GROUP_LAUNCH_REQUEST_FORMAT_V1 {
            return Err(ConversationLaunchError::InvalidRequest {
                field: "format_version",
            });
        }
        check_display("user.display_name", &request.user.display_name)?;

        let conversation_id = launch_conversation_id(&request.operation_key);
        let identities = LaunchIdentities::new(conversation_id);
        let group_id = request.group_id;

        let group = GroupRepository::get(self.sources, group_id)
            .map_err(LaunchSourceError::Group)?
            .ok_or(ConversationLaunchError::GroupNotFound { group_id })?;
        if group.group.status == lettuce_characters::LifecycleStatus::Archived {
            return Err(ConversationLaunchError::GroupArchived { group_id });
        }
        let members = policy::ordered_members(&group.group.members);
        match policy::member_shape(&members) {
            policy::MemberShape::TooFew => {
                return Err(ConversationLaunchError::TooFewMembers {
                    min: policy::MIN_GROUP_MEMBERS,
                });
            }
            policy::MemberShape::TooMany => {
                return Err(ConversationLaunchError::TooManyMembers {
                    max: policy::MAX_GROUP_MEMBERS,
                });
            }
            policy::MemberShape::AllMuted => {
                return Err(ConversationLaunchError::AllMembersMuted { group_id });
            }
            policy::MemberShape::Launchable => {}
        }
        let mut characters = Vec::with_capacity(members.len());
        for member in &members {
            let character_id = member.character_id;
            let details = CharacterRepository::get(self.sources, character_id)
                .map_err(LaunchSourceError::Character)?
                .ok_or(ConversationLaunchError::MemberCharacterNotFound { character_id })?;
            if details.character.status == lettuce_characters::LifecycleStatus::Archived {
                return Err(ConversationLaunchError::MemberCharacterArchived { character_id });
            }
            if policy::is_companion(&details.character.defaults) {
                return Err(ConversationLaunchError::MemberCharacterCompanion { character_id });
            }
            characters.push(details);
        }

        let title = if !request.title.trim().is_empty() {
            request.title.clone()
        } else if !group.group.name.trim().is_empty() {
            group.group.name.clone()
        } else {
            let names: Vec<String> = characters
                .iter()
                .map(|details| policy::character_display_name(&details.character))
                .collect();
            policy::derive_group_title(&names, MAX_DISPLAY_BYTES)
        };
        check_display("title", &title)?;

        let chat_mode = policy::group_chat_mode(group.group.chat_mode);
        let memory_mode = policy::memory_mode_of(group.group.memory_policy);
        let persona = self.resolve_group_persona(request.persona, &group.group.persona)?;
        let scene = match chat_mode {
            GroupChatModeSnapshot::Conversation => Selected::Disabled,
            GroupChatModeSnapshot::Roleplay => self.resolve_group_scene(group_id, &group)?,
        };
        let variants = group
            .starting_scene
            .as_ref()
            .map_or(&[][..], |starting| starting.variants.as_slice());
        let scene_text = scene
            .value()
            .and_then(|value| policy::resolve_group_scene_text(value, variants));
        let prompt = self.resolve_group_prompt(&group.group, chat_mode)?;

        let group_bindings =
            GroupLorebookBindingRepository::list_group_bindings(self.sources, group_id)
                .map_err(LaunchSourceError::Binding)?;
        let member_books_disabled = group.group.disable_character_lorebooks;
        let mut member_bindings: Vec<Vec<LorebookBinding>> = Vec::new();
        if !member_books_disabled {
            for member in &members {
                member_bindings.push(
                    CharacterLorebookBindingRepository::list_character_bindings(
                        self.sources,
                        member.character_id,
                    )
                    .map_err(LaunchSourceError::Binding)?,
                );
            }
        }
        let persona_bindings = match persona.value() {
            Some(value) => {
                PersonaLorebookBindingRepository::list_persona_bindings(self.sources, value.id)
                    .map_err(LaunchSourceError::Binding)?
            }
            None => Vec::new(),
        };
        self.check_for_group_drift(&group, &characters, persona.value())?;

        let mut group_lorebooks = Selected::Explicit(policy::enabled_lorebooks(&group_bindings));
        let mut member_lorebooks: Vec<Selected<Vec<LorebookId>>> = if member_books_disabled {
            members.iter().map(|_| Selected::Disabled).collect()
        } else {
            member_bindings
                .iter()
                .map(|bindings| Selected::Inherited(policy::enabled_lorebooks(bindings)))
                .collect()
        };
        let mut persona_lorebooks = persona.with(policy::enabled_lorebooks(&persona_bindings));
        for scope in [group_lorebooks.value(), persona_lorebooks.value()]
            .into_iter()
            .flatten()
            .chain(member_lorebooks.iter().filter_map(Selected::value))
        {
            if policy::lorebook_bound_exceeded(scope.len()) {
                return Err(ConversationLaunchError::TooManyLorebooks {
                    max: policy::MAX_LAUNCH_LOREBOOKS,
                });
            }
        }

        let mut requested = LorebookRegistry::default();
        for id in group_lorebooks
            .value()
            .into_iter()
            .flatten()
            .chain(
                member_lorebooks
                    .iter()
                    .filter_map(Selected::value)
                    .flatten(),
            )
            .chain(persona_lorebooks.value().into_iter().flatten())
        {
            requested.register(*id);
        }
        if policy::lorebook_bound_exceeded(requested.ordered().len()) {
            return Err(ConversationLaunchError::TooManyLorebooks {
                max: policy::MAX_LAUNCH_LOREBOOKS,
            });
        }
        let books = self.load_lorebooks(requested.ordered(), &HashSet::new())?;
        let kept: Vec<LorebookId> = books.iter().map(|book| book.book.id).collect();
        group_lorebooks = group_lorebooks.map(|ids| retain(ids, &kept));
        member_lorebooks = member_lorebooks
            .into_iter()
            .map(|scope| scope.map(|ids| retain(ids, &kept)))
            .collect();
        persona_lorebooks = persona_lorebooks.map(|ids| retain(ids, &kept));

        let member_model_choices: Vec<Selected<ModelProfileId>> = members
            .iter()
            .zip(&characters)
            .map(|(member, details)| {
                policy::member_model_choice(member, &details.character.defaults)
            })
            .collect();
        let group_model_choice = if policy::group_model_needed(&member_model_choices) {
            let settings =
                GlobalSettingsStore::load(self.sources).map_err(LaunchSourceError::Settings)?;
            policy::application_model_choice(settings.default_model_profile_id)
        } else {
            Selected::Disabled
        };
        let mut model_registry = ModelRegistry::default();
        for id in group_model_choice
            .value()
            .into_iter()
            .chain(member_model_choices.iter().filter_map(Selected::value))
        {
            model_registry.register(*id);
        }
        let mut models = Vec::with_capacity(model_registry.ordered().len());
        for model_profile_id in model_registry.ordered() {
            models.push(self.load_model(*model_profile_id)?);
        }

        let group_draft = documents::draft(
            identities.artifact(ArtifactSlot::Group),
            group.group.revision,
            documents::group_body(&group.group, &members),
        )?;
        let mut member_drafts = Vec::with_capacity(characters.len());
        for (ordinal, details) in characters.iter().enumerate() {
            member_drafts.push(documents::draft(
                identities.artifact(ArtifactSlot::GroupMember(ordinal)),
                details.character.revision,
                documents::character_body(&details.character),
            )?);
        }
        let mut persona_draft = None;
        if let Some(value) = persona.value() {
            persona_draft = Some(documents::draft(
                identities.artifact(ArtifactSlot::Persona),
                value.revision,
                documents::persona_body(value),
            )?);
        }
        let mut scene_draft = None;
        if let Some(value) = scene.value() {
            scene_draft = Some(documents::draft(
                identities.artifact(ArtifactSlot::Scene),
                value.revision,
                documents::scene_body(value, variants),
            )?);
        }
        let mut prompt_draft = None;
        if let Some(value) = prompt.value() {
            prompt_draft = Some(documents::draft(
                identities.artifact(ArtifactSlot::Prompt),
                value.revision,
                documents::prompt_body(value),
            )?);
        }
        let mut lorebook_drafts = Vec::with_capacity(books.len());
        let mut lorebook_snapshots: Vec<(LorebookId, LorebookLaunchSnapshot)> =
            Vec::with_capacity(books.len());
        for (ordinal, book) in books.iter().enumerate() {
            let draft = documents::draft(
                identities.artifact(ArtifactSlot::Lorebook(ordinal)),
                book.book.revision,
                documents::lorebook_body(book),
            )?;
            lorebook_snapshots.push((
                book.book.id,
                LorebookLaunchSnapshot {
                    snapshot_ref: draft.reference(),
                    source_id: book.book.id,
                    source_revision: book.book.revision,
                    name: book.book.name.clone(),
                },
            ));
            lorebook_drafts.push(draft);
        }
        let mut model_drafts = Vec::with_capacity(models.len());
        let mut model_snapshots: Vec<(ModelProfileId, ModelSelectionSnapshot)> =
            Vec::with_capacity(models.len());
        for (ordinal, (model_profile_id, (profile, account))) in
            model_registry.ordered().iter().zip(&models).enumerate()
        {
            let draft = documents::draft(
                identities.artifact(ArtifactSlot::GroupModel(ordinal)),
                profile.revision,
                documents::model_body(profile, account),
            )?;
            model_snapshots.push((*model_profile_id, model_snapshot(profile, account, &draft)));
            model_drafts.push(draft);
        }

        let user_participant = ConversationParticipantDraft {
            id: identities.user_participant(),
            role: ParticipantRole::User,
            ordinal: 0,
            source: ParticipantSource::User,
            enabled: true,
            muted: false,
            display_name: request.user.display_name.clone(),
            authored_description: request.user.authored_description.clone(),
            model_selection: SnapshotSelection::Disabled,
        };
        let mut participants = vec![user_participant];
        let mut member_snapshots = Vec::with_capacity(members.len());
        for (ordinal, member) in members.iter().enumerate() {
            let details = &characters[ordinal];
            let model_override = member_model_choices[ordinal]
                .map(|id| collect_model(id, &model_snapshots))
                .into_snapshot();
            participants.push(ConversationParticipantDraft {
                id: identities.group_member_participant(ordinal),
                role: ParticipantRole::Character,
                ordinal: u32::try_from(ordinal + 1).unwrap_or(u32::MAX),
                source: ParticipantSource::Character(details.character.id),
                enabled: true,
                muted: member.muted,
                display_name: policy::character_display_name(&details.character),
                authored_description: None,
                model_selection: model_override.clone(),
            });
            member_snapshots.push(GroupMemberLaunchSnapshot {
                character: character_snapshot(&details.character, &member_drafts[ordinal]),
                ordinal: member.ordinal,
                enabled: true,
                muted: member.muted,
                model_override,
                lorebooks: member_lorebooks[ordinal]
                    .as_ref()
                    .map(|ids| collect_lorebooks(ids, &lorebook_snapshots))
                    .into_snapshot(),
            });
        }
        let initial_participant_policy = GroupParticipantPolicyDocument {
            members: participants
                .iter()
                .filter(|participant| participant.role == ParticipantRole::Character)
                .map(|participant| GroupParticipantPolicySnapshot {
                    participant_id: participant.id,
                    enabled: participant.enabled,
                    muted: participant.muted,
                    model_override: participant.model_selection.clone(),
                })
                .collect(),
            revision: Revision::INITIAL,
            created_at: now,
            updated_at: now,
        };

        let persona_snapshot = match persona_draft.as_ref() {
            None => SnapshotSelection::Disabled,
            Some(draft) => persona
                .as_ref()
                .map(|value| PersonaLaunchSnapshot {
                    snapshot_ref: draft.reference(),
                    source_id: value.id,
                    source_revision: value.revision,
                    title: value.title.clone(),
                    nickname: value.nickname.clone(),
                    lorebooks: persona_lorebooks
                        .as_ref()
                        .map(|ids| collect_lorebooks(ids, &lorebook_snapshots))
                        .into_snapshot(),
                })
                .into_snapshot(),
        };
        let scene_snapshot = match scene_draft.as_ref() {
            None => SnapshotSelection::Disabled,
            Some(draft) => scene
                .map(|value| SceneLaunchSnapshot {
                    snapshot_ref: draft.reference(),
                    source_id: value.id,
                    source_revision: value.revision,
                    title: policy::scene_title(scene_text.as_deref(), value.ordinal),
                })
                .into_snapshot(),
        };
        let prompt_snapshot = match prompt_draft.as_ref() {
            None => SnapshotSelection::Disabled,
            Some(draft) => prompt
                .as_ref()
                .map(|value| PromptLaunchSnapshot {
                    snapshot_ref: draft.reference(),
                    source_id: value.id,
                    source_revision: value.revision,
                    title: value.name.clone(),
                    purpose: match chat_mode {
                        GroupChatModeSnapshot::Conversation => {
                            PromptPurposeSnapshot::GroupConversational
                        }
                        GroupChatModeSnapshot::Roleplay => PromptPurposeSnapshot::GroupRoleplay,
                    },
                })
                .into_snapshot(),
        };
        let group_model_snapshot = group_model_choice
            .map(|id| collect_model(id, &model_snapshots))
            .into_snapshot();

        let mut entries = Vec::new();
        if let (Some(draft), Some(text)) = (scene_draft.as_ref(), scene_text.clone()) {
            entries.push(InitialMessageDraft {
                message_id: identities.scene_message(),
                revision_id: identities.scene_revision(),
                origin: InitialMessageOrigin::SelectedScene {
                    snapshot_ref: draft.reference(),
                },
                role: MessageRole::Scene,
                author_participant_id: None,
                parts: vec![MessagePart::Text { text }],
            });
        }
        let initial_timeline = InitialTimelineDraft {
            format_version: 1,
            entries,
        };

        let group_ref = group_draft.reference();
        let mut drafts: Vec<SnapshotArtifactDraft> = vec![group_draft];
        drafts.extend(member_drafts);
        drafts.extend(persona_draft);
        drafts.extend(scene_draft);
        drafts.extend(prompt_draft);
        drafts.extend(lorebook_drafts);
        drafts.extend(model_drafts);

        let member_intents: Vec<GroupMemberIntent<'_>> = members
            .iter()
            .enumerate()
            .map(|(ordinal, member)| GroupMemberIntent {
                ordinal: member.ordinal,
                character_id: member.character_id,
                enabled: true,
                muted: member.muted,
                model: member_model_choices[ordinal],
                lorebooks: member_lorebooks[ordinal].as_ref().map(Vec::as_slice),
            })
            .collect();
        let intent = GroupLaunchIntent {
            title: &title,
            chat_mode,
            memory_mode,
            members: &member_intents,
            scene: scene.map(|value| value.id),
            prompt: prompt.as_ref().map(|value| value.id),
            lorebooks: group_lorebooks.as_ref().map(Vec::as_slice),
            persona_lorebooks: persona_lorebooks.as_ref().map(Vec::as_slice),
            model: group_model_choice,
        };
        let ordered: Vec<&SnapshotArtifactDraft> = drafts.iter().collect();
        let request_digest = group_request_digest(request, &intent, &ordered, &initial_timeline)
            .ok_or(ConversationLaunchError::InvalidRequest {
                field: "request_digest",
            })?;

        let details = GroupConversationDetails {
            format_version: 1,
            group: GroupLaunchSnapshot {
                snapshot_ref: group_ref,
                source_id: group.group.id,
                source_revision: group.group.revision,
                name: group.group.name.clone(),
                members: member_snapshots,
                chat_mode,
                speaker_selection: policy::group_speaker_selection(group.group.speaker_selection),
                memory: SnapshotSelection::Inherited(MemorySettingsSnapshot {
                    policy_ref: None,
                    mode: memory_mode,
                    selected_revision_ids: Vec::new(),
                }),
                disable_character_lorebook: group.group.disable_character_lorebooks,
                persona: persona_snapshot,
                scene: scene_snapshot,
                prompt: prompt_snapshot,
                lorebooks: group_lorebooks
                    .as_ref()
                    .map(|ids| collect_lorebooks(ids, &lorebook_snapshots))
                    .into_snapshot(),
                model: group_model_snapshot,
            },
            initial_participant_policy,
        };

        let plan = CreateConversationPlan {
            conversation_id,
            title,
            kind: ConversationKind::Group(details),
            participants,
            initial_timeline,
            operation: OperationToken {
                key: request.operation_key.clone(),
                request_digest,
            },
        };
        Ok(PreparedConversationLaunch::new(plan, drafts)?)
    }

    /// The starting scene is authored on the group profile, so it carries the
    /// inherited provenance. A pointer that no longer resolves is an error,
    /// while an archived scene leaves the launch without one.
    fn resolve_group_scene<'d>(
        &self,
        group_id: GroupId,
        details: &'d GroupDetails,
    ) -> Result<Selected<&'d Scene>, ConversationLaunchError> {
        let Some(scene_id) = details.group.starting_scene_id else {
            return Ok(Selected::Disabled);
        };
        let starting = details
            .starting_scene
            .as_ref()
            .filter(|starting| starting.scene.id == scene_id)
            .ok_or(ConversationLaunchError::SceneNotFound { scene_id })?;
        if starting.scene.owner != SceneOwner::Group(group_id) {
            return Err(ConversationLaunchError::SceneNotOwnedByGroup { scene_id, group_id });
        }
        if starting.scene.status == lettuce_characters::LifecycleStatus::Archived {
            return Ok(Selected::Disabled);
        }
        Ok(Selected::Inherited(&starting.scene))
    }

    /// The group's own prompt for the mode, then the bundled built-in for that
    /// mode. Both tiers are inherited: nothing in a group launch request names
    /// a prompt, so an archived one degrades instead of failing. A member's
    /// direct prompt is never a group fallback.
    fn resolve_group_prompt(
        &self,
        group: &GroupProfile,
        chat_mode: GroupChatModeSnapshot,
    ) -> Result<Selected<PromptDocument>, ConversationLaunchError> {
        let (purpose, built_in, authored) = match chat_mode {
            GroupChatModeSnapshot::Conversation => (
                PromptPurpose::GroupChatConversational,
                BuiltInPromptId::GroupChat,
                group.group_conversation_prompt_id,
            ),
            GroupChatModeSnapshot::Roleplay => (
                PromptPurpose::GroupChatRoleplay,
                BuiltInPromptId::GroupChatRoleplay,
                group.group_roleplay_prompt_id,
            ),
        };
        if let Some(prompt_id) = authored {
            match PromptRepository::lookup_exact(self.sources, prompt_id, purpose)
                .map_err(LaunchSourceError::Prompt)?
            {
                PromptLookupResult::Missing => {
                    return Err(ConversationLaunchError::PromptNotFound { prompt_id });
                }
                PromptLookupResult::PurposeMismatch { .. } => {
                    return Err(ConversationLaunchError::PromptWrongPurpose { prompt_id });
                }
                PromptLookupResult::Available { document } => {
                    return Ok(Selected::Inherited(document));
                }
                PromptLookupResult::Archived { .. } => {}
            }
        }
        Ok(Selected::Inherited(
            self.built_in_prompt(built_in, purpose)?,
        ))
    }

    /// Scans the purpose-filtered library for the bundled document, which is
    /// identified by its stored provenance key rather than by a caller-supplied
    /// identity.
    fn built_in_prompt(
        &self,
        built_in: BuiltInPromptId,
        purpose: PromptPurpose,
    ) -> Result<PromptDocument, ConversationLaunchError> {
        let mut cursor = None;
        loop {
            let page = PromptRepository::page(
                self.sources,
                PromptLibraryQuery {
                    page: PageRequest {
                        cursor,
                        limit: lettuce_types::PageLimit::default(),
                    },
                    status: LifecycleFilter::Active,
                    purpose: Some(purpose),
                },
            )
            .map_err(LaunchSourceError::Prompt)?;
            if let Some(document) = page
                .items
                .into_iter()
                .find(|document| is_built_in(document, built_in))
            {
                return Ok(document);
            }
            match page.next_cursor {
                Some(next) => cursor = Some(next),
                None => return Err(ConversationLaunchError::BuiltInPromptMissing { purpose }),
            }
        }
    }

    /// A group launch request only says inherit, explicit, or off. Inherit
    /// walks the group profile's own selection first and only then the
    /// application default, so a cast keeps the persona it was authored with.
    fn resolve_group_persona(
        &self,
        selection: LaunchSelection<lettuce_types::PersonaId>,
        profile: &lettuce_characters::Selection<lettuce_types::PersonaId>,
    ) -> Result<Selected<Persona>, ConversationLaunchError> {
        if !matches!(selection, LaunchSelection::Inherit) {
            return self.resolve_persona(selection);
        }
        match profile {
            lettuce_characters::Selection::Disabled => Ok(Selected::Disabled),
            lettuce_characters::Selection::Inherit => {
                self.resolve_persona(LaunchSelection::Inherit)
            }
            lettuce_characters::Selection::Explicit(persona_id) => {
                let persona_id = *persona_id;
                let persona = PersonaRepository::get(self.sources, persona_id)
                    .map_err(LaunchSourceError::Persona)?
                    .ok_or(ConversationLaunchError::PersonaNotFound { persona_id })?;
                if persona.status == lettuce_characters::LifecycleStatus::Archived {
                    return self.resolve_persona(LaunchSelection::Inherit);
                }
                Ok(Selected::Inherited(persona))
            }
        }
    }

    fn check_for_group_drift(
        &self,
        group: &GroupDetails,
        characters: &[lettuce_characters::CharacterDetails],
        persona: Option<&Persona>,
    ) -> Result<(), ConversationLaunchError> {
        let group_id = group.group.id;
        let reread = GroupRepository::get(self.sources, group_id)
            .map_err(LaunchSourceError::Group)?
            .ok_or(ConversationLaunchError::GroupNotFound { group_id })?;
        let mut member_revisions = Vec::with_capacity(characters.len());
        for details in characters {
            let character_id = details.character.id;
            let after = CharacterRepository::get(self.sources, character_id)
                .map_err(LaunchSourceError::Character)?
                .ok_or(ConversationLaunchError::MemberCharacterNotFound { character_id })?;
            member_revisions.push((
                character_id,
                details.character.revision,
                after.character.revision,
            ));
        }
        let persona_revisions = match persona {
            Some(value) => {
                let after = PersonaRepository::get(self.sources, value.id)
                    .map_err(LaunchSourceError::Persona)?
                    .ok_or(ConversationLaunchError::PersonaNotFound {
                        persona_id: value.id,
                    })?;
                Some((value.id, value.revision, after.revision))
            }
            None => None,
        };
        match policy::detect_group_source_drift(
            (group_id, group.group.revision, reread.group.revision),
            &member_revisions,
            persona_revisions,
        ) {
            Some(changed) => Err(ConversationLaunchError::SourceChanged { changed }),
            None => Ok(()),
        }
    }
}

fn is_built_in(document: &PromptDocument, built_in: BuiltInPromptId) -> bool {
    matches!(
        &document.provenance,
        PromptProvenance::BuiltIn { key, .. }
            if BuiltInPromptId::from_key_or_alias(key) == Some(built_in)
    )
}

fn character_snapshot(
    character: &lettuce_characters::Character,
    draft: &SnapshotArtifactDraft,
) -> CharacterLaunchSnapshot {
    CharacterLaunchSnapshot {
        snapshot_ref: draft.reference(),
        source_id: character.id,
        source_revision: character.revision,
        name: character.profile.name.clone(),
        nickname: character.profile.nickname.clone(),
    }
}

fn model_snapshot(
    profile: &ModelProfile,
    account: &ProviderAccount,
    draft: &SnapshotArtifactDraft,
) -> ModelSelectionSnapshot {
    ModelSelectionSnapshot {
        snapshot_ref: draft.reference(),
        source_id: profile.id,
        source_revision: profile.revision,
        provider_kind: policy::provider_kind(account.protocol),
        external_model_id: profile.external_model_id.clone(),
        display_name: profile.display_name.clone(),
        context_length: profile.config.context_length,
        max_output_tokens: profile.config.max_output_tokens,
    }
}

fn collect_model(
    id: ModelProfileId,
    snapshots: &[(ModelProfileId, ModelSelectionSnapshot)],
) -> ModelSelectionSnapshot {
    snapshots
        .iter()
        .find(|(candidate, _)| *candidate == id)
        .map(|(_, snapshot)| snapshot.clone())
        .expect("every resolved model is registered before its snapshot is used")
}

fn retain(ids: Vec<LorebookId>, kept: &[LorebookId]) -> Vec<LorebookId> {
    ids.into_iter().filter(|id| kept.contains(id)).collect()
}

fn collect_lorebooks(
    ids: &[LorebookId],
    snapshots: &[(LorebookId, LorebookLaunchSnapshot)],
) -> Vec<LorebookLaunchSnapshot> {
    ids.iter()
        .filter_map(|id| {
            snapshots
                .iter()
                .find(|(candidate, _)| candidate == id)
                .map(|(_, snapshot)| snapshot.clone())
        })
        .collect()
}

fn check_display(field: &'static str, value: &str) -> Result<(), ConversationLaunchError> {
    if value.trim().is_empty() || value.len() > MAX_DISPLAY_BYTES {
        return Err(ConversationLaunchError::InvalidRequest { field });
    }
    Ok(())
}
