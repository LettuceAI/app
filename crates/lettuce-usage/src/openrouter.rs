use crate::ModelPricing;
use lettuce_conversations::ProviderReportedCost;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct OpenRouterEndpointPricing {
    pub provider_name: String,
    pub provider_display_name: Option<String>,
    pub tag: Option<String>,
    pub pricing: ModelPricing,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct OpenRouterCostEvidence {
    pub generation: OpenRouterGenerationDetails,
    pub endpoint: OpenRouterEndpointPricing,
}

impl OpenRouterCostEvidence {
    pub(crate) fn billing_usage(
        &self,
        response_id: Option<&str>,
        usage: &lettuce_conversations::InferenceUsage,
    ) -> Result<lettuce_conversations::InferenceUsage, crate::UsageLedgerError> {
        let generation = &self.generation;
        if response_id != Some(generation.generation_id.as_str())
            || generation.generation_id.trim().is_empty()
            || generation.model.trim().is_empty()
            || !generation
                .provider_name
                .as_deref()
                .is_some_and(|name| self.endpoint.matches_provider(name))
        {
            return Err(crate::UsageLedgerError::Invalid);
        }
        let mut billing = usage.clone();
        billing.input_tokens = generation
            .native_prompt_tokens
            .ok_or(crate::UsageLedgerError::Invalid)?;
        billing.output_tokens = generation
            .native_completion_tokens
            .ok_or(crate::UsageLedgerError::Invalid)?;
        billing.cached_input_tokens = generation
            .native_cached_tokens
            .or(usage.cached_input_tokens);
        billing.reasoning_tokens = generation
            .native_reasoning_tokens
            .or(usage.reasoning_tokens);
        billing.provider_reported_cost = generation.total_cost.or(usage.provider_reported_cost);
        Ok(billing)
    }
}

impl OpenRouterEndpointPricing {
    pub(crate) fn matches_provider(&self, name: &str) -> bool {
        !name.trim().is_empty()
            && (self.provider_name.trim().eq_ignore_ascii_case(name.trim())
                || self
                    .provider_display_name
                    .as_deref()
                    .is_some_and(|display| display.trim().eq_ignore_ascii_case(name.trim())))
    }
}
