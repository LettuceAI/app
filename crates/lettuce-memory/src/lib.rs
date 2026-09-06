//! Durable memories, extraction, retrieval, and consolidation.

#![deny(unsafe_op_in_unsafe_fn)]

mod model;
mod port;
mod run;
mod structured_fallback;
mod tool;

pub use model::{
    DynamicMemoryPendingApproval, DynamicMemoryRunMode, MAX_MEMORY_SUMMARY_BYTES,
    MAX_MEMORY_SUMMARY_SOURCE_MESSAGES, MemoryCategory, MemoryItem, MemoryPolicy,
    MemorySpaceSnapshot, MemorySummary, MemoryValidationError, Score, memory_revision_id,
};
pub use port::{
    DynamicMemoryApprovalRepository, DynamicMemoryBackgroundRoundCommit,
    DynamicMemoryBackgroundRoundSettlement, DynamicMemoryPreparationPlan,
    DynamicMemoryPreparationPlanError, DynamicMemoryPreparationRepository,
    DynamicMemoryRecoveredChild, DynamicMemoryRoundCommit, DynamicMemoryRoundCommitError,
    DynamicMemoryRoundCommitResult, DynamicMemoryRoundRepository, DynamicMemoryRunRepository,
    DynamicMemoryRunRepositoryError, DynamicMemorySuffixRewind, DynamicMemorySuffixRewindError,
    DynamicMemorySuffixRewindReceipt, DynamicMemorySuffixRewindRepository,
    DynamicMemorySummaryCheckpoint, DynamicMemorySummaryCommit, MemoryChangeSet, MemoryRepository,
    MemoryRepositoryError, MemorySummaryChange, MemorySummaryCommit, MemorySummaryRepository,
    PersistedMemoryCreatePreparation,
};
pub use run::{
    DynamicMemoryAttempt, DynamicMemoryAttemptFailureCode, DynamicMemoryAttemptRecovery,
    DynamicMemoryAttemptStatus, DynamicMemoryInferenceRound, DynamicMemoryRoundFinishReason,
    DynamicMemoryRun, DynamicMemoryRunAttemptAdmission, DynamicMemoryRunError,
    DynamicMemorySourceMessage, DynamicMemoryStructuredFallbackFormat, DynamicMemorySummaryWindow,
    DynamicMemoryToolCallEvidence, MAX_DYNAMIC_MEMORY_INFERENCE_ROUNDS,
    MAX_DYNAMIC_MEMORY_SOURCE_MESSAGES, NewDynamicMemoryAttemptRecovery,
    NewDynamicMemoryInferenceRound, NewDynamicMemoryRunAttempt, NewDynamicMemoryToolCall,
};
pub use structured_fallback::{
    MEMORY_OPERATIONS_JSON_FALLBACK_PROMPT, MEMORY_OPERATIONS_XML_FALLBACK_PROMPT,
    StructuredFallbackError, memory_operations_fallback_prompt, parse_memory_operations_from_text,
};
pub use tool::{
    CreateMemoryPreparation, MemoryBatchResult, MemoryToolArguments, MemoryToolCall,
    MemoryToolError, MemoryToolOutcome, MemoryToolReducer, MemoryToolRejection, MemoryToolResult,
    SemanticDuplicateEvidence, SoftDeleteReason, dynamic_memory_tool_request,
    dynamic_memory_tool_request_for_run, dynamic_memory_tool_request_with_source_requirement,
};
