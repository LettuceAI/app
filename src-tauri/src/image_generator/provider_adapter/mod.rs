use reqwest::multipart::Form;
use serde_json::Value;
use std::collections::HashMap;

use super::types::ImageGenerationRequest;

pub mod automatic1111;
pub mod google_gemini;
pub mod nanogpt;
pub mod openai;
pub mod openrouter;
pub mod pollinations;
pub mod stability;
pub mod xai;

pub enum ImageRequestPayload {
    None,
    Json(Value),
    Multipart(Form),
}

pub trait ImageProviderAdapter: Send + Sync {
    fn method(&self) -> String {
        "POST".into()
    }

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

    /// Return `true` when the provider sends the image as raw bytes rather than a
    /// JSON envelope. Defaults to `false` so existing adapters are unaffected.
    fn expects_binary_response(&self) -> bool {
        false
    }

    #[allow(dead_code)]
    fn supports_stream(&self) -> bool {
        false
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
        "openai" => Ok(Box::new(openai::OpenAIAdapter)),
        "openrouter" => Ok(Box::new(openrouter::OpenRouterAdapter)),
        "gemini" => Ok(Box::new(google_gemini::GoogleGeminiAdapter)),
        "stability" => Ok(Box::new(stability::StabilityAdapter)),
        "pollinations-image" => Ok(Box::new(pollinations::PollinationsImageAdapter)),
        "xai" => Ok(Box::new(xai::XAIAdapter)),
        "nanogpt" => Ok(Box::new(nanogpt::NanoGPTAdapter)),
        _ => Err(format!(
            "Provider {} does not support image generation",
            provider_id
        )),
    }
}
