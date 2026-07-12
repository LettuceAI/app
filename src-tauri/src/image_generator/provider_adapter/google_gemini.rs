use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;

use super::{ImageProviderAdapter, ImageRequestPayload, ImageResponseData};
use crate::image_generator::types::ImageGenerationRequest;

pub struct GoogleGeminiAdapter;

#[derive(Serialize)]
struct GeminiContent<'a> {
    role: &'a str,
    parts: Vec<GeminiPart<'a>>,
}

#[derive(Serialize)]
struct GeminiPart<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    inline_data: Option<GeminiInlineDataRequest<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiInlineDataRequest<'a> {
    mime_type: &'a str,
    data: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiRequest<'a> {
    contents: Vec<GeminiContent<'a>>,
    generation_config: Option<GeminiGenerationConfig>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiGenerationConfig {
    response_modalities: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<GeminiResponseFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    image_config: Option<GeminiImageConfig>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiResponseFormat {
    image: GeminiImageFormat,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiImageFormat {
    aspect_ratio: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiImageConfig {
    aspect_ratio: String,
}

/// Developer API wants `responseFormat.image.aspectRatio`; Vertex/Express wants
/// `imageConfig.aspectRatio` and 400s on `responseFormat`.
enum GeminiAspectRatioStyle {
    ResponseFormat,
    ImageConfig,
}

fn gemini_aspect_ratio(size: Option<&str>) -> Option<String> {
    let (width, height) = size?.split_once('x')?;
    let width = width.trim().parse::<u32>().ok()?;
    let height = height.trim().parse::<u32>().ok()?;
    if width == 0 || height == 0 {
        return None;
    }
    let mut a = width;
    let mut b = height;
    while b != 0 {
        (a, b) = (b, a % b);
    }
    let divisor = a;
    let ratio = format!("{}:{}", width / divisor, height / divisor);
    matches!(ratio.as_str(), "1:1" | "2:3" | "3:2" | "3:4" | "4:3" | "4:5" | "5:4" | "9:16" | "16:9" | "21:9")
        .then_some(ratio)
}

fn build_gemini_payload(
    request: &ImageGenerationRequest,
    aspect_ratio_style: GeminiAspectRatioStyle,
) -> Result<ImageRequestPayload, String> {
    let mut parts = Vec::new();
    parts.push(GeminiPart {
        text: Some(&request.prompt),
        inline_data: None,
    });

    if let Some(input_images) = &request.input_images {
        for image in input_images {
            let Some((mime_type, data)) = image
                .strip_prefix("data:")
                .and_then(|rest| rest.split_once(";base64,"))
            else {
                return Err(
                    "Gemini image editing requires each input image as a base64 data URL"
                        .to_string(),
                );
            };
            if mime_type.is_empty() || data.is_empty() {
                return Err("Gemini image editing received an empty image data URL".to_string());
            }
            parts.push(GeminiPart {
                text: None,
                inline_data: Some(GeminiInlineDataRequest { mime_type, data }),
            });
        }
    }

    let content = GeminiContent {
        role: "user",
        parts,
    };

    let supports_text = request
        .output_modalities
        .as_ref()
        .map(|scopes| scopes.iter().any(|s| s.eq_ignore_ascii_case("text")))
        .unwrap_or(false);
    let mut response_modalities = Vec::new();
    if supports_text {
        response_modalities.push("TEXT".to_string());
    }
    response_modalities.push("IMAGE".to_string());

    let aspect_ratio = gemini_aspect_ratio(request.size.as_deref());
    let (response_format, image_config) = match aspect_ratio_style {
        GeminiAspectRatioStyle::ResponseFormat => (
            aspect_ratio.map(|aspect_ratio| GeminiResponseFormat {
                image: GeminiImageFormat { aspect_ratio },
            }),
            None,
        ),
        GeminiAspectRatioStyle::ImageConfig => (
            None,
            aspect_ratio.map(|aspect_ratio| GeminiImageConfig { aspect_ratio }),
        ),
    };

    let req = GeminiRequest {
        contents: vec![content],
        generation_config: Some(GeminiGenerationConfig {
            response_modalities,
            response_format,
            image_config,
        }),
    };

    Ok(ImageRequestPayload::Json(
        serde_json::to_value(req).unwrap_or_else(|_| json!({})),
    ))
}

#[derive(Deserialize)]
struct GeminiResponse {
    candidates: Vec<GeminiCandidate>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiCandidate {
    #[serde(default)]
    content: GeminiResponseContent,
    #[serde(default)]
    finish_reason: Option<String>,
    #[serde(default)]
    finish_message: Option<String>,
}

#[derive(Deserialize, Default)]
struct GeminiResponseContent {
    #[serde(default)]
    parts: Vec<GeminiResponsePart>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiResponsePart {
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    inline_data: Option<GeminiInlineData>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiInlineData {
    mime_type: String,
    data: String, // Base64 encoded
}

impl ImageProviderAdapter for GoogleGeminiAdapter {
    fn endpoint(&self, base_url: &str, _request: &ImageGenerationRequest) -> String {
        base_url.trim_end_matches('/').to_string()
    }

    fn requires_api_key(&self) -> bool {
        true
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
            for (k, v) in extra.iter() {
                headers.insert(k.clone(), v.clone());
            }
        }

        headers
    }

    fn payload(&self, request: &ImageGenerationRequest) -> Result<ImageRequestPayload, String> {
        build_gemini_payload(request, GeminiAspectRatioStyle::ResponseFormat)
    }

    fn parse_response(&self, response: Value) -> Result<Vec<ImageResponseData>, String> {
        let gemini_response: GeminiResponse = serde_json::from_value(response).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to parse response: {}", e),
            )
        })?;

        if gemini_response.candidates.is_empty() {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                "No candidates in response",
            ));
        }

        let mut images = Vec::new();
        let mut block_reason: Option<String> = None;
        for candidate in &gemini_response.candidates {
            let text = candidate.content.parts.first().and_then(|p| p.text.clone());
            let mut image_data_found = false;

            for part in &candidate.content.parts {
                if let Some(inline_data) = &part.inline_data {
                    let data_url =
                        format!("data:{};base64,{}", inline_data.mime_type, inline_data.data);
                    images.push(ImageResponseData {
                        url: None,
                        b64_json: Some(data_url),
                        text: text.clone(),
                    });
                    image_data_found = true;
                }
            }

            if !image_data_found {
                if let Some(t) = text {
                    images.push(ImageResponseData {
                        url: None,
                        b64_json: None,
                        text: Some(t),
                    });
                } else if candidate.finish_reason.as_deref().is_some_and(|r| r != "STOP") {
                    // Gemini blocked the request entirely (e.g. IMAGE_PROHIBITED_CONTENT):
                    // content has no parts at all, only a finishReason/finishMessage.
                    block_reason = Some(
                        candidate
                            .finish_message
                            .clone()
                            .or_else(|| candidate.finish_reason.clone())
                            .unwrap_or_default(),
                    );
                }
            }
        }

        if images.is_empty() {
            if let Some(reason) = block_reason {
                return Err(crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Gemini declined to generate the image: {}", reason),
                ));
            }
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                "No images found in response",
            ));
        }

        Ok(images)
    }
}

// Express variant: aiplatform.googleapis.com + x-goog-api-key, and imageConfig
// instead of responseFormat for aspect ratio (see GeminiAspectRatioStyle)
pub struct GeminiAgentPlatformExpressAdapter;

impl ImageProviderAdapter for GeminiAgentPlatformExpressAdapter {
    fn endpoint(&self, base_url: &str, request: &ImageGenerationRequest) -> String {
        // share the chat adapter's normalization so /v1 and /v1beta bases upgrade consistently
        use crate::chat_manager::provider_adapter::gemini_agent_platform_express::{
            bare_model_id, express_base, MODEL_RESOURCE_PREFIX,
        };
        let base = express_base(base_url);
        format!(
            "{}/{}{}:generateContent",
            base,
            MODEL_RESOURCE_PREFIX,
            urlencoding::encode(bare_model_id(&request.model))
        )
    }

    fn requires_api_key(&self) -> bool {
        true
    }

    fn required_auth_headers(&self) -> &'static [&'static str] {
        &["x-goog-api-key"]
    }

    fn headers(
        &self,
        api_key: &str,
        extra: Option<&HashMap<String, String>>,
    ) -> HashMap<String, String> {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".into(), "application/json".into());
        headers.insert("x-goog-api-key".into(), api_key.to_string());
        if let Some(extra) = extra {
            for (k, v) in extra.iter() {
                headers.insert(k.clone(), v.clone());
            }
        }
        headers
    }

    fn payload(&self, request: &ImageGenerationRequest) -> Result<ImageRequestPayload, String> {
        build_gemini_payload(request, GeminiAspectRatioStyle::ImageConfig)
    }

    fn parse_response(&self, response: Value) -> Result<Vec<ImageResponseData>, String> {
        GoogleGeminiAdapter.parse_response(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_dimensions_to_supported_gemini_aspect_ratios() {
        assert_eq!(gemini_aspect_ratio(Some("1024x576")), Some("16:9".to_string()));
        assert_eq!(gemini_aspect_ratio(Some("1024x1024")), Some("1:1".to_string()));
        assert_eq!(gemini_aspect_ratio(Some("800x600")), Some("4:3".to_string()));
    }

    fn request_with_size(size: &str) -> ImageGenerationRequest {
        ImageGenerationRequest {
            prompt: "a cat".to_string(),
            model: "gemini-3.1-flash-image".to_string(),
            provider_id: "gemini".to_string(),
            credential_id: "cred".to_string(),
            advanced_model_settings: None,
            input_images: None,
            output_modalities: None,
            size: Some(size.to_string()),
            quality: None,
            style: None,
            n: None,
            session_id: None,
            character_id: None,
            character_name: None,
            usage_source: None,
        }
    }

    fn payload_json(payload: ImageRequestPayload) -> Value {
        match payload {
            ImageRequestPayload::Json(v) => v,
            ImageRequestPayload::Multipart(_) => panic!("expected JSON payload"),
        }
    }

    #[test]
    fn developer_api_sends_response_format_aspect_ratio() {
        let body = payload_json(
            GoogleGeminiAdapter
                .payload(&request_with_size("1024x1024"))
                .unwrap(),
        );
        assert_eq!(
            body["generationConfig"]["responseFormat"]["image"]["aspectRatio"],
            "1:1"
        );
        assert!(body["generationConfig"]["imageConfig"].is_null());
    }

    #[test]
    fn express_adapter_sends_image_config_aspect_ratio() {
        let body = payload_json(
            GeminiAgentPlatformExpressAdapter
                .payload(&request_with_size("1024x1024"))
                .unwrap(),
        );
        assert_eq!(body["generationConfig"]["imageConfig"]["aspectRatio"], "1:1");
        assert!(body["generationConfig"]["responseFormat"].is_null());
    }

    #[test]
    fn surfaces_block_reason_when_content_has_no_parts() {
        // Captured verbatim from a real IMAGE_PROHIBITED_CONTENT response.
        let response = json!({
            "candidates": [{
                "content": {"role": "model"},
                "finishMessage": "Unable to show the generated image. The image was filtered out because it violated Google's Responsible AI practices. Try rephrasing the prompt. If you think this was an error, send feedback. Support code: 11030041.",
                "finishReason": "IMAGE_PROHIBITED_CONTENT"
            }],
            "createTime": "2026-07-11T15:14:35.409613Z",
            "modelVersion": "gemini-3.1-flash-image",
            "responseId": "211Sao2AGf-HoLAP_svcmA4",
            "usageMetadata": {"promptTokenCount": 2658, "totalTokenCount": 2658}
        });

        let err = GoogleGeminiAdapter.parse_response(response).unwrap_err();
        assert!(
            err.contains("Responsible AI practices"),
            "expected block reason in error, got: {err}"
        );
    }
}
