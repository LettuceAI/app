//! Speech: Whisper models, transcription, dictation from the microphone,
//! the ASR learning library, audio providers and voices, synthesis and
//! message playback, and Kokoro.

mod dictation;
mod errors;
mod host;
mod providers;
mod state;
mod synthesize;
mod transcribe;
mod whisper;

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

pub use providers::{audio_provider_verify, audio_provider_voices, audio_provider_voices_refresh, tts_models, tts_voice_design_models, tts_cache_stats, tts_cache_clear};

pub use synthesize::tts_synthesize;
