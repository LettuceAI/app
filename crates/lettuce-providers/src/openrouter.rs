use lettuce_network::JsonStaticHeader;

use crate::descriptor::{
    ApiKeyRequirement, ParameterFlags, PromptCachingSupport, ProviderDescriptor, ReasoningSupport,
};
use crate::openai_compatible::OpenAiWireProvider;

const OPENROUTER_HEADERS: [JsonStaticHeader; 2] = [
    JsonStaticHeader {
        name: "x-openrouter-title",
        value: "LettuceAI",
    },
    JsonStaticHeader {
        name: "x-openrouter-categories",
        value: "roleplay",
    },
];

pub(crate) struct OpenRouter;

impl OpenAiWireProvider for OpenRouter {
    fn descriptor(&self) -> &'static ProviderDescriptor {
        &DESCRIPTOR
    }

    fn static_headers(&self) -> &'static [JsonStaticHeader] {
        &OPENROUTER_HEADERS
    }
}

pub(crate) const DESCRIPTOR: ProviderDescriptor = ProviderDescriptor {
    kind: "openrouter",
    display_name: "OpenRouter",
    protocol: lettuce_models::ProviderProtocol::OpenAiCompatible,
    aliases: &[],
    default_endpoint: Some("https://openrouter.ai/api"),
    endpoint_editable: false,
    api_key: ApiKeyRequirement::Required,
    auth_header: "Authorization",
    streaming: true,
    lists_models: true,
    verifies_key: true,
    reasoning: ReasoningSupport::Dynamic,
    prompt_caching: PromptCachingSupport::Supported,
    parameters: ParameterFlags {
        context_length: true,
        ..ParameterFlags::PENALTIES_TOP_K_BUDGET
    },
    extra_body_keys: &["promptCachingTtl", "provider"],
};
