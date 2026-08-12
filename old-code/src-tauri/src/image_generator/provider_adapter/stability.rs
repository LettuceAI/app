use base64::{engine::general_purpose::STANDARD, Engine as _};
use reqwest::multipart::{Form, Part};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;

use super::{parse_size_dimensions, ImageProviderAdapter, ImageRequestPayload, ImageResponseData};
use crate::image_generator::types::ImageGenerationRequest;

pub struct StabilityAdapter;

#[derive(Deserialize)]
struct StabilityResponse {
    artifacts: Vec<StabilityArtifact>,
}

#[derive(Deserialize)]
struct StabilityArtifact {
    #[serde(default)]
    base64: Option<String>,
}

fn decode_data_url(data_url: &str) -> Result<(String, Vec<u8>), String> {
    let (prefix, payload) = data_url
        .split_once(',')
        .ok_or_else(|| "Invalid data URL format".to_string())?;
    let mime_type = prefix
        .strip_prefix("data:")
        .and_then(|value| value.strip_suffix(";base64"))
        .ok_or_else(|| "Invalid data URL prefix".to_string())?;
    let bytes = STANDARD
        .decode(payload)
        .map_err(|error| format!("Failed to decode base64 image: {}", error))?;
    Ok((mime_type.to_string(), bytes))
}

fn file_extension_for_mime(mime_type: &str) -> &'static str {
    match mime_type {
        "image/png" => "png",
        "image/webp" => "webp",
        "image/jpeg" | "image/jpg" => "jpg",
        _ => "png",
    }
}

impl ImageProviderAdapter for StabilityAdapter {
    fn endpoint(&self, base_url: &str, request: &ImageGenerationRequest) -> String {
        let trimmed = base_url.trim_end_matches('/');
        if request
            .input_images
            .as_ref()
            .is_some_and(|images| !images.is_empty())
        {
            format!("{}/v1/generation/{}/image-to-image", trimmed, request.model)
        } else {
            format!("{}/v1/generation/{}/text-to-image", trimmed, request.model)
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
        headers.insert("Accept".into(), "application/json".into());
        headers.insert("Content-Type".into(), "application/json".into());

        if let Some(extra) = extra {
            for (key, value) in extra {
                headers.insert(key.clone(), value.clone());
            }
        }

        headers
    }

    fn payload(&self, request: &ImageGenerationRequest) -> Result<ImageRequestPayload, String> {
        let (width, height) = parse_size_dimensions(request.size.as_deref(), 1024, 1024);

        if let Some(image) = request
            .input_images
            .as_ref()
            .and_then(|images| images.first())
        {
            let (mime_type, bytes) = decode_data_url(image)?;
            let extension = file_extension_for_mime(&mime_type);
            let filename = format!("input.{}", extension);
            let image_part = Part::bytes(bytes)
                .file_name(filename)
                .mime_str(&mime_type)
                .map_err(|error| format!("Failed to attach image: {}", error))?;

            let form = Form::new()
                .text("init_image_mode", "IMAGE_STRENGTH".to_string())
                .text("image_strength", "0.35".to_string())
                .text("samples", request.n.unwrap_or(1).to_string())
                .text("width", width.to_string())
                .text("height", height.to_string())
                .text("text_prompts[0][text]", request.prompt.clone())
                .part("init_image", image_part);

            return Ok(ImageRequestPayload::Multipart(form));
        }

        let body = json!({
            "text_prompts": [
                {
                    "text": request.prompt,
                    "weight": 1,
                }
            ],
            "width": width,
            "height": height,
            "samples": request.n.unwrap_or(1),
        });

        Ok(ImageRequestPayload::Json(body))
    }

    fn parse_response(&self, response: Value) -> Result<Vec<ImageResponseData>, String> {
        let parsed: StabilityResponse = serde_json::from_value(response).map_err(|error| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to parse response: {}", error),
            )
        })?;

        Ok(parsed
            .artifacts
            .into_iter()
            .filter_map(|artifact| artifact.base64)
            .map(|image| ImageResponseData {
                url: None,
                b64_json: Some(image),
                text: None,
            })
            .collect())
    }
}
