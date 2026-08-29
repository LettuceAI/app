//! Embedding and auxiliary analysis runtimes.
//!
#![deny(unsafe_op_in_unsafe_fn)]

mod onnx;
mod projection;

pub use onnx::*;
pub use projection::*;
