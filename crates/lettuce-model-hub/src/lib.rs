//! Model discovery, verified downloads, compatibility, and installation.
//!
#![deny(unsafe_op_in_unsafe_fn)]

mod companion_emotion;
mod embedding;
mod kokoro;
mod kokoro_install;
mod whisper;

pub use companion_emotion::*;
pub use embedding::*;
pub use kokoro::*;
pub use kokoro_install::*;
pub use whisper::*;
