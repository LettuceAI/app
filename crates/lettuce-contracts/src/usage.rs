use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct UsageClearBeforeRequest {
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub before: i64,
    pub client_operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct UsageCleared {
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub removed: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct UsageRow {
    pub id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub timestamp: i64,
    pub status: UsageStatus,
    pub session_id: Option<String>,
    pub character_id: Option<String>,
    pub character_name: Option<String>,
    pub model_id: Option<String>,
    pub model_name: Option<String>,
    pub provider_kind: Option<String>,
    pub provider_label: Option<String>,
    pub operation_type: Option<String>,
    pub finish_reason: Option<String>,
    pub provider_response_id: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub prompt_tokens: Option<u64>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub cached_prompt_tokens: Option<u64>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub cache_write_tokens: Option<u64>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub completion_tokens: Option<u64>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub reasoning_tokens: Option<u64>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub image_tokens: Option<u64>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub audio_tokens: Option<u64>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub web_search_requests: Option<u64>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub total_tokens: Option<u64>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub memory_tokens: Option<u64>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub summary_tokens: Option<u64>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub input_image_count: Option<u64>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub output_image_count: Option<u64>,
    pub prompt_cost: Option<f64>,
    pub cache_read_cost: Option<f64>,
    pub cache_write_cost: Option<f64>,
    pub completion_cost: Option<f64>,
    pub reasoning_cost: Option<f64>,
    pub request_cost: Option<f64>,
    pub web_search_cost: Option<f64>,
    pub total_cost: Option<f64>,
    pub api_cost: Option<f64>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct UsageTotals {
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub requests: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub successful_requests: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub failed_requests: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub cancelled_requests: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub interrupted_requests: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub pending_requests: u64,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub prompt_tokens: Option<u64>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub completion_tokens: Option<u64>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub total_tokens: Option<u64>,
    pub total_cost: Option<f64>,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub unknown_token_requests: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub unknown_cost_requests: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum UsageStatus {
    Pending,
    Succeeded,
    Failed,
    Cancelled,
    Interrupted,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum UsageSort {
    #[default]
    NewestFirst,
    OldestFirst,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum UsageGroupBy {
    Day,
    Model,
    Provider,
    Character,
    Operation,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct UsageDateRange {
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub start: Option<i64>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub end: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct UsageFilters {
    #[serde(default)]
    pub range: UsageDateRange,
    pub provider_kind: Option<String>,
    pub model_id: Option<String>,
    pub character_id: Option<String>,
    pub operation_kind: Option<String>,
    pub status: Option<UsageStatus>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct UsageQueryRequest {
    pub filters: UsageFilters,
    pub sort: UsageSort,
    pub cursor: Option<String>,
    pub limit: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct UsagePage {
    pub items: Vec<UsageRow>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct UsageStatsRequest {
    pub range: UsageDateRange,
    pub provider_kind: Option<String>,
    pub group_by: UsageGroupBy,
    pub time_zone: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct UsageGroupTotals {
    pub key: Option<String>,
    pub label: Option<String>,
    pub totals: UsageTotals,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct UsageStats {
    pub totals: UsageTotals,
    pub groups: Vec<UsageGroupTotals>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct UsageExportCsvRequest {
    pub filters: UsageFilters,
    pub target: crate::FileTarget,
}
