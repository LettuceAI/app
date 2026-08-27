use crate::descriptor::{
    ApiKeyRequirement, ParameterFlags, PromptCachingSupport, ProviderDescriptor, ReasoningSupport,
};
use crate::openai_compatible::OpenAiWireProvider;

pub(crate) struct Anannas;

impl OpenAiWireProvider for Anannas {
    fn descriptor(&self) -> &'static ProviderDescriptor {
        &DESCRIPTOR
    }
}

pub(crate) const DESCRIPTOR: ProviderDescriptor = ProviderDescriptor {
    kind: "anannas",
    display_name: "Anannas AI",
    protocol: lettuce_models::ProviderProtocol::OpenAiCompatible,
    aliases: &[],
    default_endpoint: Some("https://api.anannas.ai/v1"),
    endpoint_editable: false,
    api_key: ApiKeyRequirement::Required,
    auth_header: "Authorization",
    streaming: true,
    lists_models: true,
    verifies_key: true,
    reasoning: ReasoningSupport::Effort,
    prompt_caching: PromptCachingSupport::None,
    parameters: ParameterFlags::PENALTIES,
    extra_body_keys: &[],
};
