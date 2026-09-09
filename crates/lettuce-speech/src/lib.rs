//! Speech recognition, synthesis, voices, corrections, and local runtimes.
//!
//! ASR and TTS remain independent internal modules while sharing artifact,
//! audio, job, and platform contracts at one crate boundary.

#![deny(unsafe_op_in_unsafe_fn)]

mod asr;
mod learning;
mod openai_tts;
mod tts;
mod whisper_runtime;

pub use asr::*;
pub use learning::*;
pub use openai_tts::*;
pub use tts::*;
pub use whisper_runtime::*;
