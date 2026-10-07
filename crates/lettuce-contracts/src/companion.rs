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
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub valid_from: i64,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub valid_until: Option<i64>,
    pub locked: bool,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub created_at: i64,
    pub source_memory_ids: Vec<String>,
    pub supersedes: Vec<String>,
    pub superseded_by: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CompanionEmotionVector {
    pub warmth: f64,
    pub trust: f64,
    pub calm: f64,
    pub vulnerability: f64,
    pub longing: f64,
    pub hurt: f64,
    pub tension: f64,
    pub irritation: f64,
    pub affection_intensity: f64,
    pub reassurance_need: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CompanionRegulationStyle {
    pub suppression: f64,
    pub volatility: f64,
    pub recovery_speed: f64,
    pub conflict_avoidance: f64,
    pub reassurance_seeking: f64,
    pub protest_behavior: f64,
    pub emotional_transparency: f64,
    pub attachment_activation: f64,
    pub pride: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CompanionSoulIdentity {
    pub essence: String,
    pub traits: String,
    pub backstory: String,
    pub appearance: String,
    pub goals: String,
    pub likes: String,
    pub voice: String,
    pub relational_style: String,
    pub vulnerabilities: String,
    pub fears: String,
    pub habits: String,
    pub boundaries: String,
    pub baseline_affect: CompanionEmotionVector,
    pub regulation_style: CompanionRegulationStyle,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CompanionRelationshipDefaults {
    pub closeness: f64,
    pub trust: f64,
    pub affection: f64,
    pub tension: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CompanionPrompting {
    pub prompt_template_id: Option<String>,
    pub style_notes: String,
}

/// The authored configuration of a companion character.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CompanionSoulConfigView {
    pub soul: CompanionSoulIdentity,
    pub authored_facts: Vec<SoulFactView>,
    pub relationship_defaults: CompanionRelationshipDefaults,
    pub prompting: CompanionPrompting,
    pub time_awareness: bool,
    pub share_memory_across_chats: bool,
    pub share_soul_growth_across_chats: bool,
}

/// What the Soul writer drafts and takes as the draft to refine: the Soul
/// identity, the authored facts and the relationship defaults.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CompanionSoulDraft {
    pub soul: CompanionSoulIdentity,
    pub authored_facts: Vec<SoulFactView>,
    pub relationship_defaults: CompanionRelationshipDefaults,
}

/// A companion's authored Soul configuration with the growth the Soul in
/// effect for the conversation (or the character) has gathered.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CompanionSoulView {
    pub character_id: String,
    pub conversation_id: Option<String>,
    pub config: CompanionSoulConfigView,
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
    pub current_soul: Option<CompanionSoulDraft>,
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
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub available_at: i64,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub expires_at: Option<i64>,
    pub recurrence: CompanionNoteRecurrence,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub recurrence_window_ms: Option<u64>,
    pub enabled: bool,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub created_at: i64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
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
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub available_at: i64,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
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
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub as_of: i64,
}
