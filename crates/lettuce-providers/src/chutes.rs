use std::borrow::Cow;

use crate::descriptor::{
    ApiKeyRequirement, ParameterFlags, PromptCachingSupport, ProviderDescriptor, ReasoningSupport,
};
use crate::openai_compatible::{OpenAiWireProvider, ReasoningWirePolicy};

pub(crate) struct Chutes;

impl OpenAiWireProvider for Chutes {
    fn descriptor(&self) -> &'static ProviderDescriptor {
        &DESCRIPTOR
    }

    fn normalize_endpoint<'a>(&self, endpoint: &'a str) -> Cow<'a, str> {
        let trimmed = endpoint.trim();
        if trimmed.is_empty() {
            return Cow::Borrowed("https://llm.chutes.ai");
        }
        if trimmed.contains("://api.chutes.ai") || trimmed.contains("://www.api.chutes.ai") {
            Cow::Owned(
                trimmed
                    .replace("://www.api.chutes.ai", "://llm.chutes.ai")
                    .replace("://api.chutes.ai", "://llm.chutes.ai"),
            )
        } else {
            Cow::Borrowed(trimmed)
        }
    }

    fn reasoning_policy(&self) -> ReasoningWirePolicy {
        ReasoningWirePolicy::MaxTokens
    }
}

pub(crate) const DESCRIPTOR: ProviderDescriptor = ProviderDescriptor {
    kind: "chutes",
    display_name: "Chutes",
    protocol: lettuce_models::ProviderProtocol::OpenAiCompatible,
    aliases: &["chutes.ai"],
    default_endpoint: Some("https://llm.chutes.ai"),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_retired_api_host_to_llm_host() {
        assert_eq!(
            Chutes.normalize_endpoint("https://api.chutes.ai/v1"),
            "https://llm.chutes.ai/v1"
        );
        assert_eq!(
            Chutes.normalize_endpoint("https://www.api.chutes.ai"),
            "https://llm.chutes.ai"
        );
        assert_eq!(
            Chutes.normalize_endpoint("https://llm.chutes.ai"),
            "https://llm.chutes.ai"
        );
        assert_eq!(Chutes.normalize_endpoint("  "), "https://llm.chutes.ai");
    }
}
