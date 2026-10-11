use lettuce_characters::{Persona, PersonaDefaultState};
use lettuce_types::{ConversationBranchId, ConversationId, OperationId, TimestampMillis};

use crate::{CanonicalChange, SyncChangeId, SyncDeviceId};

#[derive(Debug, Clone, PartialEq)]
pub enum SyncConflictValue {
    Persona(Persona),
    Default(PersonaDefaultState),
    Character(Box<lettuce_characters::CharacterDetails>),
    Lorebook(Box<lettuce_context::LorebookDetails>),
    Prompt(Box<lettuce_context::PromptDocument>),
    Deleted,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SyncConflictCandidate {
    pub change_id: Option<SyncChangeId>,
    pub device_id: Option<SyncDeviceId>,
    pub timestamp: Option<TimestampMillis>,
    pub value: SyncConflictValue,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SyncConflict {
    pub id: OperationId,
    pub entity_kind: String,
    pub entity_id: String,
    pub detected_at: TimestampMillis,
    pub current: SyncConflictCandidate,
    pub other: SyncConflictCandidate,
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
    #[error("the selected conflict candidate cannot be restored")]
    Unsupported,
    #[error("a required conflict reference is unavailable")]
    Unavailable,
    #[error("sync conflict storage failed")]
    Storage,
}

pub trait SyncConflictRepository: Send + Sync {
    /// Every supported unresolved catalog conflict, newest first.
    fn unresolved_sync_conflicts(&self) -> Result<Vec<SyncConflict>, ConflictRepositoryError>;

    fn resolve_sync_conflict(
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
    /// Every unresolved conversation fork, newest first.
    fn unresolved_conversation_forks(
        &self,
    ) -> Result<Vec<ConversationFork>, ConflictRepositoryError>;

    fn resolve_conversation_fork(
        &self,
        conversation_id: ConversationId,
        branch_id: ConversationBranchId,
        now: TimestampMillis,
    ) -> Result<(), ConflictRepositoryError>;
}
