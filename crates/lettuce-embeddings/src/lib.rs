//! Embedding and auxiliary analysis runtimes.
//!
#![deny(unsafe_op_in_unsafe_fn)]

mod calibration;
mod emotion;
mod onnx;
mod ort_runtime;
mod projection;

pub use calibration::*;
pub use emotion::*;
pub use onnx::*;
pub use ort_runtime::*;
pub use projection::*;
