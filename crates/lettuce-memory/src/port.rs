use lettuce_types::{
    CompanionEffectId, ConversationBranchId, ConversationId, DynamicMemoryRunId,
    GenerationAttemptId, GenerationTurnId, MemoryId, MemorySpaceId, MessageId, OperationId,
    Revision, TimestampMillis,
};
use serde::{Deserialize, Serialize};

use lettuce_conversations::{
    Conversation, ConversationRepositoryError, InferenceUsage, MutationCommit,
    ProviderNeutralContext, TombstoneMessage, TombstoneMessageResult,
};

use crate::{
    DynamicMemoryAttempt, DynamicMemoryAttemptFailureCode, DynamicMemoryAttemptRecovery,
    DynamicMemoryAttemptStatus, DynamicMemoryInferenceRound, DynamicMemoryPendingApproval,
    DynamicMemoryRun, DynamicMemoryRunAttemptAdmission, DynamicMemoryToolCallEvidence, MemoryItem,
    MemorySpaceSnapshot, MemorySummary, MemoryToolResult, MemoryValidationError,
    NewDynamicMemoryAttemptRecovery, NewDynamicMemoryInferenceRound, NewDynamicMemoryRunAttempt,
};

pub trait DynamicMemoryApprovalRepository: Send + Sync {
    fn get_dynamic_memory_pending_approval(
        &self,
        conversation_id: ConversationId,
        branch_id: ConversationBranchId,
    ) -> Result<Option<DynamicMemoryPendingApproval>, MemoryRepositoryError>;

    fn prompt_dynamic_memory_if_due(
        &self,
        conversation_id: ConversationId,
        branch_id: ConversationBranchId,
        unsummarized_message_count: u64,
        message_interval: u32,
        at: TimestampMillis,
    ) -> Result<Option<DynamicMemoryPendingApproval>, MemoryRepositoryError>;

    fn clear_dynamic_memory_pending_approval(
        &self,
        conversation_id: ConversationId,
        branch_id: ConversationBranchId,
    ) -> Result<(), MemoryRepositoryError>;

    fn skip_dynamic_memory_pending_approval(
        &self,
        conversation_id: ConversationId,
        branch_id: ConversationBranchId,
        at: TimestampMillis,
    ) -> Result<Option<DynamicMemoryPendingApproval>, MemoryRepositoryError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryChangeSet {
    pub space_id: MemorySpaceId,
    pub expected_revision: Revision,
    pub items: Vec<MemoryItem>,
}

impl MemoryChangeSet {
    pub fn validate(&self) -> Result<(), MemoryValidationError> {
        MemorySpaceSnapshot {
            id: self.space_id,
            revision: self.expected_revision,
            items: self.items.clone(),
        }
        .validate()
    }
}

pub trait MemoryRepository: Send + Sync {
    fn create(
        &self,
        snapshot: MemorySpaceSnapshot,
    ) -> Result<MemorySpaceSnapshot, MemoryRepositoryError>;

    fn get(&self, id: MemorySpaceId) -> Result<Option<MemorySpaceSnapshot>, MemoryRepositoryError>;

    fn get_for_branch(
        &self,
        conversation_id: ConversationId,
        branch_id: ConversationBranchId,
    ) -> Result<Option<MemorySpaceSnapshot>, MemoryRepositoryError>;

    /// Atomically verifies `expected_revision`, replaces the complete item set,
    /// and increments the memory-space revision exactly once.
    fn compare_and_apply(
        &self,
        change: MemoryChangeSet,
    ) -> Result<MemorySpaceSnapshot, MemoryRepositoryError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryRetrievalAccess {
    pub conversation_id: ConversationId,
    pub turn_id: GenerationTurnId,
    pub attempt_id: GenerationAttemptId,
    pub space_id: MemorySpaceId,
    pub expected_revision: Revision,
    pub selected_memory_ids: Vec<MemoryId>,
    pub accessed_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryRetrievalAccessReceipt {
    pub access: MemoryRetrievalAccess,
    pub resulting_revision: Revision,
    /// Selected memories that were cold before this access promoted them.
    #[serde(default)]
    pub promoted_memory_ids: Vec<MemoryId>,
}

pub trait MemoryRetrievalRepository: Send + Sync {
    fn get_retrieval_access(
        &self,
        conversation_id: ConversationId,
        turn_id: GenerationTurnId,
        attempt_id: GenerationAttemptId,
    ) -> Result<Option<MemoryRetrievalAccessReceipt>, MemoryRepositoryError>;

    /// Records one turn's retrieval bookkeeping as per-memory updates: access
    /// count, last access and cold promotion. The memory-space revision is
    /// neither checked nor advanced, so retrieval never conflicts with a
    /// running memory cycle; a selected memory removed in the meantime is
    /// skipped. `resulting_revision` is the revision the prompt was built from.
    fn apply_retrieval_access(
        &self,
        access: MemoryRetrievalAccess,
    ) -> Result<MemoryRetrievalAccessReceipt, MemoryRepositoryError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemorySummaryChange {
    pub expected_revision: Revision,
    pub summary: MemorySummary,
}

impl MemorySummaryChange {
    pub fn validate(&self) -> Result<(), MemoryValidationError> {
        if self.expected_revision.get() == 0 {
            return Err(MemoryValidationError::InvalidRevision);
        }
        self.summary.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemorySummaryCommit {
    pub memory: MemorySpaceSnapshot,
    pub summary: MemorySummary,
}

pub trait MemorySummaryRepository: Send + Sync {
    fn get_summary(
        &self,
        space_id: MemorySpaceId,
    ) -> Result<Option<MemorySummary>, MemoryRepositoryError>;

    fn get_summary_for_branch(
        &self,
        space_id: MemorySpaceId,
        conversation_id: ConversationId,
        branch_id: ConversationBranchId,
    ) -> Result<Option<MemorySummary>, MemoryRepositoryError> {
        let _ = (conversation_id, branch_id);
        self.get_summary(space_id)
    }

    /// The summary cursor of one branch, including inside shared pools.
    fn summary_cursor(
        &self,
        space_id: MemorySpaceId,
        conversation_id: ConversationId,
        branch_id: ConversationBranchId,
    ) -> Result<u64, MemoryRepositoryError> {
        let _ = conversation_id;
        Ok(self
            .get_summary(space_id)?
            .filter(|summary| summary.branch_id == branch_id)
            .map_or(0, |summary| summary.window_end))
    }

    /// Atomically verifies the memory-space revision, replaces its cumulative
    /// summary and ordered source cursor, and increments the root revision.
    fn compare_and_apply_summary(
        &self,
        change: MemorySummaryChange,
    ) -> Result<MemorySummaryCommit, MemoryRepositoryError>;
}

pub trait MemoryTokenCountRepository: Send + Sync {
    fn count_unknown_item(
        &self,
        space_id: MemorySpaceId,
        memory_id: MemoryId,
        source_text: &str,
        token_count: u32,
    ) -> Result<bool, MemoryRepositoryError>;

    fn count_unknown_summary(
        &self,
        summary: &MemorySummary,
        token_count: u32,
    ) -> Result<bool, MemoryRepositoryError>;
}

pub trait DynamicMemoryRunRepository: Send + Sync {
    fn apply_dynamic_memory_cycle_finish(
        &self,
        _attempt_id: lettuce_types::DynamicMemoryAttemptId,
        _change: MemoryChangeSet,
        _at: TimestampMillis,
    ) -> Result<MemorySpaceSnapshot, MemoryRepositoryError> {
        Err(MemoryRepositoryError::Failure(
            "cycle settlement is unavailable".into(),
        ))
    }

    fn list_dynamic_memory_runs(
        &self,
        _conversation_id: ConversationId,
    ) -> Result<Vec<DynamicMemoryRun>, DynamicMemoryRunRepositoryError> {
        Err(DynamicMemoryRunRepositoryError::Invalid)
    }

    fn admit_dynamic_memory_run_attempt(
        &self,
        admission: NewDynamicMemoryRunAttempt,
    ) -> Result<DynamicMemoryRunAttemptAdmission, DynamicMemoryRunRepositoryError>;

    fn load_dynamic_memory_run(
        &self,
        id: lettuce_types::DynamicMemoryRunId,
    ) -> Result<DynamicMemoryRun, DynamicMemoryRunRepositoryError>;

    fn load_dynamic_memory_attempt(
        &self,
        id: lettuce_types::DynamicMemoryAttemptId,
    ) -> Result<DynamicMemoryAttempt, DynamicMemoryRunRepositoryError>;

    fn load_latest_dynamic_memory_attempt(
        &self,
        run_id: lettuce_types::DynamicMemoryRunId,
    ) -> Result<DynamicMemoryAttempt, DynamicMemoryRunRepositoryError>;

    fn transition_dynamic_memory_attempt(
        &self,
        id: lettuce_types::DynamicMemoryAttemptId,
        expected_revision: Revision,
        next: DynamicMemoryAttemptStatus,
        failure: Option<DynamicMemoryAttemptFailureCode>,
        at: TimestampMillis,
    ) -> Result<DynamicMemoryAttempt, DynamicMemoryRunRepositoryError>;

    fn recover_dynamic_memory_attempt(
        &self,
        recovery: NewDynamicMemoryAttemptRecovery,
    ) -> Result<DynamicMemoryAttemptRecovery, DynamicMemoryRunRepositoryError>;

    fn admit_dynamic_memory_inference_round(
        &self,
        run_id: lettuce_types::DynamicMemoryRunId,
        attempt_id: lettuce_types::DynamicMemoryAttemptId,
        expected_round_ordinal: u8,
        expected_next_call_ordinal: u16,
        round: NewDynamicMemoryInferenceRound,
    ) -> Result<DynamicMemoryInferenceRound, DynamicMemoryRunRepositoryError>;

    fn list_dynamic_memory_inference_rounds(
        &self,
        run_id: lettuce_types::DynamicMemoryRunId,
        attempt_id: lettuce_types::DynamicMemoryAttemptId,
    ) -> Result<Vec<DynamicMemoryInferenceRound>, DynamicMemoryRunRepositoryError>;

    fn list_dynamic_memory_tool_calls(
        &self,
        run_id: lettuce_types::DynamicMemoryRunId,
        attempt_id: lettuce_types::DynamicMemoryAttemptId,
    ) -> Result<Vec<DynamicMemoryToolCallEvidence>, DynamicMemoryRunRepositoryError>;

    fn load_dynamic_memory_round_settlement(
        &self,
        _run_id: lettuce_types::DynamicMemoryRunId,
        _attempt_id: lettuce_types::DynamicMemoryAttemptId,
        _round_ordinal: u8,
    ) -> Result<Option<DynamicMemoryBackgroundRoundSettlement>, DynamicMemoryRunRepositoryError>
    {
        Err(DynamicMemoryRunRepositoryError::Invalid)
    }

    /// Atomically applies one background tool round and records its typed
    /// results. Repeating the exact commit returns the stored settlement.
    fn commit_dynamic_memory_background_round(
        &self,
        _commit: DynamicMemoryBackgroundRoundCommit,
        _at: TimestampMillis,
    ) -> Result<DynamicMemoryBackgroundRoundSettlement, DynamicMemoryRunRepositoryError> {
        Err(DynamicMemoryRunRepositoryError::Invalid)
    }

    fn load_dynamic_memory_summary_checkpoint(
        &self,
        _run_id: lettuce_types::DynamicMemoryRunId,
    ) -> Result<Option<DynamicMemorySummaryCheckpoint>, DynamicMemoryRunRepositoryError> {
        Err(DynamicMemoryRunRepositoryError::Invalid)
    }

    fn commit_dynamic_memory_summary(
        &self,
        _commit: DynamicMemorySummaryCommit,
        _at: TimestampMillis,
    ) -> Result<DynamicMemorySummaryCheckpoint, DynamicMemoryRunRepositoryError> {
        Err(DynamicMemoryRunRepositoryError::Invalid)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DynamicMemorySuffixRewind {
    pub operation_id: OperationId,
    pub conversation_id: ConversationId,
    pub invalid_run_id: Option<DynamicMemoryRunId>,
    pub expected_memory_revision: Revision,
    pub invalidated_effect_ids: Vec<CompanionEffectId>,
    pub at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DynamicMemorySuffixRewindReceipt {
    pub operation_id: OperationId,
    pub conversation_id: ConversationId,
    pub invalid_run_id: Option<DynamicMemoryRunId>,
    pub memory: MemorySpaceSnapshot,
    pub summary: Option<MemorySummary>,
    pub invalidated_effect_ids: Vec<CompanionEffectId>,
    pub applied_at: TimestampMillis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DynamicMemorySuffixRewindError {
    #[error("dynamic-memory suffix rewind target was not found")]
    NotFound,
    #[error("dynamic-memory suffix rewind conflicts with durable state")]
    Conflict,
    #[error("dynamic-memory suffix rewind request is invalid")]
    Invalid,
    #[error("dynamic-memory suffix rewind storage failed")]
    Storage,
}

pub trait DynamicMemorySuffixRewindRepository: Send + Sync {
    fn get_dynamic_memory_suffix_rewind(
        &self,
        _operation_id: OperationId,
    ) -> Result<Option<DynamicMemorySuffixRewindReceipt>, DynamicMemorySuffixRewindError> {
        Err(DynamicMemorySuffixRewindError::Invalid)
    }

    fn rewind_dynamic_memory_suffix(
        &self,
        rewind: DynamicMemorySuffixRewind,
    ) -> Result<DynamicMemorySuffixRewindReceipt, DynamicMemorySuffixRewindError>;
}

/// A delete-after whose memory rewind is owed: the suffix tombstone and the
/// summary interval the rewind rebuilds with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingSuffixRewind {
    pub after_message_id: MessageId,
    pub tombstone: TombstoneMessage,
    pub summary_message_interval: u32,
}

/// Why an owed rewind could not finish.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwedRewindFailure {
    /// Memory changed while it was rewound.
    Conflict,
    /// The stored rewind disagrees with what the chat holds now.
    Inconsistent,
    Storage,
    Other,
}

impl OwedRewindFailure {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Conflict => "conflict",
            Self::Inconsistent => "inconsistent",
            Self::Storage => "storage",
            Self::Other => "other",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Self {
        match value {
            "conflict" => Self::Conflict,
            "inconsistent" => Self::Inconsistent,
            "storage" => Self::Storage,
            _ => Self::Other,
        }
    }
}

pub trait PendingSuffixRewindRepository: Send + Sync {
    /// Tombstones the suffix and records the owed rewind in one
    /// transaction. A replay records nothing.
    fn tombstone_suffix(
        &self,
        pending: &PendingSuffixRewind,
        now: TimestampMillis,
    ) -> Result<TombstoneMessageResult, ConversationRepositoryError>;

    /// Records the delete-after of an anchor with nothing after it as a
    /// durable operation that changes nothing, so a replay after later
    /// messages remove none of them.
    fn record_empty_suffix(
        &self,
        pending: &PendingSuffixRewind,
        now: TimestampMillis,
    ) -> Result<MutationCommit<Conversation>, ConversationRepositoryError>;

    fn pending_suffix_rewinds(
        &self,
        conversation_id: Option<ConversationId>,
    ) -> Result<Vec<PendingSuffixRewind>, DynamicMemorySuffixRewindError>;

    fn clear_pending_suffix_rewind(
        &self,
        pending: &PendingSuffixRewind,
    ) -> Result<(), DynamicMemorySuffixRewindError>;

    /// Why the conversation's oldest failed owed rewind could not finish.
    fn pending_rewind_failure(
        &self,
        conversation_id: ConversationId,
    ) -> Result<Option<OwedRewindFailure>, DynamicMemorySuffixRewindError>;

    /// Records why an owed rewind could not finish; the next attempt
    /// overwrites it and a finished rewind removes the record with it.
    fn fail_pending_suffix_rewind(
        &self,
        pending: &PendingSuffixRewind,
        failure: OwedRewindFailure,
    ) -> Result<(), DynamicMemorySuffixRewindError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicMemorySummaryCommit {
    pub run_id: lettuce_types::DynamicMemoryRunId,
    pub attempt_id: lettuce_types::DynamicMemoryAttemptId,
    pub expected_memory_revision: Revision,
    pub text: String,
    pub token_count: u32,
    pub request_context: ProviderNeutralContext,
    pub usage: Option<InferenceUsage>,
    pub provider_request_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DynamicMemorySummaryCheckpoint {
    pub run_id: lettuce_types::DynamicMemoryRunId,
    pub attempt_id: lettuce_types::DynamicMemoryAttemptId,
    pub summary: MemorySummary,
    pub expected_memory_revision: Revision,
    pub resulting_memory_revision: Revision,
    pub request_context: ProviderNeutralContext,
    pub usage: Option<InferenceUsage>,
    pub provider_request_id: Option<String>,
    pub settled_at: TimestampMillis,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DynamicMemoryBackgroundRoundCommit {
    pub run_id: lettuce_types::DynamicMemoryRunId,
    pub attempt_id: lettuce_types::DynamicMemoryAttemptId,
    pub round_ordinal: u8,
    pub space_id: MemorySpaceId,
    pub expected_memory_revision: Revision,
    pub change: Option<MemoryChangeSet>,
    pub results: Vec<MemoryToolResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DynamicMemoryBackgroundRoundSettlement {
    pub run_id: lettuce_types::DynamicMemoryRunId,
    pub attempt_id: lettuce_types::DynamicMemoryAttemptId,
    pub round_ordinal: u8,
    pub space_id: MemorySpaceId,
    pub expected_memory_revision: Revision,
    pub resulting_memory_revision: Revision,
    pub results: Vec<MemoryToolResult>,
    pub settled_at: TimestampMillis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DynamicMemoryRunRepositoryError {
    #[error("dynamic-memory run record was not found")]
    NotFound,
    #[error("dynamic-memory run operation conflicts with durable state")]
    Conflict,
    #[error("dynamic-memory run record is invalid")]
    Invalid,
    #[error("dynamic-memory run storage failed")]
    Storage,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MemoryRepositoryError {
    #[error("memory change is invalid: {0}")]
    Invalid(#[from] MemoryValidationError),
    #[error("memory space was not found")]
    NotFound,
    #[error("memory space already exists")]
    AlreadyExists,
    #[error("memory space revision conflict")]
    Conflict,
    #[error("memory repository failure: {0}")]
    Failure(String),
}
