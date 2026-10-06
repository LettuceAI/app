use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_database::ApiOperationError;
use lettuce_types::ConversationId;

use super::ApiContext;
use super::error::{IntoApiError, api_error, parse_id};
use super::messages::operation;

struct CopyFailure(ApiError);

impl From<ApiOperationError> for CopyFailure {
    fn from(error: ApiOperationError) -> Self {
        Self(api_error(
            match error {
                ApiOperationError::Conflict => ApiErrorCode::Conflict,
                ApiOperationError::InvalidData | ApiOperationError::Storage => {
                    ApiErrorCode::Internal
                }
            },
            error.to_string(),
        ))
    }
}

pub async fn conversation_duplicate(
    context: &ApiContext,
    request: dto::ConversationDuplicateRequest,
) -> Result<dto::ConversationCopyResult, ApiError> {
    let source_conversation_id = parse_id(&request.conversation_id, "conversation_id")?;
    let request_bytes = zeroize::Zeroizing::new(serde_json::to_vec(&request).map_err(|_| {
        api_error(
            ApiErrorCode::Internal,
            "the duplicate request could not be encoded",
        )
    })?);
    let operation = operation(
        request.client_operation_id.clone(),
        &[b"duplicate", &request_bytes],
    )?;
    context
        .blocking(move |context| {
            let now = context.now();
            context
                .backend()
                .database()
                .commit_api_operation(
                    "conversation_duplicate",
                    &request.client_operation_id,
                    operation.request_digest.as_str(),
                    now,
                    |scope| {
                        let commit = scope
                            .duplicate_conversation(
                                &lettuce_conversations::DuplicateConversation {
                                    source_conversation_id,
                                    conversation_id: ConversationId::new(),
                                    title: request.title,
                                    with_messages: request.with_messages,
                                    operation: operation.clone(),
                                },
                                now,
                            )
                            .map_err(|error| CopyFailure(error.into_api_error()))?;
                        Ok(dto::ConversationCopyResult {
                            conversation_id: commit.value.conversation.id.to_string(),
                            revision: commit.value.conversation.revision.get(),
                        })
                    },
                )
                .map_err(|failure: CopyFailure| failure.0)
        })
        .await
}

fn copy_persona(
    source: &lettuce_conversations::Conversation,
) -> crate::LaunchSelection<lettuce_types::PersonaId> {
    use crate::LaunchSelection;
    use lettuce_conversations::{ConversationKind, SettingProvenance, SnapshotSelection};
    if let Some(settings) = &source.current_settings {
        match settings.persona_provenance {
            SettingProvenance::Disabled => return LaunchSelection::Disabled,
            SettingProvenance::CurrentOverride => {
                return settings
                    .persona
                    .as_ref()
                    .map_or(LaunchSelection::Disabled, |persona| {
                        LaunchSelection::Explicit(persona.source_id)
                    });
            }
            SettingProvenance::LaunchInherited => {}
        }
    }
    let persona = match &source.kind {
        ConversationKind::Direct(details) => &details.persona,
        ConversationKind::Group(details) => &details.group.persona,
    };
    match persona {
        SnapshotSelection::Disabled => LaunchSelection::Disabled,
        SnapshotSelection::Inherited(value) | SnapshotSelection::Explicit(value) => {
            LaunchSelection::Explicit(value.source_id)
        }
    }
}

fn copy_receipt(
    context: &ApiContext,
    command: &str,
    key: &str,
    digest: &str,
) -> Result<Option<dto::ConversationCopyResult>, ApiError> {
    let Some(receipt) = context
        .backend()
        .database()
        .lookup_api_operation(command, key)
        .map_err(|error| CopyFailure::from(error).0)?
    else {
        return Ok(None);
    };
    if receipt.request_digest != digest {
        return Err(api_error(
            ApiErrorCode::Conflict,
            "the operation key was used for another request",
        ));
    }
    serde_json::from_value(receipt.result)
        .map(Some)
        .map_err(|_| api_error(ApiErrorCode::Internal, "the copy receipt is invalid"))
}

pub async fn conversation_branch_direct_to_character(
    context: &ApiContext,
    request: dto::ConversationCharacterCopyRequest,
) -> Result<dto::ConversationCopyResult, ApiError> {
    copy_character(context, request, false, false).await
}

pub async fn conversation_branch_to_character_from_message(
    context: &ApiContext,
    request: dto::ConversationCharacterCopyRequest,
) -> Result<dto::ConversationCopyResult, ApiError> {
    copy_character(context, request, true, false).await
}

async fn copy_character(
    context: &ApiContext,
    request: dto::ConversationCharacterCopyRequest,
    group_source: bool,
    whole: bool,
) -> Result<dto::ConversationCopyResult, ApiError> {
    use lettuce_characters::CharacterRepository;
    use lettuce_conversations::{
        ConversationKind, ConversationReader, ParticipantRole, SelectedConversationCopyKind,
    };
    let conversation_id = parse_id(&request.conversation_id, "conversation_id")?;
    let message_id = if whole {
        None
    } else {
        Some(parse_id(&request.message_id, "message_id")?)
    };
    let character_id = parse_id(&request.character_id, "character_id")?;
    let bytes = zeroize::Zeroizing::new(serde_json::to_vec(&request).map_err(|_| {
        api_error(
            ApiErrorCode::Internal,
            "the copy request could not be encoded",
        )
    })?);
    let command = if whole {
        "conversation_branch_to_character"
    } else if group_source {
        "conversation_branch_to_character_from_message"
    } else {
        "conversation_branch_direct_to_character"
    };
    let operation = operation(
        request.client_operation_id.clone(),
        &[command.as_bytes(), &bytes],
    )?;
    context
        .blocking(move |context| {
            if let Some(result) = copy_receipt(
                context,
                command,
                &request.client_operation_id,
                operation.request_digest.as_str(),
            )? {
                return Ok(result);
            }
            let database = context.backend().database();
            let source = ConversationReader::get(database, conversation_id)
                .map_err(IntoApiError::into_api_error)?;
            if group_source != matches!(source.conversation.kind, ConversationKind::Group(_)) {
                return Err(api_error(
                    ApiErrorCode::InvalidInput,
                    "the source conversation kind is invalid",
                ));
            }
            if let Some(message_id) = message_id {
                super::messages::on_timeline(
                    context,
                    conversation_id,
                    source.conversation.active_branch_id,
                    message_id,
                )?;
            }
            let group_details = match &source.conversation.kind {
                ConversationKind::Group(details) => Some(details),
                ConversationKind::Direct(_) => None,
            };
            if whole {
                let members_overridden = source
                    .conversation
                    .current_settings
                    .as_ref()
                    .is_some_and(|settings| settings.members_overridden);
                let member = if members_overridden {
                    source.conversation.participants.iter().any(|participant| {
                        participant.enabled
                            && participant.source
                                == lettuce_conversations::ParticipantSource::Character(character_id)
                    })
                } else {
                    group_details.is_some_and(|details| {
                        details
                            .group
                            .members
                            .iter()
                            .any(|member| member.character.source_id == character_id)
                    })
                };
                if !member {
                    return Err(api_error(
                        ApiErrorCode::InvalidInput,
                        "the target character is not a group member",
                    ));
                }
            }
            let character = CharacterRepository::get(database, character_id)
                .map_err(IntoApiError::into_api_error)?
                .ok_or_else(|| {
                    api_error(ApiErrorCode::NotFound, "the target character was not found")
                })?;
            let user = source
                .conversation
                .participants
                .iter()
                .find(|participant| participant.role == ParticipantRole::User)
                .ok_or_else(|| api_error(ApiErrorCode::Internal, "the source user is missing"))?;
            let title = if group_source {
                format!(
                    "{} - {}",
                    source.conversation.title, character.character.profile.name
                )
            } else {
                format!("Branch to {}", character.character.profile.name)
            };
            let key = lettuce_conversations::IdempotencyKey::new(format!(
                "{command}.{}",
                request.client_operation_id
            ))
            .map_err(|_| api_error(ApiErrorCode::InvalidInput, "the operation key is invalid"))?;
            let persona = copy_persona(&source.conversation);
            let launch_request = crate::DirectConversationLaunchRequest {
                format_version: crate::DIRECT_LAUNCH_REQUEST_FORMAT_V1,
                title,
                user: crate::DirectUserParticipant {
                    display_name: user.display_name.clone(),
                    authored_description: user.authored_description.clone(),
                },
                character_id,
                scene: crate::LaunchSelection::Inherit,
                starter: crate::LaunchSelection::Disabled,
                persona,
                operation_key: key,
            };
            let (launch, companion) = crate::ConversationLaunchPlanner::new(database)
                .prepare_direct_parts(&launch_request)
                .map_err(IntoApiError::into_api_error)?;
            let (mut plan, mut drafts) = launch.into_parts();
            plan.initial_timeline.entries.clear();
            plan.operation = operation.clone();
            if whole {
                let source_scene = match source
                    .conversation
                    .current_settings
                    .as_ref()
                    .map(|settings| (settings.scene_provenance, &settings.scene))
                {
                    Some((lettuce_conversations::SettingProvenance::Disabled, _)) => {
                        lettuce_conversations::SnapshotSelection::Disabled
                    }
                    Some((
                        lettuce_conversations::SettingProvenance::CurrentOverride,
                        Some(scene),
                    )) => {
                        drafts.push(database.copy_snapshot_draft(&scene.snapshot_ref).map_err(
                            |_| {
                                api_error(
                                    ApiErrorCode::Internal,
                                    "the source scene snapshot is unavailable",
                                )
                            },
                        )?);
                        lettuce_conversations::SnapshotSelection::Explicit(scene.clone())
                    }
                    Some((lettuce_conversations::SettingProvenance::CurrentOverride, None)) => {
                        lettuce_conversations::SnapshotSelection::Disabled
                    }
                    _ => match group_details.map(|details| &details.group.scene) {
                        Some(
                            lettuce_conversations::SnapshotSelection::Inherited(scene)
                            | lettuce_conversations::SnapshotSelection::Explicit(scene),
                        ) => {
                            drafts.push(
                                database.copy_snapshot_draft(&scene.snapshot_ref).map_err(
                                    |_| {
                                        api_error(
                                            ApiErrorCode::Internal,
                                            "the source scene snapshot is unavailable",
                                        )
                                    },
                                )?,
                            );
                            lettuce_conversations::SnapshotSelection::Explicit(scene.clone())
                        }
                        _ => lettuce_conversations::SnapshotSelection::Disabled,
                    },
                };
                if let ConversationKind::Direct(details) = &mut plan.kind {
                    details.scene = source_scene;
                }
                plan.current_settings
                    .get_or_insert_with(|| {
                        lettuce_conversations::CurrentConversationSettings::inherited(
                            lettuce_types::Revision::INITIAL,
                        )
                    })
                    .background = Some(
                    character
                        .character
                        .media
                        .links
                        .iter()
                        .find(|link| {
                            link.slot == lettuce_characters::CharacterMediaSlot::Background
                        })
                        .map_or(
                            lettuce_conversations::ConversationBackground::Hidden,
                            |link| lettuce_conversations::ConversationBackground::Image {
                                asset_id: link.asset_id,
                            },
                        ),
                );
                let required =
                    lettuce_conversations::conversation_launch_snapshot_references(&plan)
                        .into_iter()
                        .map(|reference| reference.artifact_id)
                        .collect::<std::collections::BTreeSet<_>>();
                drafts.retain(|draft| required.contains(&draft.artifact_id));
            }

            if !group_source {
                plan.current_settings
                    .get_or_insert_with(|| {
                        lettuce_conversations::CurrentConversationSettings::inherited(
                            lettuce_types::Revision::INITIAL,
                        )
                    })
                    .background = Some(lettuce_conversations::ConversationBackground::Hidden);
            }
            let launch = lettuce_conversations::PreparedConversationLaunch::new(plan, drafts)
                .map_err(|_| {
                    api_error(
                        ApiErrorCode::InvalidInput,
                        "the prepared copy launch is invalid",
                    )
                })?;
            let kind =
                if group_source {
                    let mut names = Vec::new();
                    let overridden = source
                        .conversation
                        .current_settings
                        .as_ref()
                        .is_some_and(|settings| settings.members_overridden);
                    let mut ids = if overridden {
                        source
                            .conversation
                            .participants
                            .iter()
                            .filter(|participant| participant.enabled)
                            .filter_map(|participant| match participant.source {
                                lettuce_conversations::ParticipantSource::Character(id) => Some(id),
                                _ => None,
                            })
                            .collect::<Vec<_>>()
                    } else {
                        group_details
                            .map(|details| {
                                details
                                    .group
                                    .members
                                    .iter()
                                    .map(|member| member.character.source_id)
                                    .collect()
                            })
                            .unwrap_or_default()
                    };
                    if !whole {
                        ids.extend(source.conversation.participants.iter().filter_map(
                            |participant| match participant.source {
                                lettuce_conversations::ParticipantSource::Character(id) => Some(id),
                                _ => None,
                            },
                        ));
                    }
                    ids.sort();
                    ids.dedup();
                    for id in ids {
                        let member = CharacterRepository::get(database, id)
                            .map_err(IntoApiError::into_api_error)?
                            .ok_or_else(|| {
                                api_error(
                                    ApiErrorCode::NotFound,
                                    "a source speaker character was not found",
                                )
                            })?;
                        let name = member.character.profile.name;
                        if !names.contains(&name) {
                            names.push(name);
                        }
                    }
                    if whole {
                        SelectedConversationCopyKind::GroupToCharacter {
                            placeholder_names: names,
                        }
                    } else {
                        SelectedConversationCopyKind::GroupToCharacterFromMessage {
                            placeholder_names: names,
                        }
                    }
                } else {
                    SelectedConversationCopyKind::DirectToCharacter
                };
            let now = context.now();
            database
                .commit_api_operation(
                    command,
                    &request.client_operation_id,
                    operation.request_digest.as_str(),
                    now,
                    |scope| {
                        let commit = scope
                            .create_conversation_copy(
                                lettuce_database::ConversationCopyLaunch {
                                    launch,
                                    source_conversation_id: conversation_id,
                                    source_revision: source.conversation.revision,
                                    source_branch_id: source.conversation.active_branch_id,
                                    source_group_revision: None,
                                    source_character_revision: None,
                                    through_message_id: message_id,
                                    kind,
                                    companion,
                                    initialize_companion: group_source,
                                    new_group: None,
                                },
                                now,
                            )
                            .map_err(|error| CopyFailure(error.into_api_error()))?;
                        Ok(dto::ConversationCopyResult {
                            conversation_id: commit.value.conversation.id.to_string(),
                            revision: commit.value.conversation.revision.get(),
                        })
                    },
                )
                .map_err(|failure: CopyFailure| failure.0)
        })
        .await
}

pub async fn conversation_branch_to_character(
    context: &ApiContext,
    request: dto::ConversationGroupCharacterCopyRequest,
) -> Result<dto::ConversationCopyResult, ApiError> {
    copy_character(
        context,
        dto::ConversationCharacterCopyRequest {
            conversation_id: request.conversation_id,
            message_id: String::new(),
            character_id: request.character_id,
            client_operation_id: request.client_operation_id,
        },
        true,
        true,
    )
    .await
}

pub async fn conversation_branch_direct_to_group(
    context: &ApiContext,
    request: dto::ConversationGroupCopyRequest,
) -> Result<dto::ConversationCopyResult, ApiError> {
    use lettuce_characters::{
        CharacterRepository, GroupDetails, GroupMember, GroupProfile, GroupStartingScene, Scene,
        SceneDocumentV1, SceneOwner, ScenePart,
    };
    use lettuce_conversations::{
        ConversationKind, ConversationReader, ParticipantRole, SelectedConversationCopyKind,
        SettingProvenance, SnapshotSelection,
    };
    let conversation_id = parse_id(&request.conversation_id, "conversation_id")?;
    let message_id = parse_id(&request.message_id, "message_id")?;
    let character_ids = request
        .character_ids
        .iter()
        .map(|id| parse_id(id, "character_ids"))
        .collect::<Result<Vec<lettuce_types::CharacterId>, _>>()?;
    if character_ids.len() < 2
        || character_ids
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len()
            != character_ids.len()
    {
        return Err(api_error(
            ApiErrorCode::InvalidInput,
            "at least two distinct group characters are required",
        ));
    }
    let command = "conversation_branch_direct_to_group";
    let bytes = zeroize::Zeroizing::new(serde_json::to_vec(&request).map_err(|_| {
        api_error(
            ApiErrorCode::Internal,
            "the group copy request could not be encoded",
        )
    })?);
    let operation = operation(
        request.client_operation_id.clone(),
        &[command.as_bytes(), &bytes],
    )?;
    context
        .blocking(move |context| {
            if let Some(result) = copy_receipt(
                context,
                command,
                &request.client_operation_id,
                operation.request_digest.as_str(),
            )? {
                return Ok(result);
            }
            let database = context.backend().database();
            let source = ConversationReader::get(database, conversation_id)
                .map_err(IntoApiError::into_api_error)?;
            let ConversationKind::Direct(details) = &source.conversation.kind else {
                return Err(api_error(
                    ApiErrorCode::InvalidInput,
                    "a direct source conversation is required",
                ));
            };
            let owner = details.character.source_id;
            if !character_ids.contains(&owner) {
                return Err(api_error(
                    ApiErrorCode::InvalidInput,
                    "the source owner must be a group member",
                ));
            }
            super::messages::on_timeline(
                context,
                conversation_id,
                source.conversation.active_branch_id,
                message_id,
            )?;
            let character = CharacterRepository::get(database, owner)
                .map_err(IntoApiError::into_api_error)?
                .ok_or_else(|| {
                    api_error(ApiErrorCode::NotFound, "the source owner was not found")
                })?;
            let user = source
                .conversation
                .participants
                .iter()
                .find(|participant| participant.role == ParticipantRole::User)
                .ok_or_else(|| api_error(ApiErrorCode::Internal, "the source user is missing"))?;
            let now = context.now();
            let group_id = lettuce_types::GroupId::new();
            let members = character_ids
                .iter()
                .enumerate()
                .map(|(ordinal, id)| {
                    Ok(GroupMember {
                        character_id: *id,
                        ordinal: u32::try_from(ordinal).map_err(|_| {
                            api_error(
                                ApiErrorCode::InvalidInput,
                                "group membership ordering exceeds its representation",
                            )
                        })?,
                        muted: false,
                        model_profile_override: None,
                    })
                })
                .collect::<Result<Vec<_>, ApiError>>()?;
            let mut group = GroupProfile::new(
                group_id,
                format!("{} Branch", character.character.profile.name),
                members,
                now,
            )
            .map_err(|error| api_error(ApiErrorCode::InvalidInput, error.to_string()))?;
            group.chat_mode = lettuce_characters::ChatMode::Roleplay;
            let persona = copy_persona(&source.conversation);
            group.persona = match persona {
                crate::LaunchSelection::Disabled => lettuce_characters::Selection::Disabled,
                crate::LaunchSelection::Explicit(id) => lettuce_characters::Selection::Explicit(id),
                crate::LaunchSelection::Inherit => lettuce_characters::Selection::Inherit,
            };
            group.memory_policy = match source
                .conversation
                .current_settings
                .as_ref()
                .map(|settings| (settings.memory_provenance, &settings.memory))
            {
                Some((SettingProvenance::CurrentOverride, Some(memory)))
                    if memory.mode == lettuce_conversations::MemoryModeSnapshot::Dynamic =>
                {
                    lettuce_characters::MemoryPolicy::Dynamic
                }
                Some((SettingProvenance::CurrentOverride, _))
                | Some((SettingProvenance::Disabled, _)) => {
                    lettuce_characters::MemoryPolicy::Manual
                }
                _ => character.character.defaults.memory_policy,
            };
            let selected_scene_id = source
                .conversation
                .current_settings
                .as_ref()
                .and_then(|settings| settings.scene.as_ref())
                .map(|scene| scene.source_id)
                .or(match &details.scene {
                    SnapshotSelection::Explicit(scene) | SnapshotSelection::Inherited(scene) => {
                        Some(scene.source_id)
                    }
                    SnapshotSelection::Disabled => None,
                })
                .or(character.character.defaults.default_scene_id);
            let selected_scene = selected_scene_id
                .map(|id| {
                    character
                        .scenes
                        .iter()
                        .find(|scene| scene.id == id)
                        .ok_or_else(|| {
                            api_error(
                                ApiErrorCode::NotFound,
                                "the owner's selected scene was not found",
                            )
                        })
                })
                .transpose()?;
            let scene_text = selected_scene
                .and_then(|scene| {
                    crate::launch::policy::resolve_scene_text(scene, &character.variants)
                })
                .map(|text| text.trim().to_owned())
                .filter(|text| !text.is_empty());
            let starting_scene = scene_text
                .map(|text| {
                    Scene::new(
                        lettuce_types::SceneId::new(),
                        SceneOwner::Group(group_id),
                        0,
                        SceneDocumentV1 {
                            format_version: 1,
                            parts: vec![ScenePart::Text { text }],
                        },
                        now,
                    )
                    .map(|mut scene| {
                        scene.direction =
                            selected_scene.and_then(|source| source.direction.clone());
                        GroupStartingScene {
                            scene,
                            variants: Vec::new(),
                        }
                    })
                    .map_err(|error| api_error(ApiErrorCode::InvalidInput, error.to_string()))
                })
                .transpose()?;
            group.starting_scene_id = starting_scene.as_ref().map(|scene| scene.scene.id);
            let session_background = source
                .conversation
                .current_settings
                .as_ref()
                .and_then(|settings| settings.background.as_ref())
                .and_then(|background| match background {
                    lettuce_conversations::ConversationBackground::Image { asset_id } => {
                        Some(*asset_id)
                    }
                    lettuce_conversations::ConversationBackground::Hidden => None,
                });
            group.background_asset_id = session_background
                .or_else(|| {
                    selected_scene.and_then(|scene| {
                        scene
                            .assets
                            .iter()
                            .find(|asset| {
                                asset.slot == lettuce_characters::SceneAssetSlot::Background
                            })
                            .map(|asset| asset.asset_id)
                    })
                })
                .or_else(|| {
                    character
                        .character
                        .media
                        .links
                        .iter()
                        .find(|link| {
                            link.slot == lettuce_characters::CharacterMediaSlot::Background
                        })
                        .map(|link| link.asset_id)
                });
            let group_plan = lettuce_characters::CreateGroupPlan {
                group: group.clone(),
                starting_scene: starting_scene.clone(),
            };
            let key = lettuce_conversations::IdempotencyKey::new(format!(
                "{command}.{}",
                request.client_operation_id
            ))
            .map_err(|_| api_error(ApiErrorCode::InvalidInput, "the operation key is invalid"))?;
            let launch_request = crate::GroupConversationLaunchRequest {
                format_version: crate::GROUP_LAUNCH_REQUEST_FORMAT_V1,
                title: group.name.clone(),
                user: crate::DirectUserParticipant {
                    display_name: user.display_name.clone(),
                    authored_description: user.authored_description.clone(),
                },
                group_id,
                persona,
                operation_key: key,
            };
            let launch = crate::ConversationLaunchPlanner::new(database)
                .prepare_new_group(
                    &launch_request,
                    GroupDetails {
                        group,
                        starting_scene,
                    },
                    now,
                )
                .map_err(IntoApiError::into_api_error)?;
            let (mut plan, drafts) = launch.into_parts();
            plan.operation = operation.clone();
            let launch = lettuce_conversations::PreparedConversationLaunch::new(plan, drafts)
                .map_err(|_| {
                    api_error(ApiErrorCode::InvalidInput, "the group launch is invalid")
                })?;
            database
                .commit_api_operation(
                    command,
                    &request.client_operation_id,
                    operation.request_digest.as_str(),
                    now,
                    |scope| {
                        let commit = scope
                            .create_conversation_copy(
                                lettuce_database::ConversationCopyLaunch {
                                    launch,
                                    source_conversation_id: conversation_id,
                                    source_revision: source.conversation.revision,
                                    source_branch_id: source.conversation.active_branch_id,
                                    source_group_revision: None,
                                    source_character_revision: Some((
                                        owner,
                                        character.character.revision,
                                    )),
                                    through_message_id: Some(message_id),
                                    kind: SelectedConversationCopyKind::DirectToGroup,
                                    companion: None,
                                    initialize_companion: false,
                                    new_group: Some(group_plan),
                                },
                                now,
                            )
                            .map_err(|error| CopyFailure(error.into_api_error()))?;
                        Ok(dto::ConversationCopyResult {
                            conversation_id: commit.value.conversation.id.to_string(),
                            revision: commit.value.conversation.revision.get(),
                        })
                    },
                )
                .map_err(|failure: CopyFailure| failure.0)
        })
        .await
}
