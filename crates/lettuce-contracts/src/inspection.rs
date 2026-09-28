use serde::{Deserialize, Serialize};

use crate::{AssetRef, MediaRole, MessageRole};

/// Asks for the request a reply was generated from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MessagePromptSnapshotRequest {
    pub message_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum PromptOperation {
    Send,
    Continue,
    Regenerate,
}

/// The model a request was sent to; nothing that identifies the account or
/// its credentials.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct PromptModel {
    pub display_name: String,
    pub external_model_id: String,
    pub provider_kind: String,
}

/// The sampling parameters a request resolved to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct PromptParameters {
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub top_k: Option<u32>,
    pub max_output_tokens: Option<u32>,
    pub context_length: Option<u32>,
    pub frequency_penalty: Option<f64>,
    pub presence_penalty: Option<f64>,
    pub repetition_penalty: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum PromptPart {
    Text { text: String },
    Media { asset: AssetRef, role: MediaRole },
    ToolCall { name: String, arguments: String },
    ToolResult { name: String, output: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct PromptMessage {
    pub role: MessageRole,
    pub parts: Vec<PromptPart>,
}

/// How much of the chat the request carried and what it is estimated to
/// cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct PromptBudget {
    pub selected_messages: u32,
    pub omitted_messages: u32,
    pub input_bytes: u32,
    pub estimated_input_tokens: u32,
    pub truncated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum PromptSectionKind {
    Character,
    Persona,
    Scene,
    Lorebook,
    Memories,
    AuthorNote,
    CompanionState,
    ScheduledNotes,
    GroupCast,
    PromptEntry,
}

/// What one source contributed to the prompt, with its estimated tokens. A
/// lorebook entry or prompt entry is labelled with its title or name, the
/// character and persona with theirs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct PromptSection {
    pub kind: PromptSectionKind,
    pub label: Option<String>,
    pub estimated_tokens: u32,
}

/// Why a request has no per-section breakdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum PromptSectionsUnavailable {
    PredatesBreakdown,
}

/// The request a reply was generated from, exactly as it was recorded when
/// it was sent. The sections are recorded with it, so they show what was
/// sent even when the character, persona or lorebooks changed since.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct PromptSnapshot {
    pub turn_id: String,
    pub candidate_id: String,
    pub operation: PromptOperation,
    pub model: PromptModel,
    pub streaming: bool,
    pub parameters: PromptParameters,
    pub messages: Vec<PromptMessage>,
    pub budget: PromptBudget,
    pub sections: Option<Vec<PromptSection>>,
    pub sections_unavailable: Option<PromptSectionsUnavailable>,
}

/// Renders the prompt the next LLM speaker selection would send in a group
/// chat, for a user message that is not sent yet when `user_message` is set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SpeakerSelectionPreviewRequest {
    pub conversation_id: String,
    pub user_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct SpeakerSelectionPreview {
    pub prompt: String,
}

/// One character's share of the replies on the selected branch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ParticipationStat {
    pub participant_id: String,
    pub character_id: Option<String>,
    pub name: String,
    pub enabled: bool,
    pub muted: bool,
    /// Visible replies the participant spoke on the selected branch.
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub message_count: u64,
    /// The share of all replies, rounded to a whole percent.
    pub percent: u32,
    pub last_spoke_message_id: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub last_spoke_at: Option<i64>,
}

/// Who spoke how much on the selected branch, derived from its timeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ParticipationStats {
    pub items: Vec<ParticipationStat>,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub total_messages: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MessageCompanionEffectRequest {
    pub message_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum CompanionEffectStatus {
    Processing,
    Ready,
    Failed,
    Invalidated,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct RelationshipChange {
    pub closeness: f64,
    pub trust: f64,
    pub affection: f64,
    pub tension: f64,
    pub stability: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct EmotionChange {
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

/// What a reply changed about a companion: how the relationship and the
/// felt, expressed and blocked emotions moved, the signals that came or
/// went and, once the memory cycle settled, the memories it wrote.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct MessageCompanionEffect {
    pub status: CompanionEffectStatus,
    pub summary: Option<String>,
    pub relationship: RelationshipChange,
    pub felt: EmotionChange,
    pub expressed: EmotionChange,
    pub blocked: EmotionChange,
    pub signals_added: Vec<String>,
    pub signals_removed: Vec<String>,
    pub memories_added: Vec<String>,
    pub memories_updated: Vec<String>,
    pub memories_superseded: Vec<String>,
}
