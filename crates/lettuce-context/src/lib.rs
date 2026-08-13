//! Prompt programs, lorebooks, matching, binding, and context assembly.
//!
//! Prompt and lorebook behavior remain separate internal modules with one
//! public context boundary. Provider execution and persistence stay outside.

#![deny(unsafe_op_in_unsafe_fn)]
