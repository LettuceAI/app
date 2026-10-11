use crate::{FileKind, FileSource, FileTarget, LorebookKeywordMode, PromptKind};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ImportCharacterContent {
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub definition: Option<String>,
    #[serde(default)]
    pub scenario: Option<String>,
    #[serde(default)]
    pub nickname: Option<String>,
    #[serde(default)]
    pub creator: Option<String>,
    #[serde(default)]
    pub creator_notes: Option<String>,
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Unknown>))]
    pub creator_notes_multilingual: Option<Value>,
    #[serde(default)]
    pub source: Option<Vec<String>>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Unknown>))]
    pub character_book: Option<Value>,
    pub rules: Vec<String>,
    pub scenes: Vec<ImportScene>,
    pub default_scene_id: Option<String>,
    pub default_model_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Unknown>))]
    pub companion: Option<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub companion_scheduled_notes: Vec<ImportScheduledNote>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub companion_shared_memory: Option<ImportSharedMemory>,
    #[serde(default)]
    pub memory_type: Option<String>,
    #[serde(default)]
    pub active_lorebook_ids: Vec<String>,
    #[serde(default)]
    pub lorebooks: Vec<ImportBundledLorebook>,
    pub prompt_template_id: Option<String>,
    pub system_prompt: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Unknown>))]
    pub voice_config: Option<Value>,
    pub voice_autoplay: Option<bool>,
    pub disable_avatar_gradient: bool,
    pub avatar_crop: Option<ImportCrop>,
    #[serde(default)]
    pub banner_crop: Option<ImportCrop>,
    pub custom_gradient_enabled: Option<bool>,
    pub custom_gradient_colors: Option<Vec<String>>,
    pub custom_text_color: Option<String>,
    pub custom_text_secondary: Option<String>,
    #[serde(default)]
    pub chat_templates: Vec<ImportStarter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_chat_template_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ImportCrop {
    pub x: f64,
    pub y: f64,
    pub scale: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ImportScheduledNote {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub character_id: Option<String>,
    #[serde(default)]
    pub label: String,
    pub content: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub available_at: i64,
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub expires_at: Option<i64>,
    pub recurrence: String,
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub recurrence_window_ms: Option<i64>,
    pub enabled: bool,
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub created_at: i64,
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ImportSharedMemory {
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Unknown))]
    pub memories: Value,
    #[serde(default)]
    pub memory_summary: Option<String>,
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub memory_summary_token_count: i64,
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Unknown))]
    pub memory_tool_events: Value,
    #[serde(default)]
    pub memory_status: Option<String>,
    #[serde(default)]
    pub memory_error: Option<String>,
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub memory_progress_step: Option<i64>,
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Unknown))]
    pub soul_growth: Value,
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Unknown))]
    pub relationship_states: Value,
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub created_at: i64,
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ImportBundledLorebook {
    pub lorebook: ImportLorebookMetadata,
    #[serde(default)]
    pub entries: Vec<ImportLorebookEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ImportLorebookMetadata {
    pub id: String,
    pub name: String,
    pub keyword_detection_mode: ImportKeywordDetection,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub created_at: i64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ImportLorebookEntry {
    pub id: String,
    pub lorebook_id: String,
    pub title: String,
    pub enabled: bool,
    pub always_active: bool,
    pub keywords: Vec<String>,
    pub case_sensitive: bool,
    pub keyword_match_mode: LorebookKeywordMode,
    pub content: String,
    pub priority: i32,
    pub display_order: i32,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub created_at: i64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ImportScene {
    pub id: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub created_at: Option<i64>,
    pub selected_variant_id: Option<String>,
    pub variants: Vec<ImportSceneVariant>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ImportSceneVariant {
    pub id: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub created_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ImportStarter {
    pub id: String,
    pub name: String,
    pub messages: Vec<ImportStarterMessage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub created_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ImportStarterMessage {
    pub id: String,
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ImportPersonaContent {
    pub title: String,
    pub description: String,
    pub nickname: Option<String>,
    pub is_default: Option<bool>,
    pub avatar_crop: Option<ImportCrop>,
    #[serde(default)]
    pub active_lorebook_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum ImportKeywordDetection {
    RecentMessageWindow,
    LatestUserMessage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum FileWarningCode {
    ImageDecodeFailed,
    ImageDownloadFailed,
    ImageStoreFailed,
    ImageReadFailed,
    RemoteAvatarWillDownload,
    RemoteAvatarNotDownloaded,
    FieldSkipped,
    FieldConverted,
    BundledLorebookNewCopy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct FileWarning {
    pub code: FileWarningCode,
    pub field: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ImportImagePreview {
    pub field: String,
    pub remote_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ImportPreviewContent {
    Character {
        content: Box<ImportCharacterContent>,
    },
    Persona {
        content: ImportPersonaContent,
    },
    Lorebook {
        name: String,
        keyword_detection_mode: ImportKeywordDetection,
        entries: Vec<ImportLorebookEntry>,
    },
    Prompt {
        name: String,
        purpose: PromptKind,
        entries: Vec<ImportPromptEntry>,
        condense: bool,
    },
    ChatTemplate {
        name: String,
        messages: Vec<ImportTemplateMessage>,
        scene_id: Option<String>,
        prompt_template_id: Option<String>,
        lorebook_ids_override: Option<Vec<String>>,
    },
    Model {
        name: String,
        provider_id: String,
        provider_label: String,
        display_name: String,
        input_scopes: Vec<String>,
        output_scopes: Vec<String>,
        #[cfg_attr(feature = "specta", specta(type = specta_typescript::Unknown))]
        settings: Value,
    },
    Chat {
        title: String,
        speakers: Vec<String>,
        #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
        message_count: u64,
        group: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ImportTemplateMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ImportPreviewRequest {
    pub source: FileSource,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ImportPreview {
    pub kind: FileKind,
    pub name: String,
    pub digest: String,
    pub content: ImportPreviewContent,
    pub images: Vec<ImportImagePreview>,
    pub warnings: Vec<FileWarning>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ImportOptions {
    #[serde(default)]
    pub character_id: Option<String>,
    #[serde(default)]
    pub speakers: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ImportCommitRequest {
    pub source: FileSource,
    pub digest: String,
    pub options: ImportOptions,
    pub client_operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ImportCommitResult {
    Imported {
        file_kind: FileKind,
        id: String,
        warnings: Vec<FileWarning>,
    },
    Job {
        job_id: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum CharacterExportFormat {
    Uec,
    V3,
    V2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LorebookExportFormat {
    WorldInfo,
    Usc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum EntityExportFormat {
    Json,
    Usc,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct CharacterExportRequest {
    pub id: String,
    pub format: CharacterExportFormat,
    pub target: FileTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct LorebookExportRequest {
    pub id: String,
    pub format: LorebookExportFormat,
    pub target: FileTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct EntityExportRequest {
    pub id: String,
    pub target: FileTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ModelExportRequest {
    pub id: String,
    pub format: EntityExportFormat,
    pub target: FileTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ChatTemplateExportRequest {
    pub character_id: String,
    pub starter_id: String,
    pub format: EntityExportFormat,
    pub target: FileTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct ChatExportRequest {
    pub conversation_id: String,
    pub target: FileTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct EntityExportResult {
    pub suggested_name: String,
    pub warnings: Vec<FileWarning>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ImportPromptEntry {
    pub name: String,
    pub role: String,
    pub content: String,
    pub enabled: bool,
    pub injection_position: String,
    pub depth: u32,
    pub conditional_min_messages: Option<u32>,
    pub interval_turns: Option<u32>,
    pub system_prompt: bool,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Unknown>))]
    pub conditions: Option<Value>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Unknown>))]
    pub payload: Option<Value>,
}
