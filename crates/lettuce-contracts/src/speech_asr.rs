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

/// Transcribes a picked audio file or a managed asset URL (such as a saved
/// dictation returned after admission failure). `request_id` is the idempotency key:
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
    pub audio: Option<crate::AssetRef>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct AsrVocabularyView {
    pub id: String,
    pub term: String,
    pub language: Option<String>,
    pub category: Option<String>,
    pub scope: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub priority: i64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub use_count: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub created_at: i64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct AsrCorrectionView {
    pub id: String,
    pub wrong: String,
    pub correct: String,
    pub language: Option<String>,
    pub scope: String,
    pub confidence: f64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub use_count: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub accepted_count: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub rejected_count: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub seen_count: u64,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub last_seen_at: Option<i64>,
    pub user_approved: bool,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub created_at: i64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct AsrSuggestionView {
    pub wrong: String,
    pub correct: String,
    pub language: Option<String>,
    pub scope: String,
    pub confidence: f64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub accepted_count: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub rejected_count: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub seen_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct AsrIgnoredSuggestionView {
    pub id: String,
    pub wrong: String,
    pub correct: String,
    pub language: Option<String>,
    pub scope: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub ignored_count: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub last_ignored_at: i64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub created_at: i64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct AsrVoiceExampleView {
    pub id: String,
    pub audio: crate::AssetRef,
    pub expected_text: String,
    pub whisper_output: Option<String>,
    pub language: Option<String>,
    pub scope: String,
    pub vocabulary_term_id: Option<String>,
    pub correction_id: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub created_at: i64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default, deny_unknown_fields)]
pub struct AsrLearningFilter {
    pub language: Option<String>,
    pub scopes: Vec<String>,
    pub user_approved_only: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct AsrLearningItemRequest {
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct AsrSuggestionsRequest {
    pub before: String,
    pub after: String,
    pub language: Option<String>,
    pub scope: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct AsrLearningExportRequest {
    pub target: crate::FileTarget,
    #[serde(default)]
    pub filter: AsrLearningFilter,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct AsrVocabularySaveRequest {
    pub client_operation_id: String,
    pub id: Option<String>,
    pub term: String,
    pub language: Option<String>,
    pub category: Option<String>,
    pub scope: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub priority: Option<i64>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub use_count: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct AsrCorrectionSaveRequest {
    pub client_operation_id: String,
    pub id: Option<String>,
    pub wrong: String,
    pub correct: String,
    pub language: Option<String>,
    pub scope: Option<String>,
    pub confidence: Option<f64>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub use_count: Option<u64>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub accepted_count: Option<u64>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub rejected_count: Option<u64>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub seen_count: Option<u64>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub last_seen_at: Option<i64>,
    pub user_approved: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct AsrSuggestionWriteRequest {
    pub client_operation_id: String,
    pub suggestion: AsrSuggestionView,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct AsrVoiceExampleSaveRequest {
    pub client_operation_id: String,
    pub id: Option<String>,
    pub audio_asset_id: String,
    pub expected_text: String,
    pub whisper_output: Option<String>,
    pub language: Option<String>,
    pub scope: Option<String>,
    pub vocabulary_term_id: Option<String>,
    pub correction_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct AsrLearningImportRequest {
    pub client_operation_id: String,
    pub source: crate::FileSource,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct AsrLearningImportView {
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub vocabulary_count: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub correction_count: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub ignored_suggestion_count: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub voice_example_count: u64,
}
