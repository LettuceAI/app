use crate::models::RequestCost;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum UsageOperationType {
    Chat,
    Regenerate,
    Continue,
    Summary,
    MemoryManager,
    ImageGeneration,
    AICreator,
    ReplyHelper,
    GroupChatMessage,
    GroupChatRegenerate,
    GroupChatContinue,
    GroupChatDecisionMaker,
}

impl UsageOperationType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "chat" => Some(Self::Chat),
            "regenerate" => Some(Self::Regenerate),
            "continue" => Some(Self::Continue),
            "summary" => Some(Self::Summary),
            "memory_manager" => Some(Self::MemoryManager),
            "image_generation" | "image generation" => Some(Self::ImageGeneration),
            "ai_creator" | "ai creator" => Some(Self::AICreator),
            "reply_helper" => Some(Self::ReplyHelper),
            "group_chat_message" | "group_chat" => Some(Self::GroupChatMessage),
            "group_chat_regenerate" => Some(Self::GroupChatRegenerate),
            "group_chat_continue" => Some(Self::GroupChatContinue),
            "group_chat_decision_maker" | "decision_maker" => Some(Self::GroupChatDecisionMaker),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::Regenerate => "regenerate",
            Self::Continue => "continue",
            Self::Summary => "summary",
            Self::MemoryManager => "memory_manager",
            Self::ImageGeneration => "image_generation",
            Self::AICreator => "ai_creator",
            Self::ReplyHelper => "reply_helper",
            Self::GroupChatMessage => "group_chat_message",
            Self::GroupChatRegenerate => "group_chat_regenerate",
            Self::GroupChatContinue => "group_chat_continue",
            Self::GroupChatDecisionMaker => "group_chat_decision_maker",
        }
    }
}

impl fmt::Display for UsageOperationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum UsageFinishReason {
    Stop,
    Length,
    ContentFilter,
    ToolCalls,
    Aborted,
    Error,
    Other,
}

impl UsageFinishReason {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "stop" | "end_turn" | "stop_sequence" => Some(Self::Stop),
            "length" | "max_tokens" => Some(Self::Length),
            "content_filter" | "safety" | "recitation" | "language" | "prohibited_content"
            | "spii" => Some(Self::ContentFilter),
            "tool_calls" | "function_call" | "tool_use" => Some(Self::ToolCalls),
            "aborted" => Some(Self::Aborted),
            "error" => Some(Self::Error),
            _ => Some(Self::Other),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Stop => "stop",
            Self::Length => "length",
            Self::ContentFilter => "content_filter",
            Self::ToolCalls => "tool_calls",
            Self::Aborted => "aborted",
            Self::Error => "error",
            Self::Other => "other",
        }
    }
}

impl fmt::Display for UsageFinishReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Individual request/message usage tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestUsage {
    pub id: String,                               // Unique request ID
    pub timestamp: u64,                           // Unix timestamp in milliseconds
    pub session_id: String,                       // Which session this belongs to
    pub character_id: String,                     // Which character
    pub character_name: String,                   // Character name for display
    pub model_id: String,                         // Which model
    pub model_name: String,                       // Model name for display
    pub provider_id: String, // Which provider (openai, anthropic, openrouter, etc)
    pub provider_label: String, // Provider label for display
    pub operation_type: UsageOperationType, // Type of operation
    pub finish_reason: Option<UsageFinishReason>, // Reason why the request finished

    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    #[serde(default)]
    pub cached_prompt_tokens: Option<u64>,
    #[serde(default)]
    pub cache_write_tokens: Option<u64>,

    // Token breakdown for prompt analysis
    pub memory_tokens: Option<u64>,  // Tokens from memory embeddings
    pub summary_tokens: Option<u64>, // Tokens from memory summary
    // Extended token tracking
    #[serde(default)]
    pub reasoning_tokens: Option<u64>, // Tokens used for reasoning/thinking
    #[serde(default)]
    pub image_tokens: Option<u64>, // Tokens used for image processing
    #[serde(default)]
    pub web_search_requests: Option<u64>,
    #[serde(default)]
    pub api_cost: Option<f64>,

    pub cost: Option<RequestCost>, // Calculated cost (only for OpenRouter for now)

    pub success: bool,
    pub error_message: Option<String>, // Error message if failed

    #[serde(default)]
    pub metadata: HashMap<String, String>, // Additional metadata
}

/// Summary statistics for usage tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageStats {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub total_tokens: u64,
    pub total_cost: f64,
    pub average_cost_per_request: f64,
    pub by_provider: HashMap<String, ProviderStats>,
    pub by_model: HashMap<String, ModelStats>,
    pub by_character: HashMap<String, CharacterStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStats {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub total_tokens: u64,
    pub total_cost: f64,
}

impl ProviderStats {
    pub fn empty() -> Self {
        Self {
            total_requests: 0,
            successful_requests: 0,
            total_tokens: 0,
            total_cost: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStats {
    pub provider_id: String,
    pub total_requests: u64,
    pub successful_requests: u64,
    pub total_tokens: u64,
    pub total_cost: f64,
}

impl ModelStats {
    pub fn empty(provider_id: &str) -> Self {
        Self {
            provider_id: provider_id.to_string(),
            total_requests: 0,
            successful_requests: 0,
            total_tokens: 0,
            total_cost: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterStats {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub total_tokens: u64,
    pub total_cost: f64,
}

impl CharacterStats {
    pub fn empty() -> Self {
        Self {
            total_requests: 0,
            successful_requests: 0,
            total_tokens: 0,
            total_cost: 0.0,
        }
    }
}

/// Filter for querying usage records
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UsageFilter {
    pub start_timestamp: Option<u64>, // From date
    pub end_timestamp: Option<u64>,   // To date
    pub provider_id: Option<String>,  // Filter by provider
    pub model_id: Option<String>,     // Filter by model
    pub character_id: Option<String>, // Filter by character
    pub session_id: Option<String>,   // Filter by session
    pub success_only: Option<bool>,   // Only successful requests
}
