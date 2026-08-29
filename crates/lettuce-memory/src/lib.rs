//! Durable memories, extraction, retrieval, and consolidation.

#![deny(unsafe_op_in_unsafe_fn)]

mod model;
mod port;
mod tool;

pub use model::{
    MemoryCategory, MemoryItem, MemoryPolicy, MemorySpaceSnapshot, MemoryValidationError, Score,
};
pub use port::{
    DynamicMemoryPreparationPlan, DynamicMemoryPreparationPlanError,
    DynamicMemoryPreparationRepository, DynamicMemoryRecoveredChild, DynamicMemoryRoundCommit,
    DynamicMemoryRoundCommitError, DynamicMemoryRoundCommitResult, DynamicMemoryRoundRepository,
    MemoryChangeSet, MemoryRepository, MemoryRepositoryError, PersistedMemoryCreatePreparation,
};
pub use tool::{
    CreateMemoryPreparation, MemoryBatchResult, MemoryToolArguments, MemoryToolCall,
    MemoryToolError, MemoryToolOutcome, MemoryToolReducer, MemoryToolRejection, MemoryToolResult,
    SemanticDuplicateEvidence, SoftDeleteReason, dynamic_memory_tool_request,
};
