pub(crate) mod anannas;
pub(crate) mod anthropic;
pub(crate) mod anthropic_messages;
pub(crate) mod cerebras;
pub(crate) mod chutes;
pub(crate) mod custom;
pub(crate) mod custom_anthropic;
pub(crate) mod deepseek;
pub(crate) mod featherless;
pub(crate) mod gemini;
pub(crate) mod gemini_cache;
pub(crate) mod gemini_express;
pub(crate) mod gemini_generate;
pub(crate) mod groq;
pub(crate) mod intenserp;
pub(crate) mod literouter;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub(crate) mod llama_cpp;
pub(crate) mod lmstudio;
pub(crate) mod mistral;
pub(crate) mod moonshot;
pub(crate) mod nanogpt;
pub(crate) mod nvidia;
pub(crate) mod ollama;
pub(crate) mod ollama_hub;
pub(crate) mod openai;
pub(crate) mod openai_compatible;
pub(crate) mod openrouter;
pub(crate) mod openrouter_pricing;
pub(crate) mod pollinations;
pub(crate) mod qwen;
pub(crate) mod xai;
pub(crate) mod zai;

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub use llama_cpp::{LocalLlama, LocalRuntimeExclusion};
pub use ollama_hub::{OllamaHubError, OllamaInstalledModel, OllamaPullProgress};
