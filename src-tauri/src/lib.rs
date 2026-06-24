#![allow(
    clippy::enum_variant_names,
    clippy::large_enum_variant,
    clippy::ptr_arg,
    clippy::too_many_arguments,
    clippy::type_complexity
)]

mod api;
mod app;
mod asr_manager;
pub mod chat_appearance;
pub mod chat_manager;
pub mod content_filter;
pub mod creation_helper;
pub mod discovery;
pub mod embedding;
mod engine;
mod gemini_cache;
pub mod group_chat_manager;
mod hf_browser;
mod host_api;
mod image_generator;
pub mod infra;
mod llama_cpp;
pub mod local_diffusion;
pub mod migrations;
pub mod models;
pub mod ollama;
mod platform;
pub mod pricing_cache;
pub mod providers;
pub mod storage_manager;
pub mod sync;
pub mod tls;
pub mod tokens;
mod transport;
pub mod tts_manager;
mod usage;

pub(crate) use infra::{
    abort_manager, dynamic_memory_approval, dynamic_memory_run_manager, error, logger,
    post_turn_memory_scheduler, serde_utils, utils,
};
pub(crate) use platform::android_monitor;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    app::run();
}
