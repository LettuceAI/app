use lettuce_context::{PromptDocument, PromptEntryRole};
use lettuce_context::{
    PromptEntryChatMode, PromptEntryImageSlot, PromptEntryInfoSource, SceneImageProtocolKind,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptPurpose {
    DirectChat,
    CompanionChat,
    GroupChatRoleplay,
    GroupChatConversational,
    DynamicMemorySummarizer,
    DynamicMemoryManager,
    ReplyHelperRoleplay,
    ReplyHelperConversational,
    LorebookEntryWriter,
    LorebookKeywordGenerator,
    LorebookGeneratorPlanner,
    LorebookGeneratorWriter,
    LorebookGeneratorRefine,
    LorebookGeneratorCoherence,
    AvatarGeneration,
    AvatarEditRequest,
    SceneGeneration,
    ScenePromptWriter,
    DesignReferenceWriter,
    CompanionSoulWriter,
    CompanionGrowthcycle,
    CompanionConsolidation,
    RuntimeText,
}

impl TryFrom<lettuce_context::PromptPurpose> for PromptPurpose {
    type Error = super::Error;

    fn try_from(value: lettuce_context::PromptPurpose) -> Result<Self, Self::Error> {
        Ok(match value {
            lettuce_context::PromptPurpose::Undefined => {
                return Err(super::format("undefined is not a UEC 3 prompt purpose"));
            }
            lettuce_context::PromptPurpose::DirectChat => Self::DirectChat,
            lettuce_context::PromptPurpose::CompanionChat => Self::CompanionChat,
            lettuce_context::PromptPurpose::GroupChatRoleplay => Self::GroupChatRoleplay,
            lettuce_context::PromptPurpose::GroupChatConversational => {
                Self::GroupChatConversational
            }
            lettuce_context::PromptPurpose::DynamicMemorySummarizer => {
                Self::DynamicMemorySummarizer
            }
            lettuce_context::PromptPurpose::DynamicMemoryManager => Self::DynamicMemoryManager,
            lettuce_context::PromptPurpose::ReplyHelperRoleplay => Self::ReplyHelperRoleplay,
            lettuce_context::PromptPurpose::ReplyHelperConversational => {
                Self::ReplyHelperConversational
            }
            lettuce_context::PromptPurpose::LorebookEntryWriter => Self::LorebookEntryWriter,
            lettuce_context::PromptPurpose::LorebookKeywordGenerator => {
                Self::LorebookKeywordGenerator
            }
            lettuce_context::PromptPurpose::LorebookGeneratorPlanner => {
                Self::LorebookGeneratorPlanner
            }
            lettuce_context::PromptPurpose::LorebookGeneratorWriter => {
                Self::LorebookGeneratorWriter
            }
            lettuce_context::PromptPurpose::LorebookGeneratorRefine => {
                Self::LorebookGeneratorRefine
            }
            lettuce_context::PromptPurpose::LorebookGeneratorCoherence => {
                Self::LorebookGeneratorCoherence
            }
            lettuce_context::PromptPurpose::AvatarGeneration => Self::AvatarGeneration,
            lettuce_context::PromptPurpose::AvatarEditRequest => Self::AvatarEditRequest,
            lettuce_context::PromptPurpose::SceneGeneration => Self::SceneGeneration,
            lettuce_context::PromptPurpose::ScenePromptWriter => Self::ScenePromptWriter,
            lettuce_context::PromptPurpose::DesignReferenceWriter => Self::DesignReferenceWriter,
            lettuce_context::PromptPurpose::CompanionSoulWriter => Self::CompanionSoulWriter,
            lettuce_context::PromptPurpose::CompanionGrowthcycle => Self::CompanionGrowthcycle,
            lettuce_context::PromptPurpose::CompanionConsolidation => Self::CompanionConsolidation,
            lettuce_context::PromptPurpose::RuntimeText => Self::RuntimeText,
        })
    }
}

impl From<PromptPurpose> for lettuce_context::PromptPurpose {
    fn from(value: PromptPurpose) -> Self {
        match value {
            PromptPurpose::DirectChat => Self::DirectChat,
            PromptPurpose::CompanionChat => Self::CompanionChat,
            PromptPurpose::GroupChatRoleplay => Self::GroupChatRoleplay,
            PromptPurpose::GroupChatConversational => Self::GroupChatConversational,
            PromptPurpose::DynamicMemorySummarizer => Self::DynamicMemorySummarizer,
            PromptPurpose::DynamicMemoryManager => Self::DynamicMemoryManager,
            PromptPurpose::ReplyHelperRoleplay => Self::ReplyHelperRoleplay,
            PromptPurpose::ReplyHelperConversational => Self::ReplyHelperConversational,
            PromptPurpose::LorebookEntryWriter => Self::LorebookEntryWriter,
            PromptPurpose::LorebookKeywordGenerator => Self::LorebookKeywordGenerator,
            PromptPurpose::LorebookGeneratorPlanner => Self::LorebookGeneratorPlanner,
            PromptPurpose::LorebookGeneratorWriter => Self::LorebookGeneratorWriter,
            PromptPurpose::LorebookGeneratorRefine => Self::LorebookGeneratorRefine,
            PromptPurpose::LorebookGeneratorCoherence => Self::LorebookGeneratorCoherence,
            PromptPurpose::AvatarGeneration => Self::AvatarGeneration,
            PromptPurpose::AvatarEditRequest => Self::AvatarEditRequest,
            PromptPurpose::SceneGeneration => Self::SceneGeneration,
            PromptPurpose::ScenePromptWriter => Self::ScenePromptWriter,
            PromptPurpose::DesignReferenceWriter => Self::DesignReferenceWriter,
            PromptPurpose::CompanionSoulWriter => Self::CompanionSoulWriter,
            PromptPurpose::CompanionGrowthcycle => Self::CompanionGrowthcycle,
            PromptPurpose::CompanionConsolidation => Self::CompanionConsolidation,
            PromptPurpose::RuntimeText => Self::RuntimeText,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptPosition {
    Relative,
    InChat,
    Conditional,
    Interval,
}

impl From<lettuce_context::PromptEntryPosition> for PromptPosition {
    fn from(value: lettuce_context::PromptEntryPosition) -> Self {
        match value {
            lettuce_context::PromptEntryPosition::Relative => Self::Relative,
            lettuce_context::PromptEntryPosition::InChat => Self::InChat,
            lettuce_context::PromptEntryPosition::Conditional => Self::Conditional,
            lettuce_context::PromptEntryPosition::Interval => Self::Interval,
        }
    }
}

impl From<PromptPosition> for lettuce_context::PromptEntryPosition {
    fn from(value: PromptPosition) -> Self {
        match value {
            PromptPosition::Relative => Self::Relative,
            PromptPosition::InChat => Self::InChat,
            PromptPosition::Conditional => Self::Conditional,
            PromptPosition::Interval => Self::Interval,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Prompt {
    pub name: String,
    pub purpose: PromptPurpose,
    pub condense: bool,
    pub behavior_version: super::BehaviorVersion,
    pub built_in: Option<String>,
    pub entries: Vec<PromptEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptEntry {
    pub name: String,
    pub role: PromptEntryRole,
    pub content: String,
    pub enabled: bool,
    pub position: PromptPosition,
    pub depth: u32,
    pub conditional_min_messages: Option<u32>,
    pub interval_turns: Option<u32>,
    pub system_prompt: bool,
    pub conditions: Option<PromptEntryCondition>,
    pub payload: Option<PromptEntryPayload>,
}

impl From<&lettuce_context::PromptEntry> for PromptEntry {
    fn from(entry: &lettuce_context::PromptEntry) -> Self {
        Self {
            name: entry.name.clone(),
            role: entry.role,
            content: entry.content.clone(),
            enabled: entry.enabled,
            position: entry.injection_position.into(),
            depth: entry.depth,
            conditional_min_messages: entry.conditional_min_messages,
            interval_turns: entry.interval_turns,
            system_prompt: entry.system_prompt,
            conditions: entry.conditions.clone().map(Into::into),
            payload: entry.payload.clone().map(Into::into),
        }
    }
}

impl Prompt {
    pub fn from_document(
        document: &PromptDocument,
        built_in: Option<String>,
    ) -> Result<Self, super::Error> {
        Ok(Self {
            name: document.name.clone(),
            purpose: document.purpose.try_into()?,
            condense: document.condense,
            behavior_version: match document.behavior_version {
                lettuce_context::PromptBehaviorVersion::LegacyV1 => {
                    super::BehaviorVersion::LegacyV1
                }
                lettuce_context::PromptBehaviorVersion::DeterministicV2 => {
                    super::BehaviorVersion::DeterministicV2
                }
            },
            built_in,
            entries: document.entries.iter().map(Into::into).collect(),
        })
    }
}

impl Prompt {
    pub fn into_document(
        self,
        source: Option<&PromptDocument>,
        now: lettuce_types::TimestampMillis,
    ) -> Result<PromptDocument, super::Error> {
        let document = PromptDocument {
            id: lettuce_types::PromptDocumentId::new(),
            status: lettuce_context::LifecycleStatus::Active,
            name: self.name,
            purpose: self.purpose.into(),
            condense: self.condense,
            behavior_version: match self.behavior_version {
                super::BehaviorVersion::LegacyV1 => {
                    lettuce_context::PromptBehaviorVersion::LegacyV1
                }
                super::BehaviorVersion::DeterministicV2 => {
                    lettuce_context::PromptBehaviorVersion::DeterministicV2
                }
            },
            provenance: source.map_or(lettuce_context::PromptProvenance::User, |source| {
                lettuce_context::PromptProvenance::Derived {
                    source: source.id,
                    source_name: source.name.clone(),
                }
            }),
            entries: self
                .entries
                .into_iter()
                .map(|entry| lettuce_context::PromptEntry {
                    id: lettuce_types::PromptEntryId::new(),
                    built_in_entry_key: None,
                    name: entry.name,
                    role: entry.role,
                    content: entry.content,
                    enabled: entry.enabled,
                    injection_position: entry.position.into(),
                    depth: entry.depth,
                    conditional_min_messages: entry.conditional_min_messages,
                    interval_turns: entry.interval_turns,
                    system_prompt: entry.system_prompt,
                    conditions: entry.conditions.map(Into::into),
                    payload: entry.payload.map(Into::into),
                })
                .collect(),
            revision: lettuce_types::Revision::INITIAL,
            created_at: now,
            updated_at: now,
        };
        document.validate().map_err(super::format)?;
        Ok(document)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PromptEntryCondition {
    ChatMode { value: PromptEntryChatMode },
    InfoSource { value: PromptEntryInfoSource },
    SceneGenerationEnabled { value: bool },
    AvatarGenerationEnabled { value: bool },
    IsLocalImageGenerationModel { value: bool },
    IsSceneGenerationLocalImageModel { value: bool },
    SceneImageProtocol { value: SceneImageProtocolKind },
    HasScene { value: bool },
    HasSceneDirection { value: bool },
    HasPersona { value: bool },
    MessageCountAtLeast { value: u32 },
    ParticipantCountAtLeast { value: u32 },
    KeywordAny { values: Vec<String> },
    KeywordAll { values: Vec<String> },
    KeywordNone { values: Vec<String> },
    DynamicMemoryEnabled { value: bool },
    HasMemorySummary { value: bool },
    HasKeyMemories { value: bool },
    HasLorebookContent { value: bool },
    DoesAuthorNoteExists { value: bool },
    HasActiveScheduledNote { value: bool },
    HasSubjectDescription { value: bool },
    HasCurrentDescription { value: bool },
    HasCharacterReferenceImages { value: bool },
    HasChatBackground { value: bool },
    HasPersonaReferenceImages { value: bool },
    HasCharacterReferenceText { value: bool },
    HasPersonaReferenceText { value: bool },
    InputScopeAny { values: Vec<String> },
    OutputScopeAny { values: Vec<String> },
    ProviderIdAny { values: Vec<String> },
    ReasoningEnabled { value: bool },
    VisionEnabled { value: bool },
    IsTimeAwarenessEnabled { value: bool },
    IsCompanionMode { value: bool },
    All { conditions: Vec<Self> },
    Any { conditions: Vec<Self> },
    Not { condition: Box<Self> },
}

impl From<lettuce_context::PromptEntryCondition> for PromptEntryCondition {
    fn from(value: lettuce_context::PromptEntryCondition) -> Self {
        match value {
            lettuce_context::PromptEntryCondition::ChatMode { value } => Self::ChatMode { value },
            lettuce_context::PromptEntryCondition::InfoSource { value } => {
                Self::InfoSource { value }
            }
            lettuce_context::PromptEntryCondition::SceneGenerationEnabled { value } => {
                Self::SceneGenerationEnabled { value }
            }
            lettuce_context::PromptEntryCondition::AvatarGenerationEnabled { value } => {
                Self::AvatarGenerationEnabled { value }
            }
            lettuce_context::PromptEntryCondition::IsLocalImageGenerationModel { value } => {
                Self::IsLocalImageGenerationModel { value }
            }
            lettuce_context::PromptEntryCondition::IsSceneGenerationLocalImageModel { value } => {
                Self::IsSceneGenerationLocalImageModel { value }
            }
            lettuce_context::PromptEntryCondition::SceneImageProtocol { value } => {
                Self::SceneImageProtocol { value }
            }
            lettuce_context::PromptEntryCondition::HasScene { value } => Self::HasScene { value },
            lettuce_context::PromptEntryCondition::HasSceneDirection { value } => {
                Self::HasSceneDirection { value }
            }
            lettuce_context::PromptEntryCondition::HasPersona { value } => {
                Self::HasPersona { value }
            }
            lettuce_context::PromptEntryCondition::MessageCountAtLeast { value } => {
                Self::MessageCountAtLeast { value }
            }
            lettuce_context::PromptEntryCondition::ParticipantCountAtLeast { value } => {
                Self::ParticipantCountAtLeast { value }
            }
            lettuce_context::PromptEntryCondition::KeywordAny { values } => {
                Self::KeywordAny { values }
            }
            lettuce_context::PromptEntryCondition::KeywordAll { values } => {
                Self::KeywordAll { values }
            }
            lettuce_context::PromptEntryCondition::KeywordNone { values } => {
                Self::KeywordNone { values }
            }
            lettuce_context::PromptEntryCondition::DynamicMemoryEnabled { value } => {
                Self::DynamicMemoryEnabled { value }
            }
            lettuce_context::PromptEntryCondition::HasMemorySummary { value } => {
                Self::HasMemorySummary { value }
            }
            lettuce_context::PromptEntryCondition::HasKeyMemories { value } => {
                Self::HasKeyMemories { value }
            }
            lettuce_context::PromptEntryCondition::HasLorebookContent { value } => {
                Self::HasLorebookContent { value }
            }
            lettuce_context::PromptEntryCondition::DoesAuthorNoteExists { value } => {
                Self::DoesAuthorNoteExists { value }
            }
            lettuce_context::PromptEntryCondition::HasActiveScheduledNote { value } => {
                Self::HasActiveScheduledNote { value }
            }
            lettuce_context::PromptEntryCondition::HasSubjectDescription { value } => {
                Self::HasSubjectDescription { value }
            }
            lettuce_context::PromptEntryCondition::HasCurrentDescription { value } => {
                Self::HasCurrentDescription { value }
            }
            lettuce_context::PromptEntryCondition::HasCharacterReferenceImages { value } => {
                Self::HasCharacterReferenceImages { value }
            }
            lettuce_context::PromptEntryCondition::HasChatBackground { value } => {
                Self::HasChatBackground { value }
            }
            lettuce_context::PromptEntryCondition::HasPersonaReferenceImages { value } => {
                Self::HasPersonaReferenceImages { value }
            }
            lettuce_context::PromptEntryCondition::HasCharacterReferenceText { value } => {
                Self::HasCharacterReferenceText { value }
            }
            lettuce_context::PromptEntryCondition::HasPersonaReferenceText { value } => {
                Self::HasPersonaReferenceText { value }
            }
            lettuce_context::PromptEntryCondition::InputScopeAny { values } => {
                Self::InputScopeAny { values }
            }
            lettuce_context::PromptEntryCondition::OutputScopeAny { values } => {
                Self::OutputScopeAny { values }
            }
            lettuce_context::PromptEntryCondition::ProviderIdAny { values } => {
                Self::ProviderIdAny { values }
            }
            lettuce_context::PromptEntryCondition::ReasoningEnabled { value } => {
                Self::ReasoningEnabled { value }
            }
            lettuce_context::PromptEntryCondition::VisionEnabled { value } => {
                Self::VisionEnabled { value }
            }
            lettuce_context::PromptEntryCondition::IsTimeAwarenessEnabled { value } => {
                Self::IsTimeAwarenessEnabled { value }
            }
            lettuce_context::PromptEntryCondition::IsCompanionMode { value } => {
                Self::IsCompanionMode { value }
            }
            lettuce_context::PromptEntryCondition::All { conditions } => Self::All {
                conditions: conditions.into_iter().map(Into::into).collect(),
            },
            lettuce_context::PromptEntryCondition::Any { conditions } => Self::Any {
                conditions: conditions.into_iter().map(Into::into).collect(),
            },
            lettuce_context::PromptEntryCondition::Not { condition } => Self::Not {
                condition: Box::new((*condition).into()),
            },
        }
    }
}

impl From<PromptEntryCondition> for lettuce_context::PromptEntryCondition {
    fn from(value: PromptEntryCondition) -> Self {
        match value {
            PromptEntryCondition::ChatMode { value } => Self::ChatMode { value },
            PromptEntryCondition::InfoSource { value } => Self::InfoSource { value },
            PromptEntryCondition::SceneGenerationEnabled { value } => {
                Self::SceneGenerationEnabled { value }
            }
            PromptEntryCondition::AvatarGenerationEnabled { value } => {
                Self::AvatarGenerationEnabled { value }
            }
            PromptEntryCondition::IsLocalImageGenerationModel { value } => {
                Self::IsLocalImageGenerationModel { value }
            }
            PromptEntryCondition::IsSceneGenerationLocalImageModel { value } => {
                Self::IsSceneGenerationLocalImageModel { value }
            }
            PromptEntryCondition::SceneImageProtocol { value } => {
                Self::SceneImageProtocol { value }
            }
            PromptEntryCondition::HasScene { value } => Self::HasScene { value },
            PromptEntryCondition::HasSceneDirection { value } => Self::HasSceneDirection { value },
            PromptEntryCondition::HasPersona { value } => Self::HasPersona { value },
            PromptEntryCondition::MessageCountAtLeast { value } => {
                Self::MessageCountAtLeast { value }
            }
            PromptEntryCondition::ParticipantCountAtLeast { value } => {
                Self::ParticipantCountAtLeast { value }
            }
            PromptEntryCondition::KeywordAny { values } => Self::KeywordAny { values },
            PromptEntryCondition::KeywordAll { values } => Self::KeywordAll { values },
            PromptEntryCondition::KeywordNone { values } => Self::KeywordNone { values },
            PromptEntryCondition::DynamicMemoryEnabled { value } => {
                Self::DynamicMemoryEnabled { value }
            }
            PromptEntryCondition::HasMemorySummary { value } => Self::HasMemorySummary { value },
            PromptEntryCondition::HasKeyMemories { value } => Self::HasKeyMemories { value },
            PromptEntryCondition::HasLorebookContent { value } => {
                Self::HasLorebookContent { value }
            }
            PromptEntryCondition::DoesAuthorNoteExists { value } => {
                Self::DoesAuthorNoteExists { value }
            }
            PromptEntryCondition::HasActiveScheduledNote { value } => {
                Self::HasActiveScheduledNote { value }
            }
            PromptEntryCondition::HasSubjectDescription { value } => {
                Self::HasSubjectDescription { value }
            }
            PromptEntryCondition::HasCurrentDescription { value } => {
                Self::HasCurrentDescription { value }
            }
            PromptEntryCondition::HasCharacterReferenceImages { value } => {
                Self::HasCharacterReferenceImages { value }
            }
            PromptEntryCondition::HasChatBackground { value } => Self::HasChatBackground { value },
            PromptEntryCondition::HasPersonaReferenceImages { value } => {
                Self::HasPersonaReferenceImages { value }
            }
            PromptEntryCondition::HasCharacterReferenceText { value } => {
                Self::HasCharacterReferenceText { value }
            }
            PromptEntryCondition::HasPersonaReferenceText { value } => {
                Self::HasPersonaReferenceText { value }
            }
            PromptEntryCondition::InputScopeAny { values } => Self::InputScopeAny { values },
            PromptEntryCondition::OutputScopeAny { values } => Self::OutputScopeAny { values },
            PromptEntryCondition::ProviderIdAny { values } => Self::ProviderIdAny { values },
            PromptEntryCondition::ReasoningEnabled { value } => Self::ReasoningEnabled { value },
            PromptEntryCondition::VisionEnabled { value } => Self::VisionEnabled { value },
            PromptEntryCondition::IsTimeAwarenessEnabled { value } => {
                Self::IsTimeAwarenessEnabled { value }
            }
            PromptEntryCondition::IsCompanionMode { value } => Self::IsCompanionMode { value },
            PromptEntryCondition::All { conditions } => Self::All {
                conditions: conditions.into_iter().map(Into::into).collect(),
            },
            PromptEntryCondition::Any { conditions } => Self::Any {
                conditions: conditions.into_iter().map(Into::into).collect(),
            },
            PromptEntryCondition::Not { condition } => Self::Not {
                condition: Box::new((*condition).into()),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PromptEntryPayload {
    ImageSlot { slot: PromptEntryImageSlot },
}

impl From<lettuce_context::PromptEntryPayload> for PromptEntryPayload {
    fn from(value: lettuce_context::PromptEntryPayload) -> Self {
        match value {
            lettuce_context::PromptEntryPayload::ImageSlot { slot } => Self::ImageSlot { slot },
        }
    }
}

impl From<PromptEntryPayload> for lettuce_context::PromptEntryPayload {
    fn from(value: PromptEntryPayload) -> Self {
        match value {
            PromptEntryPayload::ImageSlot { slot } => Self::ImageSlot { slot },
        }
    }
}
