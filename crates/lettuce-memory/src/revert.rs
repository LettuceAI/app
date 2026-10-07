use lettuce_types::{
    ConversationId, DynamicMemoryRunId, MemoryId, MemorySpaceId, Revision, TimestampMillis,
};
use serde::{Deserialize, Serialize};

use crate::MemoryRepositoryError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryCycleRevert {
    pub conversation_id: ConversationId,
    pub run_id: DynamicMemoryRunId,
    pub expected_revision: Revision,
    pub at: TimestampMillis,
}

/// A cycle the user reverted: the run keeps its recorded outcomes, and its
/// effect on the space's items and summary is undone once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryCycleRevertRecord {
    pub run_id: DynamicMemoryRunId,
    pub conversation_id: ConversationId,
    pub space_id: MemorySpaceId,
    pub source_revision: Revision,
    pub resulting_revision: Revision,
    pub restored_summary_run_id: Option<DynamicMemoryRunId>,
    pub reverted_at: TimestampMillis,
}

impl MemoryCycleRevertRecord {
    pub fn validate(&self) -> Result<(), MemoryRepositoryError> {
        if self.source_revision.get() == 0
            || self.source_revision.next().ok() != Some(self.resulting_revision)
            || self.restored_summary_run_id == Some(self.run_id)
        {
            return Err(MemoryRepositoryError::Invalid(
                crate::MemoryValidationError::InvalidRevision,
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum MemoryCycleRevertError {
    #[error("the cycle or the memory space was not found")]
    NotFound,
    #[error("memory changed since it was read")]
    Conflict,
    #[error("the cycle is still running")]
    Running,
    #[error("the cycle was already reverted")]
    AlreadyReverted,
    #[error("the cycle recorded nothing to revert")]
    NothingToRevert,
    #[error("a later cycle started from this cycle's result")]
    Dependent { later_run_id: DynamicMemoryRunId },
    #[error("a memory the cycle changed was edited by the user afterwards")]
    UserEdited { memory_id: MemoryId },
    #[error("the revert is invalid")]
    Invalid,
    #[error("memory storage failed")]
    Storage,
}
