use lettuce_characters::{
    CharacterRepository, ConversationStarter, Persona, PersonaRepository, Scene, StarterRole,
};
use lettuce_context::{
    CharacterLorebookBindingRepository, LorebookDetails, LorebookRepository,
    PersonaLorebookBindingRepository, PromptDocument, PromptLookupResult, PromptPurpose,
    PromptRepository,
};
use lettuce_conversations::{
    CharacterLaunchSnapshot, ConversationCreator, ConversationKind, ConversationParticipantDraft,
    ConversationReader, ConversationRepositoryError, CreateConversationPlan,
    CreateConversationResult, DirectConversationDetails, InitialMessageDraft, InitialMessageOrigin,
    InitialTimelineDraft, LorebookLaunchSnapshot, MemorySettingsSnapshot, MessagePart, MessageRole,
    ModelSelectionSnapshot, OperationToken, ParticipantRole, ParticipantSource,
    PersonaLaunchSnapshot, PreparedConversationLaunch, PromptLaunchSnapshot, PromptPurposeSnapshot,
    SceneLaunchSnapshot, SnapshotArtifactDraft, SnapshotSelection, StarterLaunchSnapshot,
};
use lettuce_models::{
    ModelKind, ModelProfile, ModelProfileRepository, ProviderAccount, ProviderAccountRepository,
};
use lettuce_settings::GlobalSettingsStore;
use lettuce_types::{LorebookId, PromptDocumentId, TimestampMillis};
use std::collections::HashSet;

use super::digest::direct_request_digest;
use super::documents;
use super::error::{DirectLaunchError, LaunchSourceError};
use super::identity::{ArtifactSlot, LaunchIdentities, launch_conversation_id};
use super::policy::{self, LorebookRegistry, Selected};
use super::request::{
    DIRECT_LAUNCH_REQUEST_FORMAT_V1, DirectConversationLaunchRequest, LaunchSelection,
};

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
    ) -> Result<CreateConversationResult, DirectLaunchError> {
        let launch = match self.prepare_direct(request) {
            Ok(launch) => launch,
            Err(error) => return Err(self.already_launched_or(request, error)),
        };
        ConversationCreator::create(self.sources, launch, now).map_err(|error| match error {
            ConversationRepositoryError::Conflict => DirectLaunchError::CreateConflict,
            other => LaunchSourceError::Conversation(other).into(),
        })
    }

    /// A source archived or deleted after a committed launch must not look
    /// like a fresh failure; the caller is told to open what already exists.
    fn already_launched_or(
        &self,
        request: &DirectConversationLaunchRequest,
        error: DirectLaunchError,
    ) -> DirectLaunchError {
        let conversation_id = launch_conversation_id(&request.operation_key);
        if ConversationReader::get(self.sources, conversation_id).is_ok() {
            return DirectLaunchError::AlreadyLaunched { conversation_id };
        }
        error
    }

    pub fn prepare_direct(
        &self,
        request: &DirectConversationLaunchRequest,
    ) -> Result<PreparedConversationLaunch, DirectLaunchError> {
        if request.format_version != DIRECT_LAUNCH_REQUEST_FORMAT_V1 {
            return Err(DirectLaunchError::InvalidRequest {
                field: "format_version",
            });
        }
        check_display("title", &request.title)?;
        check_display("user.display_name", &request.user.display_name)?;

        let conversation_id = launch_conversation_id(&request.operation_key);
        let identities = LaunchIdentities::new(conversation_id);

        let character = CharacterRepository::get(self.sources, request.character_id)
            .map_err(LaunchSourceError::Character)?
            .ok_or(DirectLaunchError::CharacterNotFound {
                character_id: request.character_id,
            })?;
        if character.character.status == lettuce_characters::LifecycleStatus::Archived {
            return Err(DirectLaunchError::CharacterArchived {
                character_id: request.character_id,
            });
        }
        if policy::is_companion(&character.character.defaults) {
            return Err(DirectLaunchError::CompanionUnsupported {
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
            return Err(DirectLaunchError::TooManyInitialMessages {
                max: policy::MAX_LAUNCH_TIMELINE_ENTRIES,
            });
        }
        let persona = self.resolve_persona(request)?;
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
            return Err(DirectLaunchError::TooManyLorebooks {
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
        let character_snapshot = CharacterLaunchSnapshot {
            snapshot_ref: character_draft.reference(),
            source_id: character.character.id,
            source_revision: character.character.revision,
            name: character.character.profile.name.clone(),
            nickname: character.character.profile.nickname.clone(),
        };
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
                .map(|(profile, account)| ModelSelectionSnapshot {
                    snapshot_ref: draft.reference(),
                    source_id: profile.id,
                    source_revision: profile.revision,
                    provider_kind: policy::provider_kind(account.protocol),
                    external_model_id: profile.external_model_id.clone(),
                    display_name: profile.display_name.clone(),
                    context_length: profile.config.context_length,
                    max_output_tokens: profile.config.max_output_tokens,
                })
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
            DirectLaunchError::InvalidRequest {
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
    ) -> Result<Selected<&'d ConversationStarter>, DirectLaunchError> {
        Ok(match policy::starter_choice(request.starter) {
            Selected::Explicit(id) => {
                Selected::Explicit(policy::find_starter(&character.starters, id).ok_or(
                    DirectLaunchError::StarterNotOwned {
                        starter_id: id,
                        character_id: request.character_id,
                    },
                )?)
            }
            Selected::Inherited(id) => Selected::Inherited(
                policy::find_starter(&character.starters, id)
                    .ok_or(DirectLaunchError::StarterNotFound { starter_id: id })?,
            ),
            Selected::Disabled => Selected::Disabled,
        })
    }

    fn resolve_scene<'d>(
        &self,
        request: &DirectConversationLaunchRequest,
        character: &'d lettuce_characters::CharacterDetails,
        starter: Selected<&ConversationStarter>,
    ) -> Result<Selected<&'d Scene>, DirectLaunchError> {
        match policy::scene_choice(request.scene, starter.map(|value| value.scene_id)) {
            policy::SceneChoice::Explicit(id) => Ok(Selected::Explicit(
                policy::find_scene(&character.scenes, id).ok_or(
                    DirectLaunchError::SceneNotOwned {
                        scene_id: id,
                        character_id: request.character_id,
                    },
                )?,
            )),
            policy::SceneChoice::Inherited(id) => {
                match policy::find_any_scene(&character.scenes, id) {
                    None => Err(DirectLaunchError::SceneNotFound { scene_id: id }),
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
                    Err(DirectLaunchError::SceneNotFound { scene_id })
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
    ) -> Result<(), DirectLaunchError> {
        let reread = CharacterRepository::get(self.sources, request.character_id)
            .map_err(LaunchSourceError::Character)?
            .ok_or(DirectLaunchError::CharacterNotFound {
                character_id: request.character_id,
            })?;
        let persona_revisions = match persona {
            Some(value) => {
                let after = PersonaRepository::get(self.sources, value.id)
                    .map_err(LaunchSourceError::Persona)?
                    .ok_or(DirectLaunchError::PersonaNotFound {
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
            Some(changed) => Err(DirectLaunchError::SourceChanged { changed }),
            None => Ok(()),
        }
    }

    fn resolve_persona(
        &self,
        request: &DirectConversationLaunchRequest,
    ) -> Result<Selected<Persona>, DirectLaunchError> {
        match request.persona {
            LaunchSelection::Explicit(persona_id) => {
                let persona = PersonaRepository::get(self.sources, persona_id)
                    .map_err(LaunchSourceError::Persona)?
                    .ok_or(DirectLaunchError::PersonaNotFound { persona_id })?;
                if persona.status == lettuce_characters::LifecycleStatus::Archived {
                    return Err(DirectLaunchError::PersonaInactive { persona_id });
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
    ) -> Result<Selected<PromptDocument>, DirectLaunchError> {
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
                return Err(DirectLaunchError::PromptNotFound { prompt_id });
            }
            PromptLookupResult::Archived { .. } if authored => {
                return Err(DirectLaunchError::PromptArchived { prompt_id });
            }
            PromptLookupResult::Archived { .. } => return Ok(Selected::Disabled),
            PromptLookupResult::PurposeMismatch { .. } => {
                return Err(DirectLaunchError::PromptWrongPurpose { prompt_id });
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
    ) -> Result<Vec<LorebookDetails>, DirectLaunchError> {
        let mut books = Vec::with_capacity(ids.len());
        for lorebook_id in ids {
            let details = LorebookRepository::get(self.sources, *lorebook_id)
                .map_err(LaunchSourceError::Lorebook)?
                .ok_or(DirectLaunchError::LorebookNotFound {
                    lorebook_id: *lorebook_id,
                })?;
            if details.book.status == lettuce_context::LifecycleStatus::Archived {
                if authored.contains(lorebook_id) {
                    return Err(DirectLaunchError::LorebookArchived {
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
    ) -> Result<Selected<(ModelProfile, ProviderAccount)>, DirectLaunchError> {
        let settings =
            GlobalSettingsStore::load(self.sources).map_err(LaunchSourceError::Settings)?;
        let choice = policy::model_choice(defaults, settings.default_model_profile_id);
        let Some(model_profile_id) = choice.value().copied() else {
            return Ok(Selected::Disabled);
        };
        let profile = ModelProfileRepository::get(self.sources, model_profile_id)
            .map_err(LaunchSourceError::Model)?
            .ok_or(DirectLaunchError::ModelNotFound { model_profile_id })?;
        if profile.kind != ModelKind::Chat {
            return Err(DirectLaunchError::NonChatModel { model_profile_id });
        }
        let provider_account_id = profile.provider_account_id;
        let account = ProviderAccountRepository::get(self.sources, provider_account_id)
            .map_err(LaunchSourceError::Model)?
            .ok_or(DirectLaunchError::ProviderNotFound {
                provider_account_id,
            })?;
        if !account.enabled {
            return Err(DirectLaunchError::ProviderDisabled {
                provider_account_id,
            });
        }
        Ok(choice.with((profile, account)))
    }
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

fn check_display(field: &'static str, value: &str) -> Result<(), DirectLaunchError> {
    if value.trim().is_empty() || value.len() > MAX_DISPLAY_BYTES {
        return Err(DirectLaunchError::InvalidRequest { field });
    }
    Ok(())
}
