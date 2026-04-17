use std::collections::HashMap;

use serde_json::{json, Value};

use super::{ModelInfo, ProviderAdapter};
use crate::chat_manager::tooling::ToolConfig;

// ---------------------------------------------------------------------------
// Shared helpers — called by both PollinationsTextAdapter and
// PollinationsImageAdapter so neither delegates to the other.
// ---------------------------------------------------------------------------

pub(crate) fn extract_base_url(base_url: &str) -> String {
    base_url.trim_end_matches('/').to_string()
}

fn pollinations_required_auth_headers() -> &'static [&'static str] {
    &["Authorization"]
}

fn pollinations_default_headers_template() -> HashMap<String, String> {
    let mut out = HashMap::new();
    out.insert("Content-Type".into(), "application/json".into());
    out.insert("Authorization".into(), "Bearer <apiKey>".into());
    out.insert("Accept".into(), "text/event-stream".into());
    out
}

fn pollinations_headers(
    api_key: &str,
    extra: Option<&HashMap<String, String>>,
) -> HashMap<String, String> {
    let mut out = HashMap::new();
    out.insert("Content-Type".into(), "application/json".into());
    out.insert("Authorization".into(), format!("Bearer {}", api_key));
    out.insert("Accept".into(), "text/event-stream".into());
    out.entry("User-Agent".into())
        .or_insert_with(|| "LettuceAI/0.1".into());
    if let Some(extra) = extra {
        for (k, v) in extra.iter() {
            out.insert(k.clone(), v.clone());
        }
    }
    out
}

/// Parse a Pollinations **text** model list response.
// Example: 
// {
//   "object": "list",
//   "data": [
//     {
//       "id": "openai",
//       "object": "model",
//       "created": 1700000000,
//       "owned_by": "pollinations"},
//     {"id": "claude",
//       "object": "model",
//       "created": 1700000000,
//       "owned_by": "pollinations"}]}
//--------------------------------------------------//
// {"type": "object",
//   "properties": {"object": {"type": "string", "const": "list"},
//     "data": {"type": "array",
//       "items": {"type": "object",
//        "properties": {"id": {"type": "string"},
//           "object": {"type": "string", "const": "model"},
//           "created": {"type": "number"},
//           "input_modalities": {"type": "array", "items": {"type": "string"}},
//           "output_modalities": {"type": "array", "items": {"type": "string"}},
//           "supported_endpoints": {"type": "array", "items": {"type": "string"}},
//           "tools": {"type": "boolean"},
//           "reasoning": {"type": "boolean"},
//           "context_length": {"type": "number"}},
//         "required": ["id", "object", "created"],
//         "description": "OpenAI-compatible model object with capability metadata"}}},
//   "required": ["object", "data"],
//   "description": "OpenAI-compatible list of available models."}
//---------------------------------------------------//
fn parse_text_models(response: &Value) -> Vec<ModelInfo> {
    let mut models = Vec::new();
    let Some(data) = response.get("data").and_then(|d| d.as_array()) else {
        return models;
    };
    for item in data {
        let Some(id) = item.get("id").and_then(|v| v.as_str()) else {
            continue;
        };
        let description_opt = item
            .get("description")
            .and_then(|d| d.as_str())
            .map(|s| s.to_string());
        models.push(ModelInfo {
            id: id.to_string(),
            display_name: description_opt.clone(),
            description: description_opt,
            context_length: item.get("context_length").and_then(|c| c.as_u64()),
            input_price: None,
            output_price: None,
        });
    }
    models
}

/// Parse a Pollinations **image** model list response.
// Example:
// [{"name": "flux", "aliases": [],
//     "pricing": {"currency": "pollen", "completionImageTokens": "0.001"},
//     "description": "Flux Schnell - Fast high-quality image generation",
//     "input_modalities": ["text"], "output_modalities": ["image"]},
//   {"name": "zimage", "aliases": ["z-image", "z-image-turbo"],
//     "pricing": {"currency": "pollen", "completionImageTokens": "0.002"},
//     "description": "Z-Image Turbo - Fast 6B Flux with 2x upscaling",
//     "input_modalities": ["text"],
//     "output_modalities": ["image"]}
//------------------------------------------------------------------//

fn parse_image_models(response: &Value) -> Vec<ModelInfo> {
    let mut models = Vec::new();
    let Some(data) = response.as_array() else {
        return models;
    };
    for item in data {
        let Some(name) = item.get("name").and_then(|v| v.as_str()) else {
            continue;
        };
        let description_opt = item
            .get("description")
            .and_then(|d| d.as_str())
            .map(|s| s.to_string());
        models.push(ModelInfo {
            id: name.to_string(),
            display_name: description_opt.clone(),
            description: description_opt,
            context_length: None,
            input_price: None,
            output_price: None,
        });
    }
    models
}

// ---------------------------------------------------------------------------
// PollinationsTextAdapter
// ---------------------------------------------------------------------------

pub struct PollinationsTextAdapter;

impl ProviderAdapter for PollinationsTextAdapter {
    fn endpoint(&self, base_url: &str) -> String {
        format!("{}/v1/chat/completions", extract_base_url(base_url))
    }

    fn system_role(&self) -> std::borrow::Cow<'static, str> {
        "system".into()
    }

    fn required_auth_headers(&self) -> &'static [&'static str] {
        pollinations_required_auth_headers()
    }

    fn default_headers_template(&self) -> HashMap<String, String> {
        pollinations_default_headers_template()
    }

    fn headers(
        &self,
        api_key: &str,
        extra: Option<&HashMap<String, String>>,
    ) -> HashMap<String, String> {
        pollinations_headers(api_key, extra)
    }

    fn body(
        &self,
        model_name: &str,
        messages_for_api: &Vec<Value>,
        _system_prompt: Option<String>,
        temperature: Option<f64>,
        top_p: Option<f64>,
        max_tokens: u32,
        context_length: Option<u32>,
        should_stream: bool,
        frequency_penalty: Option<f64>,
        presence_penalty: Option<f64>,
        _top_k: Option<u32>,
        tool_config: Option<&ToolConfig>,
        reasoning_enabled: bool,
        reasoning_effort: Option<String>,
        reasoning_budget: Option<u32>,
    ) -> Value {
        crate::chat_manager::provider_adapter::openai::OpenAIAdapter.body(
            model_name,
            messages_for_api,
            _system_prompt,
            temperature,
            top_p,
            max_tokens,
            context_length,
            should_stream,
            frequency_penalty,
            presence_penalty,
            _top_k,
            tool_config,
            reasoning_enabled,
            reasoning_effort,
            reasoning_budget,
        )
    }

    fn list_models_endpoint(&self, base_url: &str) -> String {
        format!("{}/v1/models", extract_base_url(base_url))
    }

    fn parse_models_list(&self, response: Value) -> Vec<ModelInfo> {
        parse_text_models(&response)
    }
}

// ---------------------------------------------------------------------------
// PollinationsImageAdapter
// ---------------------------------------------------------------------------

pub struct PollinationsImageAdapter;

impl ProviderAdapter for PollinationsImageAdapter {
    fn endpoint(&self, _base_url: &str) -> String {
        // This adapter in chat_manager is only used for list_models_endpoint.
        // Real image generation routing happens in the image_generator module.
        String::new()
    }

    fn system_role(&self) -> std::borrow::Cow<'static, str> {
        "system".into()
    }

    fn required_auth_headers(&self) -> &'static [&'static str] {
        pollinations_required_auth_headers()
    }

    fn default_headers_template(&self) -> HashMap<String, String> {
        let mut out = HashMap::new();
        out.insert("Content-Type".into(), "application/json".into());
        out.insert("Authorization".into(), "Bearer <apiKey>".into());
        out
    }

    fn headers(
        &self,
        api_key: &str,
        extra: Option<&HashMap<String, String>>,
    ) -> HashMap<String, String> {
        let mut out = HashMap::new();
        out.insert("Content-Type".into(), "application/json".into());
        out.insert("Authorization".into(), format!("Bearer {}", api_key));
        out.entry("User-Agent".into())
            .or_insert_with(|| "LettuceAI/0.1".into());
        if let Some(extra) = extra {
            for (k, v) in extra.iter() {
                out.insert(k.clone(), v.clone());
            }
        }
        out
    }

    fn body(
        &self,
        _model_name: &str,
        _messages_for_api: &Vec<Value>,
        _system_prompt: Option<String>,
        _temperature: Option<f64>,
        _top_p: Option<f64>,
        _max_tokens: u32,
        _context_length: Option<u32>,
        _should_stream: bool,
        _frequency_penalty: Option<f64>,
        _presence_penalty: Option<f64>,
        _top_k: Option<u32>,
        _tool_config: Option<&ToolConfig>,
        _reasoning_enabled: bool,
        _reasoning_effort: Option<String>,
        _reasoning_budget: Option<u32>,
    ) -> Value {
        json!({})
    }

    fn list_models_endpoint(&self, base_url: &str) -> String {
        format!("{}/image/models", extract_base_url(base_url))
    }

    fn parse_models_list(&self, response: Value) -> Vec<ModelInfo> {
        parse_image_models(&response)
    }
}

