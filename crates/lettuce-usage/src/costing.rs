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
    #[serde(default)]
    pub openrouter: Option<crate::OpenRouterCostEvidence>,
}

impl UsageCostBasis {
    pub fn calculate(&self, event: &UsageEvent) -> Result<RequestCost, UsageLedgerError> {
        if self.openrouter.is_some() {
            return Err(UsageLedgerError::Invalid);
        }
        let lettuce_conversations::UsageCounters::Known(tokens) = &event.record.usage else {
            return Err(UsageLedgerError::Invalid);
        };
        self.calculate_usage(
            event.record.model_profile_id,
            event.record.provider_account_id,
            tokens,
        )
    }

    pub fn calculate_job(
        &self,
        event: &crate::JobInferenceUsage,
    ) -> Result<RequestCost, UsageLedgerError> {
        let Some(crate::JobInferenceUsageResult::Response {
            usage: Some(tokens),
            provider_response_id,
        }) = &event.result
        else {
            return Err(UsageLedgerError::Invalid);
        };
        let billing;
        let tokens = if let Some(evidence) = &self.openrouter {
            if self.pricing != evidence.endpoint.pricing {
                return Err(UsageLedgerError::Invalid);
            }
            billing = evidence.billing_usage(provider_response_id.as_deref(), tokens)?;
            &billing
        } else {
            tokens
        };
        self.calculate_usage(
            Some(event.model_profile_id),
            Some(event.provider_account_id),
            tokens,
        )
    }

    pub fn from_openrouter_job(
        event: &crate::JobInferenceUsage,
        generation: crate::OpenRouterGenerationDetails,
        endpoints: &[crate::OpenRouterEndpointPricing],
        captured_at: TimestampMillis,
    ) -> Result<Option<Self>, UsageLedgerError> {
        let Some(crate::JobInferenceUsageResult::Response {
            usage: Some(usage),
            provider_response_id,
        }) = &event.result
        else {
            return Ok(None);
        };
        if provider_response_id.as_deref() != Some(generation.generation_id.as_str()) {
            return Err(UsageLedgerError::Invalid);
        }
        let Some(provider) = generation.provider_name.as_deref() else {
            return Ok(None);
        };
        let mut matches = endpoints
            .iter()
            .filter(|endpoint| endpoint.matches_provider(provider));
        let Some(endpoint) = matches.next() else {
            return Ok(None);
        };
        if matches.next().is_some()
            || generation.native_prompt_tokens.is_none()
            || generation.native_completion_tokens.is_none()
        {
            return Ok(None);
        }
        let evidence = crate::OpenRouterCostEvidence {
            generation,
            endpoint: endpoint.clone(),
        };
        let billing = evidence.billing_usage(provider_response_id.as_deref(), usage)?;
        let (
            Some(cached_prompt_tokens),
            Some(cache_write_tokens),
            Some(reasoning_tokens),
            Some(web_search_requests),
        ) = (
            billing.cached_input_tokens,
            billing.cache_write_tokens,
            billing.reasoning_tokens,
            billing.web_search_requests,
        )
        else {
            return Ok(None);
        };
        let basis = Self {
            model_profile_id: event.model_profile_id,
            provider_account_id: event.provider_account_id,
            source: format!(
                "OpenRouter generation {} endpoint {}",
                evidence.generation.generation_id,
                evidence
                    .endpoint
                    .tag
                    .as_deref()
                    .unwrap_or(&evidence.endpoint.provider_name)
            ),
            captured_at,
            pricing: endpoint.pricing.clone(),
            input: OpenRouterCostInput {
                prompt_tokens: billing.input_tokens,
                completion_tokens: billing.output_tokens,
                cached_prompt_tokens,
                cache_write_tokens,
                reasoning_tokens,
                web_search_requests,
                authoritative_total_cost: billing.provider_reported_cost.map(|cost| cost.get()),
            },
            openrouter: Some(evidence),
        };
        basis.calculate_job(event)?;
        Ok(Some(basis))
    }

    fn calculate_usage(
        &self,
        model_profile_id: Option<ModelProfileId>,
        provider_account_id: Option<ProviderAccountId>,
        tokens: &lettuce_conversations::InferenceUsage,
    ) -> Result<RequestCost, UsageLedgerError> {
        if self.source.trim().is_empty()
            || model_profile_id != Some(self.model_profile_id)
            || provider_account_id != Some(self.provider_account_id)
            || tokens.input_tokens != self.input.prompt_tokens
            || tokens.output_tokens != self.input.completion_tokens
            || tokens
                .provider_reported_cost
                .is_some_and(|value| Some(value.get()) != self.input.authoritative_total_cost)
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

    fn record_job_cost(
        &self,
        event_id: UsageEventId,
        basis: UsageCostBasis,
    ) -> Result<UsageCost, UsageLedgerError>;
    fn get_job_cost(&self, event_id: UsageEventId) -> Result<Option<UsageCost>, UsageLedgerError>;
}
