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

    fn extend_body(
        &self,
        parameters: &lettuce_models::ResolvedChatParameters,
        body: &mut serde_json::Map<String, serde_json::Value>,
    ) {
        if let Some(provider) = &parameters.openrouter.pinned_provider {
            body.insert(
                "provider".to_owned(),
                serde_json::json!({
                    "order": [provider],
                    "allow_fallbacks": false,
                }),
            );
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinned_provider_emits_exact_legacy_routing_shape() {
        let mut parameters = crate::integration_tests::parameters();
        parameters.openrouter.pinned_provider = Some("provider/tag".to_owned());
        let mut body = serde_json::Map::new();
        OpenRouter.extend_body(&parameters, &mut body);
        assert_eq!(
            body.get("provider"),
            Some(&serde_json::json!({
                "order": ["provider/tag"],
                "allow_fallbacks": false,
            }))
        );
    }
}
