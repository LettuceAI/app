//! Durable memories, extraction, retrieval, and consolidation.

#![deny(unsafe_op_in_unsafe_fn)]

mod model;
mod port;
mod repair;
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
pub use repair::{
    MEMORY_CATEGORIES, MEMORY_REPAIR_TOOL_NAME, MEMORY_REPAIR_TOOL_TEXT_KEYS,
    guess_memory_category, memory_repair_tool_request, memory_repairs_fallback_prompt_key,
};
pub use run::{
    DynamicMemoryAttempt, DynamicMemoryAttemptFailureCode, DynamicMemoryAttemptRecovery,
    DynamicMemoryAttemptStatus, DynamicMemoryInferenceRound, DynamicMemoryRoundFinishReason,
    DynamicMemoryRoundKind, DynamicMemoryRun, DynamicMemoryRunAttemptAdmission,
    DynamicMemoryRunError, DynamicMemorySourceMessage, DynamicMemoryStructuredFallbackFormat,
    DynamicMemorySummaryWindow, DynamicMemoryToolCallEvidence,
    MAX_DYNAMIC_MEMORY_ATTEMPT_TOOL_CALLS, MAX_DYNAMIC_MEMORY_INFERENCE_ROUNDS,
    MAX_DYNAMIC_MEMORY_SOURCE_MESSAGES, NewDynamicMemoryAttemptRecovery,
    NewDynamicMemoryInferenceRound, NewDynamicMemoryRunAttempt, NewDynamicMemoryToolCall,
};
pub use structured_fallback::{
    StructuredFallbackError, memory_operations_fallback_prompt_key,
    parse_memory_operations_from_text, parse_memory_repairs_from_text,
};
pub use text::{
    MemoryTextProblem, collapse_whitespace, normalize_llm_output_text, normalize_memory_text,
};
pub use tool::{
    CategoryArgument, CreateMemoryPreparation, DYNAMIC_MEMORY_TOOL_TEXT_KEYS, DuplicateKind,
    DynamicMemoryToolOptions, ListedMemory, MemoryBatchResult, MemoryCycleBudget,
    MemoryCycleFinish, MemoryReference, MemoryToolArguments, MemoryToolCall, MemoryToolError,
    MemoryToolOutcome, MemoryToolReducer, MemoryToolRejection, MemoryToolResult,
    MemoryToolSkipReason, SemanticDuplicateEvidence, SoftDeleteReason,
    dynamic_memory_tool_request_for_run, dynamic_memory_tool_shape, list_memories,
};
