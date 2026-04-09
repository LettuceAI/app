mod api;
mod app;
mod chat_appearance;
mod chat_manager;
mod content_filter;
mod creation_helper;
mod discovery;
mod embedding;
mod engine;
mod group_chat_manager;
mod hf_browser;
mod host_api;
mod image_generator;
mod infra;
mod llama_cpp;
pub mod migrations;
pub mod models;
mod ollama;
mod platform;
mod pricing_cache;
mod providers;
pub mod storage_manager;
pub mod sync;
mod transport;
mod tts_manager;
mod usage;

pub(crate) use infra::{
    abort_manager, dynamic_memory_run_manager, error, logger, serde_utils, utils,
};
pub(crate) use platform::android_monitor;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    app::run();
}
