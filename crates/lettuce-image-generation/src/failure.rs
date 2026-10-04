//! Why an image operation failed, as a category the UI can act on. The
//! engine's human text stays next to it as the diagnostic.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageFailureKind {
    Cancelled,
    LocalUnsupported,
    OutdatedRegistration,
    ModelFileMissing,
    ModelNotConfigured,
    RuntimeNotInstalled,
    RuntimeIncompatible,
    UpscalerMissing,
    InvalidRequest,
    ServerStartFailed,
    ServerNotReady,
    EngineRejected,
    EngineFailed,
    EngineTimedOut,
    OutOfMemory,
    LoraConflict,
    LoraInUse,
    LoraInvalid,
    StorageFailed,
    ProviderFailed,
    NoImageReturned,
    OutputRejected,
    ModelMissing,
    Interrupted,
    Other,
}

impl ImageFailureKind {
    pub const ALL: [Self; 25] = [
        ImageFailureKind::Cancelled,
        ImageFailureKind::LocalUnsupported,
        ImageFailureKind::OutdatedRegistration,
        ImageFailureKind::ModelFileMissing,
        ImageFailureKind::ModelNotConfigured,
        ImageFailureKind::RuntimeNotInstalled,
        ImageFailureKind::RuntimeIncompatible,
        ImageFailureKind::UpscalerMissing,
        ImageFailureKind::InvalidRequest,
        ImageFailureKind::ServerStartFailed,
        ImageFailureKind::ServerNotReady,
        ImageFailureKind::EngineRejected,
        ImageFailureKind::EngineFailed,
        ImageFailureKind::EngineTimedOut,
        ImageFailureKind::OutOfMemory,
        ImageFailureKind::LoraConflict,
        ImageFailureKind::LoraInUse,
        ImageFailureKind::LoraInvalid,
        ImageFailureKind::StorageFailed,
        ImageFailureKind::ProviderFailed,
        ImageFailureKind::NoImageReturned,
        ImageFailureKind::OutputRejected,
        ImageFailureKind::ModelMissing,
        ImageFailureKind::Interrupted,
        ImageFailureKind::Other,
    ];

    /// The label a failed job carries.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Cancelled => "image-cancelled",
            Self::LocalUnsupported => "image-local-unsupported",
            Self::OutdatedRegistration => "image-outdated-registration",
            Self::ModelFileMissing => "image-model-file-missing",
            Self::ModelNotConfigured => "image-model-not-configured",
            Self::RuntimeNotInstalled => "image-runtime-not-installed",
            Self::RuntimeIncompatible => "image-runtime-incompatible",
            Self::UpscalerMissing => "image-upscaler-missing",
            Self::InvalidRequest => "image-invalid-request",
            Self::ServerStartFailed => "image-server-start-failed",
            Self::ServerNotReady => "image-server-not-ready",
            Self::EngineRejected => "image-engine-rejected",
            Self::EngineFailed => "image-engine-failed",
            Self::EngineTimedOut => "image-engine-timed-out",
            Self::OutOfMemory => "image-out-of-memory",
            Self::LoraConflict => "image-lora-conflict",
            Self::LoraInUse => "image-lora-in-use",
            Self::LoraInvalid => "image-lora-invalid",
            Self::StorageFailed => "image-storage-failed",
            Self::ProviderFailed => "image-provider-failed",
            Self::NoImageReturned => "image-no-image-returned",
            Self::OutputRejected => "image-output-rejected",
            Self::ModelMissing => "image-model-missing",
            Self::Interrupted => "image-interrupted",
            Self::Other => "image-failed",
        }
    }

    #[must_use]
    pub fn from_label(label: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.label() == label)
    }
}

/// A failed image operation: its category and the engine's own words.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct ImageError {
    pub kind: ImageFailureKind,
    pub message: String,
}

impl ImageError {
    #[must_use]
    pub fn new(kind: ImageFailureKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    #[must_use]
    pub fn cancelled() -> Self {
        Self::new(
            ImageFailureKind::Cancelled,
            crate::sd_runtime::server::GENERATION_CANCELLED_MESSAGE,
        )
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.kind == ImageFailureKind::Cancelled
    }

    #[must_use]
    pub fn storage(message: impl Into<String>) -> Self {
        Self::new(ImageFailureKind::StorageFailed, message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_round_trips_through_its_job_label() {
        for kind in ImageFailureKind::ALL {
            assert_eq!(ImageFailureKind::from_label(kind.label()), Some(kind));
        }
        assert_eq!(ImageFailureKind::from_label("unrelated"), None);
    }
}
