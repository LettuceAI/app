//! Versioned IPC contracts and generated frontend boundary.

#![deny(unsafe_op_in_unsafe_fn)]

use lettuce_types::ProviderAccountId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderProtocolContract {
    OpenAiCompatible,
    Anthropic,
    Gemini,
    Ollama,
    LlamaCpp,
    StableDiffusion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiKeyRequirementContract {
    Required,
    Optional,
    NotUsed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningSupportContract {
    None,
    Effort,
    BudgetOnly,
    Dynamic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptCachingSupportContract {
    None,
    Supported,
    Automatic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptCacheRetentionContract {
    InMemory,
    FiveMinutes,
    OneHour,
    TwentyFourHours,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderParameterSupportContract {
    pub temperature: bool,
    pub top_p: bool,
    pub max_output_tokens: bool,
    pub context_length: bool,
    pub frequency_penalty: bool,
    pub presence_penalty: bool,
    pub top_k: bool,
    pub reasoning_budget: bool,
}

/// One provider row rendered by the settings UI. These values replace the
/// legacy frontend-owned provider and parameter-support tables.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderDescriptorContract {
    pub kind: String,
    pub display_name: String,
    pub protocol: ProviderProtocolContract,
    pub aliases: Vec<String>,
    pub default_endpoint: Option<String>,
    pub endpoint_editable: bool,
    pub api_key: ApiKeyRequirementContract,
    pub auth_header: String,
    pub streaming: bool,
    pub lists_models: bool,
    pub verifies_key: bool,
    pub reasoning: ReasoningSupportContract,
    pub prompt_caching: PromptCachingSupportContract,
    pub prompt_cache_retentions: Vec<PromptCacheRetentionContract>,
    pub parameters: ProviderParameterSupportContract,
    pub extra_body_keys: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderCatalogContract {
    pub providers: Vec<ProviderDescriptorContract>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderAccountRequest {
    pub provider_account_id: ProviderAccountId,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteModelContract {
    pub id: String,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub context_length: Option<u64>,
    pub input_modalities: Option<Vec<String>>,
    pub output_modalities: Option<Vec<String>>,
    pub supported_endpoints: Option<Vec<String>>,
    pub input_price: Option<f64>,
    pub output_price: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderModelsContract {
    pub provider_account_id: ProviderAccountId,
    pub models: Vec<RemoteModelContract>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyVerificationContract {
    pub provider_account_id: ProviderAccountId,
    pub valid: bool,
    pub status: Option<u16>,
}
