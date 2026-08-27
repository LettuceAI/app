use std::borrow::Cow;

use crate::common::AdapterError;
use crate::descriptor::{
    ApiKeyRequirement, ParameterFlags, PromptCachingSupport, ProviderDescriptor, ReasoningSupport,
};
use crate::gemini_generate::{GeminiWireProvider, validate_model_id};

pub(crate) struct Gemini;

impl GeminiWireProvider for Gemini {
    fn descriptor(&self) -> &'static ProviderDescriptor {
        &DESCRIPTOR
    }

    fn models_path(&self) -> Option<&'static str> {
        Some("/models")
    }

    fn api_base<'a>(&self, endpoint: &'a str) -> Cow<'a, str> {
        let trimmed = endpoint.trim_end_matches('/');
        match trimmed.strip_suffix("/v1") {
            Some(prefix) => Cow::Owned(format!("{prefix}/v1beta")),
            None => Cow::Borrowed(trimmed),
        }
    }

    fn generate_path(&self, model: &str) -> Result<String, AdapterError> {
        Ok(format!(
            "/models/{}:generateContent",
            validate_model_id(model)?
        ))
    }
}

pub(crate) const DESCRIPTOR: ProviderDescriptor = ProviderDescriptor {
    kind: "gemini",
    display_name: "Google (Gemini)",
    protocol: lettuce_models::ProviderProtocol::Gemini,
    aliases: &["google", "google-gemini"],
    default_endpoint: Some("https://generativelanguage.googleapis.com/v1"),
    endpoint_editable: false,
    api_key: ApiKeyRequirement::Required,
    auth_header: "x-goog-api-key",
    streaming: true,
    lists_models: true,
    verifies_key: true,
    reasoning: ReasoningSupport::Effort,
    prompt_caching: PromptCachingSupport::Supported,
    parameters: ParameterFlags {
        top_k: true,
        reasoning_budget: true,
        ..ParameterFlags::BASIC
    },
    extra_body_keys: &["promptCachingTtl"],
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upgrades_v1_bases_to_v1beta() {
        assert_eq!(
            Gemini.api_base("https://generativelanguage.googleapis.com/v1/"),
            "https://generativelanguage.googleapis.com/v1beta"
        );
        assert_eq!(
            Gemini.api_base("https://proxy.example/v1beta"),
            "https://proxy.example/v1beta"
        );
    }
}
