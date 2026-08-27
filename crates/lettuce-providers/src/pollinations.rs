use std::borrow::Cow;

use lettuce_conversations::MessageRole;
use lettuce_models::ProviderConfig;

use crate::descriptor::{
    ApiKeyRequirement, ParameterFlags, PromptCachingSupport, ProviderDescriptor, ReasoningSupport,
};
use crate::openai::OpenAi;
use crate::openai_compatible::OpenAiWireProvider;

pub(crate) struct Pollinations;

impl OpenAiWireProvider for Pollinations {
    fn descriptor(&self) -> &'static ProviderDescriptor {
        &DESCRIPTOR
    }

    fn role(&self, role: MessageRole, config: &ProviderConfig) -> Option<Cow<'static, str>> {
        OpenAi.role(role, config)
    }
}

pub(crate) const DESCRIPTOR: ProviderDescriptor = ProviderDescriptor {
    kind: "pollinations",
    display_name: "Pollinations AI",
    protocol: lettuce_models::ProviderProtocol::OpenAiCompatible,
    aliases: &[],
    default_endpoint: Some("https://gen.pollinations.ai"),
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
