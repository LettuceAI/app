//! Relationship state, growth policy, and scheduled effects.

#![deny(unsafe_op_in_unsafe_fn)]

mod effect;
mod prompt;
mod signals;
mod soul;
mod state;

pub use effect::*;
pub use prompt::*;
pub use signals::*;
pub use soul::*;
pub use state::*;
