//! Durable memories, extraction, retrieval, and consolidation.

#![deny(unsafe_op_in_unsafe_fn)]

mod model;
mod port;
mod run;
mod structured_fallback;
mod text;
mod tool;

pub use model::{
    DynamicMemoryPendingApproval, DynamicMemoryRunMode, MAX_MEMORY_SUMMARY_BYTES,
    MAX_MEMORY_SUMMARY_SOURCE_MESSAGES, MemoryCategory, MemoryItem, MemoryPolicy, MemoryShortId,
    MemorySpaceSnapshot, MemorySummary, MemoryValidationError, Score, memory_revision_id,
};
pub use port::{
    DynamicMemoryApprovalRepository, DynamicMemoryBackgroundRoundCommit,
    DynamicMemoryBackgroundRoundSettlement, DynamicMemoryRunRepository,
    DynamicMemoryRunRepositoryError, DynamicMemorySuffixRewind, DynamicMemorySuffixRewindError,
    DynamicMemorySuffixRewindReceipt, DynamicMemorySuffixRewindRepository,
    DynamicMemorySummaryCheckpoint, DynamicMemorySummaryCommit, MemoryChangeSet, MemoryRepository,
    MemoryRepositoryError, MemoryRetrievalAccess, MemoryRetrievalAccessReceipt,
    MemoryRetrievalRepository, MemorySummaryChange, MemorySummaryCommit, MemorySummaryRepository,
};
pub use run::{
    DynamicMemoryAttempt, DynamicMemoryAttemptFailureCode, DynamicMemoryAttemptRecovery,
    DynamicMemoryAttemptStatus, DynamicMemoryInferenceRound, DynamicMemoryRoundFinishReason,
    DynamicMemoryRun, DynamicMemoryRunAttemptAdmission, DynamicMemoryRunError,
    DynamicMemorySourceMessage, DynamicMemoryStructuredFallbackFormat, DynamicMemorySummaryWindow,
    DynamicMemoryToolCallEvidence, MAX_DYNAMIC_MEMORY_ATTEMPT_TOOL_CALLS,
    MAX_DYNAMIC_MEMORY_INFERENCE_ROUNDS, MAX_DYNAMIC_MEMORY_SOURCE_MESSAGES,
    NewDynamicMemoryAttemptRecovery, NewDynamicMemoryInferenceRound, NewDynamicMemoryRunAttempt,
    NewDynamicMemoryToolCall,
};
pub use structured_fallback::{
    StructuredFallbackError, memory_operations_fallback_prompt_key,
    parse_memory_operations_from_text,
};
pub use text::{
    MemoryTextProblem, collapse_whitespace, normalize_llm_output_text, normalize_memory_text,
};
pub use tool::{
    CreateMemoryPreparation, DYNAMIC_MEMORY_TOOL_TEXT_KEYS, DynamicMemoryToolOptions,
    MemoryBatchResult, MemoryReference, MemoryToolArguments, MemoryToolCall, MemoryToolError,
    MemoryToolOutcome, MemoryToolReducer, MemoryToolRejection, MemoryToolResult,
    MemoryToolSkipReason, SemanticDuplicateEvidence, SoftDeleteReason,
    dynamic_memory_tool_request_for_run, dynamic_memory_tool_shape,
};
