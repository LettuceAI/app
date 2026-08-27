use lettuce_models::ResolvedChatParameters;

use crate::descriptor::{
    ApiKeyRequirement, ParameterFlags, PromptCachingSupport, ProviderDescriptor, ReasoningSupport,
};
use crate::openai::OpenAi;
use crate::openai_compatible::{OpenAiWireProvider, WireParameters};

pub(crate) struct LiteRouter;

impl OpenAiWireProvider for LiteRouter {
    fn descriptor(&self) -> &'static ProviderDescriptor {
        &DESCRIPTOR
    }

    fn wire_parameters(&self, parameters: &ResolvedChatParameters) -> WireParameters {
        OpenAi.wire_parameters(parameters)
    }
}

pub(crate) const DESCRIPTOR: ProviderDescriptor = ProviderDescriptor {
    kind: "literouter",
    display_name: "LiteRouter",
    protocol: lettuce_models::ProviderProtocol::OpenAiCompatible,
    aliases: &[],
    default_endpoint: Some("https://api.literouter.com/v1"),
    endpoint_editable: false,
    api_key: ApiKeyRequirement::Required,
    auth_header: "Authorization",
    streaming: true,
    lists_models: true,
    verifies_key: true,
    reasoning: ReasoningSupport::None,
    prompt_caching: PromptCachingSupport::None,
    parameters: ParameterFlags::BASIC,
    extra_body_keys: &[],
};
