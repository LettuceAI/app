use std::borrow::Cow;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::types::ProviderCredential;
use crate::chat_manager::tooling::ToolConfig;

pub trait ProviderAdapter {
    fn endpoint(&self, base_url: &str) -> String;

    /// Build the complete URL including model name and any query parameters.
    /// Default implementation just returns endpoint(), but providers like Gemini can override.
    /// The `should_stream` flag indicates if streaming is requested (for Gemini, use streamGenerateContent).
    fn build_url(
        &self,
        base_url: &str,
        _model_name: &str,
        _api_key: &str,
        _should_stream: bool,
    ) -> String {
        self.endpoint(base_url)
    }

    /// Preferred system role keyword for this provider when sending a system message.
    fn system_role(&self) -> Cow<'static, str>;
    /// Whether this provider supports Server-Sent Events (streaming responses).
    fn supports_stream(&self) -> bool {
        true
    }
    /// Whether this provider requires an API key, regardless of whether auth is sent via
    /// headers, query params, or another transport.
    fn requires_api_key(&self) -> bool {
        !self.required_auth_headers().is_empty()
    }
    /// The required auth header keys for this provider (case sensitive suggestions for UI).
    fn required_auth_headers(&self) -> &'static [&'static str];
    /// A template of default headers (values redacted) to show expected headers without secrets.
    fn default_headers_template(&self) -> HashMap<String, String>;
    /// Build default headers for this provider using the given API key.
    /// `extra` headers from credentials are merged on top (overriding defaults when keys match).
    fn headers(
        &self,
        api_key: &str,
        extra: Option<&HashMap<String, String>>,
    ) -> HashMap<String, String>;
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
    ) -> Value;

    /// Endpoint to list models. Default implements OpenAI standard conventions.
    fn list_models_endpoint(&self, base_url: &str) -> String {
        let base = base_url.trim_end_matches('/');
        if base.ends_with("/v1") {
            format!("{}/models", base)
        } else {
            format!("{}/v1/models", base)
        }
    }

    /// Parse the response from list_models_endpoint. Default implements OpenAI standard.
    fn parse_models_list(&self, response: Value) -> Vec<ModelInfo> {
        let mut models = Vec::new();
        if let Some(data) = response.get("data").and_then(|d| d.as_array()) {
            for item in data {
                if let Some(id) = item.get("id").and_then(|id| id.as_str()) {
                    let input_price = item
                        .get("pricing")
                        .and_then(|pricing| pricing.get("prompt"))
                        .and_then(value_to_f64);
                    let output_price = item
                        .get("pricing")
                        .and_then(|pricing| pricing.get("completion"))
                        .and_then(value_to_f64);
                    models.push(ModelInfo {
                        id: id.to_string(),
                        display_name: item
                            .get("name") // Some providers use name as display name
                            .and_then(|n| n.as_str())
                            .map(|s| s.to_string()),
                        description: item
                            .get("description")
                            .and_then(|d| d.as_str())
                            .map(|s| s.to_string()),
                        context_length: item.get("context_length").and_then(|c| c.as_u64()),
                        input_price,
                        output_price,
                    });
                }
            }
        }
        models
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub context_length: Option<u64>,
    // Pricing per 1M tokens or similar, strictly for display/estimation if available
    pub input_price: Option<f64>,
    pub output_price: Option<f64>,
}

fn value_to_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.parse::<f64>().ok(),
        _ => None,
    }
}

// Shared OpenAI-style request used by multiple providers.
#[derive(Serialize)]
pub(crate) struct OpenAIChatRequest<'a> {
    pub(crate) model: &'a str,
    pub(crate) messages: &'a Vec<Value>,
    pub(crate) stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) temperature: Option<f64>,
    #[serde(rename = "top_p", skip_serializing_if = "Option::is_none")]
    pub(crate) top_p: Option<f64>,
    #[serde(rename = "max_tokens", skip_serializing_if = "Option::is_none")]
    pub(crate) max_tokens: Option<u32>,
    #[serde(rename = "context_length", skip_serializing_if = "Option::is_none")]
    pub(crate) context_length: Option<u32>,
    #[serde(
        rename = "max_completion_tokens",
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) max_completion_tokens: Option<u32>,
    #[serde(rename = "frequency_penalty", skip_serializing_if = "Option::is_none")]
    pub(crate) frequency_penalty: Option<f64>,
    #[serde(rename = "presence_penalty", skip_serializing_if = "Option::is_none")]
    pub(crate) presence_penalty: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) reasoning_effort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) reasoning: Option<ReasoningConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) tools: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) tool_choice: Option<Value>,
}

/// Reasoning configuration for OpenRouter and compatible providers.
/// OpenRouter expects reasoning params in a nested object.
#[derive(Serialize)]
pub(crate) struct ReasoningConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
}

pub(crate) fn extract_text_content(value: Option<&Value>) -> Option<String> {
    match value {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => Some(s.to_string()),
        Some(Value::Array(parts)) => {
            let text = parts
                .iter()
                .filter_map(|part| {
                    let obj = part.as_object()?;
                    if obj.get("type").and_then(|v| v.as_str()) != Some("text") {
                        return None;
                    }
                    obj.get("text")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                })
                .collect::<Vec<_>>()
                .join("\n");
            if text.trim().is_empty() {
                None
            } else {
                Some(text)
            }
        }
        Some(other) => Some(other.to_string()),
    }
}

pub(crate) fn extract_image_data_urls(value: Option<&Value>) -> Vec<String> {
    let Some(Value::Array(parts)) = value else {
        return Vec::new();
    };

    parts
        .iter()
        .filter_map(|part| {
            let obj = part.as_object()?;
            if obj.get("type").and_then(|v| v.as_str()) != Some("image_url") {
                return None;
            }

            obj.get("image_url")
                .and_then(|v| v.as_object())
                .and_then(|image_url| image_url.get("url"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .collect()
}

pub(crate) fn parse_data_url(data_url: &str) -> Option<(String, String)> {
    let (prefix, data) = data_url.split_once(";base64,")?;
    let mime_type = prefix.strip_prefix("data:")?;
    Some((mime_type.to_string(), data.to_string()))
}

pub(crate) fn is_visible_chat_system_message(message: &Value) -> bool {
    message.get("role").and_then(|v| v.as_str()) == Some("system")
        && message
            .get("visible_in_chat")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
}

pub(crate) fn visible_chat_system_instruction_text(message: &Value) -> Option<String> {
    if !is_visible_chat_system_message(message) {
        return None;
    }

    let content = extract_text_content(message.get("content"))?;
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return None;
    }

    Some(format!(
        "Visible system message from the chat UI. Treat this as a high-priority instruction that remains in effect unless later context overrides it.\n\n<system-message>\n{}\n</system-message>",
        trimmed
    ))
}

mod anannas;
mod anthropic;
mod automatic1111;
mod chutes;
mod deepseek;
mod featherless;
mod google_gemini;
mod groq;
mod intenserp;
mod llamacpp;
mod lmstudio;
mod mistral;
mod moonshot;
mod nanogpt;
mod nvidia;
mod ollama;
mod openai;
mod qwen;
mod stability;
mod xai;
mod zai;

mod custom;
mod custom_anthropic;
mod lettuce_engine;

pub fn adapter_for(credential: &ProviderCredential) -> Box<dyn ProviderAdapter + Send + Sync> {
    match credential.provider_id.as_str() {
        "custom" => Box::new(custom::CustomGenericAdapter::new(credential)),
        "custom-anthropic" => Box::new(custom_anthropic::CustomAnthropicAdapter::new(credential)),
        "ollama" => Box::new(ollama::OllamaAdapter),
        "intenserp" => Box::new(intenserp::IntenseRpAdapter),
        "llamacpp" => Box::new(llamacpp::LlamaCppAdapter),
        "lmstudio" => Box::new(lmstudio::LMStudioAdapter),
        "automatic1111" => Box::new(automatic1111::Automatic1111Adapter),
        "chutes" | "chutes.ai" => Box::new(chutes::ChutesAdapter),
        "anthropic" => Box::new(anthropic::AnthropicAdapter),
        "mistral" => Box::new(mistral::MistralAdapter),
        "groq" => Box::new(groq::GroqAdapter),
        "deepseek" => Box::new(deepseek::DeepSeekAdapter),
        "nanogpt" => Box::new(nanogpt::NanoGPTAdapter),
        "xai" => Box::new(xai::XAIAdapter),
        "anannas" => Box::new(anannas::AnannasAdapter),
        "google" | "google-gemini" | "gemini" => Box::new(google_gemini::GoogleGeminiAdapter),
        "zai" | "z.ai" => Box::new(zai::ZAIAdapter),
        "moonshot" | "moonshot-ai" => Box::new(moonshot::MoonshotAdapter),
        "featherless" => Box::new(featherless::FeatherlessAdapter),
        "nvidia" | "nvidia-nim" => Box::new(nvidia::NvidiaAdapter),
        "qwen" => Box::new(qwen::QwenAdapter),
        "stability" => Box::new(stability::StabilityAdapter),
        "openrouter" => Box::new(openai::OpenRouterAdapter),
        "lettuce-host" => Box::new(openai::OpenAIAdapter),
        "lettuce-engine" => Box::new(lettuce_engine::LettuceEngineAdapter),
        _ => Box::new(openai::OpenAIAdapter),
    }
}
