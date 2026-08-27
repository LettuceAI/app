use lettuce_models::ProviderConfig;

use crate::descriptor::{
    ApiKeyRequirement, ParameterFlags, PromptCachingSupport, ProviderDescriptor, ReasoningSupport,
};
use crate::openai_compatible::{AdapterError, AuthPlan, OpenAiWireProvider};

pub(crate) struct IntenseRp;

impl OpenAiWireProvider for IntenseRp {
    fn descriptor(&self) -> &'static ProviderDescriptor {
        &DESCRIPTOR
    }

    fn auth(&self, _config: &ProviderConfig) -> Result<AuthPlan, AdapterError> {
        Ok(AuthPlan::None)
    }
}

pub(crate) const DESCRIPTOR: ProviderDescriptor = ProviderDescriptor {
    kind: "intenserp",
    display_name: "IntenseRP Next (Local)",
    protocol: lettuce_models::ProviderProtocol::OpenAiCompatible,
    aliases: &[],
    default_endpoint: Some("http://127.0.0.1:7777/v1"),
    endpoint_editable: true,
    api_key: ApiKeyRequirement::NotUsed,
    auth_header: "",
    streaming: true,
    lists_models: true,
    verifies_key: false,
    reasoning: ReasoningSupport::Effort,
    prompt_caching: PromptCachingSupport::None,
    parameters: ParameterFlags::PENALTIES_TOP_K_BUDGET,
    extra_body_keys: &[],
};
