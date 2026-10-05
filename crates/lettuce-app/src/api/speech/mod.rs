//! Speech: Whisper models, transcription, dictation from the microphone,
//! the ASR learning library, audio providers and voices, synthesis and
//! message playback, and Kokoro.

mod dictation;
mod errors;
mod host;
mod kokoro;
mod learning;
mod message;
mod operations;
mod providers;
mod state;
mod synthesize;
mod transcribe;
pub(crate) mod whisper;

#[cfg(test)]
mod tests;

pub(crate) use dictation::sweep_scratch;
pub use dictation::{dictation_cancel, dictation_start, dictation_stop};
pub use host::{InstalledSpeech, NoSpeech, SpeechHost};
pub(crate) use state::SpeechApiState;
pub use transcribe::transcribe_file;
pub(crate) use transcribe::transcription_view;
pub use whisper::{
    whisper_catalog, whisper_clear_cache, whisper_delete, whisper_dictation_model_set,
    whisper_download, whisper_models_list, whisper_preload,
};

pub use providers::{
    audio_provider_api_key_rotate, audio_provider_create, audio_provider_credential_status,
    audio_provider_delete, audio_provider_update, audio_provider_verify, audio_provider_voices,
    audio_provider_voices_refresh, audio_provider_voices_search, audio_providers_list,
    tts_cache_clear, tts_cache_stats, tts_models, tts_voice_design_models, user_voice_create,
    user_voice_delete, user_voice_update, user_voices_list, voice_design_preview,
};

pub use synthesize::tts_synthesize;

pub use kokoro::{
    kokoro_blend, kokoro_install_model, kokoro_install_voices, kokoro_inventory, kokoro_phonemize,
    kokoro_tokenize_preview, kokoro_uninstall_model, kokoro_uninstall_voice, kokoro_variants,
    kokoro_voices_available, kokoro_voices_installed,
};

pub use learning::{
    asr_correction_delete, asr_correction_save, asr_corrections_list, asr_ignored_suggestions_list,
    asr_learning_export, asr_learning_import, asr_suggestion_approve, asr_suggestion_ignore,
    asr_suggestions, asr_vocabulary_delete, asr_vocabulary_list, asr_vocabulary_save,
    asr_voice_example_delete, asr_voice_example_save, asr_voice_example_suggest,
    asr_voice_examples_list,
};

pub use message::message_speak;

pub(crate) use operations::{digest as operation_digest, validate_key as validate_operation_key};
