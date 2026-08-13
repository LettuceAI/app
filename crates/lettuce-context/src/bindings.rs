//! Typed lorebook bindings. Character, persona, and group relationships are
//! separate ports so a database adapter cannot accidentally turn ownership
//! into an unvalidated `(owner_kind, owner_id)` row.

use lettuce_types::{CharacterId, GroupId, LorebookId, PersonaId, Revision, TimestampMillis};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LorebookBinding {
    pub lorebook_id: LorebookId,
    pub enabled: bool,
    pub ordinal: u32,
    pub revision: Revision,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

/// A create command contains only authored relationship data. The adapter
/// allocates the binding revision/ordinal/timestamps and follows the latest
/// lorebook revision; conversation snapshots may pin a resolved revision
/// later. Archived books remain referenced here but are excluded by activation
/// resolution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LorebookBindingCreate {
    pub lorebook_id: LorebookId,
    pub target: BindingInsertionTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub enum BindingInsertionTarget {
    Append,
    At(usize),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BindingValidationError {
    #[error("binding list contains duplicate lorebook ids")]
    DuplicateLorebook,
    #[error("binding list ordinals must be contiguous and zero-based")]
    InvalidOrdering,
    #[error("binding target index is outside the list")]
    InvalidTarget,
    #[error("binding revision must be at least one")]
    ZeroRevision,
    #[error("binding created_at must not be later than updated_at")]
    InvalidTimestampOrder,
}

pub fn validate_bindings(bindings: &[LorebookBinding]) -> Result<(), BindingValidationError> {
    let mut ids = std::collections::HashSet::with_capacity(bindings.len());
    for (ordinal, binding) in bindings.iter().enumerate() {
        if binding.revision.get() == 0 {
            return Err(BindingValidationError::ZeroRevision);
        }
        if binding.created_at > binding.updated_at {
            return Err(BindingValidationError::InvalidTimestampOrder);
        }
        if !ids.insert(binding.lorebook_id) {
            return Err(BindingValidationError::DuplicateLorebook);
        }
        if binding.ordinal as usize != ordinal {
            return Err(BindingValidationError::InvalidOrdering);
        }
    }
    Ok(())
}

/// The atomically updated binding list and the revision of the owner whose
/// revision was used as the CAS token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingMutationResult {
    pub bindings: Vec<LorebookBinding>,
    pub owner_revision: Revision,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BindingRepositoryError {
    #[error("binding validation failed: {0}")]
    Invalid(#[from] BindingValidationError),
    #[error("binding repository conflict")]
    Conflict,
    #[error("binding target was not found")]
    NotFound,
    #[error("binding repository failure: {0}")]
    Failure(String),
}

pub trait CharacterLorebookBindingRepository: Send + Sync {
    fn list_character_bindings(
        &self,
        character_id: CharacterId,
    ) -> Result<Vec<LorebookBinding>, BindingRepositoryError>;
    fn bind_character_lorebook(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        binding: LorebookBindingCreate,
        now: TimestampMillis,
    ) -> Result<BindingMutationResult, BindingRepositoryError>;
    fn unbind_character_lorebook(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        lorebook_id: LorebookId,
        now: TimestampMillis,
    ) -> Result<BindingMutationResult, BindingRepositoryError>;
    fn set_character_lorebook_enabled(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        lorebook_id: LorebookId,
        enabled: bool,
        now: TimestampMillis,
    ) -> Result<BindingMutationResult, BindingRepositoryError>;
    fn reorder_character_lorebook(
        &self,
        character_id: CharacterId,
        expected_character_revision: Revision,
        lorebook_id: LorebookId,
        target_ordinal: usize,
        now: TimestampMillis,
    ) -> Result<BindingMutationResult, BindingRepositoryError>;
}

pub trait PersonaLorebookBindingRepository: Send + Sync {
    fn list_persona_bindings(
        &self,
        persona_id: PersonaId,
    ) -> Result<Vec<LorebookBinding>, BindingRepositoryError>;
    fn bind_persona_lorebook(
        &self,
        persona_id: PersonaId,
        expected_persona_revision: Revision,
        binding: LorebookBindingCreate,
        now: TimestampMillis,
    ) -> Result<BindingMutationResult, BindingRepositoryError>;
    fn unbind_persona_lorebook(
        &self,
        persona_id: PersonaId,
        expected_persona_revision: Revision,
        lorebook_id: LorebookId,
        now: TimestampMillis,
    ) -> Result<BindingMutationResult, BindingRepositoryError>;
    fn set_persona_lorebook_enabled(
        &self,
        persona_id: PersonaId,
        expected_persona_revision: Revision,
        lorebook_id: LorebookId,
        enabled: bool,
        now: TimestampMillis,
    ) -> Result<BindingMutationResult, BindingRepositoryError>;
    fn reorder_persona_lorebook(
        &self,
        persona_id: PersonaId,
        expected_persona_revision: Revision,
        lorebook_id: LorebookId,
        target_ordinal: usize,
        now: TimestampMillis,
    ) -> Result<BindingMutationResult, BindingRepositoryError>;
}

pub trait GroupLorebookBindingRepository: Send + Sync {
    fn list_group_bindings(
        &self,
        group_id: GroupId,
    ) -> Result<Vec<LorebookBinding>, BindingRepositoryError>;
    fn bind_group_lorebook(
        &self,
        group_id: GroupId,
        expected_group_revision: Revision,
        binding: LorebookBindingCreate,
        now: TimestampMillis,
    ) -> Result<BindingMutationResult, BindingRepositoryError>;
    fn unbind_group_lorebook(
        &self,
        group_id: GroupId,
        expected_group_revision: Revision,
        lorebook_id: LorebookId,
        now: TimestampMillis,
    ) -> Result<BindingMutationResult, BindingRepositoryError>;
    fn set_group_lorebook_enabled(
        &self,
        group_id: GroupId,
        expected_group_revision: Revision,
        lorebook_id: LorebookId,
        enabled: bool,
        now: TimestampMillis,
    ) -> Result<BindingMutationResult, BindingRepositoryError>;
    fn reorder_group_lorebook(
        &self,
        group_id: GroupId,
        expected_group_revision: Revision,
        lorebook_id: LorebookId,
        target_ordinal: usize,
        now: TimestampMillis,
    ) -> Result<BindingMutationResult, BindingRepositoryError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding(ordinal: u32) -> LorebookBinding {
        LorebookBinding {
            lorebook_id: LorebookId::new(),
            enabled: true,
            ordinal,
            revision: Revision::INITIAL,
            created_at: TimestampMillis::UNIX_EPOCH,
            updated_at: TimestampMillis::UNIX_EPOCH,
        }
    }

    #[test]
    fn binding_lists_are_explicitly_ordered_and_unique() {
        assert!(validate_bindings(&[binding(0), binding(1)]).is_ok());
        assert_eq!(
            validate_bindings(&[binding(1)]),
            Err(BindingValidationError::InvalidOrdering)
        );
    }

    #[test]
    fn duplicate_book_is_rejected_even_when_ordinals_differ() {
        let mut first = binding(0);
        let mut second = binding(1);
        second.lorebook_id = first.lorebook_id;
        assert_eq!(
            validate_bindings(&[first.clone(), second]),
            Err(BindingValidationError::DuplicateLorebook)
        );
        first.ordinal = 1;
        assert!(validate_bindings(&[first]).is_err());
    }

    #[test]
    fn binding_rejects_zero_revision() {
        let mut value = binding(0);
        value.revision = Revision::new(0);
        assert_eq!(
            validate_bindings(&[value]),
            Err(BindingValidationError::ZeroRevision)
        );
    }

    #[test]
    fn binding_rejects_reversed_timestamps_and_create_has_no_adapter_fields() {
        let mut value = binding(0);
        value.created_at = TimestampMillis::new(2);
        value.updated_at = TimestampMillis::new(1);
        assert_eq!(
            validate_bindings(&[value]),
            Err(BindingValidationError::InvalidTimestampOrder)
        );

        let create = LorebookBindingCreate {
            lorebook_id: LorebookId::new(),
            target: BindingInsertionTarget::Append,
        };
        let mut encoded = serde_json::to_value(&create).expect("binding create value");
        encoded["ordinal"] = serde_json::json!(0);
        assert!(serde_json::from_value::<LorebookBindingCreate>(encoded).is_err());
    }
}
