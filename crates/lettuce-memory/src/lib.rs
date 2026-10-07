//! Durable memories, extraction, retrieval, and consolidation.

#![deny(unsafe_op_in_unsafe_fn)]

mod manual;
mod model;
mod port;
mod read;
mod repair;
mod revert;
mod run;
mod structured_fallback;
mod text;
mod tool;

pub use manual::{
    MemoryContextRevision, MemoryFieldChange, MemoryManualEdit, MemoryManualEditRecord,
    MemoryManualHistory, MemoryManualMutation, MemoryManualReduction, reduce_manual_memory,
    undo_manual_memory_edit,
};
pub use model::{
    DynamicMemoryPendingApproval, DynamicMemoryRunMode, MAX_MEMORY_ITEMS, MAX_MEMORY_SUMMARY_BYTES,
    MAX_MEMORY_TEXT_BYTES, MemoryCategory, MemoryItem, MemoryOrigin, MemoryPolicy, MemoryShortId,
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
    MemoryTokenCountRepository, OwedRewindFailure, PendingSuffixRewind,
    PendingSuffixRewindRepository,
};
pub use repair::{
    MEMORY_CATEGORIES, MEMORY_REPAIR_TOOL_NAME, MEMORY_REPAIR_TOOL_TEXT_KEYS,
    guess_memory_category, memory_repair_tool_request, memory_repairs_fallback_prompt_key,
};
pub use revert::{MemoryCycleRevert, MemoryCycleRevertError, MemoryCycleRevertRecord};
pub use run::{
    DynamicMemoryAttempt, DynamicMemoryAttemptFailureCode, DynamicMemoryAttemptRecovery,
    DynamicMemoryAttemptStatus, DynamicMemoryInferenceRound, DynamicMemoryRoundFinishReason,
    DynamicMemoryRoundKind, DynamicMemoryRun, DynamicMemoryRunAttemptAdmission,
    DynamicMemoryRunError, DynamicMemorySourceMessage, DynamicMemoryStructuredFallbackFormat,
    DynamicMemorySummaryWindow, DynamicMemoryToolCallEvidence,
    MAX_DYNAMIC_MEMORY_ATTEMPT_TOOL_CALLS, MAX_DYNAMIC_MEMORY_INFERENCE_ROUNDS,
    NewDynamicMemoryAttemptRecovery, NewDynamicMemoryInferenceRound, NewDynamicMemoryRunAttempt,
    NewDynamicMemoryToolCall,
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
    MemoryCycleFinish, MemoryCycleStart, MemoryReference, MemoryToolArguments, MemoryToolCall,
    MemoryToolError, MemoryToolOutcome, MemoryToolReducer, MemoryToolRejection, MemoryToolResult,
    MemoryToolSkipReason, SemanticDuplicateEvidence, SoftDeleteReason,
    dynamic_memory_tool_request_for_run, dynamic_memory_tool_shape, list_memories,
    skip_user_edited_calls, undo_memory_tool_outcomes,
};

pub use read::{MemoryActivityCycle, MemoryReadRepository, MemoryReadScope, cycle_revert_blocker};
