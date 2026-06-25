use serde::{Deserialize, Serialize};

use crate::chat_manager::tooling::ToolCall;

/// Draft character being built by the creation helper
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftCharacter {
    pub name: Option<String>,
    #[serde(default)]
    pub definition: Option<String>,
    pub description: Option<String>,
    pub scenes: Vec<DraftScene>,
    pub default_scene_id: Option<String>,
    pub avatar_path: Option<String>,
    pub background_image_path: Option<String>,
    pub disable_avatar_gradient: bool,
    pub default_model_id: Option<String>,
    pub prompt_template_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftScene {
    pub id: String,
    pub content: String,
    pub direction: Option<String>,
}

/// Message in the creation helper chat
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreationMessage {
    pub id: String,
    pub role: CreationMessageRole,
    pub content: String,
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
    #[serde(default)]
    pub tool_results: Vec<CreationToolResult>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocks: Vec<CreationBlock>,
    pub created_at: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CreationBlock {
    Text {
        content: String,
    },
    Tool {
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CreationMessageRole {
    User,
    Assistant,
    System,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreationToolResult {
    pub tool_call_id: String,
    pub result: serde_json::Value,
    pub success: bool,
}

/// State of a creation helper session
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreationSession {
    pub id: String,
    pub messages: Vec<CreationMessage>,
    pub draft: DraftCharacter,
    #[serde(default)]
    pub draft_history: Vec<DraftCharacter>,
    pub creation_goal: CreationGoal,
    #[serde(default)]
    pub creation_mode: CreationMode,
    #[serde(default)]
    pub target_type: Option<CreationGoal>,
    #[serde(default)]
    pub target_id: Option<String>,
    pub status: CreationStatus,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreationSessionSummary {
    pub id: String,
    pub creation_goal: CreationGoal,
    pub creation_mode: CreationMode,
    pub target_type: Option<CreationGoal>,
    pub target_id: Option<String>,
    pub status: CreationStatus,
    pub title: String,
    pub preview: String,
    pub message_count: usize,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum CreationMode {
    #[default]
    Create,
    Edit,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum CreationStatus {
    Active,
    PreviewShown,
    Completed,
    Cancelled,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CreationGoal {
    Character,
    Persona,
    Lorebook,
}

/// Uploaded image reference for use by the AI
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadedImage {
    pub id: String,
    #[serde(default)]
    pub data: String,
    pub mime_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset_id: Option<String>,
}

/// System prompt info returned by get_system_prompt_list
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemPromptInfo {
    pub id: String,
    pub name: String,
}

/// Model info returned by get_model_list
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub display_name: String,
}
