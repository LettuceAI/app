//! Provider-neutral prompt programs, lorebooks, matching, and typed bindings.
//!
//! This crate is deliberately pure: the renderer and matcher accept complete
//! snapshots, and repository traits describe synchronous lifecycle operations
//! without exposing a database, settings store, or transaction handle.

#![deny(unsafe_op_in_unsafe_fn)]

mod bindings;
mod lorebook;
mod prompt;

pub use bindings::*;
pub use lorebook::*;
pub use prompt::*;
