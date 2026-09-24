//! Local llama.cpp runtime adapter.
//!
//! `offload` holds the frozen GPU offload, KV cache and context planning
//! formulas; the llama.cpp bindings (desktop only) load models, read GGUF
//! metadata and measure compute buffers for them.

#![deny(unsafe_op_in_unsafe_fn)]

pub mod context;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub mod context_info;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub mod dflash;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub mod engine;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub mod generation;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub mod hardware;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub mod llama;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub mod mtp;
pub mod offload;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub mod prompt;
pub mod request;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub mod sampler;
pub mod sampler_profile;
pub mod tool_calls;
