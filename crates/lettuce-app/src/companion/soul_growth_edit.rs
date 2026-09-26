//! The user's direct Soul growth edits (legacy `companion_clear_soul_growth`,
//! `companion_remove_soul_growth` and `companion_set_soul_growth_lock`, which
//! named a session). The edits act on the Soul the conversation grows: its
//! character's shared Soul, or its own while the character does not share
//! Soul growth ([`SoulOwner::for_conversation`]).

use lettuce_companions::{
    SoulApplyReceipt, SoulOwner, SoulRepository, SoulRepositoryError, SoulUserEdit,
};
use lettuce_types::{OperationRecordId, TimestampMillis};

/// Applies `edit` to the current Soul. A growth or consolidation run that
/// lands between the read and the write makes the write conflict; the edit is
/// then prepared again on the newer Soul, as legacy's last save simply won.
fn edit<R: SoulRepository + ?Sized>(
    repository: &R,
    owner: SoulOwner,
    edit: SoulUserEdit,
    now: TimestampMillis,
) -> Result<Option<lettuce_companions::SoulState>, SoulRepositoryError> {
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

/// How many times a growth or consolidation change is prepared again on a
/// newer Soul after a concurrent write made its revision stale.
const SOUL_APPLY_ATTEMPTS: usize = 3;

#[derive(Debug)]
pub(crate) enum SoulApplyError<E> {
    Prepare(E),
    Soul(SoulRepositoryError),
}

/// Applies a background Soul change under `operation_id`. The change is
/// prepared on `prepared_on` first; a concurrent write that lands in between
/// makes it stale, and it is then prepared again on the current Soul, as
/// legacy's later save kept both edits' growth. A replay after an earlier
/// attempt applied on a newer Soul answers that attempt's stored receipt.
/// Returns the receipt with the change set this call applied, or `None` when
/// the receipt was stored by an earlier call whose change set differed.
pub(crate) fn apply_on_latest_soul<R, E>(
    repository: &R,
    owner: SoulOwner,
    operation_id: OperationRecordId,
    prepared_on: &lettuce_companions::SoulState,
    prepare: impl Fn(&lettuce_companions::SoulState) -> Result<lettuce_companions::SoulChangeSet, E>,
) -> Result<(SoulApplyReceipt, Option<lettuce_companions::SoulChangeSet>), SoulApplyError<E>>
where
    R: SoulRepository + ?Sized,
{
    let mut change_set = prepare(prepared_on).map_err(SoulApplyError::Prepare)?;
    for attempt in 1..=SOUL_APPLY_ATTEMPTS {
        match repository.apply(owner, operation_id, change_set.clone()) {
            Ok(receipt) => return Ok((receipt, Some(change_set))),
            Err(SoulRepositoryError::OperationMismatch) => {
                return match repository.receipt(operation_id) {
                    Ok(Some(receipt)) if receipt.owner == owner => Ok((receipt, None)),
                    Ok(_) => Err(SoulApplyError::Soul(SoulRepositoryError::OperationMismatch)),
                    Err(error) => Err(SoulApplyError::Soul(error)),
                };
            }
            Err(SoulRepositoryError::Conflict) if attempt < SOUL_APPLY_ATTEMPTS => {
                let current = repository
                    .get(owner)
                    .map_err(SoulApplyError::Soul)?
                    .ok_or(SoulApplyError::Soul(SoulRepositoryError::NotFound))?;
                tracing::info!(
                    attempt,
                    "a concurrent Soul write landed first; preparing the change again"
                );
                change_set = prepare(&current).map_err(SoulApplyError::Prepare)?;
            }
            Err(error) => return Err(SoulApplyError::Soul(error)),
        }
    }
    Err(SoulApplyError::Soul(SoulRepositoryError::Conflict))
}

/// How many of an operation's proposed facts the stored Soul holds, for a
/// replay answered from an earlier call's receipt. Proposed fact ids are
/// unique to their operation.
pub(crate) fn stored_growth_count(
    facts: &[lettuce_companions::SoulFact],
    proposal_ids: &[String],
) -> usize {
    proposal_ids
        .iter()
        .filter(|id| facts.iter().any(|fact| &fact.id == *id))
        .count()
}

/// Removes every Soul growth entry, authored ones included, as legacy did;
/// answers how many there were.
pub fn clear_companion_soul_growth<R: SoulRepository + ?Sized>(
    repository: &R,
    owner: SoulOwner,
    now: TimestampMillis,
) -> Result<u32, SoulRepositoryError> {
    Ok(
        edit(repository, owner, SoulUserEdit::ClearAll, now)?.map_or(0, |state| {
            u32::try_from(state.facts.len()).unwrap_or(u32::MAX)
        }),
    )
}

/// Removes one Soul growth entry; false when there is none with that id.
/// Legacy removed by list position; entries now have stable ids.
pub fn remove_companion_soul_growth<R: SoulRepository + ?Sized>(
    repository: &R,
    owner: SoulOwner,
    fact_id: &str,
    now: TimestampMillis,
) -> Result<bool, SoulRepositoryError> {
    Ok(edit(
        repository,
        owner,
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
    owner: SoulOwner,
    fact_id: &str,
    locked: bool,
    now: TimestampMillis,
) -> Result<bool, SoulRepositoryError> {
    Ok(edit(
        repository,
        owner,
        SoulUserEdit::SetLocked {
            fact_id: fact_id.to_owned(),
            locked,
        },
        now,
    )?
    .is_some_and(|state| state.facts.iter().any(|fact| fact.id == fact_id)))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use lettuce_companions::{
        ProposedSoulFact, SoulCategory, SoulChangeSet, SoulFactKind, SoulFactPolicy, SoulState,
        apply_change_set, prepare_growth_change_set,
    };
    use lettuce_types::{CharacterId, Revision};

    use super::*;

    #[derive(Default)]
    struct Souls {
        state: Mutex<Option<SoulState>>,
        receipts: Mutex<HashMap<OperationRecordId, (SoulApplyReceipt, SoulChangeSet)>>,
    }

    impl SoulRepository for Souls {
        fn create(
            &self,
            _: SoulOwner,
            state: SoulState,
            _: TimestampMillis,
        ) -> Result<SoulState, SoulRepositoryError> {
            *self.state.lock().expect("state") = Some(state.clone());
            Ok(state)
        }

        fn get(&self, _: SoulOwner) -> Result<Option<SoulState>, SoulRepositoryError> {
            Ok(self.state.lock().expect("state").clone())
        }

        fn apply(
            &self,
            owner: SoulOwner,
            operation_id: OperationRecordId,
            change_set: SoulChangeSet,
        ) -> Result<SoulApplyReceipt, SoulRepositoryError> {
            let mut receipts = self.receipts.lock().expect("receipts");
            if let Some((receipt, applied)) = receipts.get(&operation_id) {
                return if *applied == change_set {
                    Ok(receipt.clone())
                } else {
                    Err(SoulRepositoryError::OperationMismatch)
                };
            }
            let mut state = self.state.lock().expect("state");
            let next =
                apply_change_set(state.as_ref().expect("soul"), &change_set).map_err(|error| {
                    match error {
                        lettuce_companions::SoulPolicyError::StaleRevision => {
                            SoulRepositoryError::Conflict
                        }
                        other => SoulRepositoryError::Invalid(other),
                    }
                })?;
            let receipt = SoulApplyReceipt {
                operation_id,
                owner,
                expected_revision: change_set.expected_revision,
                resulting_revision: next.revision,
                applied_at: change_set.applied_at,
            };
            *state = Some(next);
            receipts.insert(operation_id, (receipt.clone(), change_set));
            Ok(receipt)
        }

        fn receipt(
            &self,
            operation_id: OperationRecordId,
        ) -> Result<Option<SoulApplyReceipt>, SoulRepositoryError> {
            Ok(self
                .receipts
                .lock()
                .expect("receipts")
                .get(&operation_id)
                .map(|(receipt, _)| receipt.clone()))
        }
    }

    fn proposed(id: &str, slot: &str) -> ProposedSoulFact {
        ProposedSoulFact {
            id: id.to_owned(),
            category: SoulCategory::Likes,
            value: format!("value-{id}"),
            kind: SoulFactKind::Add,
            policy: SoulFactPolicy::Adaptive,
            slot: slot.to_owned(),
            confidence: 0.75,
            weight: 0.8,
            valid_until: None,
            locked: false,
            source_memory_ids: vec!["memory".to_owned()],
            supersedes: Vec::new(),
        }
    }

    #[test]
    fn growth_is_prepared_again_on_a_soul_a_concurrent_edit_changed() {
        let souls = Souls::default();
        let owner = SoulOwner::Character(CharacterId::new());
        let initial = SoulState {
            revision: Revision::INITIAL,
            facts: Vec::new(),
        };
        souls
            .create(owner, initial.clone(), TimestampMillis::new(1))
            .expect("create");
        let concurrent = prepare_growth_change_set(
            &initial,
            initial.revision,
            vec![proposed("tea", "drink")],
            TimestampMillis::new(2),
        )
        .expect("concurrent change");
        souls
            .apply(owner, OperationRecordId::new(), concurrent)
            .expect("concurrent write");

        let operation_id = OperationRecordId::new();
        let prepare = |soul: &SoulState| {
            prepare_growth_change_set(
                soul,
                soul.revision,
                vec![proposed("maps", "hobby")],
                TimestampMillis::new(3),
            )
        };
        let (receipt, applied) =
            apply_on_latest_soul(&souls, owner, operation_id, &initial, prepare)
                .expect("growth applies on the newer soul");
        assert_eq!(receipt.expected_revision, Revision::new(2));
        assert_eq!(applied.map(|applied| applied.additions.len()), Some(1));
        let facts = souls.get(owner).expect("get").expect("soul").facts;
        assert_eq!(
            facts
                .iter()
                .map(|fact| fact.id.as_str())
                .collect::<Vec<_>>(),
            ["tea", "maps"]
        );

        let (replayed, replayed_change) =
            apply_on_latest_soul(&souls, owner, operation_id, &initial, prepare)
                .expect("a replay answers the applied receipt");
        assert_eq!(replayed, receipt);
        assert_eq!(replayed_change, None);
        assert_eq!(
            stored_growth_count(&facts, &["maps".to_owned(), "gone".to_owned()]),
            1
        );
        assert_eq!(souls.get(owner).expect("get").expect("soul").facts, facts);
    }
}
