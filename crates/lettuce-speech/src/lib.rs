//! Speech recognition, synthesis, voices, corrections, and local runtimes.
//!
//! ASR and TTS remain independent internal modules while sharing artifact,
//! audio, job, and platform contracts at one crate boundary.

#![deny(unsafe_op_in_unsafe_fn)]

mod asr;
mod elevenlabs_tts;
mod fish_speech_tts;
mod fish_tts;
mod gemini_tts;
mod learning;
mod kokoro_phonemizer;
mod openai_tts;
mod remote_tts;
mod tts;
mod tts_catalog;
mod tts_discovery;
mod tts_verification;
mod voice_design;
mod whisper_runtime;

pub use asr::*;
pub use elevenlabs_tts::*;
pub use fish_speech_tts::*;
pub use fish_tts::*;
pub use gemini_tts::*;
pub use learning::*;
pub use kokoro_phonemizer::*;
pub use openai_tts::*;
pub use remote_tts::*;
pub use tts::*;
pub use tts_catalog::*;
pub use tts_discovery::*;
pub use tts_verification::*;
pub use voice_design::*;
pub use whisper_runtime::*;
