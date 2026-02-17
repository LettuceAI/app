use std::borrow::Cow;
use std::collections::HashMap;

use serde_json::{json, Value};

use super::ProviderAdapter;
use crate::chat_manager::tooling::ToolConfig;

/// Stub adapter for the Lettuce Engine provider.
///
/// The Engine is NOT a standard LLM proxy — it has its own chat/character system.
/// This adapter exists only because `providers::config` calls `adapter_for()` for
/// every registered provider to build the config cache. None of the methods are
/// used in practice for Engine-backed conversations.
pub struct LettuceEngineAdapter;

impl ProviderAdapter for LettuceEngineAdapter {
    fn endpoint(&self, base_url: &str) -> String {
        format!("{}/health", base_url.trim_end_matches('/'))
    }

    fn system_role(&self) -> Cow<'static, str> {
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
        if let Some(extra) = extra {
            for (k, v) in extra {
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
        _temperature: f64,
        _top_p: f64,
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
        // Engine-backed characters never use this adapter for chat.
        json!({})
    }
}
