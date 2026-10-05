use serde::{Deserialize, Serialize};

/// The local speech model a job or call needs installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SpeechModelKind {
    Whisper,
    Kokoro,
}

/// A runtime a local speech engine needs on the device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SpeechRuntimeKind {
    Espeak,
    OnnxRuntime,
}

/// Why a speech call or job failed, where the user can act on it. Every
/// variant but `RetriesExhausted` is terminal at once: retrying cannot fix it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum SpeechFailure {
    ModelRequired { model: SpeechModelKind },
    VoiceCreationOutcomeUnknown,
    VoiceCreationProviderRejected { status: u16 },
    SecretMissing,
    SecretStoreUnavailable,
    VoiceMissing,
    RuntimeMissing { runtime: SpeechRuntimeKind },
    RetriesExhausted { cause: SpeechTransientFailure },
    MicrophonePermissionDenied,
    NoMicrophone,
    NoAudioCaptured,
}

/// The final failure before the bounded speech retry budget was exhausted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum SpeechTransientFailure {
    Unavailable,
    NetworkUnavailable,
    ProviderUnavailable { status: u16 },
}
