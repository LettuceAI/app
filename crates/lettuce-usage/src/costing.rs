use crate::{ModelPricing, OpenRouterCostInput, RequestCost, UsageEvent, UsageLedgerError};
use lettuce_types::{ModelProfileId, ProviderAccountId, TimestampMillis, UsageEventId};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UsageCostBasis {
    pub model_profile_id: ModelProfileId,
    pub provider_account_id: ProviderAccountId,
    pub source: String,
    pub captured_at: TimestampMillis,
    pub pricing: ModelPricing,
    pub input: OpenRouterCostInput,
}

impl UsageCostBasis {
    pub fn calculate(&self, event: &UsageEvent) -> Result<RequestCost, UsageLedgerError> {
        let lettuce_conversations::UsageCounters::Known(tokens) = &event.record.usage else {
            return Err(UsageLedgerError::Invalid);
        };
        if self.source.trim().is_empty()
            || event.record.model_profile_id != Some(self.model_profile_id)
            || event.record.provider_account_id != Some(self.provider_account_id)
            || tokens.input_tokens != self.input.prompt_tokens
            || tokens.output_tokens != self.input.completion_tokens
            || tokens
                .cache_write_tokens
                .is_some_and(|value| value != self.input.cache_write_tokens)
            || tokens
                .web_search_requests
                .is_some_and(|value| value != self.input.web_search_requests)
            || tokens
                .cached_input_tokens
                .is_some_and(|value| value != self.input.cached_prompt_tokens)
            || tokens
                .reasoning_tokens
                .is_some_and(|value| value != self.input.reasoning_tokens)
            || self
                .input
                .authoritative_total_cost
                .is_some_and(|value| !value.is_finite() || value < 0.0)
        {
            return Err(UsageLedgerError::Invalid);
        }
        crate::calculate_openrouter_request_cost(&self.input, &self.pricing)
            .ok_or(UsageLedgerError::Invalid)
    }
}

#[derive(Debug, Clone)]
pub struct UsageCost {
    pub event_id: UsageEventId,
    pub basis: UsageCostBasis,
    pub cost: RequestCost,
}

pub trait UsageCostLedger: Send + Sync {
    fn record_cost(
        &self,
        event_id: UsageEventId,
        basis: UsageCostBasis,
    ) -> Result<UsageCost, UsageLedgerError>;
    fn get_cost(&self, event_id: UsageEventId) -> Result<Option<UsageCost>, UsageLedgerError>;
}
