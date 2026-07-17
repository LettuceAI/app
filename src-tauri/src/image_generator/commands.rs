use std::collections::{HashMap, HashSet};

use tauri::AppHandle;
use uuid::Uuid;

use crate::chat_manager::types::ProviderId;
use crate::chat_manager::{prompting::request as chat_request, types::UsageSummary};
use crate::providers::config::resolve_base_url;
use crate::usage::{
    add_usage_record,
    tracking::{RequestUsage, UsageFinishReason, UsageOperationType},
};
use crate::utils::{log_error, log_info, now_millis};

use super::provider_adapter::{get_adapter, ImageRequestPayload, ImageResponseData};
use super::storage::save_image;
use super::types::{GeneratedImage, ImageGenerationRequest, ImageGenerationResponse, ImageLora};

fn merged_lora_keywords(base: Option<&[ImageLora]>, request: Option<&[ImageLora]>) -> Vec<String> {
    let mut keywords = Vec::new();
    let mut seen = HashSet::new();
    let mut active_loras = base.unwrap_or_default().iter().collect::<Vec<_>>();
    for lora in request.unwrap_or_default() {
        if let Some(existing) = active_loras.iter_mut().find(|existing| {
            existing.path == lora.path && existing.is_high_noise == lora.is_high_noise
        }) {
            *existing = lora;
        } else {
            active_loras.push(lora);
        }
    }
    for keyword in active_loras.iter().flat_map(|lora| lora.keywords.iter()) {
        let keyword = keyword.trim();
        if keyword.is_empty() {
            continue;
        }
        if seen.insert(keyword.to_lowercase()) {
            keywords.push(keyword.to_string());
        }
    }
    keywords
}

fn active_lora_keywords(request: &ImageGenerationRequest) -> Vec<String> {
    merged_lora_keywords(
        request
            .advanced_model_settings
            .as_ref()
            .and_then(|settings| settings.sd_base_loras.as_deref()),
        request.loras.as_deref(),
    )
}

fn compose_image_prompt(
    prompt: &str,
    pre_prompt: Option<&str>,
    lora_keywords: &[String],
) -> String {
    let mut parts = Vec::new();
    let existing_prompt_text = format!("{}\n{}", pre_prompt.unwrap_or_default(), prompt);
    let existing_prompt_text = existing_prompt_text.to_lowercase();
    if let Some(pre_prompt) = pre_prompt.map(str::trim).filter(|value| !value.is_empty()) {
        parts.push(pre_prompt.to_string());
    }
    parts.extend(
        lora_keywords
            .iter()
            .map(|keyword| keyword.trim())
            .filter(|keyword| !keyword.is_empty())
            .filter(|keyword| !existing_prompt_text.contains(&keyword.to_lowercase()))
            .map(str::to_string),
    );
    let prompt = prompt.trim();
    if !prompt.is_empty() {
        parts.push(prompt.to_string());
    }
    parts.join(", ")
}

fn gemini_image_endpoint(base_url: &str, model: &str, api_key: &str) -> String {
    let base = base_url.trim_end_matches('/');
    let base = base
        .strip_suffix("/v1beta")
        .or_else(|| base.strip_suffix("/v1"))
        .unwrap_or(base);
    format!(
        "{}/v1beta/models/{}:generateContent?key={}",
        base, model, api_key
    )
}

fn record_image_generation_usage(
    app: &AppHandle,
    request: &ImageGenerationRequest,
    provider_label: &str,
    usage_summary: Option<&UsageSummary>,
    success: bool,
    error_message: Option<String>,
    image_count: usize,
) {
    let mut metadata = HashMap::new();
    metadata.insert("image_generation".to_string(), "true".to_string());
    metadata.insert(
        "input_image_count".to_string(),
        request
            .input_images
            .as_ref()
            .map_or(0, Vec::len)
            .to_string(),
    );
    metadata.insert("output_image_count".to_string(), image_count.to_string());
    if let Some(source) = request.usage_source.as_deref() {
        metadata.insert("usage_source".to_string(), source.to_string());
    }

    let session_id = request
        .session_id
        .clone()
        .unwrap_or_else(|| "image_generation".to_string());
    let character_id = request
        .character_id
        .clone()
        .unwrap_or_else(|| "image_generation".to_string());
    let character_name = request
        .character_name
        .clone()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "Image Generation".to_string());

    let usage = RequestUsage {
        id: Uuid::new_v4().to_string(),
        timestamp: now_millis().unwrap_or(0),
        session_id,
        character_id,
        character_name,
        model_id: request.model.clone(),
        model_name: request.model.clone(),
        provider_id: request.provider_id.clone(),
        provider_label: provider_label.to_string(),
        operation_type: UsageOperationType::ImageGeneration,
        finish_reason: Some(if success {
            UsageFinishReason::Stop
        } else {
            UsageFinishReason::Error
        }),
        prompt_tokens: usage_summary.and_then(|usage| usage.prompt_tokens),
        completion_tokens: usage_summary.and_then(|usage| usage.completion_tokens),
        total_tokens: usage_summary.and_then(|usage| usage.total_tokens),
        cached_prompt_tokens: None,
        cache_write_tokens: None,
        memory_tokens: None,
        summary_tokens: None,
        reasoning_tokens: usage_summary.and_then(|usage| usage.reasoning_tokens),
        image_tokens: usage_summary.and_then(|usage| usage.image_tokens),
        audio_tokens: usage_summary.and_then(|usage| usage.audio_tokens),
        web_search_requests: usage_summary.and_then(|usage| usage.web_search_requests),
        api_cost: usage_summary.and_then(|usage| usage.api_cost),
        cost: None,
        success,
        error_message,
        metadata,
    };

    if let Err(err) = add_usage_record(app, usage) {
        log_error(
            app,
            "image_generator",
            format!("failed to record image generation usage: {}", err),
        );
    }
}

#[tauri::command]
pub async fn generate_image(
    app: AppHandle,
    mut request: ImageGenerationRequest,
) -> Result<ImageGenerationResponse, String> {
    if request.provider_id == "sdcpp" {
        super::sdcpp::hydrate_lora_keywords(&app, &mut request)?;
    }
    let pre_prompt = request
        .advanced_model_settings
        .as_ref()
        .and_then(|settings| settings.sd_extra_prompt.as_ref())
        .map(String::as_str);
    let lora_keywords = active_lora_keywords(&request);
    request.prompt = compose_image_prompt(&request.prompt, pre_prompt, &lora_keywords);

    let mut provider_label = request.provider_id.clone();

    if request.output_modalities.is_none() {
        request.output_modalities = crate::storage_manager::settings::get_model_output_scopes(
            &app,
            &request.model,
            &request.provider_id,
        )
        .ok()
        .flatten();
    }

    let result: Result<(ImageGenerationResponse, Option<UsageSummary>), String> = async {
        log_info(
            &app,
            "image_generator",
            format!("Generating image with model: {}", request.model),
        );

        let provider_cred = crate::storage_manager::providers::get_provider_credential(
            &app,
            &request.credential_id,
        )?;
        provider_label = provider_cred.label.clone();

        if request.provider_id == "sdcpp" {
            let response = super::sdcpp::generate(&app, &request).await?;
            return Ok((response, None));
        }

        if request.provider_id == "comfyui" {
            let api_key = provider_cred.api_key.clone().unwrap_or_default();
            let base_url = resolve_base_url(
                &ProviderId(request.provider_id.clone()),
                provider_cred.base_url.as_deref(),
            );
            let image_data = super::comfyui::generate(
                &app,
                &request,
                &base_url,
                &api_key,
                provider_cred.config.as_ref(),
            )
            .await?;

            let mut generated_images = Vec::new();
            for img_data in image_data {
                let image_source = match img_data.url.as_ref().or(img_data.b64_json.as_ref()) {
                    Some(source) => source,
                    None => return Err("No image data in ComfyUI response.".to_string()),
                };
                let saved = save_image(&app, image_source).await?;
                generated_images.push(GeneratedImage {
                    asset_id: saved.asset_id,
                    file_path: saved.file_path,
                    mime_type: saved.mime_type,
                    url: img_data.url,
                    width: saved.width,
                    height: saved.height,
                    text: img_data.text,
                });
            }

            return Ok((
                ImageGenerationResponse {
                    images: generated_images,
                    model: request.model.clone(),
                    provider_id: request.provider_id.clone(),
                },
                None,
            ));
        }

        let adapter = get_adapter(&request.provider_id)?;

        let api_key = if !adapter.requires_api_key() {
            provider_cred.api_key.unwrap_or_default()
        } else {
            provider_cred
                .api_key
                .ok_or_else(|| "API key not found for provider".to_string())?
        };

        let base_url_opt = provider_cred.base_url.as_deref();
        let headers_map = provider_cred.headers;

        let base_url = resolve_base_url(&ProviderId(request.provider_id.clone()), base_url_opt);

        let url = if request.provider_id == "gemini" {
            gemini_image_endpoint(&base_url, &request.model, &api_key)
        } else {
            adapter.endpoint(&base_url, &request)
        };

        let headers = adapter.headers(&api_key, headers_map.as_ref());

        let payload = adapter.payload(&request)?;

        log_info(
            &app,
            "image_generator",
            format!("Sending request to: {}", url),
        );

        let client = crate::transport::build_client(
            &app,
            None,
            false,
            Some(request.provider_id.as_str()),
            Some(url.as_str()),
        )
        .map_err(|e| crate::utils::err_msg(module_path!(), line!(), e.to_string()))?;
        let mut req_builder = client.post(&url);

        let is_multipart = matches!(payload, ImageRequestPayload::Multipart(_));
        for (key, value) in headers {
            if is_multipart && key.eq_ignore_ascii_case("content-type") {
                continue;
            }
            req_builder = req_builder.header(key, value);
        }
        req_builder = match payload {
            ImageRequestPayload::Json(body) => req_builder.json(&body),
            ImageRequestPayload::Multipart(form) => req_builder.multipart(form),
        };

        let response = req_builder.send().await.map_err(|e| {
            crate::utils::err_msg(module_path!(), line!(), format!("Request failed: {}", e))
        })?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("API error {}: {}", status, error_text),
            ));
        }

        let response_json: serde_json::Value = response.json().await.map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to parse response: {}", e),
            )
        })?;
        let usage_summary = chat_request::extract_usage(&response_json);

        log_info(
            &app,
            "image_generator",
            format!("Received response: {}", response_json),
        );

        let image_data: Vec<ImageResponseData> = adapter.parse_response(response_json)?;

        let mut generated_images = Vec::new();
        for img_data in image_data {
            let image_source = match img_data.url.as_ref().or(img_data.b64_json.as_ref()) {
                Some(source) => source,
                None => {
                    let detail = img_data
                        .text
                        .as_deref()
                        .map(str::trim)
                        .filter(|text| !text.is_empty())
                        .map(|text| text.chars().take(160).collect::<String>())
                        .map(|snippet| format!(" Provider returned text instead: {}", snippet))
                        .unwrap_or_default();

                    return Err(format!("No image URL or data in response.{}", detail));
                }
            };

            let saved = save_image(&app, image_source).await?;

            generated_images.push(GeneratedImage {
                asset_id: saved.asset_id,
                file_path: saved.file_path,
                mime_type: saved.mime_type,
                url: img_data.url,
                width: saved.width,
                height: saved.height,
                text: img_data.text,
            });
        }

        Ok((
            ImageGenerationResponse {
                images: generated_images,
                model: request.model.clone(),
                provider_id: request.provider_id.clone(),
            },
            usage_summary,
        ))
    }
    .await;

    match &result {
        Ok((response, usage_summary)) => record_image_generation_usage(
            &app,
            &request,
            &provider_label,
            usage_summary.as_ref(),
            true,
            None,
            response.images.len(),
        ),
        Err(err) => record_image_generation_usage(
            &app,
            &request,
            &provider_label,
            None,
            false,
            Some(err.clone()),
            0,
        ),
    }

    result.map(|(response, _)| response)
}

#[cfg(test)]
mod tests {
    use super::{compose_image_prompt, merged_lora_keywords, ImageLora};

    #[test]
    fn pre_prompt_is_applied_once_before_the_user_prompt() {
        assert_eq!(
            compose_image_prompt("a portrait", Some("cinematic lighting"), &[]),
            "cinematic lighting, a portrait"
        );
        assert_eq!(compose_image_prompt("a portrait", Some("  "), &[]), "a portrait");
        assert_eq!(
            compose_image_prompt("  ", Some("cinematic lighting"), &[]),
            "cinematic lighting"
        );
    }

    #[test]
    fn lora_keywords_are_inserted_between_the_pre_prompt_and_user_prompt() {
        let keywords = vec!["ArsMovieStill".to_string(), "cinematic still".to_string()];
        assert_eq!(
            compose_image_prompt("a portrait", Some("high detail"), &keywords),
            "high detail, ArsMovieStill, cinematic still, a portrait"
        );
    }

    #[test]
    fn lora_keywords_already_written_by_the_scene_writer_are_not_duplicated() {
        let keywords = vec!["ArsSamuel".to_string(), "MayaTrigger".to_string()];
        assert_eq!(
            compose_image_prompt(
                "ArsSamuel offers a mug to MayaTrigger",
                None,
                &keywords
            ),
            "ArsSamuel offers a mug to MayaTrigger"
        );
    }

    #[test]
    fn request_lora_keywords_override_model_level_keywords_for_the_same_lora() {
        let base = vec![ImageLora {
            path: "style.safetensors".to_string(),
            multiplier: 0.8,
            is_high_noise: false,
            keywords: vec!["old trigger".to_string()],
        }];
        let request = vec![ImageLora {
            path: "style.safetensors".to_string(),
            multiplier: 1.0,
            is_high_noise: false,
            keywords: vec!["new trigger".to_string()],
        }];

        assert_eq!(
            merged_lora_keywords(Some(&base), Some(&request)),
            vec!["new trigger"]
        );
    }
}
