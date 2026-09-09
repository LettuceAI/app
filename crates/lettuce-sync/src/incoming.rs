use lettuce_types::{ContentHash, OperationId, TimestampMillis};

use crate::{CanonicalChange, CausalFrontier, SyncDeviceId};

pub const MAX_INCOMING_CHANGES: usize = 256;
pub const MAX_INCOMING_PAYLOAD_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncomingBatchState {
    Staged,
    Pending,
    Committed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingBatchAdmission {
    pub state: IncomingBatchState,
    pub created: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingBatchResult {
    pub state: IncomingBatchState,
    pub applied: usize,
    pub duplicates: usize,
    pub conflicts: usize,
    pub frontier: CausalFrontier,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum IncomingChangeError {
    #[error("incoming sync batch is empty or exceeds its count or payload limit")]
    InvalidBatch,
    #[error("incoming sync batch hash does not match its canonical changes")]
    InvalidBatchHash,
    #[error("incoming sync batch identity was reused with different content")]
    Conflict,
    #[error("incoming sync batch was not found")]
    NotFound,
    #[error("incoming sync batch storage is corrupt")]
    Corrupt,
    #[error("incoming sync storage failed")]
    Storage,
}

#[must_use]
pub fn canonical_batch_hash(changes: &[CanonicalChange]) -> ContentHash {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&(changes.len() as u64).to_le_bytes());
    for change in changes {
        let fingerprint = change.fingerprint().as_str().as_bytes();
        hasher.update(&(fingerprint.len() as u64).to_le_bytes());
        hasher.update(fingerprint);
    }
    ContentHash::parse(hasher.finalize().to_hex().to_string())
        .expect("BLAKE3 always produces a valid content hash")
}

pub trait IncomingChangeRepository: Send + Sync {
    fn stage_incoming_batch(
        &self,
        peer: SyncDeviceId,
        batch_id: OperationId,
        declared_hash: &ContentHash,
        changes: &[CanonicalChange],
        now: TimestampMillis,
    ) -> Result<IncomingBatchAdmission, IncomingChangeError>;

    fn apply_incoming_batch(
        &self,
        batch_id: OperationId,
        now: TimestampMillis,
    ) -> Result<IncomingBatchResult, IncomingChangeError>;
}

#[cfg(test)]
mod tests {
    use crate::{CanonicalPayload, ChangeOperation, HybridTimestamp, SyncChangeId, SyncEntity};

    use super::*;

    fn change(id: SyncChangeId, payload: &[u8]) -> CanonicalChange {
        CanonicalChange::new(
            id,
            SyncDeviceId::from_uuid(uuid::Uuid::from_u128(1)),
            1,
            HybridTimestamp::new(TimestampMillis::new(10), 0),
            CausalFrontier::new(),
            SyncEntity::new("persona", uuid::Uuid::from_u128(2).to_string()).expect("entity"),
            ChangeOperation::Insert,
            None,
            Some(CanonicalPayload::new("persona.snapshot", 1, payload.to_vec()).expect("payload")),
        )
        .expect("change")
    }

    #[test]
    fn batch_hash_binds_order_and_change_content() {
        let first_id = SyncChangeId::from_uuid(uuid::Uuid::from_u128(3));
        let second_id = SyncChangeId::from_uuid(uuid::Uuid::from_u128(4));
        let first = change(first_id, b"first");
        let second = change(second_id, b"second");

        assert_eq!(
            canonical_batch_hash(std::slice::from_ref(&first)),
            canonical_batch_hash(std::slice::from_ref(&first))
        );
        assert_ne!(
            canonical_batch_hash(&[first.clone(), second.clone()]),
            canonical_batch_hash(&[second, first])
        );
    }
}
