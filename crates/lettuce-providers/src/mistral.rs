use lettuce_models::{ProviderConfig, ResolvedChatParameters};
use lettuce_settings::HeaderName;

use crate::descriptor::{
    ApiKeyRequirement, ParameterFlags, PromptCachingSupport, ProviderDescriptor, ReasoningSupport,
};
use crate::openai_compatible::{
    AdapterError, AuthPlan, OpenAiWireProvider, WireParameters, standard_parameters,
};

pub(crate) struct Mistral;

impl OpenAiWireProvider for Mistral {
    fn descriptor(&self) -> &'static ProviderDescriptor {
        &DESCRIPTOR
    }

    fn auth(&self, _config: &ProviderConfig) -> Result<AuthPlan, AdapterError> {
        HeaderName::new("X-API-KEY")
            .map(AuthPlan::Header)
            .map_err(|_| AdapterError::Rejected)
    }

    fn wire_parameters(&self, parameters: &ResolvedChatParameters) -> WireParameters {
        WireParameters {
            frequency_penalty: None,
            presence_penalty: None,
            ..standard_parameters(parameters)
        }
    }
}

pub(crate) const DESCRIPTOR: ProviderDescriptor = ProviderDescriptor {
    kind: "mistral",
    display_name: "Mistral AI",
    protocol: lettuce_models::ProviderProtocol::OpenAiCompatible,
    aliases: &[],
    default_endpoint: Some("https://api.mistral.ai"),
    endpoint_editable: false,
    api_key: ApiKeyRequirement::Required,
    auth_header: "X-API-KEY",
    streaming: true,
    lists_models: true,
    verifies_key: true,
    reasoning: ReasoningSupport::None,
    prompt_caching: PromptCachingSupport::None,
    parameters: ParameterFlags::PENALTIES,
    extra_body_keys: &[],
};
