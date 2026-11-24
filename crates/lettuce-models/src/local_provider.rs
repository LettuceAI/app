//! Provider kinds and default account labels shared by local model runtimes.

pub const LOCAL_LLAMA_PROVIDER_KIND: &str = "llamacpp";
pub const LOCAL_LLAMA_PROVIDER_LABEL: &str = "llama.cpp (Local)";
pub const LOCAL_DIFFUSION_PROVIDER_KIND: &str = "sdcpp";
pub const LOCAL_DIFFUSION_PROVIDER_LABEL: &str = "stable-diffusion.cpp (Local)";
/// The default label used before local provider labels were unified.
pub const LEGACY_LOCAL_DIFFUSION_PROVIDER_LABEL: &str = "stable-diffusion.cpp";

/// Text providers that run against a local endpoint and may override the
/// app-wide message context window per model. `custom` is included because its
/// OpenAI-format endpoint is often a local server.
pub fn is_local_text_provider(kind: &str) -> bool {
    [
        LOCAL_LLAMA_PROVIDER_KIND,
        "ollama",
        "lmstudio",
        "intenserp",
        "custom",
    ]
    .iter()
    .any(|local| kind.eq_ignore_ascii_case(local))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_local_text_providers_case_insensitively() {
        for kind in ["llamacpp", "ollama", "lmstudio", "intenserp", "custom", "LlamaCpp", "OLLAMA"] {
            assert!(is_local_text_provider(kind), "{kind} should be local");
        }
        for kind in ["openrouter", "anthropic", "openai", "groq", "sdcpp", ""] {
            assert!(!is_local_text_provider(kind), "{kind} should not be local");
        }
    }
}
