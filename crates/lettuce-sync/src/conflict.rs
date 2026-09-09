use lettuce_characters::{Persona, PersonaDefaultState};
use lettuce_types::{OperationId, TimestampMillis};

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
