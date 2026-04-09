use std::collections::HashMap;

use tauri::AppHandle;
use uuid::Uuid;

use crate::chat_manager::types::ProviderId;
use crate::providers::config::resolve_base_url;
use crate::usage::{
    add_usage_record,
    tracking::{RequestUsage, UsageFinishReason, UsageOperationType},
};
use crate::utils::{log_error, log_info, now_millis};

use super::provider_adapter::{get_adapter, ImageRequestPayload, ImageResponseData};
use super::storage::save_image;
use super::types::{GeneratedImage, ImageGenerationRequest, ImageGenerationResponse};

fn record_image_generation_usage(
    app: &AppHandle,
    request: &ImageGenerationRequest,
    provider_label: &str,
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

    let usage = RequestUsage {
        id: Uuid::new_v4().to_string(),
        timestamp: now_millis().unwrap_or(0),
        session_id: "image_generation".to_string(),
        character_id: "image_generation".to_string(),
        character_name: "Image Generation".to_string(),
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
        prompt_tokens: None,
        completion_tokens: None,
        total_tokens: None,
        cached_prompt_tokens: None,
        cache_write_tokens: None,
        memory_tokens: None,
        summary_tokens: None,
        reasoning_tokens: None,
        image_tokens: None,
        web_search_requests: None,
        api_cost: None,
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
    request: ImageGenerationRequest,
) -> Result<ImageGenerationResponse, String> {
    let mut provider_label = request.provider_id.clone();

    let result: Result<ImageGenerationResponse, String> = async {
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
            format!(
                "{}/v1beta/models/{}:generateContent?key={}",
                base_url, request.model, api_key
            )
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

        let client = reqwest::Client::new();
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

        Ok(ImageGenerationResponse {
            images: generated_images,
            model: request.model.clone(),
            provider_id: request.provider_id.clone(),
        })
    }
    .await;

    match &result {
        Ok(response) => record_image_generation_usage(
            &app,
            &request,
            &provider_label,
            true,
            None,
            response.images.len(),
        ),
        Err(err) => record_image_generation_usage(
            &app,
            &request,
            &provider_label,
            false,
            Some(err.clone()),
            0,
        ),
    }

    result
}
