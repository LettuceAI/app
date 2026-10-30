use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum LibraryPersonaSelection {
    Inherit,
    Explicit(String),
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LibraryGroupMember {
    pub character_id: String,
    pub ordinal: u32,
    pub muted: bool,
    #[serde(deserialize_with = "Option::deserialize")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub model_profile_override: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LibraryGroupStartingSceneDraft {
    pub scene: crate::LibrarySceneDraft,
    pub variants: Vec<crate::LibrarySceneVariantDraft>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct GroupDraft {
    pub name: String,
    pub chat_mode: crate::GroupChatMode,
    pub persona: LibraryPersonaSelection,
    pub speaker_selection: crate::SpeakerSelectionMethod,
    pub memory_policy: crate::LibraryMemoryPolicy,
    pub disable_character_lorebooks: bool,
    #[serde(deserialize_with = "Option::deserialize")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub group_conversation_prompt_id: Option<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub group_roleplay_prompt_id: Option<String>,
    pub presentation: crate::LibraryChatAppearanceV1,
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(optional))]
    pub presentation_overrides: Option<Vec<String>>,
    pub members: Vec<LibraryGroupMember>,
    #[serde(deserialize_with = "Option::deserialize")]
    #[cfg_attr(feature = "specta", specta(type = Option<String>))]
    pub background_asset_id: Option<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    #[cfg_attr(feature = "specta", specta(type = Option<LibraryGroupStartingSceneDraft>))]
    pub starting_scene: Option<LibraryGroupStartingSceneDraft>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct GroupView {
    pub id: String,
    pub archived: bool,
    pub draft: GroupDraft,
    pub assets: Vec<crate::AssetRef>,
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
pub struct GroupGetRequest {
    pub group_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct GroupSaveRequest {
    pub group_id: Option<String>,
    pub draft: GroupDraft,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub expected_revision: Option<u64>,
    pub client_operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct GroupsListRequest {
    pub query: Option<String>,
    pub sort: Option<crate::LibrarySort>,
    pub cursor: Option<String>,
    pub limit: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct GroupListItem {
    pub group: GroupView,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub last_activity_at: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct GroupPage {
    pub items: Vec<GroupListItem>,
    pub next_cursor: Option<String>,
}
