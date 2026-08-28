use std::borrow::Cow;

use lettuce_models::{ProviderConfig, ResolvedChatParameters};
use lettuce_network::JsonStaticHeader;
use serde_json::{Map, Value};

use crate::descriptor::{
    ApiKeyRequirement, ParameterFlags, PromptCachingSupport, ProviderDescriptor, ReasoningSupport,
};
use crate::openai_compatible::{
    AdapterError, NO_HEADERS, OpenAiWireProvider, ReasoningWirePolicy, WireParameters,
    standard_parameters,
};

pub(crate) struct Zai;

impl OpenAiWireProvider for Zai {
    fn descriptor(&self) -> &'static ProviderDescriptor {
        &DESCRIPTOR
    }

    fn normalize_endpoint<'a>(&self, endpoint: &'a str) -> Cow<'a, str> {
        let trimmed = endpoint.trim_end_matches('/');
        Cow::Borrowed(trimmed.strip_suffix("/chat/completions").unwrap_or(trimmed))
    }

    fn chat_path(
        &self,
        _endpoint: &str,
        _config: &ProviderConfig,
    ) -> Result<Cow<'static, str>, AdapterError> {
        Ok(Cow::Borrowed("/chat/completions"))
    }

    fn models_path(&self, _endpoint: &str, _config: &ProviderConfig) -> Option<Cow<'static, str>> {
        None
    }

    fn static_headers(&self) -> &'static [JsonStaticHeader] {
        &NO_HEADERS
    }

    fn wire_parameters(&self, parameters: &ResolvedChatParameters) -> WireParameters {
        WireParameters {
            context_length: None,
            frequency_penalty: None,
            presence_penalty: None,
            ..standard_parameters(parameters)
        }
    }

    fn reasoning_policy(&self) -> ReasoningWirePolicy {
        ReasoningWirePolicy::Zai
    }

    fn extend_body(&self, _parameters: &ResolvedChatParameters, _body: &mut Map<String, Value>) {}
}

pub(crate) const DESCRIPTOR: ProviderDescriptor = ProviderDescriptor {
    kind: "zai",
    display_name: "zAI (GLM)",
    protocol: lettuce_models::ProviderProtocol::OpenAiCompatible,
    aliases: &["z.ai"],
    default_endpoint: Some("https://api.z.ai/api/paas/v4"),
    endpoint_editable: false,
    api_key: ApiKeyRequirement::Required,
    auth_header: "Authorization",
    streaming: true,
    lists_models: false,
    verifies_key: true,
    reasoning: ReasoningSupport::Effort,
    prompt_caching: PromptCachingSupport::None,
    parameters: ParameterFlags {
        reasoning_budget: true,
        ..ParameterFlags::BASIC
    },
    extra_body_keys: &[],
};
