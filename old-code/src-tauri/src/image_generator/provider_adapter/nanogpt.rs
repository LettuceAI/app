use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;

use super::{ImageProviderAdapter, ImageRequestPayload, ImageResponseData};
use crate::image_generator::types::ImageGenerationRequest;

pub struct NanoGPTAdapter;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NanoGPTRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    n: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    size: Option<&'a str>,
    response_format: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    image_data_url: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    image_data_urls: Option<Vec<&'a str>>,
}

#[derive(Deserialize)]
struct NanoGPTImageResponse {
    data: Vec<NanoGPTImageData>,
}

#[derive(Deserialize)]
struct NanoGPTImageData {
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    b64_json: Option<String>,
}

fn normalize_nanogpt_base_url(base_url: &str) -> String {
    base_url
        .trim_end_matches('/')
        .trim_end_matches("/api")
        .to_string()
}

impl ImageProviderAdapter for NanoGPTAdapter {
    fn endpoint(&self, base_url: &str, _request: &ImageGenerationRequest) -> String {
        let normalized = normalize_nanogpt_base_url(base_url);
        if normalized.ends_with("/v1") {
            format!("{}/images/generations", normalized)
        } else {
            format!("{}/v1/images/generations", normalized)
        }
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
            for (k, v) in extra {
                headers.insert(k.clone(), v.clone());
            }
        }

        headers
    }

    fn payload(&self, request: &ImageGenerationRequest) -> Result<ImageRequestPayload, String> {
        let (image_data_url, image_data_urls) = match request.input_images.as_ref() {
            Some(images) if images.len() == 1 => (images.first().map(|s| s.as_str()), None),
            Some(images) if !images.is_empty() => (
                None,
                Some(
                    images
                        .iter()
                        .map(|image| image.as_str())
                        .collect::<Vec<_>>(),
                ),
            ),
            _ => (None, None),
        };

        let body = NanoGPTRequest {
            model: &request.model,
            prompt: &request.prompt,
            n: request.n.or(Some(1)),
            size: request.size.as_deref(),
            response_format: "b64_json",
            image_data_url,
            image_data_urls,
        };

        Ok(ImageRequestPayload::Json(
            serde_json::to_value(body).unwrap_or_else(|_| json!({})),
        ))
    }

    fn parse_response(&self, response: Value) -> Result<Vec<ImageResponseData>, String> {
        let parsed: NanoGPTImageResponse = serde_json::from_value(response).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to parse response: {}", e),
            )
        })?;

        Ok(parsed
            .data
            .into_iter()
            .map(|img| ImageResponseData {
                url: img.url,
                b64_json: img.b64_json,
                text: None,
            })
            .collect())
    }
}
