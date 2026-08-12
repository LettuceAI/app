use base64::{engine::general_purpose::STANDARD, Engine as _};
use reqwest::multipart::{Form, Part};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;

use super::{ImageProviderAdapter, ImageRequestPayload, ImageResponseData};
use crate::image_generator::types::ImageGenerationRequest;

pub struct OpenAIAdapter;

#[derive(Serialize)]
struct DalleRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    n: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    size: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    quality: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    style: Option<&'a str>,
    response_format: &'a str,
}

#[derive(Deserialize)]
struct OpenAIImageResponse {
    data: Vec<OpenAIImageData>,
}

#[derive(Deserialize)]
struct OpenAIImageData {
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    b64_json: Option<String>,
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

fn multipart_form_for_edit(request: &ImageGenerationRequest) -> Result<Form, String> {
    let mut form = Form::new()
        .text("model", request.model.clone())
        .text("prompt", request.prompt.clone());

    if let Some(n) = request.n {
        form = form.text("n", n.to_string());
    }
    if let Some(size) = request.size.as_ref() {
        form = form.text("size", size.clone());
    }
    if let Some(quality) = request.quality.as_ref() {
        form = form.text("quality", quality.clone());
    }
    form = form.text("response_format", "b64_json".to_string());

    for image in request.input_images.as_ref().into_iter().flatten() {
        let (mime_type, bytes) = decode_data_url(image)?;
        let extension = file_extension_for_mime(&mime_type);
        let filename = format!("input.{}", extension);
        let part = Part::bytes(bytes)
            .file_name(filename)
            .mime_str(&mime_type)
            .map_err(|error| format!("Failed to attach multipart image: {}", error))?;
        form = form.part("image[]", part);
    }

    Ok(form)
}

impl ImageProviderAdapter for OpenAIAdapter {
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
            for (k, v) in extra.iter() {
                headers.insert(k.clone(), v.clone());
            }
        }

        headers
    }

    fn payload(&self, request: &ImageGenerationRequest) -> Result<ImageRequestPayload, String> {
        if request
            .input_images
            .as_ref()
            .is_some_and(|images| !images.is_empty())
        {
            return Ok(ImageRequestPayload::Multipart(multipart_form_for_edit(
                request,
            )?));
        }

        let dalle_req = DalleRequest {
            model: &request.model,
            prompt: &request.prompt,
            n: request.n.or(Some(1)),
            size: request.size.as_deref(),
            quality: request.quality.as_deref(),
            style: request.style.as_deref(),
            response_format: "b64_json",
        };

        Ok(ImageRequestPayload::Json(
            serde_json::to_value(dalle_req).unwrap_or_else(|_| json!({})),
        ))
    }

    fn parse_response(&self, response: Value) -> Result<Vec<ImageResponseData>, String> {
        let openai_response: OpenAIImageResponse =
            serde_json::from_value(response).map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to parse response: {}", e),
                )
            })?;

        Ok(openai_response
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
