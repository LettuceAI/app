use lettuce_types::{MemorySpaceId, Revision};
use serde::{Deserialize, Serialize};

use lettuce_conversations::{ConversationRepositoryError, ToolExecution, ToolExecutionTransition};

use crate::{MemoryItem, MemorySpaceSnapshot, MemoryValidationError};

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

    /// Atomically verifies `expected_revision`, replaces the complete item set,
    /// and increments the memory-space revision exactly once.
    fn compare_and_apply(
        &self,
        change: MemoryChangeSet,
    ) -> Result<MemorySpaceSnapshot, MemoryRepositoryError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicMemoryRoundCommit {
    pub space_id: MemorySpaceId,
    pub change: Option<MemoryChangeSet>,
    pub execution_transitions: Vec<ToolExecutionTransition>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicMemoryRoundCommitResult {
    pub snapshot: MemorySpaceSnapshot,
    pub executions: Vec<ToolExecution>,
}

pub trait DynamicMemoryRoundRepository: MemoryRepository {
    /// Atomically commits the optional authoritative memory change and every
    /// terminal execution transition for one admitted handler round.
    fn commit_dynamic_memory_round(
        &self,
        commit: DynamicMemoryRoundCommit,
        at: lettuce_types::TimestampMillis,
    ) -> Result<DynamicMemoryRoundCommitResult, DynamicMemoryRoundCommitError>;
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DynamicMemoryRoundCommitError {
    #[error("dynamic-memory repository failed: {0}")]
    Memory(#[from] MemoryRepositoryError),
    #[error("tool execution repository failed: {0}")]
    Execution(#[from] ConversationRepositoryError),
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
