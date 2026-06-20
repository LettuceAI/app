use std::borrow::Cow;
use std::collections::HashMap;

use serde_json::{json, Value};
use urlencoding;

use super::google_gemini::GoogleGeminiAdapter;
use super::{ModelInfo, ProviderAdapter};
use crate::chat_manager::tooling::ToolConfig;

// Express mode: same Gemini wire format, just aiplatform.googleapis.com + a different URL. Rest delegated.
pub struct GeminiAgentPlatformExpressAdapter;

impl GeminiAgentPlatformExpressAdapter {
    pub fn new() -> Self {
        Self
    }
}

const MODEL_RESOURCE_PREFIX: &str = "publishers/google/models/";

// strip the resource-path prefix down to the bare id
fn bare_model_id(model_name: &str) -> &str {
    model_name
        .strip_prefix(MODEL_RESOURCE_PREFIX)
        .unwrap_or(model_name)
}

// Nano Banana image models. single source of truth for image-ness
fn is_image_model(model_name: &str) -> bool {
    bare_model_id(model_name).ends_with("-image")
}

// force base url to /v1beta1 (upgrade a trailing /v1 or /v1beta)
fn express_base(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    if trimmed.ends_with("/v1beta1") {
        trimmed.to_string()
    } else if let Some(prefix) = trimmed
        .strip_suffix("/v1beta")
        .or_else(|| trimmed.strip_suffix("/v1"))
    {
        format!("{}/v1beta1", prefix)
    } else {
        format!("{}/v1beta1", trimmed)
    }
}

impl ProviderAdapter for GeminiAgentPlatformExpressAdapter {
    fn endpoint(&self, base_url: &str) -> String {
        express_base(base_url)
    }

    fn build_url(
        &self,
        base_url: &str,
        model_name: &str,
        _api_key: &str,
        should_stream: bool,
    ) -> String {
        // auth is the x-goog-api-key header, so no ?key= here
        let base = express_base(base_url);
        let encoded = urlencoding::encode(bare_model_id(model_name));
        let verb = if should_stream {
            "streamGenerateContent?alt=sse"
        } else {
            "generateContent"
        };
        format!("{}/{}{}:{}", base, MODEL_RESOURCE_PREFIX, encoded, verb)
    }

    fn system_role(&self) -> Cow<'static, str> {
        GoogleGeminiAdapter.system_role()
    }

    fn supports_stream(&self) -> bool {
        GoogleGeminiAdapter.supports_stream()
    }

    fn requires_api_key(&self) -> bool {
        true
    }

    fn disables_streaming_for_model(&self, model_name: &str) -> bool {
        // no point streaming a single generated image
        is_image_model(model_name)
    }

    fn required_auth_headers(&self) -> &'static [&'static str] {
        GoogleGeminiAdapter.required_auth_headers()
    }

    fn default_headers_template(&self) -> HashMap<String, String> {
        GoogleGeminiAdapter.default_headers_template()
    }

    fn headers(
        &self,
        api_key: &str,
        extra: Option<&HashMap<String, String>>,
    ) -> HashMap<String, String> {
        GoogleGeminiAdapter.headers(api_key, extra)
    }

    #[allow(clippy::too_many_arguments)]
    fn body(
        &self,
        model_name: &str,
        messages_for_api: &Vec<Value>,
        system_prompt: Option<String>,
        temperature: Option<f64>,
        top_p: Option<f64>,
        max_tokens: u32,
        context_length: Option<u32>,
        should_stream: bool,
        frequency_penalty: Option<f64>,
        presence_penalty: Option<f64>,
        top_k: Option<u32>,
        tool_config: Option<&ToolConfig>,
        reasoning_enabled: bool,
        reasoning_effort: Option<String>,
        reasoning_budget: Option<u32>,
    ) -> Value {
        let mut body = GoogleGeminiAdapter.body(
            model_name,
            messages_for_api,
            system_prompt,
            temperature,
            top_p,
            max_tokens,
            context_length,
            should_stream,
            frequency_penalty,
            presence_penalty,
            top_k,
            tool_config,
            reasoning_enabled,
            reasoning_effort,
            reasoning_budget,
        );
        // image models need responseModalities to actually emit images
        if is_image_model(model_name) {
            if let Some(cfg) = body
                .as_object_mut()
                .and_then(|b| b.entry("generationConfig").or_insert_with(|| json!({})).as_object_mut())
            {
                cfg.insert("responseModalities".into(), json!(["TEXT", "IMAGE"]));
            }
        }
        body
    }

    fn known_models(&self) -> Vec<ModelInfo> {
        // express keys can't list models, so hardcode a whitelist. unavailable ones 404 at chat time
        const TEXT_IN: &[&str] = &["text", "image"];
        const TEXT_OUT: &[&str] = &["text"];
        const IMAGE_OUT: &[&str] = &["text", "image"];
        const MODELS: &[(&str, &str, u64)] = &[
            ("gemini-3.1-pro-preview", "Gemini 3.1 Pro Preview", 1_048_576),
            ("gemini-3.5-flash", "Gemini 3.5 Flash", 1_048_576),
            ("gemini-3-flash-preview", "Gemini 3 Flash Preview", 1_048_576),
            ("gemini-3.1-flash-lite", "Gemini 3.1 Flash-Lite", 1_048_576),
            ("gemini-3-pro-image", "Gemini 3 Pro Image (Nano Banana Pro)", 32_768),
            ("gemini-3.1-flash-image", "Gemini 3.1 Flash Image (Nano Banana 2)", 32_768),
            ("gemini-2.5-flash-image", "Gemini 2.5 Flash Image (Nano Banana)", 32_768),
            ("gemini-2.5-pro", "Gemini 2.5 Pro", 1_048_576),
            ("gemini-2.5-flash", "Gemini 2.5 Flash", 1_048_576),
        ];
        let to_vec = |m: &[&str]| Some(m.iter().map(|s| s.to_string()).collect());
        MODELS
            .iter()
            .map(|(id, display_name, context_length)| ModelInfo {
                id: id.to_string(),
                display_name: Some(display_name.to_string()),
                description: None,
                context_length: Some(*context_length),
                input_modalities: to_vec(TEXT_IN),
                output_modalities: to_vec(if is_image_model(id) { IMAGE_OUT } else { TEXT_OUT }),
                supported_endpoints: None,
                input_price: None,
                output_price: None,
            })
            .collect()
    }

    fn parse_models_list(&self, response: Value) -> Vec<ModelInfo> {
        // this endpoint returns "publisherModels" instead of "models"
        let normalized = if response.get("publisherModels").is_some() {
            json!({ "models": response["publisherModels"] })
        } else {
            response
        };
        let mut models = GoogleGeminiAdapter.parse_models_list(normalized);
        // names come back prefixed — strip to bare id
        for model in &mut models {
            if let Some(stripped) = model.id.strip_prefix(MODEL_RESOURCE_PREFIX) {
                model.id = stripped.to_string();
            }
        }
        models
    }
}
