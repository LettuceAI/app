use crate::ModelPricing;
use lettuce_conversations::ProviderReportedCost;

#[derive(Debug, Clone, PartialEq)]
pub struct OpenRouterEndpointPricing {
    pub provider_name: String,
    pub provider_display_name: Option<String>,
    pub tag: Option<String>,
    pub pricing: ModelPricing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenRouterGenerationDetails {
    pub generation_id: String,
    pub model: String,
    pub provider_name: Option<String>,
    pub native_prompt_tokens: Option<u64>,
    pub native_completion_tokens: Option<u64>,
    pub normalized_prompt_tokens: Option<u64>,
    pub normalized_completion_tokens: Option<u64>,
    pub native_cached_tokens: Option<u64>,
    pub native_reasoning_tokens: Option<u64>,
    pub total_cost: Option<ProviderReportedCost>,
}
