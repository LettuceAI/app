use lettuce_types::ConversationParticipantId;
use serde::{Deserialize, Serialize};

use crate::snapshot::{
    GroupChatModeSnapshot, LorebookLaunchSnapshot, MemorySettingsSnapshot, ModelSelectionSnapshot,
    PersonaLaunchSnapshot, PromptLaunchSnapshot, SceneLaunchSnapshot, VoiceSettingsSnapshot,
};
use crate::{
    Conversation, ConversationKind, ParticipantRole, ParticipantSource, SettingProvenance,
    SnapshotSelection, ValidationError,
};

/// The settings a generation flow should use for one conversation turn.
/// Launch values are copied from the immutable conversation snapshot and are
/// replaced only by a valid conversation-local override.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectiveConversationSettings {
    pub author_note: Option<String>,
    pub memory: Option<MemorySettingsSnapshot>,
    pub model: Option<ModelSelectionSnapshot>,
    pub voice: Option<VoiceSettingsSnapshot>,
    pub prompt: Option<PromptLaunchSnapshot>,
    pub lorebooks: Vec<LorebookLaunchSnapshot>,
    pub persona: Option<PersonaLaunchSnapshot>,
    pub scene: Option<SceneLaunchSnapshot>,
}

pub fn resolve_effective_settings(
    conversation: &Conversation,
    selected_speaker: Option<ConversationParticipantId>,
) -> Result<EffectiveConversationSettings, ValidationError> {
    conversation.validate()?;

    match (&conversation.kind, selected_speaker) {
        (ConversationKind::Direct(_), Some(_)) => Err(ValidationError::InvalidReference {
            field: "effective_settings.selected_speaker",
        }),
        (ConversationKind::Direct(details), None) => Ok(EffectiveConversationSettings {
            author_note: current_or_launch(
                conversation,
                |settings| settings.author_note_provenance,
                |settings| settings.author_note.clone(),
                None,
            ),
            memory: current_or_selection(
                conversation,
                |settings| settings.memory_provenance,
                |settings| settings.memory.clone(),
                &details.memory,
            ),
            model: current_or_selection(
                conversation,
                |settings| settings.model_provenance,
                |settings| settings.model_override.clone(),
                &details.model,
            ),
            voice: current_or_selection(
                conversation,
                |settings| settings.voice_provenance,
                |settings| settings.voice.clone(),
                &details.voice,
            ),
            prompt: current_or_selection(
                conversation,
                |settings| settings.prompt_provenance,
                |settings| settings.prompt.clone(),
                &details.prompt,
            ),
            lorebooks: current_or_lorebooks(conversation, &details.lorebooks),
            persona: current_or_selection(
                conversation,
                |settings| settings.persona_provenance,
                |settings| settings.persona.clone(),
                &details.persona,
            ),
            scene: current_or_selection(
                conversation,
                |settings| settings.scene_provenance,
                |settings| settings.scene.clone(),
                &details.scene,
            ),
        }),
        (ConversationKind::Group(details), selected_speaker) => {
            let selected_speaker = selected_speaker.ok_or(ValidationError::InvalidReference {
                field: "effective_settings.selected_speaker",
            })?;
            let participant = conversation
                .participants
                .iter()
                .find(|participant| participant.id == selected_speaker)
                .filter(|participant| participant.role == ParticipantRole::Character);
            let participant = participant.ok_or(ValidationError::InvalidReference {
                field: "effective_settings.selected_speaker",
            })?;
            let character_id = match participant.source {
                ParticipantSource::Character(id) => id,
                _ => {
                    return Err(ValidationError::InvalidReference {
                        field: "effective_settings.selected_speaker",
                    });
                }
            };
            let member = details
                .group
                .members
                .iter()
                .find(|member| member.character.source_id == character_id)
                .ok_or(ValidationError::InvalidReference {
                    field: "effective_settings.selected_speaker",
                })?;

            let launch_prompt = selection_or_fallback(&member.prompt, &details.group.prompt);
            let launch_model = match &participant.model_selection {
                SnapshotSelection::Inherited(model) | SnapshotSelection::Explicit(model) => {
                    Some(model.clone())
                }
                SnapshotSelection::Disabled => selection_value(&details.group.model),
            };
            let launch_lorebooks = if details.group.disable_character_lorebook {
                selection_value(&details.group.lorebooks).unwrap_or_default()
            } else {
                merge_lorebooks(
                    selection_value(&details.group.lorebooks).unwrap_or_default(),
                    selection_value(&member.lorebooks).unwrap_or_default(),
                )
            };
            let launch_scene = match details.group.chat_mode {
                GroupChatModeSnapshot::Conversation => None,
                GroupChatModeSnapshot::Roleplay => selection_value(&details.group.scene),
            };

            Ok(EffectiveConversationSettings {
                author_note: current_or_launch(
                    conversation,
                    |settings| settings.author_note_provenance,
                    |settings| settings.author_note.clone(),
                    None,
                ),
                memory: current_or_launch(
                    conversation,
                    |settings| settings.memory_provenance,
                    |settings| settings.memory.clone(),
                    selection_value(&details.group.memory),
                ),
                model: current_or_launch(
                    conversation,
                    |settings| settings.model_provenance,
                    |settings| settings.model_override.clone(),
                    launch_model,
                ),
                voice: current_or_launch(
                    conversation,
                    |settings| settings.voice_provenance,
                    |settings| settings.voice.clone(),
                    None,
                ),
                prompt: current_or_launch(
                    conversation,
                    |settings| settings.prompt_provenance,
                    |settings| settings.prompt.clone(),
                    launch_prompt,
                ),
                lorebooks: current_or_lorebooks_value(conversation, launch_lorebooks),
                persona: current_or_launch(
                    conversation,
                    |settings| settings.persona_provenance,
                    |settings| settings.persona.clone(),
                    selection_value(&details.group.persona),
                ),
                scene: current_or_launch(
                    conversation,
                    |settings| settings.scene_provenance,
                    |settings| settings.scene.clone(),
                    launch_scene,
                ),
            })
        }
    }
}

fn current_or_selection<T: Clone>(
    conversation: &Conversation,
    provenance: impl Fn(&crate::CurrentConversationSettings) -> SettingProvenance,
    current: impl Fn(&crate::CurrentConversationSettings) -> Option<T>,
    launch: &SnapshotSelection<T>,
) -> Option<T> {
    current_or_launch(conversation, provenance, current, selection_value(launch))
}

fn current_or_launch<T: Clone>(
    conversation: &Conversation,
    provenance: impl Fn(&crate::CurrentConversationSettings) -> SettingProvenance,
    current: impl Fn(&crate::CurrentConversationSettings) -> Option<T>,
    launch: Option<T>,
) -> Option<T> {
    match conversation.current_settings.as_ref() {
        Some(settings) => match provenance(settings) {
            SettingProvenance::CurrentOverride => current(settings),
            SettingProvenance::Disabled => None,
            SettingProvenance::LaunchInherited => launch,
        },
        None => launch,
    }
}

fn current_or_lorebooks(
    conversation: &Conversation,
    launch: &SnapshotSelection<Vec<LorebookLaunchSnapshot>>,
) -> Vec<LorebookLaunchSnapshot> {
    current_or_lorebooks_value(conversation, selection_value(launch).unwrap_or_default())
}

fn current_or_lorebooks_value(
    conversation: &Conversation,
    launch: Vec<LorebookLaunchSnapshot>,
) -> Vec<LorebookLaunchSnapshot> {
    match conversation.current_settings.as_ref() {
        Some(settings) => match settings.lorebooks_provenance {
            SettingProvenance::CurrentOverride => settings.lorebooks.clone().unwrap_or_default(),
            SettingProvenance::Disabled => Vec::new(),
            SettingProvenance::LaunchInherited => launch,
        },
        None => launch,
    }
}

fn selection_value<T: Clone>(selection: &SnapshotSelection<T>) -> Option<T> {
    match selection {
        SnapshotSelection::Inherited(value) | SnapshotSelection::Explicit(value) => {
            Some(value.clone())
        }
        SnapshotSelection::Disabled => None,
    }
}

fn selection_or_fallback<T: Clone>(
    selected: &SnapshotSelection<T>,
    fallback: &SnapshotSelection<T>,
) -> Option<T> {
    match selected {
        SnapshotSelection::Inherited(value) | SnapshotSelection::Explicit(value) => {
            Some(value.clone())
        }
        SnapshotSelection::Disabled => selection_value(fallback),
    }
}

fn merge_lorebooks(
    group: Vec<LorebookLaunchSnapshot>,
    member: Vec<LorebookLaunchSnapshot>,
) -> Vec<LorebookLaunchSnapshot> {
    let mut result = group;
    for book in member {
        if !result
            .iter()
            .any(|existing| existing.source_id == book.source_id)
        {
            result.push(book);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::{
        CharacterLaunchSnapshot, GroupConversationDetails, GroupLaunchSnapshot,
        GroupMemberLaunchSnapshot, GroupParticipantPolicyDocument, GroupParticipantPolicySnapshot,
        GroupSpeakerSelectionSnapshot, MemoryModeSnapshot, PromptPurposeSnapshot,
        ProtectedSnapshotRef, SnapshotSource,
    };
    use crate::{
        Conversation, ConversationKind, ConversationLifecycle, ConversationParticipant,
        CurrentConversationSettingsPatch, ParticipantRole, ParticipantSource, PatchValue,
    };
    use lettuce_types::{
        CharacterId, ContentHash, ConversationBranchId, ConversationId, ConversationParticipantId,
        GroupId, LorebookId, ModelProfileId, PersonaId, PromptDocumentId, Revision, SceneId,
        SnapshotArtifactId, TimestampMillis,
    };

    fn reference(source: SnapshotSource) -> ProtectedSnapshotRef {
        ProtectedSnapshotRef {
            source,
            source_revision: Revision::INITIAL,
            artifact_id: SnapshotArtifactId::new(),
            digest: ContentHash::parse("ab".repeat(32)).expect("digest"),
            schema_version: 1,
            byte_size: 1,
        }
    }

    fn character(id: CharacterId, name: &str) -> CharacterLaunchSnapshot {
        CharacterLaunchSnapshot {
            snapshot_ref: reference(SnapshotSource::Character(id)),
            source_id: id,
            source_revision: Revision::INITIAL,
            name: name.into(),
            nickname: None,
        }
    }

    fn prompt(id: PromptDocumentId, purpose: PromptPurposeSnapshot) -> PromptLaunchSnapshot {
        PromptLaunchSnapshot {
            snapshot_ref: reference(SnapshotSource::Prompt(id)),
            source_id: id,
            source_revision: Revision::INITIAL,
            title: "Prompt".into(),
            purpose,
        }
    }

    fn lorebook(id: LorebookId, name: &str) -> LorebookLaunchSnapshot {
        LorebookLaunchSnapshot {
            snapshot_ref: reference(SnapshotSource::Lorebook(id)),
            source_id: id,
            source_revision: Revision::INITIAL,
            name: name.into(),
        }
    }

    fn model(id: ModelProfileId) -> ModelSelectionSnapshot {
        ModelSelectionSnapshot {
            snapshot_ref: reference(SnapshotSource::Model(id)),
            source_id: id,
            source_revision: Revision::INITIAL,
            provider_account_id: lettuce_types::ProviderAccountId::new(),
            provider_account_revision: Revision::INITIAL,
            provider_protocol: lettuce_models::ProviderProtocol::OpenAiCompatible,
            external_model_id: "model".into(),
            display_name: "Model".into(),
            context_length: None,
            max_output_tokens: None,
        }
    }

    fn persona(id: PersonaId) -> PersonaLaunchSnapshot {
        PersonaLaunchSnapshot {
            snapshot_ref: reference(SnapshotSource::Persona(id)),
            source_id: id,
            source_revision: Revision::INITIAL,
            title: "Persona".into(),
            nickname: None,
            lorebooks: SnapshotSelection::Disabled,
        }
    }

    fn scene(id: SceneId) -> SceneLaunchSnapshot {
        SceneLaunchSnapshot {
            snapshot_ref: reference(SnapshotSource::Scene(id)),
            source_id: id,
            source_revision: Revision::INITIAL,
            title: "Scene".into(),
        }
    }

    fn memory() -> MemorySettingsSnapshot {
        MemorySettingsSnapshot {
            policy_ref: None,
            mode: MemoryModeSnapshot::Manual,
            selected_revision_ids: Vec::new(),
        }
    }

    fn participant(
        id: ConversationParticipantId,
        ordinal: u32,
        role: ParticipantRole,
        source: ParticipantSource,
        model_selection: SnapshotSelection<ModelSelectionSnapshot>,
    ) -> ConversationParticipant {
        ConversationParticipant {
            id,
            role,
            ordinal,
            enabled: true,
            muted: false,
            source,
            display_name: "Participant".into(),
            authored_description: None,
            model_selection,
            revision: Revision::INITIAL,
            created_at: TimestampMillis::UNIX_EPOCH,
            updated_at: TimestampMillis::UNIX_EPOCH,
        }
    }

    fn direct_conversation() -> Conversation {
        let character_id = CharacterId::new();
        let model = model(ModelProfileId::new());
        let details = crate::snapshot::DirectConversationDetails {
            format_version: 1,
            character: character(character_id, "Character"),
            persona: SnapshotSelection::Explicit(persona(PersonaId::new())),
            scene: SnapshotSelection::Explicit(scene(SceneId::new())),
            starter: SnapshotSelection::Disabled,
            prompt: SnapshotSelection::Explicit(prompt(
                PromptDocumentId::new(),
                PromptPurposeSnapshot::Direct,
            )),
            lorebooks: SnapshotSelection::Explicit(vec![lorebook(LorebookId::new(), "Launch")]),
            model: SnapshotSelection::Explicit(model.clone()),
            memory: SnapshotSelection::Explicit(memory()),
            voice: SnapshotSelection::Disabled,
        };
        let user_id = ConversationParticipantId::new();
        let character_participant_id = ConversationParticipantId::new();
        Conversation {
            id: ConversationId::new(),
            lifecycle: ConversationLifecycle::Active,
            title: "Direct".into(),
            kind: ConversationKind::Direct(details),
            active_branch_id: ConversationBranchId::new(),
            participants: vec![
                participant(
                    user_id,
                    0,
                    ParticipantRole::User,
                    ParticipantSource::User,
                    SnapshotSelection::Disabled,
                ),
                participant(
                    character_participant_id,
                    1,
                    ParticipantRole::Character,
                    ParticipantSource::Character(character_id),
                    SnapshotSelection::Explicit(model),
                ),
            ],
            current_settings: None,
            revision: Revision::INITIAL,
            created_at: TimestampMillis::UNIX_EPOCH,
            updated_at: TimestampMillis::UNIX_EPOCH,
        }
    }

    fn group_conversation(
        mode: GroupChatModeSnapshot,
    ) -> (Conversation, ConversationParticipantId) {
        let first_character = CharacterId::new();
        let second_character = CharacterId::new();
        let first_id = ConversationParticipantId::new();
        let second_id = ConversationParticipantId::new();
        let cast_model = model(ModelProfileId::new());
        let group_prompt = prompt(
            PromptDocumentId::new(),
            if mode == GroupChatModeSnapshot::Roleplay {
                PromptPurposeSnapshot::GroupRoleplay
            } else {
                PromptPurposeSnapshot::GroupConversational
            },
        );
        let member_prompt = prompt(
            PromptDocumentId::new(),
            if mode == GroupChatModeSnapshot::Roleplay {
                PromptPurposeSnapshot::GroupRoleplay
            } else {
                PromptPurposeSnapshot::GroupConversational
            },
        );
        let group_book = lorebook(LorebookId::new(), "Group");
        let overlap_book = lorebook(LorebookId::new(), "Overlap");
        let member_book = lorebook(LorebookId::new(), "Member");
        let first_member = GroupMemberLaunchSnapshot {
            character: character(first_character, "First"),
            ordinal: 0,
            enabled: true,
            muted: false,
            model_override: SnapshotSelection::Disabled,
            lorebooks: SnapshotSelection::Explicit(vec![overlap_book.clone(), member_book]),
            prompt: SnapshotSelection::Explicit(member_prompt),
        };
        let second_member = GroupMemberLaunchSnapshot {
            character: character(second_character, "Second"),
            ordinal: 1,
            enabled: true,
            muted: false,
            model_override: SnapshotSelection::Disabled,
            lorebooks: SnapshotSelection::Disabled,
            prompt: SnapshotSelection::Disabled,
        };
        let group_id = GroupId::new();
        let group = GroupLaunchSnapshot {
            snapshot_ref: reference(SnapshotSource::Group(group_id)),
            source_id: group_id,
            source_revision: Revision::INITIAL,
            name: "Group".into(),
            members: vec![first_member, second_member],
            chat_mode: mode,
            speaker_selection: GroupSpeakerSelectionSnapshot::RoundRobin,
            memory: SnapshotSelection::Explicit(memory()),
            disable_character_lorebook: false,
            persona: SnapshotSelection::Explicit(persona(PersonaId::new())),
            scene: if mode == GroupChatModeSnapshot::Roleplay {
                SnapshotSelection::Explicit(scene(SceneId::new()))
            } else {
                SnapshotSelection::Disabled
            },
            prompt: SnapshotSelection::Explicit(group_prompt),
            lorebooks: SnapshotSelection::Explicit(vec![group_book, overlap_book]),
            model: SnapshotSelection::Explicit(cast_model),
        };
        let user_id = ConversationParticipantId::new();
        let details = GroupConversationDetails {
            format_version: 1,
            group,
            initial_participant_policy: GroupParticipantPolicyDocument {
                members: vec![
                    GroupParticipantPolicySnapshot {
                        participant_id: first_id,
                        enabled: true,
                        muted: false,
                        model_override: SnapshotSelection::Disabled,
                    },
                    GroupParticipantPolicySnapshot {
                        participant_id: second_id,
                        enabled: true,
                        muted: false,
                        model_override: SnapshotSelection::Disabled,
                    },
                ],
                revision: Revision::INITIAL,
                created_at: TimestampMillis::UNIX_EPOCH,
                updated_at: TimestampMillis::UNIX_EPOCH,
            },
        };
        let conversation = Conversation {
            id: ConversationId::new(),
            lifecycle: ConversationLifecycle::Active,
            title: "Group".into(),
            kind: ConversationKind::Group(details),
            active_branch_id: ConversationBranchId::new(),
            participants: vec![
                participant(
                    user_id,
                    0,
                    ParticipantRole::User,
                    ParticipantSource::User,
                    SnapshotSelection::Disabled,
                ),
                participant(
                    first_id,
                    1,
                    ParticipantRole::Character,
                    ParticipantSource::Character(first_character),
                    SnapshotSelection::Disabled,
                ),
                participant(
                    second_id,
                    2,
                    ParticipantRole::Character,
                    ParticipantSource::Character(second_character),
                    SnapshotSelection::Disabled,
                ),
            ],
            current_settings: None,
            revision: Revision::INITIAL,
            created_at: TimestampMillis::UNIX_EPOCH,
            updated_at: TimestampMillis::UNIX_EPOCH,
        };
        (conversation, first_id)
    }

    #[test]
    fn direct_settings_use_launch_values_and_current_overrides() {
        let mut conversation = direct_conversation();
        let launch = resolve_effective_settings(&conversation, None).expect("launch settings");
        assert_eq!(launch.author_note, None);
        assert_eq!(launch.lorebooks.len(), 1);
        assert!(launch.prompt.is_some());
        assert!(launch.persona.is_some());
        assert!(launch.scene.is_some());

        let override_prompt = prompt(PromptDocumentId::new(), PromptPurposeSnapshot::Direct);
        let override_book = lorebook(LorebookId::new(), "Override");
        conversation.current_settings = Some(
            CurrentConversationSettingsPatch {
                author_note: PatchValue::Set("note".into()),
                memory: PatchValue::Keep,
                model_override: PatchValue::Keep,
                voice: PatchValue::Keep,
                prompt: PatchValue::Set(override_prompt.clone()),
                lorebooks: PatchValue::Set(vec![override_book.clone()]),
                persona: PatchValue::Keep,
                scene: PatchValue::Keep,
            }
            .apply(None, None)
            .expect("settings override"),
        );
        let effective =
            resolve_effective_settings(&conversation, None).expect("effective settings");
        assert_eq!(effective.author_note.as_deref(), Some("note"));
        assert_eq!(effective.prompt, Some(override_prompt));
        assert_eq!(effective.lorebooks, vec![override_book]);
        assert_eq!(
            resolve_effective_settings(&conversation, Some(ConversationParticipantId::new()))
                .expect_err("direct speaker"),
            ValidationError::InvalidReference {
                field: "effective_settings.selected_speaker"
            }
        );
    }

    #[test]
    fn group_settings_prefer_member_and_merge_lorebooks() {
        let (conversation, speaker) = group_conversation(GroupChatModeSnapshot::Roleplay);
        let effective =
            resolve_effective_settings(&conversation, Some(speaker)).expect("group settings");
        let (group_prompt_id, member_prompt_id, second_speaker) =
            match &conversation.kind {
                ConversationKind::Group(details) => (
                    match &details.group.prompt {
                        SnapshotSelection::Inherited(prompt)
                        | SnapshotSelection::Explicit(prompt) => prompt.source_id,
                        SnapshotSelection::Disabled => panic!("group prompt"),
                    },
                    match &details.group.members[0].prompt {
                        SnapshotSelection::Inherited(prompt)
                        | SnapshotSelection::Explicit(prompt) => prompt.source_id,
                        SnapshotSelection::Disabled => panic!("member prompt"),
                    },
                    conversation
                        .participants
                        .iter()
                        .find(|participant| participant.ordinal == 2)
                        .expect("second speaker")
                        .id,
                ),
                ConversationKind::Direct(_) => unreachable!(),
            };
        assert_eq!(effective.lorebooks.len(), 3);
        assert_eq!(effective.lorebooks[0].name, "Group");
        assert_eq!(effective.lorebooks[1].name, "Overlap");
        assert_eq!(effective.lorebooks[2].name, "Member");
        assert_eq!(
            effective.prompt.as_ref().map(|prompt| prompt.source_id),
            Some(member_prompt_id)
        );
        assert!(effective.model.is_some());
        assert!(effective.memory.is_some());
        assert!(effective.persona.is_some());
        assert!(effective.scene.is_some());
        assert!(effective.voice.is_none());

        let second = resolve_effective_settings(&conversation, Some(second_speaker))
            .expect("fallback settings");
        assert_eq!(
            second.prompt.as_ref().map(|prompt| prompt.source_id),
            Some(group_prompt_id)
        );
        assert_eq!(second.lorebooks.len(), 2);

        let mut disabled_character_books = conversation.clone();
        if let ConversationKind::Group(details) = &mut disabled_character_books.kind {
            details.group.disable_character_lorebook = true;
        }
        let disabled = resolve_effective_settings(&disabled_character_books, Some(speaker))
            .expect("disabled member lorebooks");
        assert_eq!(disabled.lorebooks.len(), 2);

        assert_eq!(
            resolve_effective_settings(&conversation, None).expect_err("speaker required"),
            ValidationError::InvalidReference {
                field: "effective_settings.selected_speaker"
            }
        );
        assert_eq!(
            resolve_effective_settings(&conversation, Some(ConversationParticipantId::new()))
                .expect_err("unknown speaker"),
            ValidationError::InvalidReference {
                field: "effective_settings.selected_speaker"
            }
        );
    }

    #[test]
    fn conversational_group_rejects_launch_scene() {
        let (mut conversation, _) = group_conversation(GroupChatModeSnapshot::Conversation);
        if let ConversationKind::Group(details) = &mut conversation.kind {
            details.group.scene = SnapshotSelection::Explicit(scene(SceneId::new()));
        }
        assert_eq!(
            conversation.validate(),
            Err(ValidationError::InvalidReference {
                field: "group.scene.chat_mode"
            })
        );
    }
}
