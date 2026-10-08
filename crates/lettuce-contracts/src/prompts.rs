use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum PromptKind {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum PromptBehavior {
    LegacyV1,
    DeterministicV2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum PromptEntryRole {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum PromptEntryPosition {
    Relative,
    InChat,
    Conditional,
    Interval,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum PromptImageSlot {
    Character,
    Persona,
    ChatBackground,
    Avatar,
    References,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum PromptChatMode {
    Direct,
    Group,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum PromptInfoSource {
    Messages,
    Memory,
    Mixed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum PromptSceneImageProtocol {
    Remote,
    Local,
}

/// When a prompt entry applies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum PromptCondition {
    ChatMode { value: PromptChatMode },
    InfoSource { value: PromptInfoSource },
    SceneGenerationEnabled { value: bool },
    AvatarGenerationEnabled { value: bool },
    IsLocalImageGenerationModel { value: bool },
    IsSceneGenerationLocalImageModel { value: bool },
    SceneImageProtocol { value: PromptSceneImageProtocol },
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
    All { conditions: Vec<PromptCondition> },
    Any { conditions: Vec<PromptCondition> },
    Not { condition: Box<PromptCondition> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct PromptEntryInput {
    /// An existing entry of the prompt keeps its id; a new entry has none.
    pub entry_id: Option<String>,
    pub name: String,
    pub role: PromptEntryRole,
    pub content: String,
    pub enabled: bool,
    pub position: PromptEntryPosition,
    pub depth: u32,
    pub conditional_min_messages: Option<u32>,
    pub interval_turns: Option<u32>,
    pub system_prompt: bool,
    pub condition: Option<PromptCondition>,
    pub image_slot: Option<PromptImageSlot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct PromptEntryView {
    pub id: String,
    /// Set on the entries of a built-in prompt the catalog owns.
    pub built_in_key: Option<String>,
    pub name: String,
    pub role: PromptEntryRole,
    pub content: String,
    pub enabled: bool,
    pub position: PromptEntryPosition,
    pub depth: u32,
    pub conditional_min_messages: Option<u32>,
    pub interval_turns: Option<u32>,
    pub system_prompt: bool,
    pub condition: Option<PromptCondition>,
    pub image_slot: Option<PromptImageSlot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum PromptOrigin {
    BuiltIn {
        key: String,
        protected: bool,
        required: bool,
        edited: bool,
    },
    User,
    Derived {
        source_id: String,
        source_name: String,
        source_deleted: bool,
    },
    Imported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct PromptSummary {
    pub id: String,
    pub name: String,
    pub kind: PromptKind,
    pub archived: bool,
    pub origin: PromptOrigin,
    pub app_default: bool,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub revision: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub created_at: i64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct PromptView {
    pub prompt: PromptSummary,
    pub condense: bool,
    pub behavior: PromptBehavior,
    pub entries: Vec<PromptEntryView>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct PromptsListRequest {
    pub kind: Option<PromptKind>,
    pub lifecycle: Option<crate::LifecycleFilter>,
    pub cursor: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct PromptPage {
    pub items: Vec<PromptSummary>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct PromptGetRequest {
    pub prompt_id: String,
}

/// A prompt as the editor submits it: metadata and every entry in order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct PromptInput {
    pub name: String,
    pub kind: PromptKind,
    pub condense: bool,
    pub behavior: PromptBehavior,
    pub entries: Vec<PromptEntryInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct PromptCreateRequest {
    pub client_operation_id: String,
    pub prompt: PromptInput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct PromptUpdateRequest {
    pub client_operation_id: String,
    pub prompt_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
    pub prompt: PromptInput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct PromptDeleteRequest {
    pub client_operation_id: String,
    pub prompt_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
}

/// Resets one built-in prompt or every built-in to its catalog content and
/// entries; the user's name for it stays.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum PromptBuiltinResetRequest {
    One {
        client_operation_id: String,
        prompt_id: String,
        #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
        expected_revision: u64,
    },
    All {
        client_operation_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct PromptBuiltinResetResult {
    pub prompts: Vec<PromptView>,
}

/// Selects the app-wide default prompt; `None` returns to the built-in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct PromptAppDefaultSetRequest {
    pub client_operation_id: String,
    pub prompt_id: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_settings_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct PromptAppDefault {
    pub prompt_id: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub settings_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct PromptPlaceholdersRequest {
    pub kind: PromptKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct PromptPlaceholders {
    pub kind: PromptKind,
    pub allowed: Vec<String>,
    pub required: Vec<String>,
    pub image_slots: Vec<PromptImageSlot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct PromptValidateRequest {
    pub prompt: PromptInput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct PromptValidation {
    pub missing_placeholders: Vec<String>,
}

/// Renders a prompt. With a conversation it renders from that
/// conversation's live sources as its next turn would; without one it uses
/// the given character and persona (or none) and sample memories, summary
/// and lorebook text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct PromptPreviewRequest {
    pub prompt_id: String,
    pub conversation_id: Option<String>,
    pub character_id: Option<String>,
    pub persona_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct PromptPreviewEntry {
    pub entry_id: String,
    pub name: String,
    pub role: PromptEntryRole,
    pub position: PromptEntryPosition,
    pub depth: u32,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct PromptPreview {
    pub entries: Vec<PromptPreviewEntry>,
    /// Entries left out by their conditions, enabled flag or position.
    pub skipped_entry_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct DefaultCharacterRules {
    pub rules: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum PureModeLevel {
    Off,
    Low,
    Standard,
    Strict,
}

/// The rules a new character starts with at `pure_mode`, or at the current
/// Pure mode setting when it is unset.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct DefaultCharacterRulesRequest {
    pub pure_mode: Option<PureModeLevel>,
}
