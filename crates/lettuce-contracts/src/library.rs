use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct GroupDeleteRequest {
    pub group_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
    pub client_operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct PersonaDeleteRequest {
    pub persona_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
    pub client_operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LibraryLorebookBinding {
    pub lorebook_id: String,
    pub enabled: bool,
    pub ordinal: u32,
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
pub struct LibraryLorebookMutationResult {
    pub bindings: Vec<LibraryLorebookBinding>,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub owner_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CharacterLorebooksListRequest {
    pub character_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CharacterLorebookBindRequest {
    pub character_id: String,
    pub lorebook_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
    pub client_operation_id: String,
    pub target_ordinal: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CharacterLorebookUnbindRequest {
    pub character_id: String,
    pub lorebook_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
    pub client_operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CharacterLorebookEnableRequest {
    pub character_id: String,
    pub lorebook_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
    pub client_operation_id: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CharacterLorebookReorderRequest {
    pub character_id: String,
    pub lorebook_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
    pub client_operation_id: String,
    pub target_ordinal: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct PersonaLorebooksListRequest {
    pub persona_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct PersonaLorebookBindRequest {
    pub persona_id: String,
    pub lorebook_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
    pub client_operation_id: String,
    pub target_ordinal: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct PersonaLorebookUnbindRequest {
    pub persona_id: String,
    pub lorebook_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
    pub client_operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct PersonaLorebookEnableRequest {
    pub persona_id: String,
    pub lorebook_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
    pub client_operation_id: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct PersonaLorebookReorderRequest {
    pub persona_id: String,
    pub lorebook_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
    pub client_operation_id: String,
    pub target_ordinal: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct GroupLorebooksListRequest {
    pub group_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct GroupLorebookBindRequest {
    pub group_id: String,
    pub lorebook_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
    pub client_operation_id: String,
    pub target_ordinal: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct GroupLorebookUnbindRequest {
    pub group_id: String,
    pub lorebook_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
    pub client_operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct GroupLorebookEnableRequest {
    pub group_id: String,
    pub lorebook_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
    pub client_operation_id: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct GroupLorebookReorderRequest {
    pub group_id: String,
    pub lorebook_id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub expected_revision: u64,
    pub client_operation_id: String,
    pub target_ordinal: u32,
}
