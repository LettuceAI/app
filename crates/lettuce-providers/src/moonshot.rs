use crate::descriptor::{
    ApiKeyRequirement, ParameterFlags, PromptCachingSupport, ProviderDescriptor, ReasoningSupport,
};
use crate::openai_compatible::{OpenAiWireProvider, ReasoningWirePolicy};

pub(crate) struct Moonshot;

impl OpenAiWireProvider for Moonshot {
    fn descriptor(&self) -> &'static ProviderDescriptor {
        &DESCRIPTOR
    }

    fn reasoning_policy(&self) -> ReasoningWirePolicy {
        ReasoningWirePolicy::EnableThinking
    }
}

pub(crate) const DESCRIPTOR: ProviderDescriptor = ProviderDescriptor {
    kind: "moonshot",
    display_name: "Moonshot AI (Kimi)",
    protocol: lettuce_models::ProviderProtocol::OpenAiCompatible,
    aliases: &["moonshot-ai"],
    default_endpoint: Some("https://api.moonshot.ai/v1"),
    endpoint_editable: false,
    api_key: ApiKeyRequirement::Required,
    auth_header: "Authorization",
    streaming: true,
    lists_models: true,
    verifies_key: true,
    reasoning: ReasoningSupport::BudgetOnly,
    prompt_caching: PromptCachingSupport::None,
    parameters: ParameterFlags::PENALTIES_BUDGET,
    extra_body_keys: &[],
};
