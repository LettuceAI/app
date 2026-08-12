use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::collections::HashMap;

use super::{parse_size_dimensions, ImageProviderAdapter, ImageRequestPayload, ImageResponseData};
use crate::image_generator::types::ImageGenerationRequest;

pub struct Automatic1111Adapter;

#[derive(Deserialize)]
struct Automatic1111Response {
    images: Vec<String>,
}

fn normalize_base_url(base_url: &str) -> String {
    base_url
        .trim_end_matches('/')
        .trim_end_matches("/sdapi/v1")
        .to_string()
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
    body.insert("width".into(), json!(width));
    body.insert("height".into(), json!(height));
    body.insert("batch_size".into(), json!(request.n.unwrap_or(1)));
    body.insert("n_iter".into(), json!(1));
    body.insert("steps".into(), json!(steps));
    body.insert("cfg_scale".into(), json!(cfg_scale));
    body.insert("sampler_index".into(), json!(sampler));
    body.insert(
        "override_settings".into(),
        json!({
            "sd_model_checkpoint": request.model,
        }),
    );

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
        body.insert(
            "init_images".into(),
            Value::Array(images.iter().cloned().map(Value::String).collect()),
        );
        body.insert("denoising_strength".into(), json!(denoising_strength));
    }

    Value::Object(body)
}

impl ImageProviderAdapter for Automatic1111Adapter {
    fn endpoint(&self, base_url: &str, request: &ImageGenerationRequest) -> String {
        let base = normalize_base_url(base_url);
        let path = if request
            .input_images
            .as_ref()
            .is_some_and(|images| !images.is_empty())
        {
            "/sdapi/v1/img2img"
        } else {
            "/sdapi/v1/txt2img"
        };

        format!("{}{}", base, path)
    }

    fn required_auth_headers(&self) -> &'static [&'static str] {
        &[]
    }

    fn headers(
        &self,
        _api_key: &str,
        extra: Option<&HashMap<String, String>>,
    ) -> HashMap<String, String> {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".into(), "application/json".into());

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
        let parsed: Automatic1111Response = serde_json::from_value(response).map_err(|error| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to parse response: {}", error),
            )
        })?;

        Ok(parsed
            .images
            .into_iter()
            .map(|image| ImageResponseData {
                url: None,
                b64_json: Some(image),
                text: None,
            })
            .collect())
    }
}
