use serde::{Deserialize, Serialize};

use super::batch::{
    MAX_BATCH_BYTES, MAX_REVISION_BYTES, MAX_REVISIONS_PER_BATCH,
};
use super::model::{ChangeRevision, Frontier};
use super::planner::MissingRange;

pub const DEFAULT_BLOB_CHUNK_BYTES: u32 = 256 * 1024;
pub const SYNC_V2_PROTOCOL_VERSION: u32 = 13;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncLimits {
    pub max_revisions_per_batch: u32,
    pub max_revision_bytes: u64,
    pub max_batch_bytes: u64,
    pub blob_chunk_bytes: u32,
}

impl Default for SyncLimits {
    fn default() -> Self {
        Self {
            max_revisions_per_batch: MAX_REVISIONS_PER_BATCH as u32,
            max_revision_bytes: MAX_REVISION_BYTES as u64,
            max_batch_bytes: MAX_BATCH_BYTES as u64,
            blob_chunk_bytes: DEFAULT_BLOB_CHUNK_BYTES,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncHello {
    pub app_version: String,
    pub sync_protocol_version: u32,
    pub schema_fingerprint: String,
    pub device_id: String,
    pub session_id: String,
    pub limits: SyncLimits,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionPlan {
    pub ranges: Vec<MissingRange>,
    pub estimated_revisions: u64,
    pub estimated_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlobDescriptor {
    pub content_hash: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncV2Message {
    Hello(SyncHello),
    Frontier(Frontier),
    Plan(RevisionPlan),
    RevisionBatch {
        batch_id: String,
        batch_hash: String,
        revisions: Vec<ChangeRevision>,
    },
    BatchAck {
        batch_id: String,
        frontier: Frontier,
    },
    BlobInventory {
        blobs: Vec<BlobDescriptor>,
    },
    BlobRequest {
        content_hash: String,
        offset: u64,
    },
    BlobChunk {
        content_hash: String,
        offset: u64,
        bytes: Vec<u8>,
    },
    BlobComplete {
        content_hash: String,
        size_bytes: u64,
    },
    BlobAck {
        content_hash: String,
    },
    Quiescent {
        frontier: Frontier,
    },
    Complete {
        frontier: Frontier,
    },
    Cancel {
        reason: String,
    },
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum HelloError {
    #[error("sync v2 hello identity is incomplete")]
    InvalidIdentity,
    #[error("both peers are using device ID {device_id}; cloned devices need a new identity")]
    DuplicateDeviceIdentity { device_id: String },
    #[error("peer app version {received} does not match {expected}")]
    AppVersionMismatch { expected: String, received: String },
    #[error("peer sync protocol {received} does not match {expected}")]
    ProtocolMismatch { expected: u32, received: u32 },
    #[error("peer database schema does not match this device")]
    SchemaMismatch,
    #[error("peer advertised invalid transfer limits")]
    InvalidLimits,
}

pub fn validate_hello(
    local: &SyncHello,
    peer: &SyncHello,
) -> Result<SyncLimits, HelloError> {
    if local.device_id.is_empty()
        || local.session_id.is_empty()
        || peer.device_id.is_empty()
        || peer.session_id.is_empty()
    {
        return Err(HelloError::InvalidIdentity);
    }
    if local.device_id == peer.device_id {
        return Err(HelloError::DuplicateDeviceIdentity {
            device_id: local.device_id.clone(),
        });
    }
    if local.app_version != peer.app_version {
        return Err(HelloError::AppVersionMismatch {
            expected: local.app_version.clone(),
            received: peer.app_version.clone(),
        });
    }
    if local.sync_protocol_version != peer.sync_protocol_version {
        return Err(HelloError::ProtocolMismatch {
            expected: local.sync_protocol_version,
            received: peer.sync_protocol_version,
        });
    }
    if local.schema_fingerprint != peer.schema_fingerprint {
        return Err(HelloError::SchemaMismatch);
    }
    if !valid_limits(&local.limits) || !valid_limits(&peer.limits) {
        return Err(HelloError::InvalidLimits);
    }
    Ok(SyncLimits {
        max_revisions_per_batch: local
            .limits
            .max_revisions_per_batch
            .min(peer.limits.max_revisions_per_batch),
        max_revision_bytes: local
            .limits
            .max_revision_bytes
            .min(peer.limits.max_revision_bytes),
        max_batch_bytes: local
            .limits
            .max_batch_bytes
            .min(peer.limits.max_batch_bytes),
        blob_chunk_bytes: local
            .limits
            .blob_chunk_bytes
            .min(peer.limits.blob_chunk_bytes),
    })
}

fn valid_limits(limits: &SyncLimits) -> bool {
    limits.max_revisions_per_batch > 0
        && limits.max_revision_bytes > 0
        && limits.max_batch_bytes >= limits.max_revision_bytes
        && limits.blob_chunk_bytes > 0
}

#[cfg(test)]
mod tests {
    use super::{
        validate_hello, HelloError, SyncHello, SyncLimits, SyncV2Message,
        SYNC_V2_PROTOCOL_VERSION,
    };

    fn hello(device_id: &str) -> SyncHello {
        SyncHello {
            app_version: "1.2.3".to_string(),
            sync_protocol_version: SYNC_V2_PROTOCOL_VERSION,
            schema_fingerprint: "schema".to_string(),
            device_id: device_id.to_string(),
            session_id: format!("session-{device_id}"),
            limits: SyncLimits::default(),
        }
    }

    #[test]
    fn protocol_messages_round_trip_without_compatibility_fallbacks() {
        let message = SyncV2Message::Hello(hello("device-a"));

        let encoded = bincode::serialize(&message).unwrap();
        let decoded: SyncV2Message = bincode::deserialize(&encoded).unwrap();
        assert_eq!(decoded, message);
    }

    #[test]
    fn hello_requires_exact_versions_schema_and_distinct_devices() {
        let local = hello("device-a");
        let mut peer = hello("device-b");
        assert_eq!(
            validate_hello(&local, &peer).unwrap(),
            SyncLimits::default()
        );

        peer.schema_fingerprint = "other".to_string();
        assert_eq!(
            validate_hello(&local, &peer),
            Err(HelloError::SchemaMismatch)
        );
        assert!(matches!(
            validate_hello(&local, &hello("device-a")),
            Err(HelloError::DuplicateDeviceIdentity { .. })
        ));
    }
}
