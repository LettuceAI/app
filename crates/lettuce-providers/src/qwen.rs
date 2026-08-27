use lettuce_network::JsonStaticHeader;

use crate::descriptor::{
    ApiKeyRequirement, ParameterFlags, PromptCachingSupport, ProviderDescriptor, ReasoningSupport,
};
use crate::openai_compatible::{ACCEPT_ONLY, OpenAiWireProvider};

pub(crate) struct Qwen;

impl OpenAiWireProvider for Qwen {
    fn descriptor(&self) -> &'static ProviderDescriptor {
        &DESCRIPTOR
    }

    fn static_headers(&self) -> &'static [JsonStaticHeader] {
        &ACCEPT_ONLY
    }
}

pub(crate) const DESCRIPTOR: ProviderDescriptor = ProviderDescriptor {
    kind: "qwen",
    display_name: "Qwen",
    protocol: lettuce_models::ProviderProtocol::OpenAiCompatible,
    aliases: &[],
    default_endpoint: Some("https://dashscope.aliyuncs.com/compatible-mode/v1"),
    endpoint_editable: false,
    api_key: ApiKeyRequirement::Required,
    auth_header: "Authorization",
    streaming: true,
    lists_models: true,
    verifies_key: true,
    reasoning: ReasoningSupport::BudgetOnly,
    prompt_caching: PromptCachingSupport::None,
    parameters: ParameterFlags::PENALTIES_TOP_K_BUDGET,
    extra_body_keys: &[],
};
