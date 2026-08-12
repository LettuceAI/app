use std::time::Duration;

use base64::{engine::general_purpose::STANDARD, Engine};
use rand::Rng;
use reqwest::multipart::{Form, Part};
use serde_json::Value;
use tauri::AppHandle;
use tokio::time::sleep;
use uuid::Uuid;

use super::provider_adapter::{parse_size_dimensions, ImageResponseData};
use super::types::ImageGenerationRequest;
use crate::utils::log_info;

const POLL_INTERVAL_MS: u64 = 1500;
const MAX_POLL_ATTEMPTS: u32 = 400;

fn normalize_base_url(base_url: &str) -> String {
    base_url.trim_end_matches('/').to_string()
}

fn decode_data_url(image: &str) -> Result<(String, Vec<u8>), String> {
    let (prefix, payload) = match image.split_once(',') {
        Some(parts) => parts,
        None => ("data:image/png;base64", image),
    };
    let mime_type = prefix
        .strip_prefix("data:")
        .and_then(|value| value.strip_suffix(";base64"))
        .unwrap_or("image/png")
        .to_string();
    let bytes = STANDARD
        .decode(payload)
        .map_err(|error| format!("Failed to decode reference image: {}", error))?;
    Ok((mime_type, bytes))
}

fn extension_for_mime(mime_type: &str) -> &'static str {
    match mime_type {
        "image/png" => "png",
        "image/webp" => "webp",
        "image/jpeg" | "image/jpg" => "jpg",
        _ => "png",
    }
}

fn config_string(config: Option<&Value>, key: &str) -> Option<String> {
    config
        .and_then(|value| value.get(key))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn build_client(app: &AppHandle, url: &str) -> Result<reqwest::Client, String> {
    crate::transport::build_client(app, None, false, Some("comfyui"), Some(url))
        .map_err(|error| crate::utils::err_msg(module_path!(), line!(), error.to_string()))
}

fn apply_auth(builder: reqwest::RequestBuilder, api_key: &str) -> reqwest::RequestBuilder {
    if api_key.is_empty() {
        builder
    } else {
        builder.header("Authorization", format!("Bearer {}", api_key))
    }
}

async fn upload_image(
    app: &AppHandle,
    base_url: &str,
    api_key: &str,
    image: &str,
    index: usize,
) -> Result<String, String> {
    let (mime_type, bytes) = decode_data_url(image)?;
    let filename = format!("ref_{}.{}", index, extension_for_mime(&mime_type));
    let part = Part::bytes(bytes)
        .file_name(filename)
        .mime_str(&mime_type)
        .map_err(|error| format!("Failed to attach reference image: {}", error))?;
    let form = Form::new()
        .part("image", part)
        .text("overwrite", "true");

    let url = format!("{}/upload/image", base_url);
    let client = build_client(app, &url)?;
    let response = apply_auth(client.post(&url), api_key)
        .multipart(form)
        .send()
        .await
        .map_err(|error| format!("ComfyUI image upload failed: {}", error))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(format!("ComfyUI image upload error {}: {}", status, text));
    }

    let body: Value = response
        .json()
        .await
        .map_err(|error| format!("Failed to parse ComfyUI upload response: {}", error))?;
    let name = body
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| "ComfyUI upload response missing image name".to_string())?;
    let subfolder = body
        .get("subfolder")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if subfolder.is_empty() {
        Ok(name.to_string())
    } else {
        Ok(format!("{}/{}", subfolder, name))
    }
}

fn substitute_tokens(template: &str, request: &ImageGenerationRequest, uploaded: &[String]) -> String {
    let advanced = request.advanced_model_settings.as_ref();
    let size_override = request
        .size
        .as_deref()
        .or_else(|| advanced.and_then(|settings| settings.sd_size.as_deref()));
    let (width, height) = parse_size_dimensions(size_override, 1024, 1024);
    let steps = advanced
        .and_then(|settings| settings.sd_steps)
        .unwrap_or(28);
    let cfg = advanced
        .and_then(|settings| settings.sd_cfg_scale)
        .unwrap_or(6.5);
    let sampler = advanced
        .and_then(|settings| settings.sd_sampler.as_deref())
        .unwrap_or("euler");
    let denoise = advanced
        .and_then(|settings| settings.sd_denoising_strength)
        .unwrap_or(0.75);
    let negative = advanced
        .and_then(|settings| settings.sd_negative_prompt.as_ref())
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .unwrap_or("");
    let seed = advanced
        .and_then(|settings| settings.sd_seed)
        .map(u64::from)
        .unwrap_or_else(|| rand::thread_rng().gen_range(0..u64::from(u32::MAX)));

    let escape = |value: &str| {
        serde_json::to_string(value)
            .map(|quoted| quoted[1..quoted.len() - 1].to_string())
            .unwrap_or_default()
    };

    let mut output = template.to_string();

    for (index, name) in uploaded.iter().enumerate() {
        output = output.replace(&format!("%IMAGE{}%", index), &escape(name));
    }
    let max_tokens = uploaded.len().max(16);
    for index in uploaded.len()..max_tokens {
        output = output.replace(&format!("%IMAGE{}%", index), "");
    }
    let first_image = uploaded.first().map(String::as_str).unwrap_or("");
    output = output.replace("%IMAGE_COUNT%", &uploaded.len().to_string());
    output = output.replace("%IMAGE%", &escape(first_image));

    output = output.replace("%PROMPT%", &escape(&request.prompt));
    output = output.replace("%NEGATIVE%", &escape(negative));
    output = output.replace("%SAMPLER%", &escape(sampler));
    output = output.replace("%CKPT%", &escape(&request.model));
    output = output.replace("%WIDTH%", &width.to_string());
    output = output.replace("%HEIGHT%", &height.to_string());
    output = output.replace("%STEPS%", &steps.to_string());
    output = output.replace("%CFG%", &cfg.to_string());
    output = output.replace("%SEED%", &seed.to_string());
    output = output.replace("%DENOISE%", &denoise.to_string());

    output
}

async fn queue_prompt(
    app: &AppHandle,
    base_url: &str,
    api_key: &str,
    graph: Value,
    client_id: &str,
) -> Result<String, String> {
    let url = format!("{}/prompt", base_url);
    let client = build_client(app, &url)?;
    let response = apply_auth(client.post(&url), api_key)
        .json(&serde_json::json!({ "prompt": graph, "client_id": client_id }))
        .send()
        .await
        .map_err(|error| format!("ComfyUI queue request failed: {}", error))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(format!("ComfyUI queue error {}: {}", status, text));
    }

    let body: Value = response
        .json()
        .await
        .map_err(|error| format!("Failed to parse ComfyUI queue response: {}", error))?;
    body.get("prompt_id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "ComfyUI queue response missing prompt_id".to_string())
}

async fn poll_outputs(
    app: &AppHandle,
    base_url: &str,
    api_key: &str,
    prompt_id: &str,
) -> Result<Vec<Value>, String> {
    let url = format!("{}/history/{}", base_url, prompt_id);

    for _ in 0..MAX_POLL_ATTEMPTS {
        let client = build_client(app, &url)?;
        let response = apply_auth(client.get(&url), api_key)
            .send()
            .await
            .map_err(|error| format!("ComfyUI history request failed: {}", error))?;

        if response.status().is_success() {
            let body: Value = response
                .json()
                .await
                .map_err(|error| format!("Failed to parse ComfyUI history: {}", error))?;
            if let Some(entry) = body.get(prompt_id) {
                if let Some(status) = entry.get("status") {
                    let failed = status
                        .get("status_str")
                        .and_then(Value::as_str)
                        .map(|value| value.eq_ignore_ascii_case("error"))
                        .unwrap_or(false);
                    if failed {
                        return Err("ComfyUI reported a workflow execution error".to_string());
                    }
                }
                if let Some(outputs) = entry.get("outputs").and_then(Value::as_object) {
                    let mut images = Vec::new();
                    for node in outputs.values() {
                        if let Some(node_images) = node.get("images").and_then(Value::as_array) {
                            images.extend(node_images.iter().cloned());
                        }
                    }
                    if !images.is_empty() {
                        return Ok(images);
                    }
                }
            }
        }

        sleep(Duration::from_millis(POLL_INTERVAL_MS)).await;
    }

    Err("ComfyUI generation timed out waiting for outputs".to_string())
}

async fn fetch_image(
    app: &AppHandle,
    base_url: &str,
    api_key: &str,
    image: &Value,
) -> Result<ImageResponseData, String> {
    let filename = image
        .get("filename")
        .and_then(Value::as_str)
        .ok_or_else(|| "ComfyUI output image missing filename".to_string())?;
    let subfolder = image
        .get("subfolder")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let image_type = image
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("output");

    let url = format!(
        "{}/view?filename={}&subfolder={}&type={}",
        base_url,
        urlencoding::encode(filename),
        urlencoding::encode(subfolder),
        urlencoding::encode(image_type)
    );
    let client = build_client(app, &url)?;
    let response = apply_auth(client.get(&url), api_key)
        .send()
        .await
        .map_err(|error| format!("ComfyUI image fetch failed: {}", error))?;

    if !response.status().is_success() {
        let status = response.status();
        return Err(format!("ComfyUI image fetch error {}", status));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|error| format!("Failed to read ComfyUI image bytes: {}", error))?;

    Ok(ImageResponseData {
        url: None,
        b64_json: Some(STANDARD.encode(&bytes)),
        text: None,
    })
}

pub async fn generate(
    app: &AppHandle,
    request: &ImageGenerationRequest,
    base_url: &str,
    api_key: &str,
    config: Option<&Value>,
) -> Result<Vec<ImageResponseData>, String> {
    let base_url = normalize_base_url(base_url);
    let has_images = request
        .input_images
        .as_ref()
        .is_some_and(|images| !images.is_empty());

    let template = if has_images {
        config_string(config, "img2imgWorkflow")
            .or_else(|| config_string(config, "txt2imgWorkflow"))
    } else {
        config_string(config, "txt2imgWorkflow")
    }
    .ok_or_else(|| {
        "ComfyUI credential is missing a workflow. Paste an API-format workflow JSON in the provider settings.".to_string()
    })?;

    let mut uploaded = Vec::new();
    if has_images {
        if let Some(images) = request.input_images.as_ref() {
            for (index, image) in images.iter().enumerate() {
                let name = upload_image(app, &base_url, api_key, image, index).await?;
                uploaded.push(name);
            }
        }
    }

    let substituted = substitute_tokens(&template, request, &uploaded);
    let graph: Value = serde_json::from_str(&substituted).map_err(|error| {
        format!(
            "ComfyUI workflow JSON is invalid after substitution: {}",
            error
        )
    })?;

    let client_id = Uuid::new_v4().to_string();
    let prompt_id = queue_prompt(app, &base_url, api_key, graph, &client_id).await?;

    log_info(
        app,
        "image_generator",
        format!("ComfyUI queued prompt {}", prompt_id),
    );

    let outputs = poll_outputs(app, &base_url, api_key, &prompt_id).await?;

    let mut images = Vec::new();
    for output in outputs {
        images.push(fetch_image(app, &base_url, api_key, &output).await?);
    }

    if images.is_empty() {
        return Err("ComfyUI returned no images".to_string());
    }

    Ok(images)
}
