use lettuce_types::{ContentHash, OperationId};
use uuid::Uuid;

use crate::{
    CanonicalChange, CausalFrontier, MAX_CANONICAL_PAYLOAD_BYTES, MAX_INCOMING_CHANGES,
    MAX_INCOMING_PAYLOAD_BYTES, MEDIA_ASSET_SYNC_SCHEMA, MEDIA_ASSET_SYNC_VERSION,
    PERSONA_DEFAULT_SYNC_SCHEMA, PERSONA_DEFAULT_SYNC_VERSION, PERSONA_SYNC_SCHEMA,
    PERSONA_SYNC_VERSION, SyncDeviceId, canonical_batch_hash,
};

pub const SYNC_PROTOCOL_VERSION: u32 = 1;

const MAX_APP_VERSION_BYTES: usize = 64;
const MAX_DEVICE_NAME_BYTES: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SyncSessionId(Uuid);

impl SyncSessionId {
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

impl Default for SyncSessionId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncTransferLimits {
    max_changes_per_batch: usize,
    max_change_payload_bytes: usize,
    max_batch_payload_bytes: usize,
}

impl SyncTransferLimits {
    pub fn new(
        max_changes_per_batch: usize,
        max_change_payload_bytes: usize,
        max_batch_payload_bytes: usize,
    ) -> Result<Self, SyncSessionError> {
        if max_changes_per_batch == 0
            || max_changes_per_batch > MAX_INCOMING_CHANGES
            || max_change_payload_bytes == 0
            || max_change_payload_bytes > MAX_CANONICAL_PAYLOAD_BYTES
            || max_batch_payload_bytes < max_change_payload_bytes
            || max_batch_payload_bytes > MAX_INCOMING_PAYLOAD_BYTES
        {
            return Err(SyncSessionError::InvalidTransferLimits);
        }
        Ok(Self {
            max_changes_per_batch,
            max_change_payload_bytes,
            max_batch_payload_bytes,
        })
    }

    #[must_use]
    pub const fn max_changes_per_batch(self) -> usize {
        self.max_changes_per_batch
    }

    #[must_use]
    pub const fn max_change_payload_bytes(self) -> usize {
        self.max_change_payload_bytes
    }

    #[must_use]
    pub const fn max_batch_payload_bytes(self) -> usize {
        self.max_batch_payload_bytes
    }

    fn intersect(self, other: Self) -> Self {
        Self {
            max_changes_per_batch: self.max_changes_per_batch.min(other.max_changes_per_batch),
            max_change_payload_bytes: self
                .max_change_payload_bytes
                .min(other.max_change_payload_bytes),
            max_batch_payload_bytes: self
                .max_batch_payload_bytes
                .min(other.max_batch_payload_bytes),
        }
    }
}

impl Default for SyncTransferLimits {
    fn default() -> Self {
        Self {
            max_changes_per_batch: MAX_INCOMING_CHANGES,
            max_change_payload_bytes: MAX_CANONICAL_PAYLOAD_BYTES,
            max_batch_payload_bytes: MAX_INCOMING_PAYLOAD_BYTES,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncHello {
    app_version: String,
    protocol_version: u32,
    schema_fingerprint: ContentHash,
    device_id: SyncDeviceId,
    device_name: String,
    session_id: SyncSessionId,
    limits: SyncTransferLimits,
}

impl SyncHello {
    pub fn current(
        app_version: impl Into<String>,
        device_id: SyncDeviceId,
        device_name: impl Into<String>,
        session_id: SyncSessionId,
        limits: SyncTransferLimits,
    ) -> Result<Self, SyncSessionError> {
        Self::new(
            app_version,
            SYNC_PROTOCOL_VERSION,
            current_sync_schema_fingerprint(),
            device_id,
            device_name,
            session_id,
            limits,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        app_version: impl Into<String>,
        protocol_version: u32,
        schema_fingerprint: ContentHash,
        device_id: SyncDeviceId,
        device_name: impl Into<String>,
        session_id: SyncSessionId,
        limits: SyncTransferLimits,
    ) -> Result<Self, SyncSessionError> {
        let app_version = app_version.into();
        if !valid_text(&app_version, MAX_APP_VERSION_BYTES) {
            return Err(SyncSessionError::InvalidAppVersion);
        }
        let device_name = device_name.into();
        if !valid_text(&device_name, MAX_DEVICE_NAME_BYTES) {
            return Err(SyncSessionError::InvalidDeviceName);
        }
        Ok(Self {
            app_version,
            protocol_version,
            schema_fingerprint,
            device_id,
            device_name,
            session_id,
            limits,
        })
    }

    #[must_use]
    pub fn app_version(&self) -> &str {
        &self.app_version
    }

    #[must_use]
    pub const fn protocol_version(&self) -> u32 {
        self.protocol_version
    }

    #[must_use]
    pub const fn schema_fingerprint(&self) -> &ContentHash {
        &self.schema_fingerprint
    }

    #[must_use]
    pub const fn device_id(&self) -> SyncDeviceId {
        self.device_id
    }

    #[must_use]
    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    #[must_use]
    pub const fn session_id(&self) -> SyncSessionId {
        self.session_id
    }

    #[must_use]
    pub const fn limits(&self) -> SyncTransferLimits {
        self.limits
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NegotiatedSyncSession {
    pub peer_device_id: SyncDeviceId,
    pub peer_device_name: String,
    pub peer_session_id: SyncSessionId,
    pub limits: SyncTransferLimits,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncChangeBatch {
    batch_id: OperationId,
    batch_hash: ContentHash,
    changes: Vec<CanonicalChange>,
}

impl SyncChangeBatch {
    pub fn new(
        batch_id: OperationId,
        changes: Vec<CanonicalChange>,
        limits: SyncTransferLimits,
    ) -> Result<Self, SyncSessionError> {
        let batch_hash = canonical_batch_hash(&changes);
        Self::from_parts(batch_id, batch_hash, changes, limits)
    }

    pub fn from_parts(
        batch_id: OperationId,
        batch_hash: ContentHash,
        changes: Vec<CanonicalChange>,
        limits: SyncTransferLimits,
    ) -> Result<Self, SyncSessionError> {
        let payload_bytes = changes.iter().try_fold(0usize, |total, change| {
            total.checked_add(change.payload().map_or(0, |payload| payload.bytes().len()))
        });
        if changes.is_empty()
            || changes.len() > limits.max_changes_per_batch
            || changes.iter().any(|change| {
                change
                    .payload()
                    .is_some_and(|payload| payload.bytes().len() > limits.max_change_payload_bytes)
            })
            || payload_bytes.is_none_or(|bytes| bytes > limits.max_batch_payload_bytes)
            || canonical_batch_hash(&changes) != batch_hash
        {
            return Err(SyncSessionError::InvalidChangeBatch);
        }
        Ok(Self {
            batch_id,
            batch_hash,
            changes,
        })
    }

    #[must_use]
    pub const fn batch_id(&self) -> OperationId {
        self.batch_id
    }

    #[must_use]
    pub const fn batch_hash(&self) -> &ContentHash {
        &self.batch_hash
    }

    #[must_use]
    pub fn changes(&self) -> &[CanonicalChange] {
        &self.changes
    }

    #[must_use]
    pub fn into_changes(self) -> Vec<CanonicalChange> {
        self.changes
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncChangeFrame {
    Batch(SyncChangeBatch),
    Quiescent { frontier: CausalFrontier },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncBatchAcknowledgement {
    pub batch_id: Option<OperationId>,
    pub frontier: CausalFrontier,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SyncSessionError {
    #[error("sync app version is empty, unbounded, or contains control characters")]
    InvalidAppVersion,
    #[error("sync device name is empty, unbounded, or contains control characters")]
    InvalidDeviceName,
    #[error("sync transfer limits are invalid")]
    InvalidTransferLimits,
    #[error("sync change batch is empty, unbounded, or has an invalid hash")]
    InvalidChangeBatch,
    #[error("authenticated peer identity does not match the sync hello")]
    AuthenticatedIdentityMismatch,
    #[error("both sync peers use the same device identity")]
    DuplicateDeviceIdentity,
    #[error("both sync peers use the same session identity")]
    DuplicateSessionIdentity,
    #[error("sync app versions do not match")]
    AppVersionMismatch,
    #[error("sync protocol {received} does not match {expected}")]
    ProtocolMismatch { expected: u32, received: u32 },
    #[error("sync schema capabilities do not match")]
    SchemaMismatch,
}

pub fn negotiate_sync_session(
    local: &SyncHello,
    peer: &SyncHello,
    authenticated_peer: SyncDeviceId,
) -> Result<NegotiatedSyncSession, SyncSessionError> {
    if peer.device_id != authenticated_peer {
        return Err(SyncSessionError::AuthenticatedIdentityMismatch);
    }
    if local.device_id == peer.device_id {
        return Err(SyncSessionError::DuplicateDeviceIdentity);
    }
    if local.session_id == peer.session_id {
        return Err(SyncSessionError::DuplicateSessionIdentity);
    }
    if local.app_version != peer.app_version {
        return Err(SyncSessionError::AppVersionMismatch);
    }
    if local.protocol_version != peer.protocol_version {
        return Err(SyncSessionError::ProtocolMismatch {
            expected: local.protocol_version,
            received: peer.protocol_version,
        });
    }
    if local.schema_fingerprint != peer.schema_fingerprint {
        return Err(SyncSessionError::SchemaMismatch);
    }
    Ok(NegotiatedSyncSession {
        peer_device_id: peer.device_id,
        peer_device_name: peer.device_name.clone(),
        peer_session_id: peer.session_id,
        limits: local.limits.intersect(peer.limits),
    })
}

#[must_use]
pub fn current_sync_schema_fingerprint() -> ContentHash {
    let mut hasher = blake3::Hasher::new_derive_key("lettuce.sync.schemas.v1");
    for (schema, version) in [
        (MEDIA_ASSET_SYNC_SCHEMA, MEDIA_ASSET_SYNC_VERSION),
        (PERSONA_DEFAULT_SYNC_SCHEMA, PERSONA_DEFAULT_SYNC_VERSION),
        (PERSONA_SYNC_SCHEMA, PERSONA_SYNC_VERSION),
    ] {
        hasher.update(&(schema.len() as u64).to_le_bytes());
        hasher.update(schema.as_bytes());
        hasher.update(&version.to_le_bytes());
    }
    ContentHash::parse(hasher.finalize().to_hex().to_string())
        .expect("BLAKE3 always produces a valid content hash")
}

fn valid_text(value: &str, max_bytes: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max_bytes && !value.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChangeOperation, HybridTimestamp, SyncChangeId, SyncEntity};
    use lettuce_types::TimestampMillis;

    fn hello(device: u128, session: u128) -> SyncHello {
        SyncHello::current(
            "1.2.3",
            SyncDeviceId::from_uuid(Uuid::from_u128(device)),
            format!("Device {device}"),
            SyncSessionId::from_uuid(Uuid::from_u128(session)),
            SyncTransferLimits::default(),
        )
        .expect("hello")
    }

    fn change() -> CanonicalChange {
        CanonicalChange::new(
            SyncChangeId::from_uuid(Uuid::from_u128(30)),
            SyncDeviceId::from_uuid(Uuid::from_u128(1)),
            1,
            HybridTimestamp::new(TimestampMillis::new(1), 0),
            CausalFrontier::new(),
            SyncEntity::new("persona", Uuid::from_u128(2).to_string()).expect("entity"),
            ChangeOperation::Insert,
            None,
            Some(
                crate::CanonicalPayload::new("persona.snapshot", 1, b"payload".to_vec())
                    .expect("payload"),
            ),
        )
        .expect("change")
    }

    #[test]
    fn current_schema_fingerprint_is_stable_and_complete() {
        assert_eq!(
            current_sync_schema_fingerprint().as_str(),
            "f59d621aa4be626ced608e4f587aa0543d481b12ad0248e7e53e98d456955f17"
        );
    }

    #[test]
    fn hello_rejects_unbounded_text_and_limits() {
        let device = SyncDeviceId::new();
        let session = SyncSessionId::new();
        assert_eq!(
            SyncHello::current(
                " ",
                device,
                "Device",
                session,
                SyncTransferLimits::default()
            ),
            Err(SyncSessionError::InvalidAppVersion)
        );
        assert_eq!(
            SyncHello::current(
                "1.0.0",
                device,
                "Device\nName",
                session,
                SyncTransferLimits::default()
            ),
            Err(SyncSessionError::InvalidDeviceName)
        );
        assert_eq!(
            SyncTransferLimits::new(
                MAX_INCOMING_CHANGES + 1,
                MAX_CANONICAL_PAYLOAD_BYTES,
                MAX_INCOMING_PAYLOAD_BYTES
            ),
            Err(SyncSessionError::InvalidTransferLimits)
        );
    }

    #[test]
    fn negotiation_binds_identity_and_uses_the_smaller_limits() {
        let local = hello(1, 11);
        let peer_device = SyncDeviceId::from_uuid(Uuid::from_u128(2));
        let peer = SyncHello::current(
            "1.2.3",
            peer_device,
            "Peer",
            SyncSessionId::from_uuid(Uuid::from_u128(22)),
            SyncTransferLimits::new(12, 1024, 4096).expect("peer limits"),
        )
        .expect("peer hello");

        let negotiated =
            negotiate_sync_session(&local, &peer, peer_device).expect("negotiated session");
        assert_eq!(negotiated.peer_device_id, peer_device);
        assert_eq!(negotiated.peer_device_name, "Peer");
        assert_eq!(negotiated.limits.max_changes_per_batch(), 12);
        assert_eq!(negotiated.limits.max_change_payload_bytes(), 1024);
        assert_eq!(negotiated.limits.max_batch_payload_bytes(), 4096);
        assert_eq!(
            negotiate_sync_session(&local, &peer, SyncDeviceId::new()),
            Err(SyncSessionError::AuthenticatedIdentityMismatch)
        );
    }

    #[test]
    fn negotiation_requires_exact_compatibility_and_distinct_identities() {
        let local = hello(1, 11);
        let peer_device = SyncDeviceId::from_uuid(Uuid::from_u128(2));
        assert_eq!(
            negotiate_sync_session(&local, &hello(1, 22), local.device_id()),
            Err(SyncSessionError::DuplicateDeviceIdentity)
        );
        assert_eq!(
            negotiate_sync_session(&local, &hello(2, 11), peer_device),
            Err(SyncSessionError::DuplicateSessionIdentity)
        );

        let mut peer = hello(2, 22);
        peer.app_version = "1.2.4".to_string();
        assert_eq!(
            negotiate_sync_session(&local, &peer, peer_device),
            Err(SyncSessionError::AppVersionMismatch)
        );
        peer.app_version = local.app_version.clone();
        peer.protocol_version += 1;
        assert!(matches!(
            negotiate_sync_session(&local, &peer, peer_device),
            Err(SyncSessionError::ProtocolMismatch { .. })
        ));
        peer.protocol_version = local.protocol_version;
        peer.schema_fingerprint = ContentHash::parse("11".repeat(32)).expect("fingerprint");
        assert_eq!(
            negotiate_sync_session(&local, &peer, peer_device),
            Err(SyncSessionError::SchemaMismatch)
        );
    }

    #[test]
    fn change_batches_bind_identity_order_hash_and_negotiated_limits() {
        let batch_id = OperationId::from_uuid(Uuid::from_u128(40));
        let change = change();
        let batch = SyncChangeBatch::new(
            batch_id,
            vec![change.clone()],
            SyncTransferLimits::default(),
        )
        .expect("batch");
        assert_eq!(batch.batch_id(), batch_id);
        assert_eq!(batch.changes(), std::slice::from_ref(&change));
        assert_eq!(
            SyncChangeBatch::from_parts(
                batch_id,
                ContentHash::parse("11".repeat(32)).expect("wrong hash"),
                vec![change.clone()],
                SyncTransferLimits::default()
            ),
            Err(SyncSessionError::InvalidChangeBatch)
        );
        assert_eq!(
            SyncChangeBatch::new(
                batch_id,
                vec![change],
                SyncTransferLimits::new(1, 1, 1).expect("small limits")
            ),
            Err(SyncSessionError::InvalidChangeBatch)
        );
    }
}
