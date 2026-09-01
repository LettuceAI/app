//! Durable memories, extraction, retrieval, and consolidation.

#![deny(unsafe_op_in_unsafe_fn)]

mod model;
mod port;
mod run;
mod tool;

pub use model::{
    MemoryCategory, MemoryItem, MemoryPolicy, MemorySpaceSnapshot, MemoryValidationError, Score,
};
pub use port::{
    DynamicMemoryBackgroundRoundCommit, DynamicMemoryBackgroundRoundSettlement,
    DynamicMemoryPreparationPlan, DynamicMemoryPreparationPlanError,
    DynamicMemoryPreparationRepository, DynamicMemoryRecoveredChild, DynamicMemoryRoundCommit,
    DynamicMemoryRoundCommitError, DynamicMemoryRoundCommitResult, DynamicMemoryRoundRepository,
    DynamicMemoryRunRepository, DynamicMemoryRunRepositoryError, MemoryChangeSet, MemoryRepository,
    MemoryRepositoryError, PersistedMemoryCreatePreparation,
};
pub use run::{
    DynamicMemoryAttempt, DynamicMemoryAttemptFailureCode, DynamicMemoryAttemptRecovery,
    DynamicMemoryAttemptStatus, DynamicMemoryInferenceRound, DynamicMemoryRoundFinishReason,
    DynamicMemoryRun, DynamicMemoryRunAttemptAdmission, DynamicMemoryRunError,
    DynamicMemorySourceMessage, DynamicMemoryToolCallEvidence, MAX_DYNAMIC_MEMORY_INFERENCE_ROUNDS,
    MAX_DYNAMIC_MEMORY_SOURCE_MESSAGES, NewDynamicMemoryAttemptRecovery,
    NewDynamicMemoryInferenceRound, NewDynamicMemoryRunAttempt, NewDynamicMemoryToolCall,
};
pub use tool::{
    CreateMemoryPreparation, MemoryBatchResult, MemoryToolArguments, MemoryToolCall,
    MemoryToolError, MemoryToolOutcome, MemoryToolReducer, MemoryToolRejection, MemoryToolResult,
    SemanticDuplicateEvidence, SoftDeleteReason, dynamic_memory_tool_request,
};
