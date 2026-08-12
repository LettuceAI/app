use serde_json::{json, Map, Value};
use std::collections::HashMap;

use super::{parse_size_dimensions, ImageProviderAdapter, ImageRequestPayload, ImageResponseData};
use crate::image_generator::types::ImageGenerationRequest;

pub struct DiffusersAdapter;

fn normalize_base_url(base_url: &str) -> String {
    base_url.trim_end_matches('/').to_string()
}

fn strip_data_url(image: &str) -> String {
    match image.split_once("base64,") {
        Some((_, data)) => data.to_string(),
        None => image.to_string(),
    }
}

fn request_body(request: &ImageGenerationRequest) -> Value {
    let advanced = request.advanced_model_settings.as_ref();
    let size_override = request
        .size
        .as_deref()
        .or_else(|| advanced.and_then(|settings| settings.sd_size.as_deref()));
    let (width, height) = parse_size_dimensions(size_override, 1024, 1024);
    let steps = advanced
        .and_then(|settings| settings.sd_steps)
        .unwrap_or(28);
    let cfg_scale = advanced
        .and_then(|settings| settings.sd_cfg_scale)
        .unwrap_or(6.5);
    let sampler = advanced
        .and_then(|settings| settings.sd_sampler.as_deref())
        .unwrap_or("DPM++ 2M Karras");
    let denoising_strength = advanced
        .and_then(|settings| settings.sd_denoising_strength)
        .unwrap_or(0.75);

    let mut body = Map::new();
    body.insert("prompt".into(), Value::String(request.prompt.clone()));
    body.insert("model".into(), Value::String(request.model.clone()));
    body.insert("width".into(), json!(width));
    body.insert("height".into(), json!(height));
    body.insert("steps".into(), json!(steps));
    body.insert("cfg_scale".into(), json!(cfg_scale));
    body.insert("sampler".into(), Value::String(sampler.to_string()));
    body.insert("n".into(), json!(request.n.unwrap_or(1)));

    if let Some(seed) = advanced.and_then(|settings| settings.sd_seed) {
        body.insert("seed".into(), json!(seed));
    }
    if let Some(negative_prompt) = advanced
        .and_then(|settings| settings.sd_negative_prompt.as_ref())
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        body.insert(
            "negative_prompt".into(),
            Value::String(negative_prompt.to_string()),
        );
    }

    if let Some(images) = request
        .input_images
        .as_ref()
        .filter(|images| !images.is_empty())
    {
        let encoded: Vec<Value> = images
            .iter()
            .map(|image| Value::String(strip_data_url(image)))
            .collect();
        body.insert(
            "init_image".into(),
            encoded.first().cloned().unwrap_or(Value::Null),
        );
        body.insert("images".into(), Value::Array(encoded));
        body.insert("denoising_strength".into(), json!(denoising_strength));
    }

    Value::Object(body)
}

fn collect_image(value: &Value) -> Option<ImageResponseData> {
    let raw = value.as_str()?;
    let data = strip_data_url(raw);
    if data.is_empty() {
        return None;
    }
    Some(ImageResponseData {
        url: None,
        b64_json: Some(data),
        text: None,
    })
}

impl ImageProviderAdapter for DiffusersAdapter {
    fn endpoint(&self, base_url: &str, _request: &ImageGenerationRequest) -> String {
        format!("{}/generate", normalize_base_url(base_url))
    }

    fn requires_api_key(&self) -> bool {
        false
    }

    fn required_auth_headers(&self) -> &'static [&'static str] {
        &[]
    }

    fn headers(
        &self,
        api_key: &str,
        extra: Option<&HashMap<String, String>>,
    ) -> HashMap<String, String> {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".into(), "application/json".into());
        if !api_key.is_empty() {
            headers.insert("Authorization".into(), format!("Bearer {}", api_key));
        }
        if let Some(extra) = extra {
            for (key, value) in extra {
                headers.insert(key.clone(), value.clone());
            }
        }
        headers
    }

    fn payload(&self, request: &ImageGenerationRequest) -> Result<ImageRequestPayload, String> {
        Ok(ImageRequestPayload::Json(request_body(request)))
    }

    fn parse_response(&self, response: Value) -> Result<Vec<ImageResponseData>, String> {
        let mut images = Vec::new();

        if let Some(array) = response.get("images").and_then(Value::as_array) {
            for entry in array {
                if let Some(image) = collect_image(entry) {
                    images.push(image);
                }
            }
        }

        if images.is_empty() {
            if let Some(image) = response.get("image").and_then(collect_image) {
                images.push(image);
            }
        }

        if images.is_empty() {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                "Diffusers response did not contain any image data".to_string(),
            ));
        }

        Ok(images)
    }
}
