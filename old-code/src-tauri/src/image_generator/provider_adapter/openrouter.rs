use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;

use super::{ImageProviderAdapter, ImageRequestPayload, ImageResponseData};
use crate::image_generator::types::ImageGenerationRequest;

pub struct OpenRouterAdapter;

#[derive(Serialize)]
struct OpenRouterMessage<'a> {
    role: &'a str,
    content: Value,
}

#[derive(Serialize)]
struct OpenRouterRequest<'a> {
    model: &'a str,
    messages: Vec<OpenRouterMessage<'a>>,
    modalities: Vec<&'a str>,
}

#[derive(Deserialize)]
struct OpenRouterResponse {
    choices: Vec<OpenRouterChoice>,
}

#[derive(Deserialize)]
struct OpenRouterChoice {
    message: OpenRouterResponseMessage,
}

#[derive(Deserialize)]
struct OpenRouterResponseMessage {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    images: Vec<OpenRouterImage>,
}

#[derive(Deserialize)]
struct OpenRouterImage {
    #[serde(rename = "type")]
    _type: Option<String>,
    image_url: OpenRouterImageUrl,
}

#[derive(Deserialize)]
struct OpenRouterImageUrl {
    url: String,
}

impl ImageProviderAdapter for OpenRouterAdapter {
    fn endpoint(&self, base_url: &str, _request: &ImageGenerationRequest) -> String {
        let trimmed = base_url.trim_end_matches('/');
        format!("{}/v1/chat/completions", trimmed)
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
        headers.insert("HTTP-Referer".into(), "https://www.lettuceai.app/".into());

        if let Some(extra) = extra {
            for (k, v) in extra.iter() {
                headers.insert(k.clone(), v.clone());
            }
        }

        headers
    }

    fn payload(&self, request: &ImageGenerationRequest) -> Result<ImageRequestPayload, String> {
        let content = if let Some(input_images) = &request.input_images {
            let mut parts = vec![json!({
                "type": "text",
                "text": request.prompt,
            })];

            for image in input_images {
                parts.push(json!({
                    "type": "image_url",
                    "image_url": {
                        "url": image,
                    }
                }));
            }

            Value::Array(parts)
        } else {
            Value::String(request.prompt.clone())
        };

        let message = OpenRouterMessage {
            role: "user",
            content,
        };

        let mut modalities: Vec<&str> = vec!["image"];
        let supports_text = match &request.output_modalities {
            Some(scopes) => scopes.iter().any(|s| s.eq_ignore_ascii_case("text")),
            None => false,
        };
        if supports_text {
            modalities.push("text");
        }

        let req = OpenRouterRequest {
            model: &request.model,
            messages: vec![message],
            modalities,
        };

        Ok(ImageRequestPayload::Json(
            serde_json::to_value(req).unwrap_or_else(|_| json!({})),
        ))
    }

    fn parse_response(&self, response: Value) -> Result<Vec<ImageResponseData>, String> {
        let or_response: OpenRouterResponse = serde_json::from_value(response).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to parse response: {}", e),
            )
        })?;

        if or_response.choices.is_empty() {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                "No choices in response",
            ));
        }

        let mut results = Vec::new();

        for choice in or_response.choices {
            let text = choice.message.content.filter(|s| !s.is_empty());

            if !choice.message.images.is_empty() {
                for img in choice.message.images {
                    let image_url = img.image_url.url;

                    let (url, b64_json) = if image_url.starts_with("data:") {
                        (None, Some(image_url))
                    } else {
                        (Some(image_url), None)
                    };

                    results.push(ImageResponseData {
                        url,
                        b64_json,
                        text: text.clone(),
                    });
                }
            } else if let Some(t) = text {
                results.push(ImageResponseData {
                    url: None,
                    b64_json: None,
                    text: Some(t),
                });
            }
        }

        if results.is_empty() {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                "No images or text generated in response",
            ));
        }

        Ok(results)
    }
}
