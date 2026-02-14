use crate::chat_manager::provider_adapter::adapter_for;
use crate::chat_manager::types::{ProviderCredential, ProviderId};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub id: String,
    pub name: String,
    pub default_base_url: String,
    pub api_endpoint_path: String,
    pub system_role: String,
    pub supports_stream: bool,
    pub required_auth_headers: Vec<String>,
    pub default_headers: HashMap<String, String>,
}

#[tauri::command]
pub fn get_provider_configs() -> Vec<ProviderConfig> {
    get_cached_provider_configs().clone()
}

fn path_from_url(url: &str) -> String {
    // naive extraction of path from a URL without pulling in a URL parser
    if let Some(scheme_idx) = url.find("://") {
        if let Some(path_start) = url[scheme_idx + 3..].find('/') {
            return url[scheme_idx + 3 + path_start..].to_string();
        }
        return "/".to_string();
    }
    // already looks like a path
    url.to_string()
}

fn get_all_provider_configs_internal() -> Vec<ProviderConfig> {
    let base_configs = vec![
        ("chutes", "Chutes", "https://api.chutes.ai"),
        ("openai", "OpenAI", "https://api.openai.com"),
        ("anthropic", "Anthropic", "https://api.anthropic.com"),
        ("openrouter", "OpenRouter", "https://openrouter.ai/api"),
        ("mistral", "Mistral AI", "https://api.mistral.ai"),
        ("deepseek", "DeepSeek", "https://api.deepseek.com"),
        ("nanogpt", "NanoGPT", "https://nano-gpt.com/api"),
        ("xai", "xAI (Grok)", "https://api.x.ai"),
        (
            "gemini",
            "Google (Gemini)",
            "https://generativelanguage.googleapis.com/v1",
        ),
        ("zai", "zAI (GLM)", "https://api.z.ai/api/coding/paas/v4"),
        (
            "moonshot",
            "Moonshot AI (Kimi)",
            "https://api.moonshot.ai/v1",
        ),
        (
            "featherless",
            "Featherless AI",
            "https://api.featherless.ai/v1",
        ),
        (
            "qwen",
            "Qwen",
            "https://dashscope.aliyuncs.com/compatible-mode/v1",
        ),
        ("nvidia", "NVIDIA NIM", "https://integrate.api.nvidia.com/v1"),
        ("anannas", "Anannas AI", "https://api.anannas.ai/v1"),
        ("groq", "Groq", "https://api.groq.com"),
        ("ollama", "Ollama (Local)", ""),
        ("lmstudio", "LM Studio (Local)", ""),
        ("custom", "Custom (OpenAI-format)", ""),
        ("custom-anthropic", "Custom (Anthropic-format)", ""),
    ];

    base_configs
        .into_iter()
        .map(|(id, name, base)| {
            let cred = ProviderCredential {
                id: "temp".to_string(), // dummy
                provider_id: id.to_string(),
                label: name.to_string(),
                api_key: None,
                base_url: Some(base.to_string()),
                default_model: None,
                headers: None,
                config: None,
            };
            let adapter = adapter_for(&cred);
            let endpoint_full = adapter.endpoint(base);
            let api_endpoint_path = path_from_url(&endpoint_full);
            let required_auth_headers: Vec<String> = adapter
                .required_auth_headers()
                .iter()
                .map(|s| s.to_string())
                .collect();
            let default_headers = adapter.default_headers_template();
            ProviderConfig {
                id: id.to_string(),
                name: name.to_string(),
                default_base_url: base.to_string(),
                api_endpoint_path,
                system_role: adapter.system_role().to_string(),
                supports_stream: adapter.supports_stream(),
                required_auth_headers,
                default_headers,
            }
        })
        .collect()
}

fn get_cached_provider_configs() -> &'static Vec<ProviderConfig> {
    static CACHE: OnceLock<Vec<ProviderConfig>> = OnceLock::new();
    CACHE.get_or_init(|| get_all_provider_configs_internal())
}

pub fn get_provider_config(provider_id: &ProviderId) -> Option<ProviderConfig> {
    get_cached_provider_configs()
        .iter()
        .cloned()
        .find(|p| p.id == provider_id.0)
}

pub fn resolve_base_url(provider_id: &ProviderId, custom_base_url: Option<&str>) -> String {
    if let Some(custom) = custom_base_url {
        if !custom.is_empty() {
            return custom.trim_end_matches('/').to_string();
        }
    }

    get_provider_config(provider_id)
        .map(|cfg| cfg.default_base_url)
        .unwrap_or_else(|| "https://api.openai.com".to_string())
}

#[allow(dead_code)]
pub fn build_endpoint_url(provider_id: &ProviderId, custom_base_url: Option<&str>) -> String {
    let base_url = resolve_base_url(provider_id, custom_base_url);
    let trimmed = base_url.trim_end_matches('/');

    // If base_url already contains /v1, don't add it again
    if trimmed.ends_with("/v1") {
        format!("{}/chat/completions", trimmed)
    } else {
        format!("{}/v1/chat/completions", trimmed)
    }
}

#[allow(dead_code)]
pub fn get_system_role(provider_id: &ProviderId) -> Cow<'static, str> {
    let cred = ProviderCredential {
        id: "temp".to_string(),
        provider_id: provider_id.0.clone(),
        label: "".to_string(),
        api_key: None,
        base_url: None,
        default_model: None,
        headers: None,
        config: None,
    };
    adapter_for(&cred).system_role()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_base_url_with_custom() {
        let result = resolve_base_url(&ProviderId("openai".into()), Some("https://custom.com"));
        assert_eq!(result, "https://custom.com");
    }

    #[test]
    fn test_resolve_base_url_default() {
        let result = resolve_base_url(&ProviderId("openai".into()), None);
        assert_eq!(result, "https://api.openai.com");
    }

    #[test]
    fn test_build_endpoint_url() {
        let result = build_endpoint_url(&ProviderId("openai".into()), None);
        assert_eq!(result, "https://api.openai.com/v1/chat/completions");
    }

    #[test]
    fn test_build_endpoint_url_with_v1_already_in_base() {
        let result =
            build_endpoint_url(&ProviderId("openai".into()), Some("https://custom.com/v1"));
        assert_eq!(result, "https://custom.com/v1/chat/completions");
    }
}
