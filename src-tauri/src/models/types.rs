use serde::{Deserialize, Serialize};

/// Pricing information for a model (values are USD costs expressed as strings).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelPricing {
    /// Price per input token in USD.
    pub prompt: String,
    /// Price per output token in USD.
    pub completion: String,
    /// Flat price per request in USD.
    #[serde(default)]
    pub request: String,
    /// Price per image-related unit in USD.
    #[serde(default)]
    pub image: String,
    /// Price per output image-related unit in USD.
    #[serde(default)]
    pub image_output: String,
    /// Price per web search
    #[serde(default)]
    pub web_search: String,
    /// Price per internal reasoning token
    #[serde(default)]
    pub internal_reasoning: String,
    /// Price per cached prompt token read in USD.
    #[serde(default)]
    pub input_cache_read: String,
    /// Price per cached prompt token write in USD.
    #[serde(default)]
    pub input_cache_write: String,
}

/// Cost calculation result for a single request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestCost {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    #[serde(default)]
    pub regular_prompt_tokens: u64,
    #[serde(default)]
    pub cached_prompt_tokens: u64,
    #[serde(default)]
    pub cache_write_tokens: u64,
    #[serde(default)]
    pub reasoning_tokens: u64,
    #[serde(default)]
    pub web_search_requests: u64,
    /// Cost for prompt tokens
    pub prompt_cost: f64,
    /// Prompt cost before cache discounts/writes.
    #[serde(default)]
    pub prompt_base_cost: f64,
    /// Cost for reading cached prompt tokens.
    #[serde(default)]
    pub cache_read_cost: f64,
    /// Cost for writing cacheable prompt tokens.
    #[serde(default)]
    pub cache_write_cost: f64,
    /// Cost for completion tokens
    pub completion_cost: f64,
    /// Visible completion token cost before reasoning/request/search charges.
    #[serde(default)]
    pub completion_base_cost: f64,
    /// Cost of billed reasoning tokens.
    #[serde(default)]
    pub reasoning_cost: f64,
    /// Flat per-request charge.
    #[serde(default)]
    pub request_cost: f64,
    /// Cost from web-search tool calls.
    #[serde(default)]
    pub web_search_cost: f64,
    /// Total cost in USD
    pub total_cost: f64,
    /// Provider-reported total, when available.
    #[serde(default)]
    pub authoritative_total_cost: Option<f64>,
}
