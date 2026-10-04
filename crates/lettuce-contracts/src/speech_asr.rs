use serde::{Deserialize, Serialize};

/// A Whisper model the catalog offers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct WhisperCatalogModel {
    pub id: String,
    pub filename: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub size_bytes: u64,
    pub english_only: bool,
    pub quantized: bool,
    pub recommended: bool,
    pub recommended_for_mobile: bool,
    pub recommended_for_desktop: bool,
    pub installed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct WhisperCatalog {
    pub models: Vec<WhisperCatalogModel>,
}

/// An installed Whisper model. `dictation` marks the one dictation uses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct WhisperInstalledModel {
    pub id: String,
    pub filename: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub size_bytes: u64,
    pub english_only: bool,
    pub quantized: bool,
    pub dictation: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct WhisperInstalledModels {
    pub models: Vec<WhisperInstalledModel>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct WhisperModelRequest {
    pub model_id: String,
}

/// The model dictation uses; `None` returns to the first installed one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct DictationModelSetRequest {
    pub model_id: Option<String>,
}

/// How a Whisper model runs: on the GPU or the CPU, with flash attention
/// and on which device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct WhisperRunOptions {
    pub use_gpu: bool,
    pub force_cpu: bool,
    pub flash_attention: bool,
    pub gpu_device: i32,
}

impl Default for WhisperRunOptions {
    fn default() -> Self {
        Self {
            use_gpu: true,
            force_cpu: false,
            flash_attention: true,
            gpu_device: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct WhisperPreloadRequest {
    pub model_id: Option<String>,
    #[serde(default)]
    pub run: WhisperRunOptions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct WhisperCacheCleared {
    pub cleared: u32,
}

/// What a transcription asks of Whisper beyond the audio. A chat dictation
/// uses the defaults: the conversation and global vocabularies, GPU and a
/// model that stays loaded.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default, deny_unknown_fields)]
pub struct TranscribeOptions {
    pub language: Option<String>,
    pub scopes: Vec<String>,
    pub initial_prompt: Option<String>,
    pub translate: bool,
    pub detect_language: bool,
    pub threads: Option<u32>,
    pub run: WhisperRunOptions,
    pub keep_model_loaded: bool,
}

impl Default for TranscribeOptions {
    fn default() -> Self {
        Self {
            language: None,
            scopes: vec!["conversation".to_owned(), "global".to_owned()],
            initial_prompt: None,
            translate: false,
            detect_language: false,
            threads: None,
            run: WhisperRunOptions::default(),
            keep_model_loaded: true,
        }
    }
}

/// Transcribes a picked audio file. `request_id` is the idempotency key:
/// repeating the request returns its job, another request under the same id
/// is `Conflict`. `model_id` defaults to the dictation model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct TranscribeFileRequest {
    pub request_id: String,
    pub source: crate::FileSource,
    pub model_id: Option<String>,
    #[serde(default)]
    pub options: TranscribeOptions,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct TranscriptSegment {
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub start_ms: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub end_ms: u64,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct AppliedCorrectionView {
    pub correction_id: String,
    pub wrong: String,
    pub correct: String,
    pub matched_text: String,
}

/// What a finished transcription job produced: the raw text, the text after
/// the user's corrections, and the audio it came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct TranscriptionView {
    pub request_id: String,
    pub audio: crate::AssetRef,
    pub model_id: String,
    pub raw_text: String,
    pub text: String,
    pub detected_language: Option<String>,
    pub segments: Vec<TranscriptSegment>,
    pub applied_corrections: Vec<AppliedCorrectionView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct DictationStartRequest {
    pub conversation_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct DictationStarted {
    pub capture_id: String,
}

/// Ends a capture and transcribes it. `model_id` defaults to the dictation
/// model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct DictationStopRequest {
    pub capture_id: String,
    pub model_id: Option<String>,
    #[serde(default)]
    pub options: TranscribeOptions,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct DictationCancelRequest {
    pub capture_id: String,
}
