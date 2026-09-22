//! ComfyUI: the account's API-format workflow with legacy's `%TOKEN%`
//! substitutions, queued, polled every 1.5 s for up to 400 attempts, and
//! its output images fetched.

use std::time::Duration;

use base64::Engine as _;
use lettuce_image_generation::{ProviderImageRequest, sd_runtime::payload::parse_size_dimensions};
use lettuce_models::ProviderConfig;
use lettuce_network::{BulkHttpClient, JsonAuth, JsonQueryParameter, MultipartField};
use lettuce_settings::SecretValue;
use serde_json::{Value, json};

use super::adapters::ImageResponseData;
use lettuce_image_generation::ImageProviderError;

const POLL_INTERVAL: Duration = Duration::from_millis(1500);
const MAX_POLL_ATTEMPTS: u32 = 400;
const MISSING_WORKFLOW: &str = "ComfyUI credential is missing a workflow. Paste an API-format workflow JSON in the provider settings.";

fn failed(message: impl Into<String>) -> ImageProviderError {
    ImageProviderError::Failed(message.into())
}

fn auth(api_key: Option<&SecretValue>) -> JsonAuth {
    api_key
        .and_then(|key| {
            key.with(|value| {
                if value.is_empty() {
                    None
                } else {
                    SecretValue::new(value).ok()
                }
            })
        })
        .map_or(JsonAuth::None, JsonAuth::Bearer)
}

fn trimmed(value: Option<&String>) -> Option<String> {
    value
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn extension_for_mime(mime_type: &str) -> &'static str {
    match mime_type {
        "image/png" => "png",
        "image/webp" => "webp",
        "image/jpeg" | "image/jpg" => "jpg",
        _ => "png",
    }
}

fn escape(value: &str) -> String {
    let quoted = serde_json::to_string(value).unwrap_or_default();
    quoted
        .get(1..quoted.len().saturating_sub(1))
        .unwrap_or_default()
        .to_owned()
}

/// Legacy `substitute_tokens`; an unset seed is random in `0..u32::MAX`.
pub(super) fn substitute_tokens(
    template: &str,
    request: &ProviderImageRequest,
    uploaded: &[String],
    random_seed: impl FnOnce() -> u64,
) -> String {
    let settings = &request.settings;
    let (width, height) = parse_size_dimensions(
        request.size.as_deref().or(settings.size.as_deref()),
        1024,
        1024,
    );
    let steps = settings.steps.unwrap_or(28);
    let cfg = settings.cfg_scale.unwrap_or(6.5);
    let sampler = settings.sampler.as_deref().unwrap_or("euler");
    let denoise = settings.denoising_strength.unwrap_or(0.75);
    let negative = settings
        .negative_prompt
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("");
    let seed = settings.seed.map_or_else(random_seed, u64::from);
    let mut output = template.to_owned();
    for (index, name) in uploaded.iter().enumerate() {
        output = output.replace(&format!("%IMAGE{index}%"), &escape(name));
    }
    for index in uploaded.len()..uploaded.len().max(16) {
        output = output.replace(&format!("%IMAGE{index}%"), "");
    }
    let first_image = uploaded.first().map_or("", String::as_str);
    output = output.replace("%IMAGE_COUNT%", &uploaded.len().to_string());
    output = output.replace("%IMAGE%", &escape(first_image));
    output = output.replace("%PROMPT%", &escape(&request.prompt));
    output = output.replace("%NEGATIVE%", &escape(negative));
    output = output.replace("%SAMPLER%", &escape(sampler));
    output = output.replace("%CKPT%", &escape(&request.external_model_id));
    output = output.replace("%WIDTH%", &width.to_string());
    output = output.replace("%HEIGHT%", &height.to_string());
    output = output.replace("%STEPS%", &steps.to_string());
    output = output.replace("%CFG%", &cfg.to_string());
    output = output.replace("%SEED%", &seed.to_string());
    output = output.replace("%DENOISE%", &denoise.to_string());
    output
}

fn random_seed() -> u64 {
    let bytes = lettuce_types::OperationId::new().to_string();
    let hash = blake3::hash(bytes.as_bytes());
    let value = u64::from_le_bytes(hash.as_bytes()[..8].try_into().unwrap_or([0; 8]));
    value % u64::from(u32::MAX)
}

pub(super) async fn generate(
    http: &BulkHttpClient,
    request: &ProviderImageRequest,
    base_url: &str,
    api_key: Option<&SecretValue>,
    insecure: bool,
) -> Result<Vec<ImageResponseData>, ImageProviderError> {
    let base = base_url.trim_end_matches('/');
    let has_images = !request.input_images.is_empty();
    let config = match &request.account.config {
        ProviderConfig::ComfyUi(config) => Some(config),
        _ => None,
    };
    let template = if has_images {
        config
            .and_then(|config| trimmed(config.img2img_workflow.as_ref()))
            .or_else(|| config.and_then(|config| trimmed(config.txt2img_workflow.as_ref())))
    } else {
        config.and_then(|config| trimmed(config.txt2img_workflow.as_ref()))
    }
    .ok_or_else(|| failed(MISSING_WORKFLOW))?;

    let mut uploaded = Vec::with_capacity(request.input_images.len());
    for (index, image) in request.input_images.iter().enumerate() {
        let response = http
            .post_multipart(
                base,
                "/upload/image",
                vec![
                    MultipartField::File {
                        name: "image".to_owned(),
                        filename: format!("ref_{index}.{}", extension_for_mime(&image.mime_type)),
                        mime_type: image.mime_type.clone(),
                        bytes: image.bytes.clone(),
                    },
                    MultipartField::Text {
                        name: "overwrite".to_owned(),
                        value: "true".to_owned(),
                    },
                ],
                &[],
                auth(api_key),
                Vec::new(),
                insecure,
            )
            .await
            .map_err(|error| failed(format!("ComfyUI image upload failed: {error}")))?;
        if !(200..300).contains(&response.status) {
            return Err(failed(format!(
                "ComfyUI image upload error {}: {}",
                lettuce_network::status_text(response.status),
                String::from_utf8_lossy(&response.body)
            )));
        }
        let body = serde_json::from_slice::<Value>(&response.body)
            .map_err(|error| failed(format!("Failed to parse ComfyUI upload response: {error}")))?;
        let name = body
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| failed("ComfyUI upload response missing image name"))?;
        let subfolder = body
            .get("subfolder")
            .and_then(Value::as_str)
            .unwrap_or_default();
        uploaded.push(if subfolder.is_empty() {
            name.to_owned()
        } else {
            format!("{subfolder}/{name}")
        });
    }

    let substituted = substitute_tokens(&template, request, &uploaded, random_seed);
    let graph = serde_json::from_str::<Value>(&substituted).map_err(|error| {
        failed(format!(
            "ComfyUI workflow JSON is invalid after substitution: {error}"
        ))
    })?;
    let client_id = lettuce_types::OperationId::new().to_string();
    let response = http
        .post_json(
            base,
            "/prompt",
            &[],
            serde_json::to_vec(&json!({"prompt": graph, "client_id": client_id}))
                .map_err(|error| failed(format!("ComfyUI queue request failed: {error}")))?,
            &[],
            auth(api_key),
            Vec::new(),
            insecure,
        )
        .await
        .map_err(|error| failed(format!("ComfyUI queue request failed: {error}")))?;
    if !(200..300).contains(&response.status) {
        return Err(failed(format!(
            "ComfyUI queue error {}: {}",
            lettuce_network::status_text(response.status),
            String::from_utf8_lossy(&response.body)
        )));
    }
    let body = serde_json::from_slice::<Value>(&response.body)
        .map_err(|error| failed(format!("Failed to parse ComfyUI queue response: {error}")))?;
    let prompt_id = body
        .get("prompt_id")
        .and_then(Value::as_str)
        .ok_or_else(|| failed("ComfyUI queue response missing prompt_id"))?
        .to_owned();
    tracing::info!(
        component = "image_generator",
        "ComfyUI queued prompt {prompt_id}"
    );

    let outputs = poll_outputs(http, base, api_key, &prompt_id, insecure).await?;
    let mut images = Vec::with_capacity(outputs.len());
    for output in outputs {
        images.push(fetch_image(http, base, api_key, &output, insecure).await?);
    }
    if images.is_empty() {
        return Err(failed("ComfyUI returned no images"));
    }
    Ok(images)
}

async fn poll_outputs(
    http: &BulkHttpClient,
    base: &str,
    api_key: Option<&SecretValue>,
    prompt_id: &str,
    insecure: bool,
) -> Result<Vec<Value>, ImageProviderError> {
    let path = format!("/history/{prompt_id}");
    for _ in 0..MAX_POLL_ATTEMPTS {
        let response = http
            .get(base, &path, &[], &[], auth(api_key), Vec::new(), insecure)
            .await
            .map_err(|error| failed(format!("ComfyUI history request failed: {error}")))?;
        if (200..300).contains(&response.status) {
            let body = serde_json::from_slice::<Value>(&response.body)
                .map_err(|error| failed(format!("Failed to parse ComfyUI history: {error}")))?;
            if let Some(entry) = body.get(prompt_id) {
                let errored = entry
                    .get("status")
                    .and_then(|status| status.get("status_str"))
                    .and_then(Value::as_str)
                    .is_some_and(|value| value.eq_ignore_ascii_case("error"));
                if errored {
                    return Err(failed("ComfyUI reported a workflow execution error"));
                }
                if let Some(outputs) = entry.get("outputs").and_then(Value::as_object) {
                    let images = outputs
                        .values()
                        .filter_map(|node| node.get("images").and_then(Value::as_array))
                        .flatten()
                        .cloned()
                        .collect::<Vec<_>>();
                    if !images.is_empty() {
                        return Ok(images);
                    }
                }
            }
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
    Err(failed("ComfyUI generation timed out waiting for outputs"))
}

async fn fetch_image(
    http: &BulkHttpClient,
    base: &str,
    api_key: Option<&SecretValue>,
    image: &Value,
    insecure: bool,
) -> Result<ImageResponseData, ImageProviderError> {
    let filename = image
        .get("filename")
        .and_then(Value::as_str)
        .ok_or_else(|| failed("ComfyUI output image missing filename"))?;
    let subfolder = image
        .get("subfolder")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let kind = image
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("output");
    let response = http
        .get(
            base,
            "/view",
            &[
                JsonQueryParameter {
                    name: "filename",
                    value: filename,
                },
                JsonQueryParameter {
                    name: "subfolder",
                    value: subfolder,
                },
                JsonQueryParameter {
                    name: "type",
                    value: kind,
                },
            ],
            &[],
            auth(api_key),
            Vec::new(),
            insecure,
        )
        .await
        .map_err(|error| failed(format!("ComfyUI image fetch failed: {error}")))?;
    if !(200..300).contains(&response.status) {
        return Err(failed(format!(
            "ComfyUI image fetch error {}",
            lettuce_network::status_text(response.status)
        )));
    }
    Ok(ImageResponseData {
        url: None,
        b64_json: Some(base64::engine::general_purpose::STANDARD.encode(&response.body)),
        text: None,
    })
}
