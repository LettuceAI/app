use lettuce_models::ResolvedChatParameters;

use crate::descriptor::{
    ApiKeyRequirement, ParameterFlags, PromptCachingSupport, ProviderDescriptor, ReasoningSupport,
};
use crate::openai::OpenAi;
use crate::openai_compatible::{OpenAiWireProvider, ReasoningWirePolicy, WireParameters};

pub(crate) struct Cerebras;

impl OpenAiWireProvider for Cerebras {
    fn descriptor(&self) -> &'static ProviderDescriptor {
        &DESCRIPTOR
    }

    fn wire_parameters(&self, parameters: &ResolvedChatParameters) -> WireParameters {
        OpenAi.wire_parameters(parameters)
    }

    fn reasoning_policy(&self) -> ReasoningWirePolicy {
        OpenAi.reasoning_policy()
    }
}

pub(crate) const DESCRIPTOR: ProviderDescriptor = ProviderDescriptor {
    kind: "cerebras",
    display_name: "Cerebras",
    protocol: lettuce_models::ProviderProtocol::OpenAiCompatible,
    aliases: &["cerebras.ai"],
    default_endpoint: Some("https://api.cerebras.ai/v1"),
    endpoint_editable: false,
    api_key: ApiKeyRequirement::Required,
    auth_header: "Authorization",
    streaming: true,
    lists_models: true,
    verifies_key: true,
    reasoning: ReasoningSupport::Effort,
    prompt_caching: PromptCachingSupport::None,
    parameters: ParameterFlags::PENALTIES_BUDGET,
    extra_body_keys: &[],
};
