//! Provider kinds and default account labels shared by local model runtimes.

pub const LOCAL_LLAMA_PROVIDER_KIND: &str = "llamacpp";
pub const LOCAL_LLAMA_PROVIDER_LABEL: &str = "llama.cpp (Local)";
pub const LOCAL_DIFFUSION_PROVIDER_KIND: &str = "sdcpp";
pub const LOCAL_DIFFUSION_PROVIDER_LABEL: &str = "stable-diffusion.cpp (Local)";
/// The default label used before local provider labels were unified.
pub const LEGACY_LOCAL_DIFFUSION_PROVIDER_LABEL: &str = "stable-diffusion.cpp";
