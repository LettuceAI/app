use std::borrow::Cow;
use std::collections::HashMap;

use serde_json::{json, Value};
use urlencoding;

use super::google_gemini::GoogleGeminiAdapter;
use super::ProviderAdapter;
use crate::chat_manager::tooling::ToolConfig;

// Express mode: same Gemini wire format, just aiplatform.googleapis.com + a different URL. Rest delegated.
pub struct GeminiAgentPlatformExpressAdapter;

impl GeminiAgentPlatformExpressAdapter {
    pub fn new() -> Self {
        Self
    }
}

pub const MODEL_RESOURCE_PREFIX: &str = "publishers/google/models/";

// strip the resource-path prefix down to the bare id
pub fn bare_model_id(model_name: &str) -> &str {
    model_name
        .strip_prefix(MODEL_RESOURCE_PREFIX)
        .unwrap_or(model_name)
}

// Nano Banana image models. single source of truth for image-ness
fn is_image_model(model_name: &str) -> bool {
    bare_model_id(model_name).ends_with("-image")
}

// force base url to /v1beta1 (upgrade a trailing /v1 or /v1beta)
pub fn express_base(base_url: &str) -> String {
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

    // No list_models_endpoint/parse_models_list: the Express endpoint can't list models
    // with an API key, so there's no catalog to fetch — get_remote_models returns an empty
    // list for this provider and the user enters the model id manually.
}
