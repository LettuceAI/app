use lettuce_models::ProviderConfig;
use lettuce_settings::HeaderName;

use crate::anthropic_messages::AnthropicWireProvider;
use crate::common::{AdapterError, AuthPlan};
use crate::descriptor::{
    ApiKeyRequirement, ParameterFlags, PromptCachingSupport, ProviderDescriptor, ReasoningSupport,
};

pub(crate) struct Anthropic;

impl AnthropicWireProvider for Anthropic {
    fn descriptor(&self) -> &'static ProviderDescriptor {
        &DESCRIPTOR
    }

    fn auth(&self, _config: &ProviderConfig) -> Result<AuthPlan, AdapterError> {
        HeaderName::new("x-api-key")
            .map(AuthPlan::Header)
            .map_err(|_| AdapterError::Rejected)
    }
}

pub(crate) const DESCRIPTOR: ProviderDescriptor = ProviderDescriptor {
    kind: "anthropic",
    display_name: "Anthropic",
    protocol: lettuce_models::ProviderProtocol::Anthropic,
    aliases: &[],
    default_endpoint: Some("https://api.anthropic.com"),
    endpoint_editable: false,
    api_key: ApiKeyRequirement::Required,
    auth_header: "x-api-key",
    streaming: true,
    lists_models: true,
    verifies_key: true,
    reasoning: ReasoningSupport::BudgetOnly,
    prompt_caching: PromptCachingSupport::Supported,
    parameters: ParameterFlags {
        top_k: true,
        reasoning_budget: true,
        ..ParameterFlags::BASIC
    },
    extra_body_keys: &["promptCachingTtl"],
};
