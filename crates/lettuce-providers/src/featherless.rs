use crate::descriptor::{
    ApiKeyRequirement, ParameterFlags, PromptCachingSupport, ProviderDescriptor, ReasoningSupport,
};
use crate::openai_compatible::OpenAiWireProvider;

pub(crate) struct Featherless;

impl OpenAiWireProvider for Featherless {
    fn descriptor(&self) -> &'static ProviderDescriptor {
        &DESCRIPTOR
    }
}

pub(crate) const DESCRIPTOR: ProviderDescriptor = ProviderDescriptor {
    kind: "featherless",
    display_name: "Featherless AI",
    protocol: lettuce_models::ProviderProtocol::OpenAiCompatible,
    aliases: &[],
    default_endpoint: Some("https://api.featherless.ai/v1"),
    endpoint_editable: false,
    api_key: ApiKeyRequirement::Required,
    auth_header: "Authorization",
    streaming: true,
    lists_models: true,
    verifies_key: true,
    reasoning: ReasoningSupport::None,
    prompt_caching: PromptCachingSupport::None,
    parameters: ParameterFlags::PENALTIES,
    extra_body_keys: &[],
};
