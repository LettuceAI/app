//! Structured prompt programs and their pure renderer.

use lettuce_types::{PromptDocumentId, PromptEntryId, Revision, TimestampMillis};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::lorebook::keyword_matches;

pub const MAX_LABEL_SCALARS: usize = 256;
pub const MAX_LABEL_BYTES: usize = 1024;
pub const MAX_PROSE_BYTES: usize = 1024 * 1024;
pub const MAX_AUTHORED_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_PROMPT_ENTRIES: usize = 512;
pub const MAX_CHILDREN: usize = 10_000;
pub const MAX_CONDITION_DEPTH: usize = 16;
pub const MAX_CONDITION_NODES: usize = 256;
const MAX_RENDER_VALUE_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum LifecycleStatus {
    #[default]
    Active,
    Archived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum PromptBehaviorVersion {
    #[default]
    LegacyV1,
    DeterministicV2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum PromptPurpose {
    #[default]
    Undefined,
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum PromptProvenance {
    BuiltIn { key: String, seed_version: u32 },
    User,
    Derived { source: PromptDocumentId },
    Imported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum PromptEntryRole {
    #[default]
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum PromptEntryPosition {
    #[default]
    Relative,
    InChat,
    Conditional,
    Interval,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PromptEntryImageSlot {
    Character,
    Persona,
    ChatBackground,
    Avatar,
    References,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum PromptEntryPayload {
    ImageSlot { slot: PromptEntryImageSlot },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum PromptEntryChatMode {
    #[default]
    Direct,
    Group,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum PromptEntryInfoSource {
    #[default]
    Messages,
    Memory,
    Mixed,
}

/// The closed condition vocabulary used by legacy prompt entries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum PromptEntryCondition {
    ChatMode { value: PromptEntryChatMode },
    InfoSource { value: PromptEntryInfoSource },
    SceneGenerationEnabled { value: bool },
    AvatarGenerationEnabled { value: bool },
    IsLocalImageGenerationModel { value: bool },
    IsSceneGenerationLocalImageModel { value: bool },
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromptEntry {
    pub id: PromptEntryId,
    pub name: String,
    pub role: PromptEntryRole,
    pub content: String,
    pub enabled: bool,
    pub injection_position: PromptEntryPosition,
    pub depth: u32,
    pub conditional_min_messages: Option<u32>,
    pub interval_turns: Option<u32>,
    /// Retained because the legacy runtime executes disabled system entries.
    pub system_prompt: bool,
    pub conditions: Option<PromptEntryCondition>,
    pub payload: Option<PromptEntryPayload>,
}

impl Default for PromptEntry {
    fn default() -> Self {
        Self {
            id: PromptEntryId::new(),
            name: String::new(),
            role: PromptEntryRole::System,
            content: String::new(),
            enabled: true,
            injection_position: PromptEntryPosition::Relative,
            depth: 0,
            conditional_min_messages: None,
            interval_turns: None,
            system_prompt: false,
            conditions: None,
            payload: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromptDocument {
    pub id: PromptDocumentId,
    pub status: LifecycleStatus,
    pub name: String,
    pub purpose: PromptPurpose,
    /// The structured entries are the sole operational authority. Legacy text
    /// is converted by transfer adapters and is not a second live field.
    pub entries: Vec<PromptEntry>,
    pub condense: bool,
    pub behavior_version: PromptBehaviorVersion,
    pub provenance: PromptProvenance,
    pub revision: Revision,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

/// Name used by context assembly plans for the structured operational form.
pub type PromptProgram = PromptDocument;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PromptValidationError {
    #[error("{field} must not be blank")]
    Blank { field: &'static str },
    #[error("{field} exceeds the 256 Unicode scalar limit")]
    LabelTooLong { field: &'static str },
    #[error("{field} exceeds the 1 KiB encoded limit")]
    LabelTooLarge { field: &'static str },
    #[error("{field} exceeds the 1 MiB limit")]
    ProseTooLarge { field: &'static str },
    #[error("prompt has too many entries")]
    TooManyEntries,
    #[error("prompt contains duplicate entry id {0}")]
    DuplicateEntry(PromptEntryId),
    #[error("prompt authored payload exceeds 8 MiB")]
    AuthoredPayloadTooLarge,
    #[error("condition AST exceeds depth {0}")]
    ConditionTooDeep(usize),
    #[error("condition AST exceeds {0} nodes")]
    TooManyConditionNodes(usize),
    #[error("interval injection requires a non-zero interval")]
    InvalidInterval,
    #[error("condition values exceed the ordered collection limit")]
    TooManyConditionValues,
    #[error("condition value exceeds the 1 KiB limit")]
    ConditionValueTooLarge,
    #[error("built-in prompt provenance requires a non-blank key")]
    InvalidBuiltInKey,
    #[error("derived prompt provenance requires a source different from the document")]
    InvalidDerivedSource,
    #[error("prompt revision must be at least one")]
    ZeroRevision,
    #[error("prompt condition match context exceeds the 1 MiB limit")]
    MatchContextTooLarge,
    #[error("prompt could not be serialized for validation")]
    Serialization,
}

pub(crate) fn validate_label(
    value: &str,
    field: &'static str,
) -> Result<(), PromptValidationError> {
    if value.trim().is_empty() {
        return Err(PromptValidationError::Blank { field });
    }
    if value.chars().count() > MAX_LABEL_SCALARS {
        return Err(PromptValidationError::LabelTooLong { field });
    }
    if value.len() > MAX_LABEL_BYTES {
        return Err(PromptValidationError::LabelTooLarge { field });
    }
    Ok(())
}

pub(crate) fn validate_prose(
    value: &str,
    field: &'static str,
) -> Result<(), PromptValidationError> {
    if value.len() > MAX_PROSE_BYTES {
        Err(PromptValidationError::ProseTooLarge { field })
    } else {
        Ok(())
    }
}

impl PromptEntryCondition {
    pub fn validate(&self) -> Result<(), PromptValidationError> {
        let (depth, nodes) = self.shape(1)?;
        if depth > MAX_CONDITION_DEPTH {
            return Err(PromptValidationError::ConditionTooDeep(MAX_CONDITION_DEPTH));
        }
        if nodes > MAX_CONDITION_NODES {
            return Err(PromptValidationError::TooManyConditionNodes(
                MAX_CONDITION_NODES,
            ));
        }
        Ok(())
    }

    fn shape(&self, depth: usize) -> Result<(usize, usize), PromptValidationError> {
        let list = |values: &[String]| -> Result<(), PromptValidationError> {
            if values.len() > MAX_CHILDREN {
                return Err(PromptValidationError::TooManyConditionValues);
            }
            if values.iter().any(|value| value.len() > MAX_LABEL_BYTES) {
                return Err(PromptValidationError::ConditionValueTooLarge);
            }
            Ok(())
        };
        let mut max_depth = depth;
        let mut nodes = 1;
        match self {
            Self::KeywordAny { values }
            | Self::KeywordAll { values }
            | Self::KeywordNone { values }
            | Self::InputScopeAny { values }
            | Self::OutputScopeAny { values }
            | Self::ProviderIdAny { values } => list(values)?,
            Self::All { conditions } | Self::Any { conditions } => {
                if conditions.len() > MAX_CHILDREN {
                    return Err(PromptValidationError::TooManyConditionValues);
                }
                for condition in conditions {
                    let (child_depth, child_nodes) = condition.shape(depth + 1)?;
                    max_depth = max_depth.max(child_depth);
                    nodes += child_nodes;
                }
            }
            Self::Not { condition } => {
                let (child_depth, child_nodes) = condition.shape(depth + 1)?;
                max_depth = max_depth.max(child_depth);
                nodes += child_nodes;
            }
            _ => {}
        }
        Ok((max_depth, nodes))
    }
}

impl PromptEntry {
    pub fn validate(&self) -> Result<(), PromptValidationError> {
        validate_label(&self.name, "entry name")?;
        validate_prose(&self.content, "entry content")?;
        if self.injection_position == PromptEntryPosition::Interval
            && self.interval_turns.unwrap_or_default() == 0
        {
            return Err(PromptValidationError::InvalidInterval);
        }
        if let Some(condition) = &self.conditions {
            condition.validate()?;
        }
        Ok(())
    }
}

impl PromptDocument {
    pub fn validate(&self) -> Result<(), PromptValidationError> {
        validate_label(&self.name, "prompt name")?;
        if self.revision.get() == 0 {
            return Err(PromptValidationError::ZeroRevision);
        }
        if self.entries.len() > MAX_PROMPT_ENTRIES {
            return Err(PromptValidationError::TooManyEntries);
        }
        let mut ids = std::collections::HashSet::with_capacity(self.entries.len());
        for entry in &self.entries {
            if !ids.insert(entry.id) {
                return Err(PromptValidationError::DuplicateEntry(entry.id));
            }
            entry.validate()?;
        }
        match &self.provenance {
            PromptProvenance::BuiltIn { key, .. } if key.trim().is_empty() => {
                return Err(PromptValidationError::InvalidBuiltInKey);
            }
            PromptProvenance::Derived { source } if *source == self.id => {
                return Err(PromptValidationError::InvalidDerivedSource);
            }
            _ => {}
        }
        let encoded = serde_json::to_vec(self).map_err(|_| PromptValidationError::Serialization)?;
        if encoded.len() > MAX_AUTHORED_BYTES {
            return Err(PromptValidationError::AuthoredPayloadTooLarge);
        }
        Ok(())
    }
}

/// Immutable facts used to evaluate all condition variants. The fields mirror
/// the old runtime deliberately, including the companion-only scheduled-note
/// rule and the legacy keyword matcher.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PromptConditionContext {
    pub chat_mode: PromptEntryChatMode,
    pub info_source: PromptEntryInfoSource,
    pub scene_generation_enabled: bool,
    pub avatar_generation_enabled: bool,
    pub is_local_image_generation_model: bool,
    pub is_scene_generation_local_image_model: bool,
    pub has_scene: bool,
    pub has_scene_direction: bool,
    pub has_persona: bool,
    pub message_count: usize,
    pub participant_count: usize,
    pub recent_text: String,
    pub dynamic_memory_enabled: bool,
    pub has_memory_summary: bool,
    pub has_key_memories: bool,
    pub has_lorebook_content: bool,
    pub does_author_note_exists: bool,
    pub has_active_scheduled_note: bool,
    pub has_subject_description: bool,
    pub has_current_description: bool,
    pub has_character_reference_images: bool,
    pub has_chat_background: bool,
    pub has_persona_reference_images: bool,
    pub has_character_reference_text: bool,
    pub has_persona_reference_text: bool,
    pub input_scopes: Vec<String>,
    pub output_scopes: Vec<String>,
    pub provider_id: Option<String>,
    pub reasoning_enabled: bool,
    pub vision_enabled: bool,
    pub time_awareness_enabled: bool,
    pub companion_mode_enabled: bool,
}

pub fn matches_condition(
    condition: &PromptEntryCondition,
    context: &PromptConditionContext,
) -> bool {
    match condition {
        PromptEntryCondition::ChatMode { value } => *value == context.chat_mode,
        PromptEntryCondition::InfoSource { value } => *value == context.info_source,
        PromptEntryCondition::SceneGenerationEnabled { value } => {
            *value == context.scene_generation_enabled
        }
        PromptEntryCondition::AvatarGenerationEnabled { value } => {
            *value == context.avatar_generation_enabled
        }
        PromptEntryCondition::IsLocalImageGenerationModel { value } => {
            *value == context.is_local_image_generation_model
        }
        PromptEntryCondition::IsSceneGenerationLocalImageModel { value } => {
            *value == context.is_scene_generation_local_image_model
        }
        PromptEntryCondition::HasScene { value } => *value == context.has_scene,
        PromptEntryCondition::HasSceneDirection { value } => *value == context.has_scene_direction,
        PromptEntryCondition::HasPersona { value } => *value == context.has_persona,
        PromptEntryCondition::MessageCountAtLeast { value } => {
            context.message_count >= *value as usize
        }
        PromptEntryCondition::ParticipantCountAtLeast { value } => {
            context.participant_count >= *value as usize
        }
        PromptEntryCondition::KeywordAny { values } => values
            .iter()
            .any(|value| keyword_matches(value, &context.recent_text, false).unwrap_or(false)),
        PromptEntryCondition::KeywordAll { values } => {
            let values = values
                .iter()
                .map(String::as_str)
                .filter(|v| !v.trim().is_empty());
            let collected: Vec<_> = values.collect();
            !collected.is_empty()
                && collected.iter().all(|value| {
                    keyword_matches(value, &context.recent_text, false).unwrap_or(false)
                })
        }
        PromptEntryCondition::KeywordNone { values } => !values
            .iter()
            .any(|value| keyword_matches(value, &context.recent_text, false).unwrap_or(false)),
        PromptEntryCondition::DynamicMemoryEnabled { value } => {
            *value == context.dynamic_memory_enabled
        }
        PromptEntryCondition::HasMemorySummary { value } => *value == context.has_memory_summary,
        PromptEntryCondition::HasKeyMemories { value } => *value == context.has_key_memories,
        PromptEntryCondition::HasLorebookContent { value } => {
            *value == context.has_lorebook_content
        }
        PromptEntryCondition::DoesAuthorNoteExists { value } => {
            *value == context.does_author_note_exists
        }
        PromptEntryCondition::HasActiveScheduledNote { value } => {
            context.companion_mode_enabled && *value == context.has_active_scheduled_note
        }
        PromptEntryCondition::HasSubjectDescription { value } => {
            *value == context.has_subject_description
        }
        PromptEntryCondition::HasCurrentDescription { value } => {
            *value == context.has_current_description
        }
        PromptEntryCondition::HasCharacterReferenceImages { value } => {
            *value == context.has_character_reference_images
        }
        PromptEntryCondition::HasChatBackground { value } => *value == context.has_chat_background,
        PromptEntryCondition::HasPersonaReferenceImages { value } => {
            *value == context.has_persona_reference_images
        }
        PromptEntryCondition::HasCharacterReferenceText { value } => {
            *value == context.has_character_reference_text
        }
        PromptEntryCondition::HasPersonaReferenceText { value } => {
            *value == context.has_persona_reference_text
        }
        PromptEntryCondition::InputScopeAny { values } => {
            scope_matches(values, &context.input_scopes)
        }
        PromptEntryCondition::OutputScopeAny { values } => {
            scope_matches(values, &context.output_scopes)
        }
        PromptEntryCondition::ProviderIdAny { values } => {
            context.provider_id.as_ref().is_some_and(|id| {
                values
                    .iter()
                    .any(|value| !value.trim().is_empty() && value.eq_ignore_ascii_case(id.trim()))
            })
        }
        PromptEntryCondition::ReasoningEnabled { value } => *value == context.reasoning_enabled,
        PromptEntryCondition::VisionEnabled { value } => *value == context.vision_enabled,
        PromptEntryCondition::IsTimeAwarenessEnabled { value } => {
            *value == context.time_awareness_enabled
        }
        PromptEntryCondition::IsCompanionMode { value } => *value == context.companion_mode_enabled,
        PromptEntryCondition::All { conditions } => conditions
            .iter()
            .all(|item| matches_condition(item, context)),
        PromptEntryCondition::Any { conditions } => {
            !conditions.is_empty()
                && conditions
                    .iter()
                    .any(|item| matches_condition(item, context))
        }
        PromptEntryCondition::Not { condition } => !matches_condition(condition, context),
    }
}

fn scope_matches(values: &[String], scopes: &[String]) -> bool {
    values.iter().any(|value| {
        let wanted = value.trim();
        !wanted.is_empty()
            && scopes
                .iter()
                .any(|scope| scope.trim().eq_ignore_ascii_case(wanted))
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PromptRenderValues {
    pub character_name: String,
    pub character_description: String,
    pub persona_name: String,
    pub persona_description: String,
    pub scene: String,
    pub scene_direction: String,
    pub lorebook: String,
    pub author_note: String,
    pub context_summary: String,
    pub key_memories: String,
    pub content_rules: String,
    pub user_name: String,
    pub user_description: String,
    pub ai_name: String,
    pub ai_description: String,
    /// Purpose-specific values use a closed vocabulary; callers cannot inject
    /// arbitrary placeholder names into the renderer.
    pub purpose_values: BTreeMap<PromptVariable, String>,
}

/// The complete legacy placeholder vocabulary accepted by this renderer.
/// Values unavailable for a particular purpose deliberately render empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PromptVariable {
    CompanionState,
    ScheduledNotes,
    GroupCharacters,
    PreviousSummary,
    Character,
    MaxEntries,
    CurrentMemoryTokens,
    HotTokenBudget,
    CurrentDraft,
    LorebookName,
    CharacterName,
    SessionTitle,
    SelectedMessages,
    MemorySummary,
    SelectedMemories,
    DirectionPrompt,
    ExistingEntries,
    EntryTitle,
    EntryContent,
    ExistingKeywords,
    Brief,
    TargetCount,
    SourceExcerpts,
    Outline,
    EntryCategory,
    EntryProposedKeys,
    EntryRationale,
    RelevantExcerpts,
    EntryKeywords,
    EntryAlwaysActive,
    UserFeedback,
    DraftedEntries,
    AvatarSubjectName,
    AvatarSubjectDescription,
    AvatarRequest,
    CurrentAvatarPrompt,
    EditRequest,
    RecentMessages,
    SceneRequest,
    SubjectName,
    SubjectDescription,
    CurrentDescription,
    CharacterDefinition,
    CharacterDescription,
    OpeningContext,
    CurrentSoul,
    UserNotes,
    CompanionName,
    ChangeableCategories,
    CurrentGrowth,
    NewMemories,
    AuthoredCore,
    CurrentCore,
    AccumulatedGrowth,
    Date,
    DateFull,
    Weekday,
    TimeHour,
    TimeMinute,
    TimeSecond,
    TimeFull,
    Time12HourFormat,
    TimeTimezone,
    TimeTimezoneName,
    DatetimeIso,
    CharacterReferenceText,
    PersonaReferenceText,
    ChatBackgroundReferenceText,
    CharacterImage,
    PersonaImage,
    ChatBackgroundImage,
    AvatarImage,
    ReferenceImages,
    CharacterLoraKeywords,
    PersonaLoraKeywords,
}

impl PromptVariable {
    const fn placeholder(self) -> &'static str {
        match self {
            Self::CompanionState => "{{companion_state}}",
            Self::ScheduledNotes => "{{scheduled_notes}}",
            Self::GroupCharacters => "{{group_characters}}",
            Self::PreviousSummary => "{{prev_summary}}",
            Self::Character => "{{character}}",
            Self::MaxEntries => "{{max_entries}}",
            Self::CurrentMemoryTokens => "{{current_memory_tokens}}",
            Self::HotTokenBudget => "{{hot_token_budget}}",
            Self::CurrentDraft => "{{current_draft}}",
            Self::LorebookName => "{{lorebook_name}}",
            Self::CharacterName => "{{character_name}}",
            Self::SessionTitle => "{{session_title}}",
            Self::SelectedMessages => "{{selected_messages}}",
            Self::MemorySummary => "{{memory_summary}}",
            Self::SelectedMemories => "{{selected_memories}}",
            Self::DirectionPrompt => "{{direction_prompt}}",
            Self::ExistingEntries => "{{existing_entries}}",
            Self::EntryTitle => "{{entry_title}}",
            Self::EntryContent => "{{entry_content}}",
            Self::ExistingKeywords => "{{existing_keywords}}",
            Self::Brief => "{{brief}}",
            Self::TargetCount => "{{target_count}}",
            Self::SourceExcerpts => "{{source_excerpts}}",
            Self::Outline => "{{outline}}",
            Self::EntryCategory => "{{entry_category}}",
            Self::EntryProposedKeys => "{{entry_proposed_keys}}",
            Self::EntryRationale => "{{entry_rationale}}",
            Self::RelevantExcerpts => "{{relevant_excerpts}}",
            Self::EntryKeywords => "{{entry_keywords}}",
            Self::EntryAlwaysActive => "{{entry_always_active}}",
            Self::UserFeedback => "{{user_feedback}}",
            Self::DraftedEntries => "{{drafted_entries}}",
            Self::AvatarSubjectName => "{{avatar_subject_name}}",
            Self::AvatarSubjectDescription => "{{avatar_subject_description}}",
            Self::AvatarRequest => "{{avatar_request}}",
            Self::CurrentAvatarPrompt => "{{current_avatar_prompt}}",
            Self::EditRequest => "{{edit_request}}",
            Self::RecentMessages => "{{recent_messages}}",
            Self::SceneRequest => "{{scene_request}}",
            Self::SubjectName => "{{subject_name}}",
            Self::SubjectDescription => "{{subject_description}}",
            Self::CurrentDescription => "{{current_description}}",
            Self::CharacterDefinition => "{{char.definition}}",
            Self::CharacterDescription => "{{char.description}}",
            Self::OpeningContext => "{{opening_context}}",
            Self::CurrentSoul => "{{current_soul}}",
            Self::UserNotes => "{{user_notes}}",
            Self::CompanionName => "{{companion.name}}",
            Self::ChangeableCategories => "{{changeable_categories}}",
            Self::CurrentGrowth => "{{current_growth}}",
            Self::NewMemories => "{{new_memories}}",
            Self::AuthoredCore => "{{authored_core}}",
            Self::CurrentCore => "{{current_core}}",
            Self::AccumulatedGrowth => "{{accumulated_growth}}",
            Self::Date => "{{date}}",
            Self::DateFull => "{{date_full}}",
            Self::Weekday => "{{weekday}}",
            Self::TimeHour => "{{time_hour}}",
            Self::TimeMinute => "{{time_minute}}",
            Self::TimeSecond => "{{time_second}}",
            Self::TimeFull => "{{time_full}}",
            Self::Time12HourFormat => "{{time_12hour_format}}",
            Self::TimeTimezone => "{{time_timezone}}",
            Self::TimeTimezoneName => "{{time_timezone_name}}",
            Self::DatetimeIso => "{{datetime_iso}}",
            Self::CharacterReferenceText => "{{reference[character]}}",
            Self::PersonaReferenceText => "{{reference[persona]}}",
            Self::ChatBackgroundReferenceText => "{{reference[chatBackground]}}",
            Self::CharacterImage => "{{image[character]}}",
            Self::PersonaImage => "{{image[persona]}}",
            Self::ChatBackgroundImage => "{{image[chatBackground]}}",
            Self::AvatarImage => "{{image[avatar]}}",
            Self::ReferenceImages => "{{image[references]}}",
            Self::CharacterLoraKeywords => "{{lora_keywords[character]}}",
            Self::PersonaLoraKeywords => "{{lora_keywords[persona]}}",
        }
    }

    const ALL: &[Self] = &[
        Self::CompanionState,
        Self::ScheduledNotes,
        Self::GroupCharacters,
        Self::PreviousSummary,
        Self::Character,
        Self::MaxEntries,
        Self::CurrentMemoryTokens,
        Self::HotTokenBudget,
        Self::CurrentDraft,
        Self::LorebookName,
        Self::CharacterName,
        Self::SessionTitle,
        Self::SelectedMessages,
        Self::MemorySummary,
        Self::SelectedMemories,
        Self::DirectionPrompt,
        Self::ExistingEntries,
        Self::EntryTitle,
        Self::EntryContent,
        Self::ExistingKeywords,
        Self::Brief,
        Self::TargetCount,
        Self::SourceExcerpts,
        Self::Outline,
        Self::EntryCategory,
        Self::EntryProposedKeys,
        Self::EntryRationale,
        Self::RelevantExcerpts,
        Self::EntryKeywords,
        Self::EntryAlwaysActive,
        Self::UserFeedback,
        Self::DraftedEntries,
        Self::AvatarSubjectName,
        Self::AvatarSubjectDescription,
        Self::AvatarRequest,
        Self::CurrentAvatarPrompt,
        Self::EditRequest,
        Self::RecentMessages,
        Self::SceneRequest,
        Self::SubjectName,
        Self::SubjectDescription,
        Self::CurrentDescription,
        Self::CharacterDefinition,
        Self::CharacterDescription,
        Self::OpeningContext,
        Self::CurrentSoul,
        Self::UserNotes,
        Self::CompanionName,
        Self::ChangeableCategories,
        Self::CurrentGrowth,
        Self::NewMemories,
        Self::AuthoredCore,
        Self::CurrentCore,
        Self::AccumulatedGrowth,
        Self::Date,
        Self::DateFull,
        Self::Weekday,
        Self::TimeHour,
        Self::TimeMinute,
        Self::TimeSecond,
        Self::TimeFull,
        Self::Time12HourFormat,
        Self::TimeTimezone,
        Self::TimeTimezoneName,
        Self::DatetimeIso,
        Self::CharacterReferenceText,
        Self::PersonaReferenceText,
        Self::ChatBackgroundReferenceText,
        Self::CharacterImage,
        Self::PersonaImage,
        Self::ChatBackgroundImage,
        Self::AvatarImage,
        Self::ReferenceImages,
        Self::CharacterLoraKeywords,
        Self::PersonaLoraKeywords,
    ];

    /// Mirrors the legacy prompt editor's allowed-variable contract.
    #[must_use]
    pub const fn is_allowed_for(self, purpose: PromptPurpose) -> bool {
        use PromptPurpose as Purpose;
        use PromptVariable as Variable;
        if matches!(purpose, Purpose::Undefined)
            || matches!(
                self,
                Variable::Date
                    | Variable::DateFull
                    | Variable::Weekday
                    | Variable::TimeHour
                    | Variable::TimeMinute
                    | Variable::TimeSecond
                    | Variable::TimeFull
                    | Variable::Time12HourFormat
                    | Variable::TimeTimezone
                    | Variable::TimeTimezoneName
                    | Variable::DatetimeIso
            )
        {
            return true;
        }
        match purpose {
            Purpose::DirectChat | Purpose::CompanionChat => {
                matches!(self, Variable::CompanionState | Variable::ScheduledNotes)
            }
            Purpose::GroupChatRoleplay => false,
            Purpose::GroupChatConversational => matches!(self, Variable::GroupCharacters),
            Purpose::DynamicMemorySummarizer => {
                matches!(self, Variable::PreviousSummary | Variable::Character)
            }
            Purpose::DynamicMemoryManager => matches!(
                self,
                Variable::MaxEntries | Variable::CurrentMemoryTokens | Variable::HotTokenBudget
            ),
            Purpose::ReplyHelperRoleplay | Purpose::ReplyHelperConversational => {
                matches!(self, Variable::CurrentDraft)
            }
            Purpose::LorebookEntryWriter => matches!(
                self,
                Variable::LorebookName
                    | Variable::CharacterName
                    | Variable::SessionTitle
                    | Variable::SelectedMessages
                    | Variable::MemorySummary
                    | Variable::SelectedMemories
                    | Variable::DirectionPrompt
                    | Variable::ExistingEntries
            ),
            Purpose::LorebookKeywordGenerator => matches!(
                self,
                Variable::EntryTitle
                    | Variable::EntryContent
                    | Variable::ExistingKeywords
                    | Variable::DirectionPrompt
            ),
            Purpose::LorebookGeneratorPlanner => matches!(
                self,
                Variable::Brief | Variable::TargetCount | Variable::SourceExcerpts
            ),
            Purpose::LorebookGeneratorWriter => matches!(
                self,
                Variable::Brief
                    | Variable::Outline
                    | Variable::EntryTitle
                    | Variable::EntryCategory
                    | Variable::EntryProposedKeys
                    | Variable::EntryRationale
                    | Variable::RelevantExcerpts
            ),
            Purpose::LorebookGeneratorRefine => matches!(
                self,
                Variable::Brief
                    | Variable::Outline
                    | Variable::EntryTitle
                    | Variable::EntryKeywords
                    | Variable::EntryAlwaysActive
                    | Variable::EntryContent
                    | Variable::UserFeedback
                    | Variable::RelevantExcerpts
            ),
            Purpose::LorebookGeneratorCoherence => matches!(self, Variable::DraftedEntries),
            Purpose::AvatarGeneration => matches!(
                self,
                Variable::AvatarSubjectName
                    | Variable::AvatarSubjectDescription
                    | Variable::AvatarRequest
            ),
            Purpose::AvatarEditRequest => matches!(
                self,
                Variable::AvatarSubjectName
                    | Variable::AvatarSubjectDescription
                    | Variable::CurrentAvatarPrompt
                    | Variable::EditRequest
            ),
            Purpose::SceneGeneration | Purpose::ScenePromptWriter => matches!(
                self,
                Variable::CharacterReferenceText
                    | Variable::PersonaReferenceText
                    | Variable::ChatBackgroundReferenceText
                    | Variable::CharacterImage
                    | Variable::PersonaImage
                    | Variable::ChatBackgroundImage
                    | Variable::RecentMessages
                    | Variable::SceneRequest
                    | Variable::CharacterLoraKeywords
                    | Variable::PersonaLoraKeywords
            ),
            Purpose::DesignReferenceWriter => matches!(
                self,
                Variable::SubjectName
                    | Variable::SubjectDescription
                    | Variable::CurrentDescription
                    | Variable::AvatarImage
                    | Variable::ReferenceImages
            ),
            Purpose::CompanionSoulWriter => matches!(
                self,
                Variable::CharacterDefinition
                    | Variable::CharacterDescription
                    | Variable::OpeningContext
                    | Variable::CurrentSoul
                    | Variable::UserNotes
            ),
            Purpose::CompanionGrowthcycle => matches!(
                self,
                Variable::CompanionName
                    | Variable::ChangeableCategories
                    | Variable::CurrentGrowth
                    | Variable::NewMemories
            ),
            Purpose::CompanionConsolidation => matches!(
                self,
                Variable::CompanionName
                    | Variable::AuthoredCore
                    | Variable::CurrentCore
                    | Variable::AccumulatedGrowth
            ),
            Purpose::Undefined => true,
        }
    }
}

impl PromptRenderValues {
    fn render(&self, source: &str) -> String {
        let mut rendered = source.to_string();
        let replacements = [
            ("{{char.name}}", self.character_name.as_str()),
            ("{{char.desc}}", self.character_description.as_str()),
            ("{{persona.name}}", self.persona_name.as_str()),
            ("{{persona.desc}}", self.persona_description.as_str()),
            ("{{user.name}}", self.user_name.as_str()),
            ("{{user.desc}}", self.user_description.as_str()),
            ("{{ai_name}}", self.ai_name.as_str()),
            ("{{ai_description}}", self.ai_description.as_str()),
            ("{{persona_name}}", self.persona_name.as_str()),
            ("{{persona_description}}", self.persona_description.as_str()),
            ("{{user_name}}", self.user_name.as_str()),
            ("{{user_description}}", self.user_description.as_str()),
            ("{{char}}", self.character_name.as_str()),
            ("{{persona}}", self.persona_name.as_str()),
            ("{{user}}", self.user_name.as_str()),
            ("{{scene}}", self.scene.as_str()),
            ("{{scene_direction}}", self.scene_direction.as_str()),
            ("{{lorebook}}", self.lorebook.as_str()),
            ("{{author_note}}", self.author_note.as_str()),
            ("{{context_summary}}", self.context_summary.as_str()),
            ("{{key_memories}}", self.key_memories.as_str()),
            ("{{content_rules}}", self.content_rules.as_str()),
            // The old runtime intentionally leaves authored character rules empty.
            ("{{rules}}", ""),
            ("{{ai_rules}}", ""),
        ];
        for (placeholder, value) in replacements {
            rendered = rendered.replace(placeholder, value);
        }
        for variable in PromptVariable::ALL {
            rendered = rendered.replace(
                variable.placeholder(),
                self.purpose_values.get(variable).map_or("", String::as_str),
            );
        }
        rendered
    }
}

fn validate_render_values(
    purpose: PromptPurpose,
    values: &PromptRenderValues,
) -> Result<(), PromptRenderError> {
    let mut total = 0_usize;
    for (variable, value) in &values.purpose_values {
        if value.is_empty() {
            continue;
        }
        if !variable.is_allowed_for(purpose) {
            return Err(PromptRenderError::DisallowedPurposeVariable {
                variable: *variable,
                purpose,
            });
        }
        if value.len() > MAX_RENDER_VALUE_BYTES {
            return Err(PromptRenderError::RenderValueTooLarge {
                variable: *variable,
            });
        }
        total = total
            .checked_add(value.len())
            .ok_or(PromptRenderError::RenderValuesTooLarge)?;
        if total > MAX_AUTHORED_BYTES {
            return Err(PromptRenderError::RenderValuesTooLarge);
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PromptRenderContext {
    pub conditions: PromptConditionContext,
    pub values: PromptRenderValues,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedPromptMessage {
    pub entry_id: PromptEntryId,
    pub role: PromptEntryRole,
    pub content: String,
    pub depth: u32,
    pub payload: Option<PromptEntryPayload>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RenderedPrompt {
    pub document_id: PromptDocumentId,
    pub document_revision: Revision,
    pub relative: Vec<RenderedPromptMessage>,
    pub in_chat: Vec<RenderedPromptMessage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptPreview {
    pub rendered: RenderedPrompt,
    pub explanation: PromptExplanation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptEntryExplanation {
    pub entry_id: PromptEntryId,
    pub selected: bool,
    pub reason: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptExplanation {
    pub document_id: PromptDocumentId,
    pub behavior_version: PromptBehaviorVersion,
    pub entries: Vec<PromptEntryExplanation>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PromptRenderError {
    #[error("prompt validation failed: {0}")]
    Invalid(#[from] PromptValidationError),
    #[error("prompt variable {variable:?} is not allowed for {purpose:?}")]
    DisallowedPurposeVariable {
        variable: PromptVariable,
        purpose: PromptPurpose,
    },
    #[error("prompt render value {variable:?} exceeds the 1 MiB limit")]
    RenderValueTooLarge { variable: PromptVariable },
    #[error("prompt render values exceed the 8 MiB limit")]
    RenderValuesTooLarge,
}

pub fn render_prompt(
    document: &PromptDocument,
    context: &PromptRenderContext,
) -> Result<RenderedPrompt, PromptRenderError> {
    document.validate()?;
    crate::lorebook::validate_match_context(&context.conditions.recent_text)
        .map_err(|_| PromptValidationError::MatchContextTooLarge)?;
    validate_render_values(document.purpose, &context.values)?;
    let mut rendered = RenderedPrompt {
        document_id: document.id,
        document_revision: document.revision,
        ..RenderedPrompt::default()
    };
    for entry in &document.entries {
        if !entry_selected(document.behavior_version, entry, context) {
            continue;
        }
        let message = render_entry(entry, &context.values);
        match entry.injection_position {
            PromptEntryPosition::Relative => rendered.relative.push(message),
            PromptEntryPosition::InChat => rendered.in_chat.push(message),
            PromptEntryPosition::Conditional => {
                let minimum = entry.conditional_min_messages.unwrap_or(1) as usize;
                if context.conditions.message_count >= minimum {
                    rendered.in_chat.push(message);
                }
            }
            PromptEntryPosition::Interval => {
                let interval = entry.interval_turns.unwrap_or_default() as usize;
                let turns = context.conditions.message_count;
                if interval > 0 && turns > 0 && turns % interval == 0 {
                    rendered.in_chat.push(message);
                }
            }
        }
    }
    Ok(rendered)
}

fn entry_selected(
    behavior: PromptBehaviorVersion,
    entry: &PromptEntry,
    context: &PromptRenderContext,
) -> bool {
    let enabled = match behavior {
        PromptBehaviorVersion::LegacyV1 => entry.enabled || entry.system_prompt,
        PromptBehaviorVersion::DeterministicV2 => entry.enabled,
    };
    enabled
        && entry
            .conditions
            .as_ref()
            .is_none_or(|condition| matches_condition(condition, &context.conditions))
}

fn render_entry(entry: &PromptEntry, values: &PromptRenderValues) -> RenderedPromptMessage {
    RenderedPromptMessage {
        entry_id: entry.id,
        role: entry.role,
        content: values.render(&entry.content),
        depth: entry.depth,
        payload: entry.payload.clone(),
    }
}

pub fn explain_prompt(
    document: &PromptDocument,
    context: &PromptRenderContext,
) -> Result<PromptExplanation, PromptRenderError> {
    document.validate()?;
    crate::lorebook::validate_match_context(&context.conditions.recent_text)
        .map_err(|_| PromptValidationError::MatchContextTooLarge)?;
    validate_render_values(document.purpose, &context.values)?;
    let entries = document
        .entries
        .iter()
        .map(|entry| {
            let enabled = match document.behavior_version {
                PromptBehaviorVersion::LegacyV1 => entry.enabled || entry.system_prompt,
                PromptBehaviorVersion::DeterministicV2 => entry.enabled,
            };
            let condition_matches = entry
                .conditions
                .as_ref()
                .is_none_or(|condition| matches_condition(condition, &context.conditions));
            let position_matches = match entry.injection_position {
                PromptEntryPosition::Relative | PromptEntryPosition::InChat => true,
                PromptEntryPosition::Conditional => {
                    context.conditions.message_count
                        >= entry.conditional_min_messages.unwrap_or(1) as usize
                }
                PromptEntryPosition::Interval => {
                    let interval = entry.interval_turns.unwrap_or_default() as usize;
                    interval > 0
                        && context.conditions.message_count > 0
                        && context.conditions.message_count % interval == 0
                }
            };
            let (selected, reason) = if !enabled {
                (false, "disabled")
            } else if !condition_matches {
                (false, "condition_false")
            } else if !position_matches {
                (false, "injection_schedule")
            } else {
                (true, "selected")
            };
            PromptEntryExplanation {
                entry_id: entry.id,
                selected,
                reason,
            }
        })
        .collect();
    Ok(PromptExplanation {
        document_id: document.id,
        behavior_version: document.behavior_version,
        entries,
    })
}

pub fn preview_prompt(
    document: &PromptDocument,
    context: &PromptRenderContext,
) -> Result<PromptPreview, PromptRenderError> {
    Ok(PromptPreview {
        rendered: render_prompt(document, context)?,
        explanation: explain_prompt(document, context)?,
    })
}

pub trait PromptRepository: Send + Sync {
    fn create_user_draft(
        &self,
        document: PromptDocument,
        now: TimestampMillis,
    ) -> Result<PromptDocument, PromptRepositoryError>;
    fn derive_built_in(
        &self,
        source: PromptDocumentId,
        derived: PromptDocument,
        now: TimestampMillis,
    ) -> Result<PromptDocument, PromptRepositoryError>;
    fn get(&self, id: PromptDocumentId) -> Result<Option<PromptDocument>, PromptRepositoryError>;
    fn list_by_purpose(
        &self,
        purpose: PromptPurpose,
    ) -> Result<Vec<PromptDocument>, PromptRepositoryError>;
    fn replace_entry_set(
        &self,
        id: PromptDocumentId,
        expected_revision: Revision,
        entries: Vec<PromptEntry>,
        now: TimestampMillis,
    ) -> Result<PromptDocument, PromptRepositoryError>;
    fn patch_entry(
        &self,
        id: PromptDocumentId,
        expected_revision: Revision,
        entry: PromptEntry,
        now: TimestampMillis,
    ) -> Result<PromptDocument, PromptRepositoryError>;
    fn reorder_entry(
        &self,
        id: PromptDocumentId,
        expected_revision: Revision,
        entry: PromptEntryId,
        target_index: usize,
        now: TimestampMillis,
    ) -> Result<PromptDocument, PromptRepositoryError>;
    fn validate(&self, document: &PromptDocument) -> Result<(), PromptRepositoryError>;
    fn archive(
        &self,
        id: PromptDocumentId,
        expected_revision: Revision,
        now: TimestampMillis,
    ) -> Result<(), PromptRepositoryError>;
    fn restore(
        &self,
        id: PromptDocumentId,
        expected_revision: Revision,
        now: TimestampMillis,
    ) -> Result<(), PromptRepositoryError>;
    fn resolve_compatible(
        &self,
        purpose: PromptPurpose,
        context: &PromptRenderContext,
    ) -> Result<PromptResolution, PromptRepositoryError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptResolution {
    pub selected: Option<PromptDocument>,
    pub rejected: Vec<PromptRejection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptRejection {
    pub document_id: PromptDocumentId,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PromptRepositoryError {
    #[error("prompt validation failed: {0}")]
    Invalid(#[from] PromptValidationError),
    #[error("prompt repository conflict")]
    Conflict,
    #[error("prompt not found")]
    NotFound,
    #[error("prompt repository failure: {0}")]
    Failure(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document(entry: PromptEntry) -> PromptDocument {
        PromptDocument {
            id: PromptDocumentId::new(),
            status: LifecycleStatus::Active,
            name: "test prompt".into(),
            purpose: PromptPurpose::DirectChat,
            entries: vec![entry],
            condense: false,
            behavior_version: PromptBehaviorVersion::LegacyV1,
            provenance: PromptProvenance::User,
            revision: Revision::INITIAL,
            created_at: TimestampMillis::UNIX_EPOCH,
            updated_at: TimestampMillis::UNIX_EPOCH,
        }
    }

    #[test]
    fn condition_ast_is_exhaustive_and_nested() {
        let condition = PromptEntryCondition::All {
            conditions: vec![
                PromptEntryCondition::ChatMode {
                    value: PromptEntryChatMode::Direct,
                },
                PromptEntryCondition::Any {
                    conditions: vec![PromptEntryCondition::KeywordAny {
                        values: vec!["rain".into()],
                    }],
                },
                PromptEntryCondition::Not {
                    condition: Box::new(PromptEntryCondition::HasKeyMemories { value: true }),
                },
            ],
        };
        let context = PromptConditionContext {
            recent_text: "rain outside".into(),
            ..PromptConditionContext::default()
        };
        assert!(matches_condition(&condition, &context));
        assert!(condition.validate().is_ok());
    }

    #[test]
    fn legacy_runs_disabled_system_entry_but_v2_does_not() {
        let entry = PromptEntry {
            id: PromptEntryId::new(),
            name: "legacy".into(),
            content: "{{char}} {{rules}}".into(),
            enabled: false,
            system_prompt: true,
            ..PromptEntry::default()
        };
        let mut context = PromptRenderContext::default();
        context.values.character_name = "Ada".into();
        let mut legacy = document(entry.clone());
        assert_eq!(
            render_prompt(&legacy, &context)
                .expect("valid legacy prompt")
                .relative[0]
                .content,
            "Ada "
        );
        legacy.behavior_version = PromptBehaviorVersion::DeterministicV2;
        assert!(
            render_prompt(&legacy, &context)
                .expect("valid deterministic prompt")
                .relative
                .is_empty()
        );
    }

    #[test]
    fn invalid_interval_and_duplicate_entries_are_rejected() {
        let entry = PromptEntry {
            id: PromptEntryId::new(),
            name: "interval".into(),
            injection_position: PromptEntryPosition::Interval,
            ..PromptEntry::default()
        };
        assert_eq!(
            document(entry).validate(),
            Err(PromptValidationError::InvalidInterval)
        );

        let first = PromptEntry {
            name: "first".into(),
            ..PromptEntry::default()
        };
        let mut duplicate = document(first.clone());
        duplicate.entries.push(first);
        assert!(matches!(
            duplicate.validate(),
            Err(PromptValidationError::DuplicateEntry(_))
        ));
    }

    #[test]
    fn serde_rejects_unknown_prompt_condition_fields() {
        let raw = r#"{"type":"chatMode","value":"direct","unknown":true}"#;
        assert!(serde_json::from_str::<PromptEntryCondition>(raw).is_err());
    }

    #[test]
    fn legacy_registered_placeholders_are_closed_and_never_leak() {
        let entry = PromptEntry {
            id: PromptEntryId::new(),
            name: "compatibility".into(),
            content: "{{selected_messages}} {{image[character]}} {{rules}}".into(),
            ..PromptEntry::default()
        };
        let mut values = PromptRenderValues::default();
        values
            .purpose_values
            .insert(PromptVariable::SelectedMessages, "messages".into());
        let mut document = document(entry);
        document.purpose = PromptPurpose::LorebookEntryWriter;
        let rendered = render_prompt(
            &document,
            &PromptRenderContext {
                conditions: PromptConditionContext::default(),
                values,
            },
        )
        .expect("valid compatibility render");
        assert_eq!(rendered.relative[0].content, "messages  ");
        assert!(!rendered.relative[0].content.contains("{{"));
    }

    #[test]
    fn purpose_values_reject_nonempty_disallowed_variables_and_allow_scene_loras() {
        let entry = PromptEntry {
            id: PromptEntryId::new(),
            name: "purpose".into(),
            content: "{{lora_keywords[character]}}".into(),
            ..PromptEntry::default()
        };
        let mut values = PromptRenderValues::default();
        values
            .purpose_values
            .insert(PromptVariable::CharacterLoraKeywords, "mara-voss".into());
        let mut scene = document(entry.clone());
        scene.purpose = PromptPurpose::ScenePromptWriter;
        assert_eq!(
            render_prompt(
                &scene,
                &PromptRenderContext {
                    conditions: PromptConditionContext::default(),
                    values: values.clone(),
                },
            )
            .expect("scene lora variable is allowed")
            .relative[0]
                .content,
            "mara-voss"
        );
        let disallowed = document(entry);
        assert!(matches!(
            render_prompt(
                &disallowed,
                &PromptRenderContext {
                    conditions: PromptConditionContext::default(),
                    values,
                },
            ),
            Err(PromptRenderError::DisallowedPurposeVariable {
                variable: PromptVariable::CharacterLoraKeywords,
                purpose: PromptPurpose::DirectChat,
            })
        ));
    }

    #[test]
    fn prompt_rejects_zero_revision_and_oversized_match_context() {
        let entry = PromptEntry {
            id: PromptEntryId::new(),
            name: "test".into(),
            ..PromptEntry::default()
        };
        let mut invalid = document(entry.clone());
        invalid.revision = Revision::new(0);
        assert_eq!(invalid.validate(), Err(PromptValidationError::ZeroRevision));

        let context = PromptRenderContext {
            conditions: PromptConditionContext {
                recent_text: "x".repeat(crate::lorebook::MAX_MATCH_CONTEXT_BYTES + 1),
                ..PromptConditionContext::default()
            },
            values: PromptRenderValues::default(),
        };
        assert!(matches!(
            render_prompt(&document(entry), &context),
            Err(PromptRenderError::Invalid(
                PromptValidationError::MatchContextTooLarge
            ))
        ));
    }
}
