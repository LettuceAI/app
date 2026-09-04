//! Relationship state, growth policy, and scheduled effects.

#![deny(unsafe_op_in_unsafe_fn)]

mod consolidation;
mod effect;
mod growth;
mod prompt;
mod scheduled_note;
mod signals;
mod soul;
mod soul_writer;
mod state;

pub use consolidation::*;
pub use effect::*;
pub use growth::*;
pub use prompt::*;
pub use scheduled_note::*;
pub use signals::*;
pub use soul::*;
pub use soul_writer::*;
pub use state::*;
