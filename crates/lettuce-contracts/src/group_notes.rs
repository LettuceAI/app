use serde::{Deserialize, Serialize};

use crate::CharacterNoteRecurrence;

/// A scheduled note of a group. `visible_to` lists the member character ids
/// that see it; empty means every member.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct GroupNoteView {
    pub id: String,
    pub group_id: String,
    pub visible_to: Vec<String>,
    pub label: String,
    pub content: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub available_at: i64,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub expires_at: Option<i64>,
    pub recurrence: CharacterNoteRecurrence,
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
pub struct GroupNotesRequest {
    pub group_id: String,
}

/// Creates a note, or updates the note `note_id` names. The API assigns the
/// id and the timestamps. Every id in `visible_to` must be a current member
/// of the group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct GroupNoteUpsertRequest {
    pub group_id: String,
    pub note_id: Option<String>,
    pub visible_to: Vec<String>,
    pub label: String,
    pub content: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub available_at: i64,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub expires_at: Option<i64>,
    pub recurrence: CharacterNoteRecurrence,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub recurrence_window_ms: Option<u64>,
    pub enabled: bool,
    pub client_operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct GroupNoteDeleteRequest {
    pub note_id: String,
    pub client_operation_id: String,
}
