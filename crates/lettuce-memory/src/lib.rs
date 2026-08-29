//! Durable memories, extraction, retrieval, and consolidation.

#![deny(unsafe_op_in_unsafe_fn)]

mod model;
mod port;
mod tool;

pub use model::{
    MemoryCategory, MemoryItem, MemoryPolicy, MemorySpaceSnapshot, MemoryValidationError, Score,
};
pub use port::{MemoryChangeSet, MemoryRepository, MemoryRepositoryError};
pub use tool::{
    CreateMemoryPreparation, MemoryBatchResult, MemoryToolArguments, MemoryToolCall,
    MemoryToolError, MemoryToolOutcome, MemoryToolReducer, MemoryToolRejection, MemoryToolResult,
    SoftDeleteReason, dynamic_memory_tool_request,
};
