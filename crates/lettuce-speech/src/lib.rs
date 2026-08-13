//! Speech recognition, synthesis, voices, corrections, and local runtimes.
//!
//! ASR and TTS remain independent internal modules while sharing artifact,
//! audio, job, and platform contracts at one crate boundary.

#![deny(unsafe_op_in_unsafe_fn)]
