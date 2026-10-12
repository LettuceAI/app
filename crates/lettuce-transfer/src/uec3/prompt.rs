use lettuce_context::{PromptDocument, PromptEntryCondition, PromptEntryPayload, PromptEntryRole};
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
            conditions: entry.conditions.clone(),
            payload: entry.payload.clone(),
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
                    conditions: entry.conditions,
                    payload: entry.payload,
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
