//! Relationship state, growth policy, and scheduled effects.

#![deny(unsafe_op_in_unsafe_fn)]

mod signals;
mod soul;
mod state;

pub use signals::*;
pub use soul::*;
pub use state::*;
