use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::{
    chat_manager::types::{
        AdvancedModelSettings, Model, PromptTemplateType, SystemPromptEntry, SystemPromptTemplate,
    },
    storage_manager::lorebook::{Lorebook, LorebookEntry},
    sync::models::{ChatTemplate, ChatTemplateMessage},
};

pub const USC_SCHEMA_NAME: &str = "USC";
pub const USC_SCHEMA_VERSION: &str = "1.0";

pub type UscExtensions = BTreeMap<String, JsonValue>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UscSchemaInfo {
    pub name: String,
    pub version: String,
}

impl Default for UscSchemaInfo {
    fn default() -> Self {
        Self {
            name: USC_SCHEMA_NAME.to_string(),
            version: USC_SCHEMA_VERSION.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UscKind {
    SystemPromptTemplate,
    Lorebook,
    ChatTemplate,
    ModelProfile,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UscRef {
    pub kind: UscKind,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub optional: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UscMeta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authors: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(flatten)]
    pub extra: UscExtensions,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UscVariable {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UscCard<TPayload, TAppSettings> {
    pub schema: UscSchemaInfo,
    pub kind: UscKind,
    pub payload: TPayload,
    #[serde(
        rename = "app_specific_settings",
        skip_serializing_if = "Option::is_none"
    )]
    pub app_specific_settings: Option<TAppSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<UscMeta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<UscExtensions>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UscSystemPromptTemplatePayload {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub prompt_type: PromptTemplateType,
    pub content: String,
    #[serde(default)]
    pub entries: Vec<SystemPromptEntry>,
    #[serde(default)]
    pub condense_prompt_entries: bool,
    pub created_at: u64,
    pub updated_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variables: Option<Vec<UscVariable>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requires: Option<Vec<UscRef>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UscSystemPromptTemplateEditorSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_entry_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collapsed_entry_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview_target_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub view_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UscSystemPromptTemplateAppSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pinned: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protected: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub editor: Option<UscSystemPromptTemplateEditorSettings>,
    #[serde(flatten)]
    pub extra: UscExtensions,
}

pub type UscSystemPromptTemplateCard =
    UscCard<UscSystemPromptTemplatePayload, UscSystemPromptTemplateAppSettings>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UscLorebookScope {
    Global,
    Character,
    Group,
    Session,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UscLorebookEntry {
    pub id: String,
    pub title: String,
    pub enabled: bool,
    pub always_active: bool,
    pub keywords: Vec<String>,
    pub case_sensitive: bool,
    pub content: String,
    pub priority: i32,
    pub display_order: i32,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UscLorebookPayload {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keyword_detection_mode: Option<String>,
    pub entries: Vec<UscLorebookEntry>,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<UscLorebookScope>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requires: Option<Vec<UscRef>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UscLorebookEditorSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_entry_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collapsed_entry_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_query: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UscLorebookAssignmentHints {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub character_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UscLorebookAppSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pinned: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub editor: Option<UscLorebookEditorSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignment_hints: Option<UscLorebookAssignmentHints>,
    #[serde(flatten)]
    pub extra: UscExtensions,
}

pub type UscLorebookCard = UscCard<UscLorebookPayload, UscLorebookAppSettings>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UscChatTemplateMessage {
    pub id: String,
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UscChatTemplatePayload {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub messages: Vec<UscChatTemplateMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt_template: Option<UscRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lorebook_ids_override: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variables: Option<Vec<UscVariable>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opening_notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requires: Option<Vec<UscRef>>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UscChatTemplateEditorSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_message_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview_character_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview_scene_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub view_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UscChatTemplateAppSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pinned: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub editor: Option<UscChatTemplateEditorSettings>,
    #[serde(flatten)]
    pub extra: UscExtensions,
}

pub type UscChatTemplateCard = UscCard<UscChatTemplatePayload, UscChatTemplateAppSettings>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UscReasoningSupport {
    None,
    Native,
    Budget,
    Effort,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UscModelProfilePayload {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub provider_id: String,
    pub provider_label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub input_scopes: Vec<String>,
    #[serde(default)]
    pub output_scopes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub advanced_model_settings: Option<AdvancedModelSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt_template: Option<UscRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_support: Option<UscReasoningSupport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requires: Option<Vec<UscRef>>,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UscModelProfileEditorSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_verified_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_verification_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browse_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UscModelProfileAppSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pinned: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_candidate: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub editor: Option<UscModelProfileEditorSettings>,
    #[serde(flatten)]
    pub extra: UscExtensions,
}

pub type UscModelProfileCard = UscCard<UscModelProfilePayload, UscModelProfileAppSettings>;

#[derive(Clone)]
pub enum AnyUscCard {
    SystemPromptTemplate(UscSystemPromptTemplateCard),
    Lorebook(UscLorebookCard),
    ChatTemplate(UscChatTemplateCard),
    ModelProfile(UscModelProfileCard),
}

impl std::fmt::Debug for AnyUscCard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind = match self {
            AnyUscCard::SystemPromptTemplate(_) => "SystemPromptTemplate",
            AnyUscCard::Lorebook(_) => "Lorebook",
            AnyUscCard::ChatTemplate(_) => "ChatTemplate",
            AnyUscCard::ModelProfile(_) => "ModelProfile",
        };

        f.debug_tuple("AnyUscCard").field(&kind).finish()
    }
}

impl AnyUscCard {
    pub fn kind(&self) -> UscKind {
        match self {
            AnyUscCard::SystemPromptTemplate(_) => UscKind::SystemPromptTemplate,
            AnyUscCard::Lorebook(_) => UscKind::Lorebook,
            AnyUscCard::ChatTemplate(_) => UscKind::ChatTemplate,
            AnyUscCard::ModelProfile(_) => UscKind::ModelProfile,
        }
    }

    pub fn to_json_value(&self) -> Result<JsonValue, String> {
        match self {
            AnyUscCard::SystemPromptTemplate(card) => serde_json::to_value(card),
            AnyUscCard::Lorebook(card) => serde_json::to_value(card),
            AnyUscCard::ChatTemplate(card) => serde_json::to_value(card),
            AnyUscCard::ModelProfile(card) => serde_json::to_value(card),
        }
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to serialize USC card: {}", e),
            )
        })
    }
}

pub fn is_usc_value(value: &JsonValue) -> bool {
    value
        .get("schema")
        .and_then(|schema| schema.get("name"))
        .and_then(|name| name.as_str())
        == Some(USC_SCHEMA_NAME)
}

pub fn parse_usc_value(value: &JsonValue) -> Result<AnyUscCard, String> {
    if !is_usc_value(value) {
        return Err("Invalid USC: missing or unsupported schema name".to_string());
    }

    let kind = serde_json::from_value::<UscKind>(
        value
            .get("kind")
            .cloned()
            .ok_or_else(|| "Invalid USC: missing kind".to_string())?,
    )
    .map_err(|e| {
        crate::utils::err_msg(module_path!(), line!(), format!("Invalid USC kind: {}", e))
    })?;

    match kind {
        UscKind::SystemPromptTemplate => {
            serde_json::from_value::<UscSystemPromptTemplateCard>(value.clone())
                .map(AnyUscCard::SystemPromptTemplate)
        }
        UscKind::Lorebook => {
            serde_json::from_value::<UscLorebookCard>(value.clone()).map(AnyUscCard::Lorebook)
        }
        UscKind::ChatTemplate => serde_json::from_value::<UscChatTemplateCard>(value.clone())
            .map(AnyUscCard::ChatTemplate),
        UscKind::ModelProfile => serde_json::from_value::<UscModelProfileCard>(value.clone())
            .map(AnyUscCard::ModelProfile),
    }
    .map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Invalid USC payload: {}", e),
        )
    })
}

pub fn parse_usc_json(json: &str) -> Result<AnyUscCard, String> {
    let value: JsonValue = serde_json::from_str(json).map_err(|e| {
        crate::utils::err_msg(module_path!(), line!(), format!("Invalid USC JSON: {}", e))
    })?;
    parse_usc_value(&value)
}

pub fn create_system_prompt_template_usc(
    template: &SystemPromptTemplate,
) -> UscSystemPromptTemplateCard {
    UscCard {
        schema: UscSchemaInfo::default(),
        kind: UscKind::SystemPromptTemplate,
        payload: UscSystemPromptTemplatePayload {
            id: template.id.clone(),
            name: template.name.clone(),
            description: None,
            prompt_type: template.prompt_type,
            content: template.content.clone(),
            entries: template.entries.clone(),
            condense_prompt_entries: template.condense_prompt_entries,
            created_at: template.created_at,
            updated_at: template.updated_at,
            variables: None,
            requires: None,
        },
        app_specific_settings: None,
        meta: None,
        extensions: None,
    }
}

pub fn create_lorebook_usc(lorebook: &Lorebook, entries: &[LorebookEntry]) -> UscLorebookCard {
    UscCard {
        schema: UscSchemaInfo::default(),
        kind: UscKind::Lorebook,
        payload: UscLorebookPayload {
            id: lorebook.id.clone(),
            name: lorebook.name.clone(),
            description: None,
            keyword_detection_mode: Some(match lorebook.keyword_detection_mode {
                crate::storage_manager::lorebook::LorebookKeywordDetectionMode::RecentMessageWindow => {
                    "recentMessageWindow".to_string()
                }
                crate::storage_manager::lorebook::LorebookKeywordDetectionMode::LatestUserMessage => {
                    "latestUserMessage".to_string()
                }
            }),
            entries: entries
                .iter()
                .map(|entry| UscLorebookEntry {
                    id: entry.id.clone(),
                    title: entry.title.clone(),
                    enabled: entry.enabled,
                    always_active: entry.always_active,
                    keywords: entry.keywords.clone(),
                    case_sensitive: entry.case_sensitive,
                    content: entry.content.clone(),
                    priority: entry.priority,
                    display_order: entry.display_order,
                    created_at: entry.created_at,
                    updated_at: entry.updated_at,
                })
                .collect(),
            created_at: lorebook.created_at,
            updated_at: lorebook.updated_at,
            scope: None,
            target_ids: None,
            requires: None,
        },
        app_specific_settings: None,
        meta: None,
        extensions: None,
    }
}

pub fn create_chat_template_usc(
    template: &ChatTemplate,
    messages: &[ChatTemplateMessage],
) -> UscChatTemplateCard {
    let mut ordered_messages: Vec<&ChatTemplateMessage> = messages.iter().collect();
    ordered_messages.sort_by_key(|message| message.idx);

    UscCard {
        schema: UscSchemaInfo::default(),
        kind: UscKind::ChatTemplate,
        payload: UscChatTemplatePayload {
            id: template.id.clone(),
            name: template.name.clone(),
            description: None,
            messages: ordered_messages
                .into_iter()
                .map(|message| UscChatTemplateMessage {
                    id: message.id.clone(),
                    role: message.role.clone(),
                    content: message.content.clone(),
                })
                .collect(),
            scene_id: template.scene_id.clone(),
            system_prompt_template: template.prompt_template_id.as_ref().map(|id| UscRef {
                kind: UscKind::SystemPromptTemplate,
                id: id.clone(),
                name: None,
                optional: None,
            }),
            lorebook_ids_override: template
                .lorebook_ids_override
                .as_ref()
                .and_then(|raw| serde_json::from_str::<Vec<String>>(raw).ok()),
            variables: None,
            opening_notes: None,
            requires: None,
            created_at: template.created_at,
        },
        app_specific_settings: None,
        meta: None,
        extensions: None,
    }
}

pub fn create_model_profile_usc(model: &Model) -> UscModelProfileCard {
    UscCard {
        schema: UscSchemaInfo::default(),
        kind: UscKind::ModelProfile,
        payload: UscModelProfilePayload {
            id: model.id.clone(),
            name: model.name.clone(),
            display_name: model.display_name.clone(),
            provider_id: model.provider_id.clone(),
            provider_label: model.provider_label.clone(),
            description: None,
            input_scopes: model.input_scopes.clone(),
            output_scopes: model.output_scopes.clone(),
            advanced_model_settings: model.advanced_model_settings.clone(),
            system_prompt_template: model.prompt_template_id.as_ref().map(|id| UscRef {
                kind: UscKind::SystemPromptTemplate,
                id: id.clone(),
                name: None,
                optional: None,
            }),
            system_prompt: model.system_prompt.clone(),
            reasoning_support: None,
            capabilities: None,
            requires: None,
            created_at: model.created_at,
        },
        app_specific_settings: None,
        meta: None,
        extensions: None,
    }
}
