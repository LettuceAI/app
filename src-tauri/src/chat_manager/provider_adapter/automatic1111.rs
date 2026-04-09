use std::collections::HashMap;

use serde_json::{json, Value};

use super::{ModelInfo, ProviderAdapter};
use crate::chat_manager::tooling::ToolConfig;

pub struct Automatic1111Adapter;

impl ProviderAdapter for Automatic1111Adapter {
    fn endpoint(&self, base_url: &str) -> String {
        let trimmed = base_url
            .trim_end_matches('/')
            .trim_end_matches("/sdapi/v1")
            .to_string();
        format!("{}/sdapi/v1/txt2img", trimmed)
    }

    fn system_role(&self) -> std::borrow::Cow<'static, str> {
        "system".into()
    }

    fn supports_stream(&self) -> bool {
        false
    }

    fn required_auth_headers(&self) -> &'static [&'static str] {
        &[]
    }

    fn default_headers_template(&self) -> HashMap<String, String> {
        let mut out = HashMap::new();
        out.insert("Content-Type".into(), "application/json".into());
        out
    }

    fn headers(
        &self,
        _api_key: &str,
        extra: Option<&HashMap<String, String>>,
    ) -> HashMap<String, String> {
        let mut out = HashMap::new();
        out.insert("Content-Type".into(), "application/json".into());
        if let Some(extra) = extra {
            for (key, value) in extra {
                out.insert(key.clone(), value.clone());
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
        let trimmed = base_url
            .trim_end_matches('/')
            .trim_end_matches("/sdapi/v1")
            .to_string();
        format!("{}/sdapi/v1/sd-models", trimmed)
    }

    fn parse_models_list(&self, response: Value) -> Vec<ModelInfo> {
        response
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|item| {
                let id = item
                    .get("title")
                    .or_else(|| item.get("model_name"))
                    .and_then(|value| value.as_str())?;
                Some(ModelInfo {
                    id: id.to_string(),
                    display_name: item
                        .get("model_name")
                        .and_then(|value| value.as_str())
                        .map(|value| value.to_string()),
                    description: item
                        .get("filename")
                        .and_then(|value| value.as_str())
                        .map(|value| value.to_string()),
                    context_length: None,
                    input_price: None,
                    output_price: None,
                })
            })
            .collect()
    }
}
