use serde_json::Value;
use std::collections::HashMap;

use super::{parse_size_dimensions, ImageProviderAdapter, ImageRequestPayload, ImageResponseData};
use crate::image_generator::types::ImageGenerationRequest;

pub struct PollinationsAdapter;

impl PollinationsAdapter {
    fn resolve_size(request: &ImageGenerationRequest) -> (u32, u32) {
        let advanced = request.advanced_model_settings.as_ref();
        let size_override = request
            .size
            .as_deref()
            .or_else(|| advanced.and_then(|s| s.sd_size.as_deref()));
        parse_size_dimensions(size_override, 1024, 1024)
    }

    fn extract_negative_prompt(request: &ImageGenerationRequest) -> Option<String> {
        request
            .advanced_model_settings
            .as_ref()
            .and_then(|s| s.sd_negative_prompt.as_ref())
            .map(|v| v.trim())
            .filter(|v| !v.is_empty())
            .map(|s| s.to_string())
    }

    fn extract_base_url(base_url: &str) -> String {
        let mut clean = base_url.trim_end_matches('/');
        if let Some(stripped) = clean.strip_suffix("/v1") {
            clean = stripped.trim_end_matches('/');
        }
        if let Ok(url) = reqwest::Url::parse(clean) {
            if let Some(host) = url.host_str() {
                let port = url.port().map(|p| format!(":{}", p)).unwrap_or_default();
                return format!("{}://{}{}", url.scheme(), host, port);
            }
        }
        clean.to_string()
    }
}

impl ImageProviderAdapter for PollinationsAdapter {
    fn method(&self) -> String {
        "GET".into()
    }

    fn endpoint(&self, base_url: &str, request: &ImageGenerationRequest) -> String {
        let base = Self::extract_base_url(base_url);
        let prompt_encoded = urlencoding::encode(&request.prompt);
        let model = &request.model;
        let (width, height) = Self::resolve_size(request);
        let quality = request.quality.as_deref().unwrap_or("medium");

        // Here is how you do the "OR" fallback safely in Rust!
        let negative = Self::extract_negative_prompt(request)
            .unwrap_or_else(|| "worst quality, blurry, watermark, text".to_string());
            
        let neg_encoded = urlencoding::encode(&negative);

        format!(
            "{base}/image/{prompt_encoded}?model={model}&width={width}&height={height}&quality={quality}&negative_prompt={neg_encoded}"
        )
    }

    fn required_auth_headers(&self) -> &'static [&'static str] {
        &["Authorization"]
    }

    fn headers(
        &self,
        api_key: &str,
        extra: Option<&HashMap<String, String>>,
    ) -> HashMap<String, String> {
        let mut headers = HashMap::new();
        headers.insert("Authorization".into(), format!("Bearer {}", api_key));
        headers.insert("Accept".into(), "image/jpeg, image/png".into());

        if let Some(extra) = extra {
            for (k, v) in extra.iter() {
                headers.insert(k.clone(), v.clone());
            }
        }

        headers
    }

    fn payload(&self, _request: &ImageGenerationRequest) -> Result<ImageRequestPayload, String> {
        Ok(ImageRequestPayload::None)
    }

    fn expects_binary_response(&self) -> bool {
        true
    }

    fn parse_response(&self, _response: Value) -> Result<Vec<ImageResponseData>, String> {
        // Pollinations image generation returns raw bytes, not a JSON envelope.
        // parse_response is only called in the JSON branch of commands.rs, which
        // is bypassed for this adapter via expects_binary_response() == true.
        Err("PollinationsAdapter: unexpected JSON response path".to_string())
    }
}
