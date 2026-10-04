//! Image generation: requests and their jobs, the playground, the local
//! stable-diffusion.cpp engine, LoRAs, Hugging Face image bundles, CivitAI
//! and the avatar and design reference helpers.

mod engine;
mod generate;
mod helpers;
mod library;
mod state;

use std::sync::Arc;

use lettuce_contracts::{self as dto, ApiError, ApiErrorCode, ApiErrorDetails};
use lettuce_image_generation::sd_runtime::server::LocalDiffusionEngine;
use lettuce_image_generation::{ImageError, ImageFailureKind};

use super::ApiContext;
use super::error::api_error;

pub(crate) use engine::fit_estimate;
pub use engine::{
    sd_bundle_runnability, sd_catalog, sd_component_library, sd_compute_policy_get,
    sd_compute_policy_save, sd_detect_model_file, sd_disk_usage, sd_model_install, sd_model_repair,
    sd_model_uninstall, sd_models_installed, sd_runnability, sd_runtime_delete, sd_runtime_install,
    sd_runtime_inventory, sd_runtime_releases, sd_runtime_switch, sd_upscalers_install,
    sd_upscalers_list, sd_upscalers_remove,
};
pub(crate) use generate::{generation_view, record_upscale};
pub use generate::{
    image_capabilities, image_generate, image_upscale, playground_history_delete,
    playground_history_list,
};
pub use helpers::{avatar_gradient, avatar_prompt, image_design_reference};
pub use library::{
    civitai_auth_clear, civitai_auth_save, civitai_auth_status, civitai_lora_download,
    civitai_model, civitai_search, hf_image_bundle_files, hf_image_bundle_install,
    hf_image_bundle_profiles, hf_image_bundle_retry, hf_image_bundle_retry_registration,
    hf_image_bundle_search, image_models_downloaded, lora_keywords_discover, loras_delete,
    loras_import, loras_list, loras_update_keywords,
};
pub(crate) use library::{lora_discovery, to_engine_lora};
pub(crate) use state::ImageApiState;

pub(crate) const fn failure_kind(kind: ImageFailureKind) -> dto::ImageFailureKind {
    match kind {
        ImageFailureKind::Cancelled => dto::ImageFailureKind::Cancelled,
        ImageFailureKind::LocalUnsupported => dto::ImageFailureKind::LocalUnsupported,
        ImageFailureKind::OutdatedRegistration => dto::ImageFailureKind::OutdatedRegistration,
        ImageFailureKind::ModelFileMissing => dto::ImageFailureKind::ModelFileMissing,
        ImageFailureKind::ModelNotConfigured => dto::ImageFailureKind::ModelNotConfigured,
        ImageFailureKind::RuntimeNotInstalled => dto::ImageFailureKind::RuntimeNotInstalled,
        ImageFailureKind::RuntimeIncompatible => dto::ImageFailureKind::RuntimeIncompatible,
        ImageFailureKind::UpscalerMissing => dto::ImageFailureKind::UpscalerMissing,
        ImageFailureKind::InvalidRequest => dto::ImageFailureKind::InvalidRequest,
        ImageFailureKind::ServerStartFailed => dto::ImageFailureKind::ServerStartFailed,
        ImageFailureKind::ServerNotReady => dto::ImageFailureKind::ServerNotReady,
        ImageFailureKind::EngineRejected => dto::ImageFailureKind::EngineRejected,
        ImageFailureKind::EngineFailed => dto::ImageFailureKind::EngineFailed,
        ImageFailureKind::EngineTimedOut => dto::ImageFailureKind::EngineTimedOut,
        ImageFailureKind::OutOfMemory => dto::ImageFailureKind::OutOfMemory,
        ImageFailureKind::LoraConflict => dto::ImageFailureKind::LoraConflict,
        ImageFailureKind::LoraInUse => dto::ImageFailureKind::LoraInUse,
        ImageFailureKind::LoraInvalid => dto::ImageFailureKind::LoraInvalid,
        ImageFailureKind::StorageFailed => dto::ImageFailureKind::StorageFailed,
        ImageFailureKind::ProviderFailed => dto::ImageFailureKind::ProviderFailed,
        ImageFailureKind::NoImageReturned => dto::ImageFailureKind::NoImageReturned,
        ImageFailureKind::OutputRejected => dto::ImageFailureKind::OutputRejected,
        ImageFailureKind::ModelMissing => dto::ImageFailureKind::ModelMissing,
        ImageFailureKind::Interrupted => dto::ImageFailureKind::Interrupted,
        ImageFailureKind::Other => dto::ImageFailureKind::Other,
    }
}

/// An image operation's failure as the API reports it: the category in the
/// details, the engine's words as the message.
pub(crate) fn image_error(error: ImageError) -> ApiError {
    let code = match error.kind {
        ImageFailureKind::Cancelled => ApiErrorCode::Cancelled,
        ImageFailureKind::LocalUnsupported => ApiErrorCode::Unsupported,
        ImageFailureKind::InvalidRequest | ImageFailureKind::LoraInvalid => {
            ApiErrorCode::InvalidInput
        }
        ImageFailureKind::LoraConflict | ImageFailureKind::LoraInUse => ApiErrorCode::Conflict,
        ImageFailureKind::ModelMissing => ApiErrorCode::NotFound,
        ImageFailureKind::StorageFailed
        | ImageFailureKind::Interrupted
        | ImageFailureKind::Other => ApiErrorCode::Internal,
        ImageFailureKind::OutdatedRegistration
        | ImageFailureKind::ModelFileMissing
        | ImageFailureKind::ModelNotConfigured
        | ImageFailureKind::RuntimeNotInstalled
        | ImageFailureKind::RuntimeIncompatible
        | ImageFailureKind::UpscalerMissing
        | ImageFailureKind::ServerStartFailed
        | ImageFailureKind::ServerNotReady
        | ImageFailureKind::EngineRejected
        | ImageFailureKind::EngineFailed
        | ImageFailureKind::EngineTimedOut
        | ImageFailureKind::OutOfMemory
        | ImageFailureKind::ProviderFailed
        | ImageFailureKind::NoImageReturned
        | ImageFailureKind::OutputRejected => ApiErrorCode::Unavailable,
    };
    ApiError {
        code,
        message: error.message,
        details: Some(ApiErrorDetails::Image {
            failure: failure_kind(error.kind),
        }),
    }
}

pub(crate) fn unsupported() -> ApiError {
    image_error(ImageError::new(
        ImageFailureKind::LocalUnsupported,
        "Local stable-diffusion.cpp image generation is desktop-only.",
    ))
}

/// The embedded stable-diffusion.cpp engine; `Unsupported` where the host
/// has none (mobile).
pub(crate) fn engine(context: &ApiContext) -> Result<Arc<LocalDiffusionEngine>, ApiError> {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        context
            .backend()
            .local_diffusion()
            .cloned()
            .ok_or_else(unsupported)
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let _ = context;
        Err(unsupported())
    }
}

pub(crate) fn internal(error: impl std::fmt::Display) -> ApiError {
    api_error(ApiErrorCode::Internal, error.to_string())
}
