use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum CharacterNoteRecurrence {
    None,
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CharacterNoteView {
    pub id: String,
    pub character_id: String,
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
pub struct CharacterNotesRequest {
    pub character_id: String,
}

/// Creates a note, or updates the note `note_id` names. The API assigns the
/// id and the timestamps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CharacterNoteUpsertRequest {
    pub character_id: String,
    pub note_id: Option<String>,
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
pub struct CharacterNoteDeleteRequest {
    pub note_id: String,
    pub client_operation_id: String,
}

/// The notes that apply to the character at `as_of`, a time the user chose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CharacterNotesActivePreviewRequest {
    pub character_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub as_of: i64,
}
