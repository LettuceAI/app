//! Speech: Whisper models, transcription, dictation from the microphone,
//! the ASR learning library, audio providers and voices, synthesis and
//! message playback, and Kokoro.

mod dictation;
mod errors;
mod learning;
mod message;
mod host;
mod kokoro;
mod providers;
mod state;
mod synthesize;
mod transcribe;
pub(crate) mod whisper;

#[cfg(test)]
mod tests;

pub use dictation::{dictation_cancel, dictation_start, dictation_stop};
pub use host::{InstalledSpeech, NoSpeech, SpeechHost};
pub(crate) use dictation::sweep_scratch;
pub(crate) use state::SpeechApiState;
pub(crate) use transcribe::transcription_view;
pub use transcribe::transcribe_file;
pub use whisper::{
    whisper_catalog, whisper_clear_cache, whisper_delete, whisper_dictation_model_set,
    whisper_download, whisper_models_list, whisper_preload,
};

pub use providers::{voice_design_preview, audio_provider_voices_search, audio_providers_list, audio_provider_update, audio_provider_delete, user_voices_list, user_voice_update, user_voice_delete, audio_provider_verify, audio_provider_voices, audio_provider_voices_refresh, tts_models, tts_voice_design_models, tts_cache_stats, tts_cache_clear};

pub use synthesize::tts_synthesize;

pub use kokoro::{kokoro_inventory, kokoro_variants, kokoro_voices_installed, kokoro_voices_available, kokoro_install_model, kokoro_install_voices, kokoro_uninstall_model, kokoro_uninstall_voice, kokoro_blend, kokoro_phonemize, kokoro_tokenize_preview};

pub use learning::{asr_voice_example_suggest, asr_learning_export, asr_vocabulary_list, asr_corrections_list, asr_ignored_suggestions_list, asr_voice_examples_list, asr_vocabulary_delete, asr_correction_delete, asr_voice_example_delete, asr_suggestions};

pub use message::message_speak;
