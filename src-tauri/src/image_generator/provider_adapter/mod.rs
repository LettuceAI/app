use reqwest::multipart::Form;
use serde_json::Value;
use std::collections::HashMap;

use super::types::ImageGenerationRequest;

pub mod automatic1111;
pub mod diffusers;
pub mod google_gemini;
pub mod literouter;
pub mod nanogpt;
pub mod openai;
pub mod openrouter;
pub mod pollinations;
pub mod stability;
pub mod xai;

pub enum ImageRequestPayload {
    Json(Value),
    Multipart(Form),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageResponseFormat {
    Json,
    Binary,
}

pub trait ImageProviderAdapter: Send + Sync {
    fn endpoint(&self, base_url: &str, request: &ImageGenerationRequest) -> String;
    fn requires_api_key(&self) -> bool {
        !self.required_auth_headers().is_empty()
    }
    #[allow(dead_code)]
    fn required_auth_headers(&self) -> &'static [&'static str];
    fn headers(
        &self,
        api_key: &str,
        extra: Option<&HashMap<String, String>>,
    ) -> HashMap<String, String>;

    fn payload(&self, request: &ImageGenerationRequest) -> Result<ImageRequestPayload, String>;
    fn parse_response(&self, response: Value) -> Result<Vec<ImageResponseData>, String>;

    fn response_format(&self) -> ImageResponseFormat {
        ImageResponseFormat::Json
    }

    #[allow(dead_code)]
    fn supports_stream(&self) -> bool {
        false
    }

    /// Another adapter to retry the request with when the provider rejects it with the
    /// given HTTP status. Adapters that talk to a newer endpoint use this to fall back
    /// to an older one for models the new endpoint does not serve.
    fn fallback_adapter(
        &self,
        _status: u16,
        _body: &str,
    ) -> Option<Box<dyn ImageProviderAdapter>> {
        None
    }
}

/// An error a provider reported inside a successful (HTTP 2xx) JSON body. OpenRouter
/// does this for upstream failures, e.g. `{"error":{"code":504,"message":"..."}}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderBodyError {
    pub code: Option<u16>,
    pub message: String,
}

impl ProviderBodyError {
    pub fn is_transient(&self) -> bool {
        matches!(self.code, Some(500 | 502 | 503 | 504 | 529))
    }

    pub fn describe(&self) -> String {
        match self.code {
            Some(code) => format!("Provider error {}: {}", code, self.message),
            None => format!("Provider error: {}", self.message),
        }
    }
}

pub fn extract_body_error(response: &Value) -> Option<ProviderBodyError> {
    match response.get("error")? {
        Value::String(message) => {
            let message = message.trim();
            (!message.is_empty()).then(|| ProviderBodyError {
                code: None,
                message: message.to_string(),
            })
        }
        Value::Object(map) => {
            let code = map.get("code").and_then(|code| {
                code.as_u64()
                    .or_else(|| code.as_str().and_then(|value| value.parse().ok()))
                    .and_then(|value| u16::try_from(value).ok())
            });
            let message = map
                .get("message")
                .or_else(|| map.get("msg"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| Value::Object(map.clone()).to_string());
            Some(ProviderBodyError { code, message })
        }
        _ => None,
    }
}

#[derive(Debug, Clone)]
pub struct ImageResponseData {
    pub url: Option<String>,
    pub b64_json: Option<String>,
    pub text: Option<String>,
}

pub fn parse_size_dimensions(
    size: Option<&str>,
    default_width: u32,
    default_height: u32,
) -> (u32, u32) {
    let Some(size) = size else {
        return (default_width, default_height);
    };

    let Some((width, height)) = size.split_once('x') else {
        return (default_width, default_height);
    };

    let width = width.parse::<u32>().ok().filter(|value| *value > 0);
    let height = height.parse::<u32>().ok().filter(|value| *value > 0);

    match (width, height) {
        (Some(width), Some(height)) => (width, height),
        _ => (default_width, default_height),
    }
}

pub fn get_adapter(provider_id: &str) -> Result<Box<dyn ImageProviderAdapter>, String> {
    match provider_id {
        "automatic1111" => Ok(Box::new(automatic1111::Automatic1111Adapter)),
        "diffusers" => Ok(Box::new(diffusers::DiffusersAdapter)),
        "openai" => Ok(Box::new(openai::OpenAIAdapter)),
        "openrouter" => Ok(Box::new(openrouter::OpenRouterAdapter)),
        "pollinations" => Ok(Box::new(pollinations::PollinationsAdapter)),
        "gemini" => Ok(Box::new(google_gemini::GoogleGeminiAdapter)),
        "gemini-agent-platform-express" => {
            Ok(Box::new(google_gemini::GeminiAgentPlatformExpressAdapter))
        }
        "stability" => Ok(Box::new(stability::StabilityAdapter)),
        "xai" => Ok(Box::new(xai::XAIAdapter)),
        "nanogpt" => Ok(Box::new(nanogpt::NanoGPTAdapter)),
        "literouter" => Ok(Box::new(literouter::LiteRouterAdapter)),
        "custom" | "lettuce-host" => Ok(Box::new(openai::OpenAIAdapter)),
        _ => Err(format!(
            "Provider {} does not support image generation",
            provider_id
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn body_error_objects_are_reported_with_their_code() {
        let error = extract_body_error(&json!({
            "error": { "code": 504, "message": "The operation was aborted" },
            "id": "gen-1"
        }))
        .unwrap();
        assert_eq!(error.code, Some(504));
        assert_eq!(error.message, "The operation was aborted");
        assert!(error.is_transient());
        assert_eq!(
            error.describe(),
            "Provider error 504: The operation was aborted"
        );
    }

    #[test]
    fn body_error_strings_and_string_codes_are_accepted() {
        let error = extract_body_error(&json!({ "error": "quota exceeded" })).unwrap();
        assert_eq!(error.code, None);
        assert_eq!(error.message, "quota exceeded");
        assert!(!error.is_transient());

        let error = extract_body_error(&json!({
            "error": { "code": "429", "message": "slow down" }
        }))
        .unwrap();
        assert_eq!(error.code, Some(429));
    }

    #[test]
    fn missing_null_or_false_error_fields_are_not_errors() {
        assert!(extract_body_error(&json!({ "choices": [] })).is_none());
        assert!(extract_body_error(&json!({ "error": null })).is_none());
        assert!(extract_body_error(&json!({ "error": false })).is_none());
        assert!(extract_body_error(&json!({ "error": "  " })).is_none());
    }
}
