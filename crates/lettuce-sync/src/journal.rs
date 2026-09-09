use lettuce_types::{ContentHash, OperationId, TimestampMillis};

use crate::{
    CanonicalChange, CanonicalPayload, CausalFrontier, ChangeOperation, SyncChangeError,
    SyncDeviceId, SyncEntity,
};

pub const MAX_OUTBOUND_CHANGES: usize = 256;
pub const MAX_OUTBOUND_PAYLOAD_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewCanonicalChange {
    entity: SyncEntity,
    operation: ChangeOperation,
    base_revision: Option<ContentHash>,
    payload: Option<CanonicalPayload>,
}

impl NewCanonicalChange {
    pub fn new(
        entity: SyncEntity,
        operation: ChangeOperation,
        base_revision: Option<ContentHash>,
        payload: Option<CanonicalPayload>,
    ) -> Result<Self, SyncChangeError> {
        match (operation, base_revision.is_some(), payload.is_some()) {
            (ChangeOperation::Insert, false, true)
            | (ChangeOperation::Update, true, true)
            | (ChangeOperation::Delete, true, false) => Ok(Self {
                entity,
                operation,
                base_revision,
                payload,
            }),
            (ChangeOperation::Insert, _, _) => Err(SyncChangeError::InvalidInsert),
            (ChangeOperation::Update, _, _) => Err(SyncChangeError::InvalidUpdate),
            (ChangeOperation::Delete, _, _) => Err(SyncChangeError::InvalidDelete),
        }
    }

    #[must_use]
    pub const fn entity(&self) -> &SyncEntity {
        &self.entity
    }

    #[must_use]
    pub const fn operation(&self) -> ChangeOperation {
        self.operation
    }

    #[must_use]
    pub const fn base_revision(&self) -> Option<&ContentHash> {
        self.base_revision.as_ref()
    }

    #[must_use]
    pub const fn payload(&self) -> Option<&CanonicalPayload> {
        self.payload.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalChangeAdmission {
    pub change: CanonicalChange,
    pub created: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboundChangeBatch {
    pub changes: Vec<CanonicalChange>,
    pub payload_bytes: usize,
    pub has_more: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LocalChangeJournalError {
    #[error("local sync change input is invalid")]
    Invalid,
    #[error("local sync operation was already used for different input")]
    Conflict,
    #[error("stored local sync change is corrupt")]
    Corrupt,
    #[error("local sync sequence or clock counter is exhausted")]
    Exhausted,
    #[error("local sync frontier is invalid")]
    InvalidFrontier,
    #[error("local sync journal is missing an expected change sequence")]
    MissingSequence,
    #[error("local sync change exceeds the requested outbound payload limit")]
    ChangeTooLarge,
    #[error("local sync change has unsatisfied causal dependencies")]
    UnsatisfiedDependencies,
    #[error("local sync journal storage failed")]
    Storage,
}

pub trait LocalChangeJournal: Send + Sync {
    fn local_device_id(
        &self,
        now: TimestampMillis,
    ) -> Result<SyncDeviceId, LocalChangeJournalError>;

    fn record_local_change(
        &self,
        operation_id: OperationId,
        request: NewCanonicalChange,
        now: TimestampMillis,
    ) -> Result<LocalChangeAdmission, LocalChangeJournalError>;

    fn local_change_for_operation(
        &self,
        operation_id: OperationId,
    ) -> Result<Option<CanonicalChange>, LocalChangeJournalError>;

    fn local_frontier(&self) -> Result<CausalFrontier, LocalChangeJournalError>;

    fn outbound_changes(
        &self,
        remote_frontier: &CausalFrontier,
        max_changes: usize,
        max_payload_bytes: usize,
    ) -> Result<OutboundChangeBatch, LocalChangeJournalError>;

    fn record_peer_acknowledgement(
        &self,
        peer: SyncDeviceId,
        frontier: &CausalFrontier,
        now: TimestampMillis,
    ) -> Result<CausalFrontier, LocalChangeJournalError>;

    fn peer_acknowledgement(
        &self,
        peer: SyncDeviceId,
    ) -> Result<CausalFrontier, LocalChangeJournalError>;
}
