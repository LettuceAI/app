//! Canonical domain changes and replication policy.

#![deny(unsafe_op_in_unsafe_fn)]

mod change;
mod journal;

pub use change::*;
pub use journal::*;
