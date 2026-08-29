use lettuce_types::{MemorySpaceId, Revision};
use serde::{Deserialize, Serialize};

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
    fn get(&self, id: MemorySpaceId) -> Result<Option<MemorySpaceSnapshot>, MemoryRepositoryError>;

    /// Atomically verifies `expected_revision`, replaces the complete item set,
    /// and increments the memory-space revision exactly once.
    fn compare_and_apply(
        &self,
        change: MemoryChangeSet,
    ) -> Result<MemorySpaceSnapshot, MemoryRepositoryError>;
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MemoryRepositoryError {
    #[error("memory change is invalid: {0}")]
    Invalid(#[from] MemoryValidationError),
    #[error("memory space was not found")]
    NotFound,
    #[error("memory space revision conflict")]
    Conflict,
    #[error("memory repository failure: {0}")]
    Failure(String),
}
