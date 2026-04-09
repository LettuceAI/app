use std::collections::HashMap;

use serde_json::{json, Value};

use super::ProviderAdapter;
use crate::chat_manager::tooling::ToolConfig;

pub struct StabilityAdapter;

impl ProviderAdapter for StabilityAdapter {
    fn endpoint(&self, base_url: &str) -> String {
        format!(
            "{}/v1/generation/stable-diffusion-xl-1024-v1-0/text-to-image",
            base_url.trim_end_matches('/')
        )
    }

    fn system_role(&self) -> std::borrow::Cow<'static, str> {
        "system".into()
    }

    fn supports_stream(&self) -> bool {
        false
    }

    fn required_auth_headers(&self) -> &'static [&'static str] {
        &["Authorization"]
    }

    fn default_headers_template(&self) -> HashMap<String, String> {
        let mut out = HashMap::new();
        out.insert("Authorization".into(), "Bearer <apiKey>".into());
        out.insert("Content-Type".into(), "application/json".into());
        out.insert("Accept".into(), "application/json".into());
        out
    }

    fn headers(
        &self,
        api_key: &str,
        extra: Option<&HashMap<String, String>>,
    ) -> HashMap<String, String> {
        let mut out = HashMap::new();
        out.insert("Authorization".into(), format!("Bearer {}", api_key));
        out.insert("Content-Type".into(), "application/json".into());
        out.insert("Accept".into(), "application/json".into());
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
}
