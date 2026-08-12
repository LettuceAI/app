use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;

use super::{ImageProviderAdapter, ImageRequestPayload, ImageResponseData};
use crate::image_generator::types::ImageGenerationRequest;

pub struct XAIAdapter;

#[derive(Serialize)]
struct XAIImageRef<'a> {
    #[serde(rename = "type")]
    kind: &'a str,
    url: &'a str,
}

#[derive(Serialize)]
struct XAIGenerationRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    n: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    aspect_ratio: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    resolution: Option<&'a str>,
}

#[derive(Serialize)]
struct XAIEditRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    image: Option<XAIImageRef<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    images: Option<Vec<XAIImageRef<'a>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    aspect_ratio: Option<&'a str>,
}

#[derive(Deserialize)]
struct XAIImageResponse {
    data: Vec<XAIImageData>,
}

#[derive(Deserialize)]
struct XAIImageData {
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    b64_json: Option<String>,
}

fn size_to_xai_settings(size: Option<&str>) -> (Option<&'static str>, Option<&'static str>) {
    match size {
        Some("1024x1024") => (Some("1:1"), Some("1k")),
        Some("1536x1024") => (Some("3:2"), Some("1k")),
        Some("1024x1536") => (Some("2:3"), Some("1k")),
        Some("2048x2048") => (Some("1:1"), Some("2k")),
        Some("2048x1024") => (Some("2:1"), Some("2k")),
        Some("1024x2048") => (Some("1:2"), Some("2k")),
        _ => (None, None),
    }
}

impl ImageProviderAdapter for XAIAdapter {
    fn endpoint(&self, base_url: &str, request: &ImageGenerationRequest) -> String {
        let trimmed = base_url.trim_end_matches('/');
        let path = if request
            .input_images
            .as_ref()
            .is_some_and(|images| !images.is_empty())
        {
            "/images/edits"
        } else {
            "/images/generations"
        };

        if trimmed.ends_with("/v1") {
            format!("{}{}", trimmed, path)
        } else {
            format!("{}/v1{}", trimmed, path)
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
        let (aspect_ratio, resolution) = size_to_xai_settings(request.size.as_deref());

        if let Some(input_images) = request
            .input_images
            .as_ref()
            .filter(|images| !images.is_empty())
        {
            let refs = input_images
                .iter()
                .map(|url| XAIImageRef {
                    kind: "image_url",
                    url: url.as_str(),
                })
                .collect::<Vec<_>>();

            let body = if refs.len() == 1 {
                XAIEditRequest {
                    model: &request.model,
                    prompt: &request.prompt,
                    image: refs.into_iter().next(),
                    images: None,
                    aspect_ratio,
                }
            } else {
                XAIEditRequest {
                    model: &request.model,
                    prompt: &request.prompt,
                    image: None,
                    images: Some(refs),
                    aspect_ratio,
                }
            };

            return Ok(ImageRequestPayload::Json(
                serde_json::to_value(body).unwrap_or_else(|_| json!({})),
            ));
        }

        let body = XAIGenerationRequest {
            model: &request.model,
            prompt: &request.prompt,
            n: request.n.or(Some(1)),
            aspect_ratio,
            resolution,
        };

        Ok(ImageRequestPayload::Json(
            serde_json::to_value(body).unwrap_or_else(|_| json!({})),
        ))
    }

    fn parse_response(&self, response: Value) -> Result<Vec<ImageResponseData>, String> {
        let parsed: XAIImageResponse = serde_json::from_value(response).map_err(|e| {
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
