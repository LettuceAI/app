use std::borrow::Cow;

use lettuce_models::{ProviderConfig, ResolvedChatParameters};

use crate::descriptor::{
    ApiKeyRequirement, ParameterFlags, PromptCachingSupport, ProviderDescriptor, ReasoningSupport,
};
use crate::openai::OpenAi;
use crate::openai_compatible::{AdapterError, OpenAiWireProvider, WireParameters};

pub(crate) struct Groq;

fn has_openai_prefix(endpoint: &str) -> bool {
    endpoint.trim_end_matches('/').ends_with("/openai")
}

impl OpenAiWireProvider for Groq {
    fn descriptor(&self) -> &'static ProviderDescriptor {
        &DESCRIPTOR
    }

    fn chat_path(
        &self,
        endpoint: &str,
        _config: &ProviderConfig,
    ) -> Result<Cow<'static, str>, AdapterError> {
        Ok(Cow::Borrowed(if has_openai_prefix(endpoint) {
            "/v1/chat/completions"
        } else {
            "/openai/v1/chat/completions"
        }))
    }

    fn models_path(&self, endpoint: &str, _config: &ProviderConfig) -> Option<Cow<'static, str>> {
        Some(Cow::Borrowed(if has_openai_prefix(endpoint) {
            "/v1/models"
        } else {
            "/openai/v1/models"
        }))
    }

    fn wire_parameters(&self, parameters: &ResolvedChatParameters) -> WireParameters {
        OpenAi.wire_parameters(parameters)
    }
}

pub(crate) const DESCRIPTOR: ProviderDescriptor = ProviderDescriptor {
    kind: "groq",
    display_name: "Groq",
    protocol: lettuce_models::ProviderProtocol::OpenAiCompatible,
    aliases: &[],
    default_endpoint: Some("https://api.groq.com"),
    endpoint_editable: false,
    api_key: ApiKeyRequirement::Required,
    auth_header: "Authorization",
    streaming: true,
    lists_models: true,
    verifies_key: true,
    reasoning: ReasoningSupport::Effort,
    prompt_caching: PromptCachingSupport::Automatic,
    parameters: ParameterFlags::PENALTIES_BUDGET,
    extra_body_keys: &[],
};
