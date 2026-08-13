//! Typed, domain-independent lifecycle contracts for long-running work.
//!
//! `lettuce-jobs` owns identity, state, leases, progress and event history. It
//! deliberately does not know about Tauri, SQLite, providers, filesystems or
//! feature-specific payloads. The in-memory store is a deterministic reference
//! implementation for tests; the database crate can implement [`JobStore`]
//! without exposing its persistence model here.

#![deny(unsafe_op_in_unsafe_fn)]

mod model;
mod store;

pub mod events;
pub mod handle;
pub mod recovery;
pub mod registry;
pub mod retention;
pub mod scheduler;

pub use model::*;
pub use store::*;
