use lettuce_characters::{
    Character, CharacterMediaSlot, ChatMode, ConversationStarter, GroupMember, GroupProfile,
    ImageRecommendation, Persona, PersonaMediaSlot, Scene, SceneAssetSlot, SceneOwner, ScenePart,
    SceneVariant, Selection, SpeakerSelection, StarterRole, VoicePreference,
};
use lettuce_context::{
    DetectionPolicy, KeywordMatchMode, LorebookBehaviorVersion, LorebookDetails,
    PromptBehaviorVersion, PromptDocument, PromptEntryChatMode, PromptEntryCondition,
    PromptEntryImageSlot, PromptEntryInfoSource, PromptEntryPayload, PromptEntryPosition,
    PromptEntryRole, PromptPurpose,
};
use lettuce_conversations::{
    ArtifactError, CharacterMediaLinkV1, CharacterMediaSlotV1, CharacterSnapshotBodyV1, ChatModeV1,
    DetectionPolicyV1, DocumentSelectionV1, GroupMemberV1, GroupSnapshotBodyV1,
    ImageRecommendationV1, InteractionModeV1, KeywordMatchModeV1, LorebookBehaviorVersionV1,
    LorebookEntryV1, LorebookSnapshotBodyV1, MemoryPolicyV1, ModalityV1, ModelKindV1,
    ModelSnapshotBodyV1, PersonaMediaLinkV1, PersonaMediaSlotV1, PersonaSnapshotBodyV1,
    PromptBehaviorVersionV1, PromptEntryChatModeV1, PromptEntryConditionV1, PromptEntryImageSlotV1,
    PromptEntryInfoSourceV1, PromptEntryPayloadV1, PromptEntryPositionV1, PromptEntryRoleV1,
    PromptEntryV1, PromptPurposeV1, PromptSnapshotBodyV1, ProviderProtocolV1, SceneAssetLinkV1,
    SceneAssetSlotV1, SceneOwnerV1, ScenePartV1, SceneSnapshotBodyV1, SceneVariantBodyV1,
    SnapshotArtifactDraft, SnapshotDocumentBody, SnapshotEnvelopeV1, SnapshotProviderDescriptorV1,
    SpeakerSelectionV1, StarterMessageV1, StarterRoleV1, StarterSnapshotBodyV1,
    build_snapshot_draft,
};
use lettuce_models::{Modality, ModelKind, ModelProfile, ProviderAccount, ProviderProtocol};
use lettuce_types::{Revision, SnapshotArtifactId};

pub(crate) fn draft<T: SnapshotDocumentBody>(
    artifact_id: SnapshotArtifactId,
    source_revision: Revision,
    payload: T,
) -> Result<SnapshotArtifactDraft, ArtifactError> {
    let envelope = SnapshotEnvelopeV1::new(source_revision, payload)
        .map_err(ArtifactError::InvalidReference)?;
    build_snapshot_draft(artifact_id, &envelope)
}

pub(crate) fn character_body(character: &Character) -> CharacterSnapshotBodyV1 {
    let defaults = &character.defaults;
    CharacterSnapshotBodyV1 {
        character_id: character.id,
        name: character.profile.name.clone(),
        nickname: character.profile.nickname.clone(),
        description: character.profile.description.clone(),
        definition: character.profile.definition.clone(),
        design_description: character.profile.design_description.clone(),
        interaction_mode: match defaults.interaction_mode {
            lettuce_characters::InteractionMode::Roleplay => InteractionModeV1::Roleplay,
            lettuce_characters::InteractionMode::Companion => InteractionModeV1::Companion,
        },
        memory_policy: match defaults.memory_policy {
            lettuce_characters::MemoryPolicy::Manual => MemoryPolicyV1::Manual,
            lettuce_characters::MemoryPolicy::Dynamic => MemoryPolicyV1::Dynamic,
        },
        model_profile_id: defaults.model_profile_id,
        default_scene_id: defaults.default_scene_id,
        default_starter_id: defaults.default_starter_id,
        direct_prompt_id: defaults.direct_prompt_id,
        group_conversation_prompt_id: defaults.group_conversation_prompt_id,
        group_roleplay_prompt_id: defaults.group_roleplay_prompt_id,
        voice: defaults.voice.as_ref().map(|voice| match voice {
            VoicePreference::VoiceProfile(id) => {
                lettuce_conversations::VoicePreferenceV1::VoiceProfile(*id)
            }
            VoicePreference::UnresolvedLegacy(locator) => {
                lettuce_conversations::VoicePreferenceV1::UnresolvedLegacy {
                    locator: locator.locator.clone(),
                }
            }
        }),
        voice_autoplay: defaults.voice_autoplay,
        image_recommendation: character
            .image_recommendation
            .as_ref()
            .map(image_recommendation),
        media: character
            .media
            .links
            .iter()
            .map(|link| CharacterMediaLinkV1 {
                asset_id: link.asset_id,
                slot: match link.slot {
                    CharacterMediaSlot::AvatarOriginal => CharacterMediaSlotV1::AvatarOriginal,
                    CharacterMediaSlot::Background => CharacterMediaSlotV1::Background,
                    CharacterMediaSlot::DesignReference => CharacterMediaSlotV1::DesignReference,
                },
                ordinal: link.ordinal,
            })
            .collect(),
        presentation_asset_ids: character
            .presentation
            .referenced_asset_ids()
            .into_iter()
            .collect(),
    }
}

pub(crate) fn group_body(group: &GroupProfile, members: &[GroupMember]) -> GroupSnapshotBodyV1 {
    GroupSnapshotBodyV1 {
        group_id: group.id,
        name: group.name.clone(),
        chat_mode: match group.chat_mode {
            ChatMode::Conversation => ChatModeV1::Conversation,
            ChatMode::Roleplay => ChatModeV1::Roleplay,
        },
        speaker_selection: match group.speaker_selection {
            SpeakerSelection::Llm => SpeakerSelectionV1::Llm,
            SpeakerSelection::Heuristic => SpeakerSelectionV1::Heuristic,
            SpeakerSelection::RoundRobin => SpeakerSelectionV1::RoundRobin,
            SpeakerSelection::Director => SpeakerSelectionV1::Director,
            SpeakerSelection::DirectorAction => SpeakerSelectionV1::DirectorAction,
        },
        memory_policy: match group.memory_policy {
            lettuce_characters::MemoryPolicy::Manual => MemoryPolicyV1::Manual,
            lettuce_characters::MemoryPolicy::Dynamic => MemoryPolicyV1::Dynamic,
        },
        disable_character_lorebooks: group.disable_character_lorebooks,
        persona: match group.persona {
            Selection::Inherit => DocumentSelectionV1::Inherit,
            Selection::Explicit(id) => DocumentSelectionV1::Explicit(id),
            Selection::Disabled => DocumentSelectionV1::Disabled,
        },
        group_conversation_prompt_id: group.group_conversation_prompt_id,
        group_roleplay_prompt_id: group.group_roleplay_prompt_id,
        members: members
            .iter()
            .map(|member| GroupMemberV1 {
                character_id: member.character_id,
                ordinal: member.ordinal,
                muted: member.muted,
                model_profile_override: member.model_profile_override,
            })
            .collect(),
        starting_scene_id: group.starting_scene_id,
        background_asset_id: group.background_asset_id,
    }
}

pub(crate) fn persona_body(persona: &Persona) -> PersonaSnapshotBodyV1 {
    PersonaSnapshotBodyV1 {
        persona_id: persona.id,
        title: persona.title.clone(),
        description: persona.description.clone(),
        nickname: persona.nickname.clone(),
        design_description: persona.design_description.clone(),
        image_recommendation: persona
            .image_recommendation
            .as_ref()
            .map(image_recommendation),
        media: persona
            .media
            .links
            .iter()
            .map(|link| PersonaMediaLinkV1 {
                asset_id: link.asset_id,
                slot: match link.slot {
                    PersonaMediaSlot::Avatar => PersonaMediaSlotV1::Avatar,
                    PersonaMediaSlot::DesignReference => PersonaMediaSlotV1::DesignReference,
                },
                ordinal: link.ordinal,
            })
            .collect(),
    }
}

pub(crate) fn scene_body(scene: &Scene, variants: &[SceneVariant]) -> SceneSnapshotBodyV1 {
    let mut ordered: Vec<&SceneVariant> = variants
        .iter()
        .filter(|variant| variant.scene_id == scene.id)
        .collect();
    ordered.sort_by_key(|variant| variant.ordinal);
    let selected_variant_id = scene
        .selected_variant_id
        .filter(|id| ordered.iter().any(|variant| variant.id == *id));
    SceneSnapshotBodyV1 {
        scene_id: scene.id,
        owner: match scene.owner {
            SceneOwner::Character(id) => SceneOwnerV1::Character(id),
            SceneOwner::Group(id) => SceneOwnerV1::Group(id),
        },
        ordinal: scene.ordinal,
        content: scene_parts(&scene.content.parts),
        direction: scene.direction.clone(),
        selected_variant_id,
        variants: ordered
            .iter()
            .enumerate()
            .map(|(ordinal, variant)| SceneVariantBodyV1 {
                variant_id: variant.id,
                ordinal: u32::try_from(ordinal).unwrap_or(u32::MAX),
                content: scene_parts(&variant.content.parts),
                direction: variant.direction.clone(),
            })
            .collect(),
        assets: scene
            .assets
            .iter()
            .map(|link| SceneAssetLinkV1 {
                id: link.id,
                asset_id: link.asset_id,
                slot: match link.slot {
                    SceneAssetSlot::Background => SceneAssetSlotV1::Background,
                    SceneAssetSlot::Inline => SceneAssetSlotV1::Inline,
                },
                ordinal: link.ordinal,
            })
            .collect(),
    }
}

pub(crate) fn starter_body(starter: &ConversationStarter) -> StarterSnapshotBodyV1 {
    StarterSnapshotBodyV1 {
        starter_id: starter.id,
        character_id: starter.character_id,
        name: starter.name.clone(),
        ordinal: starter.ordinal,
        messages: starter
            .messages
            .iter()
            .enumerate()
            .map(|(ordinal, message)| StarterMessageV1 {
                message_id: message.id,
                role: match message.role {
                    StarterRole::User => StarterRoleV1::User,
                    StarterRole::Assistant => StarterRoleV1::Assistant,
                },
                content: message.content.clone(),
                ordinal: u32::try_from(ordinal).unwrap_or(u32::MAX),
            })
            .collect(),
        scene_id: starter.scene_id,
        prompt_id: starter.prompt_id,
        lorebooks: match &starter.lorebooks {
            Selection::Inherit => DocumentSelectionV1::Inherit,
            Selection::Explicit(ids) => DocumentSelectionV1::Explicit(ids.clone()),
            Selection::Disabled => DocumentSelectionV1::Disabled,
        },
    }
}

pub(crate) fn lorebook_body(details: &LorebookDetails) -> LorebookSnapshotBodyV1 {
    LorebookSnapshotBodyV1 {
        lorebook_id: details.book.id,
        name: details.book.name.clone(),
        detection_policy: match details.book.detection_policy {
            DetectionPolicy::RecentMessageWindow => DetectionPolicyV1::RecentMessageWindow,
            DetectionPolicy::LatestUserMessage => DetectionPolicyV1::LatestUserMessage,
        },
        icon_asset_id: details.book.icon_asset_id,
        behavior_version: match details.book.behavior_version {
            LorebookBehaviorVersion::LegacyV1 => LorebookBehaviorVersionV1::LegacyV1,
            LorebookBehaviorVersion::DeterministicV2 => LorebookBehaviorVersionV1::DeterministicV2,
        },
        entries: details
            .entries
            .iter()
            .enumerate()
            .map(|(ordinal, entry)| LorebookEntryV1 {
                entry_id: entry.id,
                title: entry.title.clone(),
                enabled: entry.enabled,
                always_active: entry.always_active,
                keywords: entry.keywords.clone(),
                case_sensitive: entry.case_sensitive,
                match_mode: match entry.match_mode {
                    KeywordMatchMode::Literal => KeywordMatchModeV1::Literal,
                    KeywordMatchMode::Regex => KeywordMatchModeV1::Regex,
                },
                content: entry.content.clone(),
                priority: entry.priority,
                ordinal: u32::try_from(ordinal).unwrap_or(u32::MAX),
            })
            .collect(),
    }
}

pub(crate) fn prompt_body(document: &PromptDocument) -> PromptSnapshotBodyV1 {
    PromptSnapshotBodyV1 {
        prompt_id: document.id,
        name: document.name.clone(),
        purpose: prompt_purpose(document.purpose),
        condense: document.condense,
        behavior_version: match document.behavior_version {
            PromptBehaviorVersion::LegacyV1 => PromptBehaviorVersionV1::LegacyV1,
            PromptBehaviorVersion::DeterministicV2 => PromptBehaviorVersionV1::DeterministicV2,
        },
        entries: document
            .entries
            .iter()
            .enumerate()
            .map(|(ordinal, entry)| PromptEntryV1 {
                entry_id: entry.id,
                built_in_entry_key: entry.built_in_entry_key.clone(),
                name: entry.name.clone(),
                role: match entry.role {
                    PromptEntryRole::System => PromptEntryRoleV1::System,
                    PromptEntryRole::User => PromptEntryRoleV1::User,
                    PromptEntryRole::Assistant => PromptEntryRoleV1::Assistant,
                },
                content: entry.content.clone(),
                enabled: entry.enabled,
                injection_position: match entry.injection_position {
                    PromptEntryPosition::Relative => PromptEntryPositionV1::Relative,
                    PromptEntryPosition::InChat => PromptEntryPositionV1::InChat,
                    PromptEntryPosition::Conditional => PromptEntryPositionV1::Conditional,
                    PromptEntryPosition::Interval => PromptEntryPositionV1::Interval,
                },
                depth: entry.depth,
                conditional_min_messages: entry.conditional_min_messages,
                interval_turns: entry.interval_turns,
                system_prompt: entry.system_prompt,
                condition: entry.conditions.as_ref().map(prompt_condition),
                payload: entry.payload.as_ref().map(|payload| match payload {
                    PromptEntryPayload::ImageSlot { slot } => PromptEntryPayloadV1::ImageSlot {
                        slot: match slot {
                            PromptEntryImageSlot::Character => PromptEntryImageSlotV1::Character,
                            PromptEntryImageSlot::Persona => PromptEntryImageSlotV1::Persona,
                            PromptEntryImageSlot::ChatBackground => {
                                PromptEntryImageSlotV1::ChatBackground
                            }
                            PromptEntryImageSlot::Avatar => PromptEntryImageSlotV1::Avatar,
                            PromptEntryImageSlot::References => PromptEntryImageSlotV1::References,
                        },
                    },
                }),
                ordinal: u32::try_from(ordinal).unwrap_or(u32::MAX),
            })
            .collect(),
    }
}

pub(crate) fn model_body(profile: &ModelProfile, account: &ProviderAccount) -> ModelSnapshotBodyV1 {
    ModelSnapshotBodyV1 {
        model_profile_id: profile.id,
        external_model_id: profile.external_model_id.clone(),
        display_name: profile.display_name.clone(),
        kind: match profile.kind {
            ModelKind::Chat => ModelKindV1::Chat,
            ModelKind::Image => ModelKindV1::Image,
            ModelKind::Embedding => ModelKindV1::Embedding,
            ModelKind::Speech => ModelKindV1::Speech,
        },
        input_modalities: profile
            .config
            .input_modalities
            .iter()
            .copied()
            .map(modality)
            .collect(),
        output_modalities: profile
            .config
            .output_modalities
            .iter()
            .copied()
            .map(modality)
            .collect(),
        temperature: profile.config.temperature,
        context_length: profile.config.context_length,
        max_output_tokens: profile.config.max_output_tokens,
        provider: SnapshotProviderDescriptorV1 {
            provider_account_id: account.id,
            provider_revision: account.revision,
            provider_kind_label: account.provider_kind.clone(),
            protocol: match account.protocol {
                ProviderProtocol::OpenAiCompatible => ProviderProtocolV1::OpenAiCompatible,
                ProviderProtocol::Anthropic => ProviderProtocolV1::Anthropic,
                ProviderProtocol::Gemini => ProviderProtocolV1::Gemini,
                ProviderProtocol::Ollama => ProviderProtocolV1::Ollama,
                ProviderProtocol::LlamaCpp => ProviderProtocolV1::LlamaCpp,
                ProviderProtocol::StableDiffusion => ProviderProtocolV1::StableDiffusion,
            },
            display_name: account.label.clone(),
        },
    }
}

const fn modality(value: Modality) -> ModalityV1 {
    match value {
        Modality::Text => ModalityV1::Text,
        Modality::Image => ModalityV1::Image,
        Modality::Audio => ModalityV1::Audio,
    }
}

fn image_recommendation(value: &ImageRecommendation) -> ImageRecommendationV1 {
    ImageRecommendationV1 {
        artifact_id: value.artifact_id,
        unresolved_legacy_name: value.unresolved_legacy_name.clone(),
        strength: value.strength,
    }
}

fn scene_parts(parts: &[ScenePart]) -> Vec<ScenePartV1> {
    parts
        .iter()
        .map(|part| match part {
            ScenePart::Text { text } => ScenePartV1::Text { text: text.clone() },
            ScenePart::InlineAsset { link_id } => ScenePartV1::InlineAsset { link_id: *link_id },
        })
        .collect()
}

pub(crate) const fn prompt_purpose(value: PromptPurpose) -> PromptPurposeV1 {
    match value {
        PromptPurpose::Undefined => PromptPurposeV1::Undefined,
        PromptPurpose::DirectChat => PromptPurposeV1::DirectChat,
        PromptPurpose::CompanionChat => PromptPurposeV1::CompanionChat,
        PromptPurpose::GroupChatRoleplay => PromptPurposeV1::GroupChatRoleplay,
        PromptPurpose::GroupChatConversational => PromptPurposeV1::GroupChatConversational,
        PromptPurpose::DynamicMemorySummarizer => PromptPurposeV1::DynamicMemorySummarizer,
        PromptPurpose::DynamicMemoryManager => PromptPurposeV1::DynamicMemoryManager,
        PromptPurpose::ReplyHelperRoleplay => PromptPurposeV1::ReplyHelperRoleplay,
        PromptPurpose::ReplyHelperConversational => PromptPurposeV1::ReplyHelperConversational,
        PromptPurpose::LorebookEntryWriter => PromptPurposeV1::LorebookEntryWriter,
        PromptPurpose::LorebookKeywordGenerator => PromptPurposeV1::LorebookKeywordGenerator,
        PromptPurpose::LorebookGeneratorPlanner => PromptPurposeV1::LorebookGeneratorPlanner,
        PromptPurpose::LorebookGeneratorWriter => PromptPurposeV1::LorebookGeneratorWriter,
        PromptPurpose::LorebookGeneratorRefine => PromptPurposeV1::LorebookGeneratorRefine,
        PromptPurpose::LorebookGeneratorCoherence => PromptPurposeV1::LorebookGeneratorCoherence,
        PromptPurpose::AvatarGeneration => PromptPurposeV1::AvatarGeneration,
        PromptPurpose::AvatarEditRequest => PromptPurposeV1::AvatarEditRequest,
        PromptPurpose::SceneGeneration => PromptPurposeV1::SceneGeneration,
        PromptPurpose::ScenePromptWriter => PromptPurposeV1::ScenePromptWriter,
        PromptPurpose::DesignReferenceWriter => PromptPurposeV1::DesignReferenceWriter,
        PromptPurpose::CompanionSoulWriter => PromptPurposeV1::CompanionSoulWriter,
        PromptPurpose::CompanionGrowthcycle => PromptPurposeV1::CompanionGrowthcycle,
        PromptPurpose::CompanionConsolidation => PromptPurposeV1::CompanionConsolidation,
    }
}

fn prompt_condition(value: &PromptEntryCondition) -> PromptEntryConditionV1 {
    match value {
        PromptEntryCondition::ChatMode { value } => PromptEntryConditionV1::ChatMode {
            value: match value {
                PromptEntryChatMode::Direct => PromptEntryChatModeV1::Direct,
                PromptEntryChatMode::Group => PromptEntryChatModeV1::Group,
            },
        },
        PromptEntryCondition::InfoSource { value } => PromptEntryConditionV1::InfoSource {
            value: match value {
                PromptEntryInfoSource::Messages => PromptEntryInfoSourceV1::Messages,
                PromptEntryInfoSource::Memory => PromptEntryInfoSourceV1::Memory,
                PromptEntryInfoSource::Mixed => PromptEntryInfoSourceV1::Mixed,
            },
        },
        PromptEntryCondition::SceneGenerationEnabled { value } => {
            PromptEntryConditionV1::SceneGenerationEnabled { value: *value }
        }
        PromptEntryCondition::AvatarGenerationEnabled { value } => {
            PromptEntryConditionV1::AvatarGenerationEnabled { value: *value }
        }
        PromptEntryCondition::IsLocalImageGenerationModel { value } => {
            PromptEntryConditionV1::IsLocalImageGenerationModel { value: *value }
        }
        PromptEntryCondition::IsSceneGenerationLocalImageModel { value } => {
            PromptEntryConditionV1::IsSceneGenerationLocalImageModel { value: *value }
        }
        PromptEntryCondition::HasScene { value } => {
            PromptEntryConditionV1::HasScene { value: *value }
        }
        PromptEntryCondition::HasSceneDirection { value } => {
            PromptEntryConditionV1::HasSceneDirection { value: *value }
        }
        PromptEntryCondition::HasPersona { value } => {
            PromptEntryConditionV1::HasPersona { value: *value }
        }
        PromptEntryCondition::MessageCountAtLeast { value } => {
            PromptEntryConditionV1::MessageCountAtLeast { value: *value }
        }
        PromptEntryCondition::ParticipantCountAtLeast { value } => {
            PromptEntryConditionV1::ParticipantCountAtLeast { value: *value }
        }
        PromptEntryCondition::KeywordAny { values } => PromptEntryConditionV1::KeywordAny {
            values: values.clone(),
        },
        PromptEntryCondition::KeywordAll { values } => PromptEntryConditionV1::KeywordAll {
            values: values.clone(),
        },
        PromptEntryCondition::KeywordNone { values } => PromptEntryConditionV1::KeywordNone {
            values: values.clone(),
        },
        PromptEntryCondition::DynamicMemoryEnabled { value } => {
            PromptEntryConditionV1::DynamicMemoryEnabled { value: *value }
        }
        PromptEntryCondition::HasMemorySummary { value } => {
            PromptEntryConditionV1::HasMemorySummary { value: *value }
        }
        PromptEntryCondition::HasKeyMemories { value } => {
            PromptEntryConditionV1::HasKeyMemories { value: *value }
        }
        PromptEntryCondition::HasLorebookContent { value } => {
            PromptEntryConditionV1::HasLorebookContent { value: *value }
        }
        PromptEntryCondition::DoesAuthorNoteExists { value } => {
            PromptEntryConditionV1::DoesAuthorNoteExists { value: *value }
        }
        PromptEntryCondition::HasActiveScheduledNote { value } => {
            PromptEntryConditionV1::HasActiveScheduledNote { value: *value }
        }
        PromptEntryCondition::HasSubjectDescription { value } => {
            PromptEntryConditionV1::HasSubjectDescription { value: *value }
        }
        PromptEntryCondition::HasCurrentDescription { value } => {
            PromptEntryConditionV1::HasCurrentDescription { value: *value }
        }
        PromptEntryCondition::HasCharacterReferenceImages { value } => {
            PromptEntryConditionV1::HasCharacterReferenceImages { value: *value }
        }
        PromptEntryCondition::HasChatBackground { value } => {
            PromptEntryConditionV1::HasChatBackground { value: *value }
        }
        PromptEntryCondition::HasPersonaReferenceImages { value } => {
            PromptEntryConditionV1::HasPersonaReferenceImages { value: *value }
        }
        PromptEntryCondition::HasCharacterReferenceText { value } => {
            PromptEntryConditionV1::HasCharacterReferenceText { value: *value }
        }
        PromptEntryCondition::HasPersonaReferenceText { value } => {
            PromptEntryConditionV1::HasPersonaReferenceText { value: *value }
        }
        PromptEntryCondition::InputScopeAny { values } => PromptEntryConditionV1::InputScopeAny {
            values: values.clone(),
        },
        PromptEntryCondition::OutputScopeAny { values } => PromptEntryConditionV1::OutputScopeAny {
            values: values.clone(),
        },
        PromptEntryCondition::ProviderIdAny { values } => PromptEntryConditionV1::ProviderIdAny {
            values: values.clone(),
        },
        PromptEntryCondition::ReasoningEnabled { value } => {
            PromptEntryConditionV1::ReasoningEnabled { value: *value }
        }
        PromptEntryCondition::VisionEnabled { value } => {
            PromptEntryConditionV1::VisionEnabled { value: *value }
        }
        PromptEntryCondition::IsTimeAwarenessEnabled { value } => {
            PromptEntryConditionV1::IsTimeAwarenessEnabled { value: *value }
        }
        PromptEntryCondition::IsCompanionMode { value } => {
            PromptEntryConditionV1::IsCompanionMode { value: *value }
        }
        PromptEntryCondition::All { conditions } => PromptEntryConditionV1::All {
            conditions: conditions.iter().map(prompt_condition).collect(),
        },
        PromptEntryCondition::Any { conditions } => PromptEntryConditionV1::Any {
            conditions: conditions.iter().map(prompt_condition).collect(),
        },
        PromptEntryCondition::Not { condition } => PromptEntryConditionV1::Not {
            condition: Box::new(prompt_condition(condition)),
        },
    }
}
