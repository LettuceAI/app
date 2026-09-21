//! Local llama.cpp runtime adapter.
//!
//! `offload` holds the frozen GPU offload, KV cache and context planning
//! formulas; the llama.cpp bindings (desktop only) load models, read GGUF
//! metadata and measure compute buffers for them.

#![deny(unsafe_op_in_unsafe_fn)]

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub mod engine;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub mod llama;
pub mod offload;
