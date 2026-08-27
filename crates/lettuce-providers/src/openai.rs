use std::borrow::Cow;

use lettuce_conversations::MessageRole;
use lettuce_models::{PromptCacheRetention, PromptCaching, ProviderConfig, ResolvedChatParameters};

use crate::descriptor::{
    ApiKeyRequirement, ParameterFlags, PromptCachingSupport, ProviderDescriptor, ReasoningSupport,
};
use crate::openai_compatible::{OpenAiWireProvider, WireParameters, standard_parameters};

pub(crate) struct OpenAi;

impl OpenAiWireProvider for OpenAi {
    fn descriptor(&self) -> &'static ProviderDescriptor {
        &DESCRIPTOR
    }

    fn role(&self, role: MessageRole, _config: &ProviderConfig) -> Option<Cow<'static, str>> {
        Some(Cow::Borrowed(match role {
            MessageRole::System | MessageRole::Scene => "developer",
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
        }))
    }

    fn wire_parameters(&self, parameters: &ResolvedChatParameters) -> WireParameters {
        WireParameters {
            context_length: None,
            ..standard_parameters(parameters)
        }
    }

    fn extend_body(
        &self,
        parameters: &ResolvedChatParameters,
        body: &mut serde_json::Map<String, serde_json::Value>,
    ) {
        let Some(PromptCaching::Enabled { retention }) = parameters.prompt_caching else {
            return;
        };
        let retention = match retention {
            PromptCacheRetention::InMemory => "in_memory",
            PromptCacheRetention::TwentyFourHours => "24h",
            PromptCacheRetention::FiveMinutes | PromptCacheRetention::OneHour => return,
        };
        body.insert(
            "prompt_cache_retention".to_owned(),
            serde_json::Value::String(retention.to_owned()),
        );
    }
}

pub(crate) const DESCRIPTOR: ProviderDescriptor = ProviderDescriptor {
    kind: "openai",
    display_name: "OpenAI",
    protocol: lettuce_models::ProviderProtocol::OpenAiCompatible,
    aliases: &[],
    default_endpoint: Some("https://api.openai.com"),
    endpoint_editable: false,
    api_key: ApiKeyRequirement::Required,
    auth_header: "Authorization",
    streaming: true,
    lists_models: true,
    verifies_key: true,
    reasoning: ReasoningSupport::Effort,
    prompt_caching: PromptCachingSupport::RequestRetention,
    parameters: ParameterFlags::PENALTIES_BUDGET,
    extra_body_keys: &[],
};
