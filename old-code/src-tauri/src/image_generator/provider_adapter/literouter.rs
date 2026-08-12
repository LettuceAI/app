use serde_json::{json, Value};
use std::collections::HashMap;

use super::{
    parse_size_dimensions, ImageProviderAdapter, ImageRequestPayload, ImageResponseData,
    ImageResponseFormat,
};
use crate::image_generator::types::ImageGenerationRequest;

pub struct LiteRouterAdapter;

const LITEROUTER_IMAGE_BASE_URL: &str = "https://image.literouter.com";
const LITEROUTER_DEFAULT_DIMENSION: u32 = 1024;

fn normalize_literouter_image_base_url(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    if trimmed.is_empty() {
        return LITEROUTER_IMAGE_BASE_URL.to_string();
    }
    if trimmed.contains("image.literouter.com") {
        return trimmed.to_string();
    }
    if trimmed.contains("literouter.com") {
        return LITEROUTER_IMAGE_BASE_URL.to_string();
    }
    trimmed.to_string()
}

fn seed_from_request(request: &ImageGenerationRequest) -> Option<u32> {
    request
        .advanced_model_settings
        .as_ref()
        .and_then(|settings| settings.sd_seed)
}

impl ImageProviderAdapter for LiteRouterAdapter {
    fn endpoint(&self, base_url: &str, _request: &ImageGenerationRequest) -> String {
        format!("{}/generate", normalize_literouter_image_base_url(base_url))
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
        headers.insert("Content-Type".into(), "application/json".into());

        if let Some(extra) = extra {
            for (key, value) in extra.iter() {
                headers.insert(key.clone(), value.clone());
            }
        }

        headers
    }

    fn payload(&self, request: &ImageGenerationRequest) -> Result<ImageRequestPayload, String> {
        let (width, height) = parse_size_dimensions(
            request.size.as_deref(),
            LITEROUTER_DEFAULT_DIMENSION,
            LITEROUTER_DEFAULT_DIMENSION,
        );

        let mut body = json!({
            "prompt": request.prompt,
            "model": request.model,
            "width": width,
            "height": height,
        });

        if let Some(seed) = seed_from_request(request) {
            body["seed"] = json!(seed);
        }

        Ok(ImageRequestPayload::Json(body))
    }

    fn response_format(&self) -> ImageResponseFormat {
        ImageResponseFormat::Binary
    }

    fn parse_response(&self, _response: Value) -> Result<Vec<ImageResponseData>, String> {
        Err("LiteRouter returns binary image data, not JSON".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(size: Option<&str>) -> ImageGenerationRequest {
        ImageGenerationRequest {
            prompt: "a cat".into(),
            model: "sdxl-turbo".into(),
            provider_id: "literouter".into(),
            credential_id: "cred".into(),
            advanced_model_settings: None,
            input_images: None,
            mask_image: None,
            loras: None,
            output_modalities: None,
            size: size.map(str::to_string),
            quality: None,
            style: None,
            n: None,
            session_id: None,
            character_id: None,
            character_name: None,
            usage_source: None,
        }
    }

    #[test]
    fn chat_base_url_is_redirected_to_the_image_host() {
        assert_eq!(
            LiteRouterAdapter.endpoint("https://api.literouter.com/v1", &request(None)),
            "https://image.literouter.com/generate"
        );
    }

    #[test]
    fn an_explicit_image_host_is_preserved() {
        assert_eq!(
            LiteRouterAdapter.endpoint("https://image.literouter.com", &request(None)),
            "https://image.literouter.com/generate"
        );
    }

    #[test]
    fn a_self_hosted_base_url_is_left_alone() {
        assert_eq!(
            LiteRouterAdapter.endpoint("https://gateway.internal/lr", &request(None)),
            "https://gateway.internal/lr/generate"
        );
    }

    #[test]
    fn size_maps_to_width_and_height_with_documented_defaults() {
        let ImageRequestPayload::Json(body) =
            LiteRouterAdapter.payload(&request(Some("512x768"))).unwrap()
        else {
            panic!("expected json payload");
        };
        assert_eq!(body["width"], 512);
        assert_eq!(body["height"], 768);

        let ImageRequestPayload::Json(body) = LiteRouterAdapter.payload(&request(None)).unwrap()
        else {
            panic!("expected json payload");
        };
        assert_eq!(body["width"], 1024);
        assert_eq!(body["height"], 1024);
        assert_eq!(body["model"], "sdxl-turbo");
        assert!(body.get("seed").is_none());
    }

    #[test]
    fn binary_responses_are_declared() {
        assert_eq!(
            LiteRouterAdapter.response_format(),
            ImageResponseFormat::Binary
        );
    }
}
