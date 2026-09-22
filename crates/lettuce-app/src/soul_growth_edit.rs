//! The user's direct Soul growth edits (legacy `companion_clear_soul_growth`,
//! `companion_remove_soul_growth` and `companion_set_soul_growth_lock`). Soul
//! growth is the companion's, shared by all of its conversations, so the
//! edits name the character.

use lettuce_companions::{SoulOwner, SoulRepository, SoulRepositoryError, SoulUserEdit};
use lettuce_types::{CharacterId, OperationRecordId, TimestampMillis};

/// Applies `edit` to the current Soul. A growth or consolidation run that
/// lands between the read and the write makes the write conflict; the edit is
/// then prepared again on the newer Soul, as legacy's last save simply won.
fn edit<R: SoulRepository + ?Sized>(
    repository: &R,
    character_id: CharacterId,
    edit: SoulUserEdit,
    now: TimestampMillis,
) -> Result<Option<lettuce_companions::SoulState>, SoulRepositoryError> {
    let owner = SoulOwner::Character(character_id);
    for _ in 0..3 {
        let Some(state) = repository.get(owner)? else {
            return Ok(None);
        };
        let Some(change_set) = lettuce_companions::prepare_user_edit(&state, edit.clone(), now)
            .map_err(SoulRepositoryError::Invalid)?
        else {
            return Ok(Some(state));
        };
        match repository.apply(owner, OperationRecordId::new(), change_set) {
            Ok(_) => return Ok(Some(state)),
            Err(SoulRepositoryError::Conflict) => continue,
            Err(error) => return Err(error),
        }
    }
    Err(SoulRepositoryError::Conflict)
}

/// Removes every Soul growth entry, authored ones included, as legacy did;
/// answers how many there were.
pub fn clear_companion_soul_growth<R: SoulRepository + ?Sized>(
    repository: &R,
    character_id: CharacterId,
    now: TimestampMillis,
) -> Result<u32, SoulRepositoryError> {
    Ok(
        edit(repository, character_id, SoulUserEdit::ClearAll, now)?.map_or(0, |state| {
            u32::try_from(state.facts.len()).unwrap_or(u32::MAX)
        }),
    )
}

/// Removes one Soul growth entry; false when there is none with that id.
/// Legacy removed by list position; entries now have stable ids.
pub fn remove_companion_soul_growth<R: SoulRepository + ?Sized>(
    repository: &R,
    character_id: CharacterId,
    fact_id: &str,
    now: TimestampMillis,
) -> Result<bool, SoulRepositoryError> {
    Ok(edit(
        repository,
        character_id,
        SoulUserEdit::Remove {
            fact_id: fact_id.to_owned(),
        },
        now,
    )?
    .is_some_and(|state| state.facts.iter().any(|fact| fact.id == fact_id)))
}

/// Locks or unlocks one Soul growth entry; true when it exists, whether or not
/// the lock changed.
pub fn set_companion_soul_growth_lock<R: SoulRepository + ?Sized>(
    repository: &R,
    character_id: CharacterId,
    fact_id: &str,
    locked: bool,
    now: TimestampMillis,
) -> Result<bool, SoulRepositoryError> {
    Ok(edit(
        repository,
        character_id,
        SoulUserEdit::SetLocked {
            fact_id: fact_id.to_owned(),
            locked,
        },
        now,
    )?
    .is_some_and(|state| state.facts.iter().any(|fact| fact.id == fact_id)))
}
