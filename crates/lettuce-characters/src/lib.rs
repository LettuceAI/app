//! Mutable, revision-checked authored character/context profiles.
//!
//! Each authored aggregate remains an independent module and lifecycle.
//! Conversation history, transfer compatibility envelopes, prompt/lorebook
//! internals, and concrete persistence stay outside this crate.

#![deny(unsafe_op_in_unsafe_fn)]

mod character;
mod constants;
mod error;
mod group;
mod persona;
mod ports;
mod presentation;
mod scene;
mod selection;
mod starter;

pub use character::*;
pub use error::*;
pub use group::*;
pub use persona::*;
pub use ports::*;
pub use presentation::*;
pub use scene::*;
pub use selection::*;
pub use starter::*;
