use serde::{Deserialize, Serialize};

/// Which Soul a view or edit acts on: the character's, shared by all of its
/// chats, or one chat's own while the character does not share Soul growth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SoulOwnerKind {
    Character,
    Conversation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SoulCategory {
    Essence,
    Traits,
    Backstory,
    Appearance,
    Goals,
    Likes,
    Voice,
    RelationalStyle,
    Vulnerabilities,
    Fears,
    Habits,
    Boundaries,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SoulFactPolicy {
    Current,
    Adaptive,
    Historical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SoulFactKind {
    Add,
    Adjust,
    Authored,
    Consolidated,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SoulFactView {
    pub id: String,
    pub category: SoulCategory,
    pub value: String,
    pub kind: SoulFactKind,
    pub policy: SoulFactPolicy,
    pub slot: String,
    pub confidence: f64,
    pub evidence_count: u32,
    pub weight: f64,
    pub valid_from: i64,
    pub valid_until: Option<i64>,
    pub locked: bool,
    pub created_at: i64,
    pub superseded_by: Option<String>,
    pub superseded_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SoulGrowthView {
    pub owner: SoulOwnerKind,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub revision: u64,
    pub active_count: u32,
    pub superseded_count: u32,
    pub facts: Vec<SoulFactView>,
}

/// A companion's authored Soul configuration with the growth the Soul in
/// effect for the conversation (or the character) has gathered. `config` is
/// the authored configuration document of the character.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CompanionSoulView {
    pub character_id: String,
    pub conversation_id: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Unknown))]
    pub config: serde_json::Value,
    pub growth: SoulGrowthView,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CompanionSoulGetRequest {
    pub character_id: String,
    pub conversation_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CompanionSoulGrowthClearRequest {
    pub character_id: String,
    pub conversation_id: Option<String>,
    pub client_operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CompanionSoulGrowthRemoveRequest {
    pub character_id: String,
    pub conversation_id: Option<String>,
    pub fact_id: String,
    pub client_operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CompanionSoulGrowthLockRequest {
    pub character_id: String,
    pub conversation_id: Option<String>,
    pub fact_id: String,
    pub locked: bool,
    pub client_operation_id: String,
}

/// Asks the Soul writer to draft a Soul from the character's text and the
/// unsaved draft. The draft arrives as the job's result; nothing is saved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CompanionSoulWriterRunRequest {
    pub character_name: String,
    pub character_definition: Option<String>,
    pub character_description: Option<String>,
    pub opening_context: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Unknown>))]
    pub current_soul: Option<serde_json::Value>,
    pub user_notes: Option<String>,
    pub model_profile_id: Option<String>,
    pub client_operation_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum CompanionNoteRecurrence {
    None,
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CompanionNoteView {
    pub id: String,
    pub character_id: String,
    pub label: String,
    pub content: String,
    pub available_at: i64,
    pub expires_at: Option<i64>,
    pub recurrence: CompanionNoteRecurrence,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub recurrence_window_ms: Option<u64>,
    pub enabled: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CompanionNotesRequest {
    pub character_id: String,
}

/// Creates a note, or updates the note `note_id` names. The API assigns the
/// id and the timestamps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CompanionNoteUpsertRequest {
    pub character_id: String,
    pub note_id: Option<String>,
    pub label: String,
    pub content: String,
    pub available_at: i64,
    pub expires_at: Option<i64>,
    pub recurrence: CompanionNoteRecurrence,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub recurrence_window_ms: Option<u64>,
    pub enabled: bool,
    pub client_operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CompanionNoteDeleteRequest {
    pub note_id: String,
    pub client_operation_id: String,
}

/// The notes that apply to the companion at `as_of`, a time the user chose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CompanionNotesActivePreviewRequest {
    pub character_id: String,
    pub as_of: i64,
}
