//! One-line Tauri wrappers over `lettuce_app::api`, one module per domain.

pub mod app;
pub mod character_notes;
pub mod group_notes;
pub mod characters;
pub mod library;
pub mod companion;
pub mod conversations;
pub mod developer;
pub mod files;
pub mod hugging_face;
pub mod image;
pub mod jobs;
pub mod local_models;
pub mod memory;
pub mod memory_models;
pub mod ollama;
pub mod speech;

pub mod lorebooks;

pub mod prompts;

pub mod providers;

pub mod model_profiles;

pub mod settings;

pub mod logs;

pub mod metrics;

pub(crate) mod usage;

pub(crate) mod storage;

pub mod sync;

pub mod creator;
