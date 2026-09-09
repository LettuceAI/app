use std::collections::BTreeMap;

use lettuce_types::{ContentHash, TimestampMillis};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const CANONICAL_CHANGE_VERSION: u32 = 1;
pub const MAX_CANONICAL_PAYLOAD_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_FRONTIER_DEVICES: usize = 1_024;

const MAX_ENTITY_KIND_BYTES: usize = 64;
const MAX_ENTITY_ID_BYTES: usize = 512;
const MAX_SCHEMA_BYTES: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SyncChangeId(Uuid);

impl SyncChangeId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    #[must_use]
    pub const fn from_uuid(value: Uuid) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl Default for SyncChangeId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SyncDeviceId(Uuid);

impl SyncDeviceId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    #[must_use]
    pub const fn from_uuid(value: Uuid) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl Default for SyncDeviceId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct HybridTimestamp {
    wall_time: TimestampMillis,
    counter: u32,
}

impl HybridTimestamp {
    #[must_use]
    pub const fn new(wall_time: TimestampMillis, counter: u32) -> Self {
        Self { wall_time, counter }
    }

    #[must_use]
    pub const fn wall_time(self) -> TimestampMillis {
        self.wall_time
    }

    #[must_use]
    pub const fn counter(self) -> u32 {
        self.counter
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChangeOperation {
    Insert,
    Update,
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SyncEntity {
    kind: String,
    id: String,
}

impl SyncEntity {
    pub fn new(kind: impl Into<String>, id: impl Into<String>) -> Result<Self, SyncChangeError> {
        let kind = kind.into();
        let id = id.into();
        if !validate_token(&kind, MAX_ENTITY_KIND_BYTES) {
            return Err(SyncChangeError::InvalidEntityKind);
        }
        if !validate_text(&id, MAX_ENTITY_ID_BYTES) {
            return Err(SyncChangeError::InvalidEntityId);
        }
        Ok(Self { kind, id })
    }

    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonicalPayload {
    schema: String,
    version: u32,
    bytes: Vec<u8>,
    content_hash: ContentHash,
}

impl CanonicalPayload {
    pub fn new(
        schema: impl Into<String>,
        version: u32,
        bytes: Vec<u8>,
    ) -> Result<Self, SyncChangeError> {
        let schema = schema.into();
        if !validate_token(&schema, MAX_SCHEMA_BYTES) {
            return Err(SyncChangeError::InvalidPayloadSchema);
        }
        if version == 0 {
            return Err(SyncChangeError::InvalidPayloadVersion);
        }
        if bytes.is_empty() || bytes.len() > MAX_CANONICAL_PAYLOAD_BYTES {
            return Err(SyncChangeError::InvalidPayloadSize {
                received: bytes.len(),
                limit: MAX_CANONICAL_PAYLOAD_BYTES,
            });
        }
        let content_hash = content_hash(&bytes)?;
        Ok(Self {
            schema,
            version,
            bytes,
            content_hash,
        })
    }

    #[must_use]
    pub fn schema(&self) -> &str {
        &self.schema
    }

    #[must_use]
    pub const fn version(&self) -> u32 {
        self.version
    }

    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    #[must_use]
    pub const fn content_hash(&self) -> &ContentHash {
        &self.content_hash
    }
}

pub type CausalFrontier = BTreeMap<SyncDeviceId, u64>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonicalChange {
    version: u32,
    id: SyncChangeId,
    origin_device: SyncDeviceId,
    origin_sequence: u64,
    timestamp: HybridTimestamp,
    base_frontier: CausalFrontier,
    entity: SyncEntity,
    operation: ChangeOperation,
    base_revision: Option<ContentHash>,
    payload: Option<CanonicalPayload>,
    fingerprint: ContentHash,
}

impl CanonicalChange {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: SyncChangeId,
        origin_device: SyncDeviceId,
        origin_sequence: u64,
        timestamp: HybridTimestamp,
        base_frontier: CausalFrontier,
        entity: SyncEntity,
        operation: ChangeOperation,
        base_revision: Option<ContentHash>,
        payload: Option<CanonicalPayload>,
    ) -> Result<Self, SyncChangeError> {
        validate_change_contract(
            origin_device,
            origin_sequence,
            &base_frontier,
            operation,
            &base_revision,
            &payload,
        )?;
        let fingerprint = change_fingerprint(
            id,
            origin_device,
            origin_sequence,
            timestamp,
            &base_frontier,
            &entity,
            operation,
            base_revision.as_ref(),
            payload.as_ref(),
        )?;
        Ok(Self {
            version: CANONICAL_CHANGE_VERSION,
            id,
            origin_device,
            origin_sequence,
            timestamp,
            base_frontier,
            entity,
            operation,
            base_revision,
            payload,
            fingerprint,
        })
    }

    #[must_use]
    pub const fn version(&self) -> u32 {
        self.version
    }

    #[must_use]
    pub const fn id(&self) -> SyncChangeId {
        self.id
    }

    #[must_use]
    pub const fn origin_device(&self) -> SyncDeviceId {
        self.origin_device
    }

    #[must_use]
    pub const fn origin_sequence(&self) -> u64 {
        self.origin_sequence
    }

    #[must_use]
    pub const fn timestamp(&self) -> HybridTimestamp {
        self.timestamp
    }

    #[must_use]
    pub const fn base_frontier(&self) -> &CausalFrontier {
        &self.base_frontier
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

    #[must_use]
    pub const fn fingerprint(&self) -> &ContentHash {
        &self.fingerprint
    }

    #[must_use]
    pub fn observes(&self, other: &Self) -> bool {
        (self.origin_device == other.origin_device && self.origin_sequence > other.origin_sequence)
            || self
                .base_frontier
                .get(&other.origin_device)
                .is_some_and(|sequence| *sequence >= other.origin_sequence)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SyncChangeError {
    #[error("sync entity kind must be a bounded lowercase identifier")]
    InvalidEntityKind,
    #[error("sync entity ID is blank, unsafe, or too long")]
    InvalidEntityId,
    #[error("canonical payload schema must be a bounded lowercase identifier")]
    InvalidPayloadSchema,
    #[error("canonical payload version must be at least one")]
    InvalidPayloadVersion,
    #[error("canonical payload has {received} bytes; the limit is {limit}")]
    InvalidPayloadSize { received: usize, limit: usize },
    #[error("origin sequence must be at least one")]
    InvalidOriginSequence,
    #[error("causal frontier has too many devices")]
    FrontierTooLarge,
    #[error("causal frontier sequences must be at least one")]
    InvalidFrontierSequence,
    #[error("origin frontier must precede the new origin sequence")]
    InvalidOriginFrontier,
    #[error("insert changes require a payload and no base revision")]
    InvalidInsert,
    #[error("update changes require a payload and base revision")]
    InvalidUpdate,
    #[error("delete changes require a base revision and no payload")]
    InvalidDelete,
    #[error("failed to construct a canonical content hash")]
    InvalidHash,
}

fn validate_change_contract(
    origin_device: SyncDeviceId,
    origin_sequence: u64,
    base_frontier: &CausalFrontier,
    operation: ChangeOperation,
    base_revision: &Option<ContentHash>,
    payload: &Option<CanonicalPayload>,
) -> Result<(), SyncChangeError> {
    if origin_sequence == 0 {
        return Err(SyncChangeError::InvalidOriginSequence);
    }
    if base_frontier.len() > MAX_FRONTIER_DEVICES {
        return Err(SyncChangeError::FrontierTooLarge);
    }
    if base_frontier.values().any(|sequence| *sequence == 0) {
        return Err(SyncChangeError::InvalidFrontierSequence);
    }
    if base_frontier
        .get(&origin_device)
        .is_some_and(|sequence| *sequence >= origin_sequence)
    {
        return Err(SyncChangeError::InvalidOriginFrontier);
    }
    match (operation, base_revision.is_some(), payload.is_some()) {
        (ChangeOperation::Insert, false, true) => Ok(()),
        (ChangeOperation::Update, true, true) => Ok(()),
        (ChangeOperation::Delete, true, false) => Ok(()),
        (ChangeOperation::Insert, _, _) => Err(SyncChangeError::InvalidInsert),
        (ChangeOperation::Update, _, _) => Err(SyncChangeError::InvalidUpdate),
        (ChangeOperation::Delete, _, _) => Err(SyncChangeError::InvalidDelete),
    }
}

#[allow(clippy::too_many_arguments)]
fn change_fingerprint(
    id: SyncChangeId,
    origin_device: SyncDeviceId,
    origin_sequence: u64,
    timestamp: HybridTimestamp,
    base_frontier: &CausalFrontier,
    entity: &SyncEntity,
    operation: ChangeOperation,
    base_revision: Option<&ContentHash>,
    payload: Option<&CanonicalPayload>,
) -> Result<ContentHash, SyncChangeError> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"lettuce-sync-canonical-change-v1\0");
    hasher.update(&CANONICAL_CHANGE_VERSION.to_le_bytes());
    hasher.update(id.as_uuid().as_bytes());
    hasher.update(origin_device.as_uuid().as_bytes());
    hasher.update(&origin_sequence.to_le_bytes());
    hasher.update(&timestamp.wall_time().get().to_le_bytes());
    hasher.update(&timestamp.counter().to_le_bytes());
    hash_len(&mut hasher, base_frontier.len());
    for (device, sequence) in base_frontier {
        hasher.update(device.as_uuid().as_bytes());
        hasher.update(&sequence.to_le_bytes());
    }
    hash_bytes(&mut hasher, entity.kind().as_bytes());
    hash_bytes(&mut hasher, entity.id().as_bytes());
    hasher.update(&[match operation {
        ChangeOperation::Insert => 1,
        ChangeOperation::Update => 2,
        ChangeOperation::Delete => 3,
    }]);
    hash_optional(
        &mut hasher,
        base_revision.map(|value| value.as_str().as_bytes()),
    );
    match payload {
        Some(payload) => {
            hasher.update(&[1]);
            hash_bytes(&mut hasher, payload.schema().as_bytes());
            hasher.update(&payload.version().to_le_bytes());
            hash_bytes(&mut hasher, payload.content_hash().as_str().as_bytes());
            hash_len(&mut hasher, payload.bytes().len());
        }
        None => {
            hasher.update(&[0]);
        }
    }
    ContentHash::parse(hasher.finalize().to_hex().to_string())
        .map_err(|_| SyncChangeError::InvalidHash)
}

fn content_hash(bytes: &[u8]) -> Result<ContentHash, SyncChangeError> {
    ContentHash::parse(blake3::hash(bytes).to_hex().to_string())
        .map_err(|_| SyncChangeError::InvalidHash)
}

fn hash_optional(hasher: &mut blake3::Hasher, bytes: Option<&[u8]>) {
    match bytes {
        Some(bytes) => {
            hasher.update(&[1]);
            hash_bytes(hasher, bytes);
        }
        None => {
            hasher.update(&[0]);
        }
    }
}

fn hash_bytes(hasher: &mut blake3::Hasher, bytes: &[u8]) {
    hash_len(hasher, bytes.len());
    hasher.update(bytes);
}

fn hash_len(hasher: &mut blake3::Hasher, len: usize) {
    hasher.update(&(len as u64).to_le_bytes());
}

fn validate_token(value: &str, max_bytes: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_bytes
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && value
            .bytes()
            .next_back()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-' | b'_')
        })
}

fn validate_text(value: &str, max_bytes: usize) -> bool {
    !value.is_empty()
        && value.trim() == value
        && value.len() <= max_bytes
        && !value.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use lettuce_types::{ContentHash, TimestampMillis};
    use uuid::Uuid;

    use super::*;

    fn device(value: u128) -> SyncDeviceId {
        SyncDeviceId::from_uuid(Uuid::from_u128(value))
    }

    fn change(origin: SyncDeviceId, sequence: u64, frontier: CausalFrontier) -> CanonicalChange {
        CanonicalChange::new(
            SyncChangeId::from_uuid(Uuid::from_u128(sequence.into())),
            origin,
            sequence,
            HybridTimestamp::new(TimestampMillis::new(100), 0),
            frontier,
            SyncEntity::new("conversation.message", "message-1").expect("entity"),
            ChangeOperation::Insert,
            None,
            Some(
                CanonicalPayload::new("conversation.message", 1, b"canonical".to_vec())
                    .expect("payload"),
            ),
        )
        .expect("change")
    }

    fn hash(byte: &str) -> ContentHash {
        ContentHash::parse(byte.repeat(64)).expect("hash")
    }

    #[test]
    fn canonical_change_retains_identity_operation_and_payload() {
        let value = change(device(1), 1, BTreeMap::new());

        assert_eq!(value.version(), CANONICAL_CHANGE_VERSION);
        assert_eq!(value.origin_sequence(), 1);
        assert_eq!(value.entity().kind(), "conversation.message");
        assert_eq!(value.entity().id(), "message-1");
        assert_eq!(value.operation(), ChangeOperation::Insert);
        assert_eq!(value.payload().expect("payload").bytes(), b"canonical");
        assert!(value.base_revision().is_none());
    }

    #[test]
    fn fingerprints_are_stable_and_bind_causal_and_payload_facts() {
        let first = change(device(1), 2, BTreeMap::from([(device(1), 1)]));
        let replay = change(device(1), 2, BTreeMap::from([(device(1), 1)]));
        let changed = change(device(1), 2, BTreeMap::from([(device(2), 1)]));
        let changed_payload = CanonicalChange::new(
            first.id(),
            first.origin_device(),
            first.origin_sequence(),
            first.timestamp(),
            first.base_frontier().clone(),
            first.entity().clone(),
            first.operation(),
            first.base_revision().cloned(),
            Some(
                CanonicalPayload::new("conversation.message", 1, b"changed".to_vec())
                    .expect("payload"),
            ),
        )
        .expect("change");

        assert_eq!(first.fingerprint(), replay.fingerprint());
        assert_ne!(first.fingerprint(), changed.fingerprint());
        assert_ne!(first.fingerprint(), changed_payload.fingerprint());
        assert_eq!(
            first.payload().map(CanonicalPayload::content_hash),
            replay.payload().map(CanonicalPayload::content_hash)
        );
    }

    #[test]
    fn causal_observation_uses_origin_order_and_explicit_frontier() {
        let first = change(device(1), 1, BTreeMap::new());
        let next = change(device(1), 2, BTreeMap::from([(device(1), 1)]));
        let peer = change(device(2), 1, BTreeMap::from([(device(1), 1)]));

        assert!(next.observes(&first));
        assert!(peer.observes(&first));
        assert!(!first.observes(&peer));
    }

    #[test]
    fn insert_update_and_delete_have_distinct_evidence_contracts() {
        let payload = || CanonicalPayload::new("persona", 1, vec![1]).expect("payload");
        let base = hash("a");
        let make = |operation, base_revision, payload| {
            CanonicalChange::new(
                SyncChangeId::new(),
                device(1),
                1,
                HybridTimestamp::new(TimestampMillis::new(1), 0),
                BTreeMap::new(),
                SyncEntity::new("persona", "persona-1").expect("entity"),
                operation,
                base_revision,
                payload,
            )
        };

        assert!(make(ChangeOperation::Insert, None, Some(payload())).is_ok());
        assert!(make(ChangeOperation::Update, Some(base.clone()), Some(payload())).is_ok());
        assert!(make(ChangeOperation::Delete, Some(base), None).is_ok());
        assert_eq!(
            make(ChangeOperation::Insert, Some(hash("b")), Some(payload())),
            Err(SyncChangeError::InvalidInsert)
        );
        assert_eq!(
            make(ChangeOperation::Update, None, Some(payload())),
            Err(SyncChangeError::InvalidUpdate)
        );
        assert_eq!(
            make(ChangeOperation::Delete, Some(hash("c")), Some(payload())),
            Err(SyncChangeError::InvalidDelete)
        );
    }

    #[test]
    fn identifiers_and_payloads_are_bounded_before_hashing() {
        assert_eq!(
            SyncEntity::new("Conversation", "id"),
            Err(SyncChangeError::InvalidEntityKind)
        );
        assert_eq!(
            SyncEntity::new("conversation", " id"),
            Err(SyncChangeError::InvalidEntityId)
        );
        assert_eq!(
            CanonicalPayload::new("conversation", 0, vec![1]),
            Err(SyncChangeError::InvalidPayloadVersion)
        );
        assert_eq!(
            CanonicalPayload::new("conversation", 1, Vec::new()),
            Err(SyncChangeError::InvalidPayloadSize {
                received: 0,
                limit: MAX_CANONICAL_PAYLOAD_BYTES,
            })
        );
    }

    #[test]
    fn causal_frontiers_reject_zero_future_and_unbounded_entries() {
        let build = |sequence, frontier| {
            CanonicalChange::new(
                SyncChangeId::new(),
                device(1),
                sequence,
                HybridTimestamp::new(TimestampMillis::new(1), 0),
                frontier,
                SyncEntity::new("persona", "persona-1").expect("entity"),
                ChangeOperation::Insert,
                None,
                Some(CanonicalPayload::new("persona", 1, vec![1]).expect("payload")),
            )
        };

        assert_eq!(
            build(0, BTreeMap::new()),
            Err(SyncChangeError::InvalidOriginSequence)
        );
        assert_eq!(
            build(2, BTreeMap::from([(device(2), 0)])),
            Err(SyncChangeError::InvalidFrontierSequence)
        );
        assert_eq!(
            build(2, BTreeMap::from([(device(1), 2)])),
            Err(SyncChangeError::InvalidOriginFrontier)
        );
        let oversized = (1..=(MAX_FRONTIER_DEVICES + 1))
            .map(|value| (device(value as u128 + 10), 1))
            .collect();
        assert_eq!(build(2, oversized), Err(SyncChangeError::FrontierTooLarge));
    }
}
