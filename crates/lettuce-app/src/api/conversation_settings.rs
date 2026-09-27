//! A conversation's settings, participants, title and lifecycle.

use lettuce_characters::{CharacterRepository, GroupProfile, LifecycleStatus, Selection};
use lettuce_context::{
    CharacterLorebookBindingRepository, GroupLorebookBindingRepository,
    PersonaLorebookBindingRepository,
};
use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_conversations::{
    ArchiveConversation, Conversation, ConversationBackground, ConversationKind,
    ConversationLifecycle, ConversationReader, ConversationRepository, GroupChatModeSnapshot,
    GroupSpeakerSelectionSnapshot, MemoryModeSnapshot, ParticipantRole, ParticipantSource,
    RenameConversation, RestoreConversation, SettingProvenance, SnapshotSelection,
};
use lettuce_types::{
    AssetId, CharacterId, ConversationId, ConversationParticipantId, LorebookId, ModelProfileId,
    PersonaId, PromptDocumentId, Revision, SceneId,
};

use super::ApiContext;
use super::error::{IntoApiError, api_error, invalid_field, parse_id};
use super::mapping;
use crate::conversation::{self, Change, Choice, ConversationEditError};
use crate::generation::live_sources::{self, LiveGroup, MemberModel};
use crate::launch::policy::{self, PromptSource};

fn revisions(conversation: &Conversation) -> dto::ConversationRevisions {
    dto::ConversationRevisions {
        revision: conversation.revision.get(),
        settings_revision: conversation.settings_revision().map(Revision::get),
    }
}

fn source_error<E>(_: E) -> ApiError {
    api_error(
        ApiErrorCode::Internal,
        "a source of the conversation could not be read",
    )
}

/// Every setting of a conversation with its current value and where it
/// comes from, resolved the way a turn resolves it.
pub async fn conversation_settings_get(
    context: &ApiContext,
    request: dto::ConversationSettingsGetRequest,
) -> Result<dto::ConversationSettingsView, ApiError> {
    let conversation_id: ConversationId = parse_id(&request.conversation_id, "conversation_id")?;
    context
        .blocking(move |context| settings_view(context, conversation_id))
        .await
}

pub(crate) fn settings_view(
    context: &ApiContext,
    conversation_id: ConversationId,
) -> Result<dto::ConversationSettingsView, ApiError> {
    let database = context.backend().database();
    let conversation = ConversationReader::get(database, conversation_id)
        .map_err(IntoApiError::into_api_error)?
        .conversation;
    let global = lettuce_settings::GlobalSettingsStore::load(database)
        .map_err(source_error)?
        .settings;
    let live = live_sources::live_group(database, &conversation).map_err(source_error)?;
    let own = conversation.current_settings.as_ref();
    let profile = live.as_ref().and_then(|group| group.profile.as_ref());
    let persona = live_sources::live_persona(database, &conversation, profile)
        .map_err(source_error)?
        .map(|persona| persona.id);
    let memory = live_sources::live_memory(database, &conversation, &global)
        .map_err(source_error)?
        .map_or(dto::MemoryMode::Disabled, |memory| memory_mode(memory.mode));
    let memory_source = match own.map(|settings| settings.memory_provenance) {
        Some(SettingProvenance::CurrentOverride | SettingProvenance::Disabled) => {
            dto::SettingSource::Conversation
        }
        _ => match &conversation.kind {
            ConversationKind::Direct(details) => {
                if CharacterRepository::get(database, details.character.source_id)
                    .map_err(source_error)?
                    .is_some()
                {
                    dto::SettingSource::Character
                } else {
                    dto::SettingSource::Launch
                }
            }
            ConversationKind::Group(_) => group_or_launch(profile),
        },
    };
    let background = background(context, &conversation, profile);
    let view = match &conversation.kind {
        ConversationKind::Direct(details) => {
            let character = CharacterRepository::get(database, details.character.source_id)
                .map_err(source_error)?;
            let app =
                lettuce_settings::GlobalSettingsStore::load(database).map_err(source_error)?;
            let persona_source = match own.map(|settings| settings.persona_provenance) {
                Some(SettingProvenance::CurrentOverride | SettingProvenance::Disabled) => {
                    dto::SettingSource::Conversation
                }
                _ => match &details.persona {
                    SnapshotSelection::Explicit(launch) if persona == Some(launch.source_id) => {
                        dto::SettingSource::Launch
                    }
                    _ => dto::SettingSource::AppDefault,
                },
            };
            let prompt = direct_prompt(context, &conversation, details, character.as_ref(), &app)?;
            let lorebooks =
                match own.map(|settings| (settings.lorebooks_provenance, &settings.lorebooks)) {
                    Some((SettingProvenance::Disabled, _)) => {
                        choices(Vec::<LorebookId>::new(), dto::SettingSource::Conversation)
                    }
                    Some((SettingProvenance::CurrentOverride, books)) => choices(
                        books.iter().flatten().map(|book| book.source_id).collect(),
                        dto::SettingSource::Conversation,
                    ),
                    _ => match &details.lorebooks {
                        SnapshotSelection::Explicit(books) => choices(
                            books.iter().map(|book| book.source_id).collect(),
                            dto::SettingSource::Launch,
                        ),
                        SnapshotSelection::Disabled => {
                            choices(Vec::<LorebookId>::new(), dto::SettingSource::Launch)
                        }
                        SnapshotSelection::Inherited(_) => {
                            let mut ids = policy::enabled_lorebooks(
                                &database
                                    .list_character_bindings(details.character.source_id)
                                    .map_err(source_error)?,
                            );
                            if let Some(persona) = persona {
                                ids.extend(policy::enabled_lorebooks(
                                    &database
                                        .list_persona_bindings(persona)
                                        .map_err(source_error)?,
                                ));
                            }
                            choices(ids, dto::SettingSource::Character)
                        }
                    },
                };
            let model = match own
                .filter(|settings| settings.model_provenance == SettingProvenance::CurrentOverride)
                .and_then(|settings| settings.model_override.as_ref())
            {
                Some(model) => choice(Some(model.source_id), dto::SettingSource::Conversation),
                None => match character
                    .as_ref()
                    .and_then(|character| character.character.defaults.model_profile_id)
                {
                    Some(id) => choice(Some(id), dto::SettingSource::Character),
                    None => choice(app.default_model_profile_id, dto::SettingSource::AppDefault),
                },
            };
            let scene = match own.map(|settings| (settings.scene_provenance, &settings.scene)) {
                Some((SettingProvenance::CurrentOverride, scene)) => choice(
                    scene.as_ref().map(|scene| scene.source_id),
                    dto::SettingSource::Conversation,
                ),
                Some((SettingProvenance::Disabled, _)) => {
                    choice::<SceneId>(None, dto::SettingSource::Conversation)
                }
                _ => choice(
                    selection_id(&details.scene, |scene| scene.source_id),
                    dto::SettingSource::Launch,
                ),
            };
            dto::ConversationSettingsView {
                conversation_id: conversation.id.to_string(),
                kind: dto::ConversationKind::Direct,
                title: conversation.title.clone(),
                revision: conversation.revision.get(),
                settings_revision: conversation.settings_revision().map(Revision::get),
                author_note: own.and_then(|settings| settings.author_note.clone()),
                persona: choice(persona, persona_source),
                prompt,
                roleplay_prompt: None,
                lorebooks,
                model,
                background,
                scene,
                memory: dto::SettingMemory {
                    mode: memory,
                    source: memory_source,
                },
                chat_mode: None,
                disable_character_lorebooks: None,
                speaker_selection: None,
                members: None,
            }
        }
        ConversationKind::Group(details) => {
            let live = live.as_ref().ok_or_else(|| {
                api_error(ApiErrorCode::Internal, "a group chat has no group settings")
            })?;
            let persona_source = match own.map(|settings| settings.persona_provenance) {
                Some(SettingProvenance::CurrentOverride | SettingProvenance::Disabled) => {
                    dto::SettingSource::Conversation
                }
                _ => match (&details.group.persona, profile) {
                    (SnapshotSelection::Explicit(_), _) | (_, None) => dto::SettingSource::Launch,
                    (_, Some(profile)) => match profile.persona {
                        Selection::Inherit => dto::SettingSource::AppDefault,
                        Selection::Explicit(_) | Selection::Disabled => dto::SettingSource::Group,
                    },
                },
            };
            let lorebooks =
                match own.map(|settings| (settings.lorebooks_provenance, &settings.lorebooks)) {
                    Some((SettingProvenance::Disabled, _)) => {
                        choices(Vec::<LorebookId>::new(), dto::SettingSource::Conversation)
                    }
                    Some((SettingProvenance::CurrentOverride, books)) => choices(
                        books.iter().flatten().map(|book| book.source_id).collect(),
                        dto::SettingSource::Conversation,
                    ),
                    _ => match &details.group.lorebooks {
                        SnapshotSelection::Disabled => {
                            choices(Vec::<LorebookId>::new(), dto::SettingSource::Launch)
                        }
                        _ => choices(
                            policy::enabled_lorebooks(
                                &database
                                    .list_group_bindings(details.group.source_id)
                                    .map_err(source_error)?,
                            ),
                            group_or_launch(profile),
                        ),
                    },
                };
            let model = match own
                .filter(|settings| settings.model_provenance == SettingProvenance::CurrentOverride)
                .and_then(|settings| settings.model_override.as_ref())
            {
                Some(model) => choice(Some(model.source_id), dto::SettingSource::Conversation),
                None => choice::<ModelProfileId>(None, group_or_launch(profile)),
            };
            let scene = match own.map(|settings| (settings.scene_provenance, &settings.scene)) {
                Some((SettingProvenance::CurrentOverride, scene)) => choice(
                    scene.as_ref().map(|scene| scene.source_id),
                    dto::SettingSource::Conversation,
                ),
                Some((SettingProvenance::Disabled, _)) => {
                    choice::<SceneId>(None, dto::SettingSource::Conversation)
                }
                _ => match profile {
                    Some(profile) => choice(
                        profile.starting_scene_id.filter(|id| {
                            live.starting_scene.as_ref().is_some_and(|starting| {
                                starting.scene.id == *id
                                    && starting.scene.status == LifecycleStatus::Active
                            })
                        }),
                        dto::SettingSource::Group,
                    ),
                    None => choice(
                        selection_id(&details.group.scene, |scene| scene.source_id),
                        dto::SettingSource::Launch,
                    ),
                },
            };
            dto::ConversationSettingsView {
                conversation_id: conversation.id.to_string(),
                kind: dto::ConversationKind::Group,
                title: conversation.title.clone(),
                revision: conversation.revision.get(),
                settings_revision: conversation.settings_revision().map(Revision::get),
                author_note: own.and_then(|settings| settings.author_note.clone()),
                persona: choice(persona, persona_source),
                prompt: group_prompt(
                    context,
                    &conversation,
                    profile,
                    GroupChatModeSnapshot::Conversation,
                )?,
                roleplay_prompt: Some(group_prompt(
                    context,
                    &conversation,
                    profile,
                    GroupChatModeSnapshot::Roleplay,
                )?),
                lorebooks,
                model,
                background,
                scene,
                memory: dto::SettingMemory {
                    mode: memory,
                    source: memory_source,
                },
                chat_mode: Some(dto::SettingChatMode {
                    mode: mapping::group_chat_mode(live.chat_mode),
                    source: own_or_group(
                        own.and_then(|settings| settings.chat_mode).is_some(),
                        profile,
                    ),
                }),
                disable_character_lorebooks: Some(dto::SettingFlag {
                    value: live.disable_character_lorebooks,
                    source: own_or_group(
                        own.and_then(|settings| settings.disable_character_lorebooks)
                            .is_some(),
                        profile,
                    ),
                }),
                speaker_selection: Some(dto::SettingSpeakerSelection {
                    method: speaker_method(live.speaker_selection),
                    source: own_or_group(
                        own.is_some_and(|settings| {
                            settings.speaker_selection_provenance
                                == SettingProvenance::CurrentOverride
                        }),
                        profile,
                    ),
                }),
                members: Some(members(context, &conversation, live)?),
            }
        }
    };
    Ok(view)
}

fn choice<T: ToString>(id: Option<T>, source: dto::SettingSource) -> dto::SettingChoice {
    dto::SettingChoice {
        id: id.map(|id| id.to_string()),
        source,
    }
}

fn choices<T: ToString>(ids: Vec<T>, source: dto::SettingSource) -> dto::SettingChoices {
    dto::SettingChoices {
        ids: ids.iter().map(ToString::to_string).collect(),
        source,
    }
}

fn selection_id<T, I>(selection: &SnapshotSelection<T>, id: impl Fn(&T) -> I) -> Option<I> {
    match selection {
        SnapshotSelection::Inherited(value) | SnapshotSelection::Explicit(value) => Some(id(value)),
        SnapshotSelection::Disabled => None,
    }
}

const fn group_or_launch(profile: Option<&GroupProfile>) -> dto::SettingSource {
    if profile.is_some() {
        dto::SettingSource::Group
    } else {
        dto::SettingSource::Launch
    }
}

const fn own_or_group(own: bool, profile: Option<&GroupProfile>) -> dto::SettingSource {
    if own {
        dto::SettingSource::Conversation
    } else {
        group_or_launch(profile)
    }
}

const fn memory_mode(mode: MemoryModeSnapshot) -> dto::MemoryMode {
    match mode {
        MemoryModeSnapshot::Manual => dto::MemoryMode::Manual,
        MemoryModeSnapshot::Dynamic => dto::MemoryMode::Dynamic,
        MemoryModeSnapshot::Disabled => dto::MemoryMode::Disabled,
    }
}

const fn speaker_method(method: GroupSpeakerSelectionSnapshot) -> dto::SpeakerSelectionMethod {
    match method {
        GroupSpeakerSelectionSnapshot::Llm => dto::SpeakerSelectionMethod::Llm,
        GroupSpeakerSelectionSnapshot::Heuristic => dto::SpeakerSelectionMethod::Heuristic,
        GroupSpeakerSelectionSnapshot::RoundRobin => dto::SpeakerSelectionMethod::RoundRobin,
        GroupSpeakerSelectionSnapshot::Director => dto::SpeakerSelectionMethod::Director,
        GroupSpeakerSelectionSnapshot::DirectorAction => {
            dto::SpeakerSelectionMethod::DirectorAction
        }
    }
}

const fn speaker_snapshot(method: dto::SpeakerSelectionMethod) -> GroupSpeakerSelectionSnapshot {
    match method {
        dto::SpeakerSelectionMethod::Llm => GroupSpeakerSelectionSnapshot::Llm,
        dto::SpeakerSelectionMethod::Heuristic => GroupSpeakerSelectionSnapshot::Heuristic,
        dto::SpeakerSelectionMethod::RoundRobin => GroupSpeakerSelectionSnapshot::RoundRobin,
        dto::SpeakerSelectionMethod::Director => GroupSpeakerSelectionSnapshot::Director,
        dto::SpeakerSelectionMethod::DirectorAction => {
            GroupSpeakerSelectionSnapshot::DirectorAction
        }
    }
}

fn background(
    context: &ApiContext,
    conversation: &Conversation,
    profile: Option<&GroupProfile>,
) -> dto::SettingBackground {
    match conversation
        .current_settings
        .as_ref()
        .and_then(|settings| settings.background)
    {
        Some(ConversationBackground::Image { asset_id }) => dto::SettingBackground {
            asset: Some(context.asset_ref(asset_id)),
            hidden: false,
            source: dto::SettingSource::Conversation,
        },
        Some(ConversationBackground::Hidden) => dto::SettingBackground {
            asset: None,
            hidden: true,
            source: dto::SettingSource::Conversation,
        },
        None => match &conversation.kind {
            ConversationKind::Direct(_) => dto::SettingBackground {
                asset: None,
                hidden: false,
                source: dto::SettingSource::Character,
            },
            ConversationKind::Group(_) => dto::SettingBackground {
                asset: profile
                    .and_then(|profile| profile.background_asset_id)
                    .map(|asset_id| context.asset_ref(asset_id)),
                hidden: false,
                source: group_or_launch(profile),
            },
        },
    }
}

fn prompt_source(source: PromptSource) -> dto::SettingSource {
    match source {
        PromptSource::Conversation => dto::SettingSource::Conversation,
        PromptSource::Starter => dto::SettingSource::Launch,
        PromptSource::Character => dto::SettingSource::Character,
        PromptSource::Group => dto::SettingSource::Group,
        PromptSource::AppDefault => dto::SettingSource::AppDefault,
    }
}

fn direct_prompt(
    context: &ApiContext,
    conversation: &Conversation,
    details: &lettuce_conversations::DirectConversationDetails,
    character: Option<&lettuce_characters::CharacterDetails>,
    app: &lettuce_settings::StoredGlobalSettings,
) -> Result<dto::SettingChoice, ApiError> {
    let database = context.backend().database();
    let own = conversation
        .current_settings
        .as_ref()
        .map(|settings| (settings.prompt_provenance, settings.prompt.as_ref()));
    if let Some((SettingProvenance::Disabled, _)) = own {
        return Ok(choice::<PromptDocumentId>(
            None,
            dto::SettingSource::Conversation,
        ));
    }
    let Some(character) = character else {
        return Ok(choice(
            selection_id(&details.prompt, |prompt| prompt.source_id),
            dto::SettingSource::Launch,
        ));
    };
    let defaults = &character.character.defaults;
    let companion =
        crate::companion::companion_clock::companion_clock_context(database, conversation)
            .map_err(source_error)?
            .companion;
    if companion {
        let document = policy::companion_prompt(
            database,
            defaults.companion_soul.as_ref(),
            app.default_prompt_document_id,
        )
        .map_err(source_error)?;
        return Ok(choice(
            document.map(|document| document.id),
            dto::SettingSource::Character,
        ));
    }
    let selected = match own {
        Some((SettingProvenance::CurrentOverride, prompt)) => prompt.map(|prompt| prompt.source_id),
        _ => None,
    };
    let starter = match &details.starter {
        SnapshotSelection::Inherited(starter) | SnapshotSelection::Explicit(starter) => character
            .starters
            .iter()
            .find(|live| live.id == starter.source_id)
            .and_then(|live| live.prompt_id),
        SnapshotSelection::Disabled => None,
    };
    Ok(
        match policy::direct_prompt_with_source(
            database,
            selected,
            starter,
            defaults.direct_prompt_id,
            app.default_prompt_document_id,
        )
        .map_err(source_error)?
        {
            Some((document, source)) => choice(Some(document.id), prompt_source(source)),
            None => choice::<PromptDocumentId>(None, dto::SettingSource::AppDefault),
        },
    )
}

fn group_prompt(
    context: &ApiContext,
    conversation: &Conversation,
    profile: Option<&GroupProfile>,
    mode: GroupChatModeSnapshot,
) -> Result<dto::SettingChoice, ApiError> {
    let own = conversation
        .current_settings
        .as_ref()
        .map(|settings| match mode {
            GroupChatModeSnapshot::Conversation => {
                (settings.prompt_provenance, settings.prompt.as_ref())
            }
            GroupChatModeSnapshot::Roleplay => (
                settings.roleplay_prompt_provenance,
                settings.roleplay_prompt.as_ref(),
            ),
        });
    let selected = match own {
        Some((SettingProvenance::Disabled, _)) => {
            return Ok(choice::<PromptDocumentId>(
                None,
                dto::SettingSource::Conversation,
            ));
        }
        Some((SettingProvenance::CurrentOverride, prompt)) => prompt.map(|prompt| prompt.source_id),
        _ => None,
    };
    let group = profile.and_then(|profile| match mode {
        GroupChatModeSnapshot::Conversation => profile.group_conversation_prompt_id,
        GroupChatModeSnapshot::Roleplay => profile.group_roleplay_prompt_id,
    });
    Ok(
        match policy::group_prompt_with_source(
            context.backend().database(),
            mode,
            [selected, None, group],
        )
        .map_err(source_error)?
        {
            Some((document, source)) => choice(Some(document.id), prompt_source(source)),
            None => choice::<PromptDocumentId>(None, dto::SettingSource::AppDefault),
        },
    )
}

fn members(
    context: &ApiContext,
    conversation: &Conversation,
    live: &LiveGroup,
) -> Result<dto::GroupMembersSettings, ApiError> {
    let database = context.backend().database();
    let profile = live.profile.as_ref();
    let own = conversation.current_settings.as_ref();
    let owned = |flag: bool| own_or_group(flag, profile);
    let members_owned = own.is_some_and(|settings| settings.members_overridden);
    let muted_owned = own.is_some_and(|settings| settings.muted_overridden);
    let models_owned = own.is_some_and(|settings| settings.member_models_overridden);
    let mut without_own_model = conversation.clone();
    if let Some(settings) = without_own_model.current_settings.as_mut() {
        settings.model_override = None;
        settings.model_provenance = SettingProvenance::LaunchInherited;
    }
    let participants = live_sources::effective_participants(conversation, profile)
        .into_iter()
        .filter(|participant| participant.role == ParticipantRole::Character)
        .map(|participant| {
            let character_id = match participant.source {
                ParticipantSource::Character(id) => Some(id),
                _ => None,
            };
            let model = match live_sources::member_model(&without_own_model, &participant, profile)
            {
                MemberModel::Snapshot(model) => choice(Some(model.source_id), owned(models_owned)),
                MemberModel::Profile(id) => choice(Some(id), dto::SettingSource::Group),
                MemberModel::Live => choice(
                    character_id
                        .map(|id| CharacterRepository::get(database, id))
                        .transpose()
                        .map_err(source_error)?
                        .flatten()
                        .and_then(|character| character.character.defaults.model_profile_id),
                    dto::SettingSource::Character,
                ),
            };
            Ok(dto::ParticipantSettings {
                participant_id: participant.id.to_string(),
                character_id: character_id.map(|id| id.to_string()),
                name: participant.display_name.clone(),
                enabled: participant.enabled,
                muted: participant.muted,
                model,
            })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    let mut participants = participants;
    for character_id in live_sources::missing_group_members(conversation, profile) {
        let Some(character) = CharacterRepository::get(database, character_id)
            .map_err(source_error)?
            .filter(|character| character.character.status == LifecycleStatus::Active)
        else {
            continue;
        };
        let member = profile.and_then(|profile| {
            profile
                .members
                .iter()
                .find(|member| member.character_id == character_id)
        });
        participants.push(dto::ParticipantSettings {
            participant_id: conversation::member_participant_id(conversation.id, character_id)
                .to_string(),
            character_id: Some(character_id.to_string()),
            name: policy::character_display_name(&character.character),
            enabled: true,
            muted: member.is_some_and(|member| member.muted),
            model: match member.and_then(|member| member.model_profile_override) {
                Some(id) => choice(Some(id), dto::SettingSource::Group),
                None => choice(
                    character.character.defaults.model_profile_id,
                    dto::SettingSource::Character,
                ),
            },
        });
    }
    Ok(dto::GroupMembersSettings {
        members_source: owned(members_owned),
        muted_source: owned(muted_owned),
        models_source: owned(models_owned),
        participants,
    })
}

pub(super) fn edit_error(error: ConversationEditError) -> ApiError {
    match error {
        ConversationEditError::InvalidInput { field } => {
            invalid_field(field, format!("{field} does not name a usable source"))
        }
        ConversationEditError::Conversation(error) => error.into_api_error(),
        ConversationEditError::Source | ConversationEditError::Snapshot => {
            api_error(ApiErrorCode::Internal, error.to_string())
        }
    }
}

fn choice_change<T: std::str::FromStr>(
    change: Option<dto::ChoiceChange>,
    field: &str,
) -> Result<Option<Choice<T>>, ApiError> {
    change
        .map(|change| {
            Ok(match change {
                dto::ChoiceChange::Set { id } => Choice::Set(parse_id(&id, field)?),
                dto::ChoiceChange::None => Choice::None,
                dto::ChoiceChange::Reset => Choice::Reset,
            })
        })
        .transpose()
}

fn id_change<T: std::str::FromStr>(
    change: Option<dto::IdChange>,
    field: &str,
) -> Result<Option<Change<T>>, ApiError> {
    change
        .map(|change| {
            Ok(match change {
                dto::IdChange::Set { id } => Change::Set(parse_id(&id, field)?),
                dto::IdChange::Reset => Change::Reset,
            })
        })
        .transpose()
}

fn settings_change(
    patch: dto::ConversationSettingsPatch,
) -> Result<conversation::ConversationSettingsChange, ApiError> {
    Ok(conversation::ConversationSettingsChange {
        persona: choice_change::<PersonaId>(patch.persona, "persona_id")?,
        prompt: choice_change::<PromptDocumentId>(patch.prompt, "prompt_id")?,
        roleplay_prompt: choice_change::<PromptDocumentId>(
            patch.roleplay_prompt,
            "roleplay_prompt_id",
        )?,
        lorebooks: patch
            .lorebooks
            .map(|change| {
                Ok::<_, ApiError>(match change {
                    dto::LorebooksChange::Set { ids } => Change::Set(
                        ids.iter()
                            .map(|id| parse_id::<LorebookId>(id, "lorebook_ids"))
                            .collect::<Result<Vec<_>, _>>()?,
                    ),
                    dto::LorebooksChange::Reset => Change::Reset,
                })
            })
            .transpose()?,
        model: id_change::<ModelProfileId>(patch.model, "model_profile_id")?,
        background: patch
            .background
            .map(|change| {
                Ok::<_, ApiError>(match change {
                    dto::BackgroundChange::Image { asset_id } => {
                        conversation::BackgroundChange::Image(parse_id::<AssetId>(
                            &asset_id,
                            "background_asset_id",
                        )?)
                    }
                    dto::BackgroundChange::Hidden => conversation::BackgroundChange::Hidden,
                    dto::BackgroundChange::Reset => conversation::BackgroundChange::Reset,
                })
            })
            .transpose()?,
        scene: id_change::<SceneId>(patch.scene, "scene_id")?,
        author_note: patch.author_note,
        speaker_selection: patch.speaker_selection.map(|change| match change {
            dto::SpeakerSelectionChange::Set { method } => Change::Set(speaker_snapshot(method)),
            dto::SpeakerSelectionChange::Reset => Change::Reset,
        }),
        memory: patch
            .memory
            .map(|change| match change {
                dto::MemoryModeChange::Set {
                    mode: dto::MemoryMode::Manual,
                } => Ok(Change::Set(MemoryModeSnapshot::Manual)),
                dto::MemoryModeChange::Set {
                    mode: dto::MemoryMode::Dynamic,
                } => Ok(Change::Set(MemoryModeSnapshot::Dynamic)),
                dto::MemoryModeChange::Set {
                    mode: dto::MemoryMode::Disabled,
                } => Err(invalid_field("memory", "memory can be manual or dynamic")),
                dto::MemoryModeChange::Reset => Ok(Change::Reset),
            })
            .transpose()?,
        chat_mode: patch.chat_mode.map(|change| match change {
            dto::ChatModeChange::Set {
                mode: dto::GroupChatMode::Conversation,
            } => Change::Set(GroupChatModeSnapshot::Conversation),
            dto::ChatModeChange::Set {
                mode: dto::GroupChatMode::Roleplay,
            } => Change::Set(GroupChatModeSnapshot::Roleplay),
            dto::ChatModeChange::Reset => Change::Reset,
        }),
        disable_character_lorebooks: patch
            .disable_character_lorebooks
            .map(|change| match change {
                dto::FlagChange::Set { value } => Change::Set(value),
                dto::FlagChange::Reset => Change::Reset,
            }),
        follow_group_members: patch.reset_members,
        follow_group_member_models: patch.reset_member_models,
    })
}

/// Changes a conversation's settings from ids and plain values, under the
/// settings revision the caller read (none when the chat has no settings of
/// its own yet); a stale revision is `Conflict`. Repeating the same change
/// against the same revision returns the first result.
pub async fn conversation_settings_update(
    context: &ApiContext,
    request: dto::ConversationSettingsUpdateRequest,
) -> Result<dto::ConversationSettingsView, ApiError> {
    let conversation_id: ConversationId = parse_id(&request.conversation_id, "conversation_id")?;
    let expected = request
        .expected_settings_revision
        .map(|revision| {
            if revision == 0 {
                Err(invalid_field(
                    "expected_settings_revision",
                    "a settings revision starts at one",
                ))
            } else {
                Ok(Revision::new(revision))
            }
        })
        .transpose()?;
    let digest = serde_json::to_vec(&request.patch)
        .map_err(|_| api_error(ApiErrorCode::Internal, "the patch could not be encoded"))?;
    let change = settings_change(request.patch)?;
    context
        .blocking(move |context| {
            let operation = conversation::edit_operation(
                format!("settings.{}", expected.map_or(0, Revision::get)),
                &[
                    b"lettuce-settings-v1",
                    conversation_id.to_string().as_bytes(),
                    &digest,
                ],
            )
            .map_err(edit_error)?;
            conversation::apply_settings_change(
                context.backend().database(),
                conversation_id,
                expected,
                operation,
                &change,
                context.now(),
            )
            .map_err(|error| match error {
                ConversationEditError::Conversation(
                    lettuce_conversations::ConversationRepositoryError::Invalid(
                        lettuce_conversations::ValidationError::InvalidReference {
                            field: "conversation_settings.create_only",
                        },
                    ),
                ) => api_error(
                    ApiErrorCode::Conflict,
                    "the conversation already has settings of its own",
                ),
                error => edit_error(error),
            })?;
            settings_view(context, conversation_id)
        })
        .await
}

/// Renames a conversation under the revision the caller read; the title is
/// trimmed and a blank one is refused.
pub async fn conversation_rename(
    context: &ApiContext,
    request: dto::ConversationRenameRequest,
) -> Result<dto::ConversationRevisions, ApiError> {
    let conversation_id: ConversationId = parse_id(&request.conversation_id, "conversation_id")?;
    let title = request.title.trim().to_owned();
    if title.is_empty() {
        return Err(invalid_field("title", "the title is blank"));
    }
    if request.expected_revision == 0 {
        return Err(invalid_field(
            "expected_revision",
            "a revision starts at one",
        ));
    }
    let expected_revision = Revision::new(request.expected_revision);
    context
        .blocking(move |context| {
            let database = context.backend().database();
            let operation = conversation::edit_operation(
                format!("rename.{}", expected_revision.get()),
                &[
                    b"lettuce-rename-v1",
                    conversation_id.to_string().as_bytes(),
                    title.as_bytes(),
                ],
            )
            .map_err(edit_error)?;
            let renamed = ConversationRepository::rename(
                database,
                &RenameConversation {
                    conversation_id,
                    expected_revision,
                    operation,
                    title,
                },
                context.now(),
            )
            .map_err(|error| match error {
                lettuce_conversations::ConversationRepositoryError::Invalid(
                    lettuce_conversations::ValidationError::TooLarge { .. },
                ) => invalid_field("title", "the title is too long"),
                error => error.into_api_error(),
            })?;
            Ok(revisions(&renamed.value))
        })
        .await
}

/// Hides a conversation from the default lists; archiving an archived one
/// changes nothing.
pub async fn conversation_archive(
    context: &ApiContext,
    request: dto::ConversationRequest,
) -> Result<dto::ConversationRevisions, ApiError> {
    change_lifecycle(context, request, ConversationLifecycle::Archived).await
}

/// Shows an archived conversation in the default lists again; restoring an
/// active one changes nothing.
pub async fn conversation_restore(
    context: &ApiContext,
    request: dto::ConversationRequest,
) -> Result<dto::ConversationRevisions, ApiError> {
    change_lifecycle(context, request, ConversationLifecycle::Active).await
}

async fn change_lifecycle(
    context: &ApiContext,
    request: dto::ConversationRequest,
    target: ConversationLifecycle,
) -> Result<dto::ConversationRevisions, ApiError> {
    let conversation_id: ConversationId = parse_id(&request.conversation_id, "conversation_id")?;
    context
        .blocking(move |context| {
            let database = context.backend().database();
            let mut attempt = 0;
            loop {
                attempt += 1;
                let conversation = ConversationReader::get(database, conversation_id)
                    .map_err(IntoApiError::into_api_error)?
                    .conversation;
                if conversation.lifecycle == target {
                    return Ok(revisions(&conversation));
                }
                let (name, archive) = match target {
                    ConversationLifecycle::Archived => ("archive", true),
                    _ => ("restore", false),
                };
                let operation = conversation::edit_operation(
                    format!("{name}.{}", conversation.revision.get()),
                    &[name.as_bytes(), conversation_id.to_string().as_bytes()],
                )
                .map_err(edit_error)?;
                let result = if archive {
                    ConversationRepository::archive(
                        database,
                        &ArchiveConversation {
                            conversation_id,
                            expected_revision: conversation.revision,
                            operation,
                        },
                        context.now(),
                    )
                    .map(|done| done.value)
                } else {
                    ConversationRepository::restore(
                        database,
                        &RestoreConversation {
                            conversation_id,
                            expected_revision: conversation.revision,
                            operation,
                        },
                        context.now(),
                    )
                    .map(|done| done.value)
                };
                match result {
                    Ok(_) => {
                        let conversation = ConversationReader::get(database, conversation_id)
                            .map_err(IntoApiError::into_api_error)?
                            .conversation;
                        return Ok(revisions(&conversation));
                    }
                    Err(lettuce_conversations::ConversationRepositoryError::StaleRevision {
                        ..
                    }) if attempt < 8 => {}
                    Err(error) => return Err(error.into_api_error()),
                }
            }
        })
        .await
}

/// Adds a character to a group chat, or enables the member it kept; the chat
/// owns its member list from then on. Repeating the call with the same key
/// returns the first result.
pub async fn conversation_participant_add(
    context: &ApiContext,
    request: dto::ConversationParticipantAddRequest,
) -> Result<dto::ConversationRevisions, ApiError> {
    let conversation_id: ConversationId = parse_id(&request.conversation_id, "conversation_id")?;
    let character_id: CharacterId = parse_id(&request.character_id, "character_id")?;
    let operation = conversation::edit_operation(
        request.client_operation_id,
        &[
            b"lettuce-participant-add-v1",
            conversation_id.to_string().as_bytes(),
            character_id.to_string().as_bytes(),
        ],
    )
    .map_err(edit_error)?;
    context
        .blocking(move |context| {
            let database = context.backend().database();
            conversation::ensure_group_members(database, conversation_id, context.now())
                .map_err(edit_error)?;
            let added = conversation::add_group_member(
                database,
                conversation_id,
                character_id,
                operation,
                true,
                context.now(),
            )
            .map_err(edit_error)?;
            Ok(revisions(&added))
        })
        .await
}

/// Enables, disables, mutes or unmutes a group member, or sets or resets its
/// model. The chat owns what the change touches from then on. At least one
/// enabled, unmuted member must remain; the change that would leave none is
/// `InvalidInput` naming `conversation.group.active_member`. Repeating the
/// call with the same key and request returns the first result; another
/// request under the key is `Conflict`.
pub async fn conversation_participant_update(
    context: &ApiContext,
    request: dto::ConversationParticipantUpdateRequest,
) -> Result<dto::ConversationRevisions, ApiError> {
    let conversation_id: ConversationId = parse_id(&request.conversation_id, "conversation_id")?;
    let participant_id: ConversationParticipantId =
        parse_id(&request.participant_id, "participant_id")?;
    let key = request.client_operation_id.clone();
    let digest = serde_json::to_vec(&(
        &request.conversation_id,
        &request.participant_id,
        request.enabled,
        request.muted,
        &request.model,
    ))
    .map_err(|_| api_error(ApiErrorCode::Internal, "the change could not be encoded"))?;
    let operation = conversation::edit_operation(key, &[b"lettuce-participant-update-v1", &digest])
        .map_err(edit_error)?;
    let change = conversation::ParticipantChange {
        enabled: request.enabled,
        muted: request.muted,
        model: id_change::<ModelProfileId>(request.model, "model_profile_id")?,
    };
    if change == conversation::ParticipantChange::default() {
        return Err(invalid_field("participant_id", "the change is empty"));
    }
    context
        .blocking(move |context| {
            let database = context.backend().database();
            let updated = conversation::update_group_participant(
                database,
                conversation_id,
                participant_id,
                &change,
                operation,
                context.now(),
            )
            .map_err(edit_error)?;
            Ok(revisions(&updated))
        })
        .await
}
