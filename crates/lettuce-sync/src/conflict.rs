use lettuce_characters::{Persona, PersonaDefaultState};
use lettuce_types::{ConversationBranchId, ConversationId, OperationId, TimestampMillis};

use crate::{CanonicalChange, SyncChangeId, SyncDeviceId};

pub const MAX_UNRESOLVED_CONFLICTS: usize = 100;

#[derive(Debug, Clone, PartialEq)]
pub enum PersonaConflictValue {
    Persona(Persona),
    Default(PersonaDefaultState),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PersonaConflictCandidate {
    pub change_id: Option<SyncChangeId>,
    pub device_id: Option<SyncDeviceId>,
    pub timestamp: Option<TimestampMillis>,
    pub value: PersonaConflictValue,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PersonaConflict {
    pub id: OperationId,
    pub entity_kind: String,
    pub entity_id: String,
    pub detected_at: TimestampMillis,
    pub current: PersonaConflictCandidate,
    pub other: PersonaConflictCandidate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictChoice {
    Current,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConflictRepositoryError {
    #[error("sync conflict was not found")]
    NotFound,
    #[error("sync conflict input is stale or conflicts with an earlier decision")]
    Conflict,
    #[error("sync conflict evidence is corrupt")]
    Corrupt,
    #[error("sync conflict storage failed")]
    Storage,
}

pub trait PersonaConflictRepository: Send + Sync {
    fn unresolved_persona_conflicts(
        &self,
        limit: usize,
    ) -> Result<Vec<PersonaConflict>, ConflictRepositoryError>;

    fn resolve_persona_conflict(
        &self,
        conflict_id: OperationId,
        expected_current_change: Option<SyncChangeId>,
        choice: ConflictChoice,
        resolution_id: OperationId,
        now: TimestampMillis,
    ) -> Result<CanonicalChange, ConflictRepositoryError>;
}

/// A branch sync created because two devices answered the same message: the
/// lower message id kept the conversation path and the other chain was
/// copied into `branch_id`. `holds_local` tells whether that chain was this
/// device's path. Nothing is dropped, so the user's choice is only which
/// branch to show (keep both, make the fork main, keep the path); resolving
/// clears the notice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationFork {
    pub conversation_id: ConversationId,
    pub branch_id: ConversationBranchId,
    pub holds_local: bool,
    pub detected_at: TimestampMillis,
}

pub trait ConversationForkRepository: Send + Sync {
    fn unresolved_conversation_forks(
        &self,
        limit: usize,
    ) -> Result<Vec<ConversationFork>, ConflictRepositoryError>;

    fn resolve_conversation_fork(
        &self,
        conversation_id: ConversationId,
        branch_id: ConversationBranchId,
        now: TimestampMillis,
    ) -> Result<(), ConflictRepositoryError>;
}
