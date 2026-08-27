use std::borrow::Cow;

use crate::common::AdapterError;
use crate::descriptor::{
    ApiKeyRequirement, ParameterFlags, PromptCachingSupport, ProviderDescriptor, ReasoningSupport,
};
use crate::gemini_generate::{GeminiWireProvider, validate_model_id};

const MODEL_RESOURCE_PREFIX: &str = "publishers/google/models/";

pub(crate) struct GeminiExpress;

fn bare_model_id(model: &str) -> &str {
    model.strip_prefix(MODEL_RESOURCE_PREFIX).unwrap_or(model)
}

fn is_image_model(model: &str) -> bool {
    bare_model_id(model).ends_with("-image")
}

impl GeminiWireProvider for GeminiExpress {
    fn descriptor(&self) -> &'static ProviderDescriptor {
        &DESCRIPTOR
    }

    fn api_base<'a>(&self, endpoint: &'a str) -> Cow<'a, str> {
        let trimmed = endpoint.trim_end_matches('/');
        if trimmed.ends_with("/v1beta1") {
            Cow::Borrowed(trimmed)
        } else if let Some(prefix) = trimmed
            .strip_suffix("/v1beta")
            .or_else(|| trimmed.strip_suffix("/v1"))
        {
            Cow::Owned(format!("{prefix}/v1beta1"))
        } else {
            Cow::Owned(format!("{trimmed}/v1beta1"))
        }
    }

    fn generate_path(&self, model: &str) -> Result<String, AdapterError> {
        if is_image_model(model) {
            return Err(AdapterError::Rejected);
        }
        Ok(format!(
            "/{MODEL_RESOURCE_PREFIX}{}:generateContent",
            validate_model_id(bare_model_id(model))?
        ))
    }
}

pub(crate) const DESCRIPTOR: ProviderDescriptor = ProviderDescriptor {
    kind: "gemini-agent-platform-express",
    display_name: "Gemini Agent Platform (Express)",
    protocol: lettuce_models::ProviderProtocol::Gemini,
    aliases: &[],
    default_endpoint: Some("https://aiplatform.googleapis.com"),
    endpoint_editable: false,
    api_key: ApiKeyRequirement::Required,
    auth_header: "x-goog-api-key",
    streaming: true,
    lists_models: false,
    verifies_key: false,
    reasoning: ReasoningSupport::Effort,
    prompt_caching: PromptCachingSupport::Automatic,
    parameters: ParameterFlags {
        top_k: true,
        reasoning_budget: true,
        ..ParameterFlags::BASIC
    },
    extra_body_keys: &[],
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forces_v1beta1_and_strips_the_resource_prefix() {
        assert_eq!(
            GeminiExpress.api_base("https://aiplatform.googleapis.com/v1"),
            "https://aiplatform.googleapis.com/v1beta1"
        );
        assert_eq!(
            GeminiExpress.api_base("https://aiplatform.googleapis.com"),
            "https://aiplatform.googleapis.com/v1beta1"
        );
        assert_eq!(
            GeminiExpress
                .generate_path("publishers/google/models/gemini-2.5-flash")
                .expect("path"),
            "/publishers/google/models/gemini-2.5-flash:generateContent"
        );
        assert_eq!(
            GeminiExpress.generate_path("gemini-2.5-flash-image"),
            Err(AdapterError::Rejected)
        );
    }
}
