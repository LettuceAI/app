pub use lettuce_media::SyncMediaAsset as CanonicalMediaAsset;
use lettuce_types::{AssetId, ContentHash, OperationId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{CanonicalPayload, ChangeOperation, NewCanonicalChange, SyncChangeError, SyncEntity};

pub const MEDIA_ASSET_SYNC_SCHEMA: &str = "media.asset";
pub const MEDIA_ASSET_SYNC_VERSION: u32 = 1;
pub const MAX_SYNC_MEDIA_ASSETS: usize = 256;
pub const MAX_SYNC_BLOB_CHUNK_BYTES: usize = 1024 * 1024;

const MEDIA_OPERATION_NAMESPACE: Uuid = Uuid::from_u128(0x3e934fc4_8568_56ee_b508_470b653b7542);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncMediaCatalog {
    assets: Vec<CanonicalMediaAsset>,
}

impl SyncMediaCatalog {
    pub fn new(assets: Vec<CanonicalMediaAsset>) -> Result<Self, MediaSyncError> {
        if assets.len() > MAX_SYNC_MEDIA_ASSETS {
            return Err(MediaSyncError::LimitExceeded);
        }
        for asset in &assets {
            asset.validate().map_err(|_| MediaSyncError::InvalidAsset)?;
        }
        let mut identities = assets
            .iter()
            .map(|value| value.asset.id)
            .collect::<Vec<_>>();
        identities.sort_unstable();
        if identities.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(MediaSyncError::InvalidAsset);
        }
        Ok(Self { assets })
    }

    #[must_use]
    pub fn assets(&self) -> &[CanonicalMediaAsset] {
        &self.assets
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncBlobChunk {
    pub content_hash: ContentHash,
    pub offset: u64,
    pub bytes: Vec<u8>,
    pub complete: bool,
}

impl SyncBlobChunk {
    pub fn validate(
        &self,
        expected_hash: &ContentHash,
        expected_offset: u64,
    ) -> Result<(), MediaSyncError> {
        if &self.content_hash != expected_hash
            || self.offset != expected_offset
            || self.bytes.len() > MAX_SYNC_BLOB_CHUNK_BYTES
            || (self.bytes.is_empty() && !self.complete)
        {
            return Err(MediaSyncError::InvalidChunk);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum MediaSyncError {
    #[error("sync media asset is invalid")]
    InvalidAsset,
    #[error("sync media catalog exceeds its limit")]
    LimitExceeded,
    #[error("sync media chunk is invalid")]
    InvalidChunk,
    #[error("sync media payload could not be encoded")]
    Encoding,
    #[error("sync media catalog storage failed")]
    Storage,
}

pub trait PersonaMediaSyncRepository: Send + Sync {
    fn referenced_persona_media(&self) -> Result<Vec<AssetId>, MediaSyncError>;
    fn pending_persona_media(&self) -> Result<Vec<CanonicalMediaAsset>, MediaSyncError>;
}

pub fn canonical_media_asset_payload(
    value: &CanonicalMediaAsset,
) -> Result<CanonicalPayload, MediaSyncError> {
    value.validate().map_err(|_| MediaSyncError::InvalidAsset)?;
    let bytes = serde_json::to_vec(value).map_err(|_| MediaSyncError::Encoding)?;
    CanonicalPayload::new(MEDIA_ASSET_SYNC_SCHEMA, MEDIA_ASSET_SYNC_VERSION, bytes)
        .map_err(|_: SyncChangeError| MediaSyncError::InvalidAsset)
}

pub fn media_asset_sync_entity(id: AssetId) -> Result<SyncEntity, MediaSyncError> {
    SyncEntity::new("media_asset", id.to_string()).map_err(|_| MediaSyncError::InvalidAsset)
}

pub fn media_asset_insert_change(
    value: &CanonicalMediaAsset,
) -> Result<NewCanonicalChange, MediaSyncError> {
    NewCanonicalChange::new(
        media_asset_sync_entity(value.asset.id)?,
        ChangeOperation::Insert,
        None,
        Some(canonical_media_asset_payload(value)?),
    )
    .map_err(|_| MediaSyncError::InvalidAsset)
}

#[must_use]
pub fn media_asset_create_operation(id: AssetId) -> OperationId {
    OperationId::from_uuid(Uuid::new_v5(
        &MEDIA_OPERATION_NAMESPACE,
        format!("create\0{id}").as_bytes(),
    ))
}

#[cfg(test)]
mod tests {
    use lettuce_media::{
        AssetKind, AssetOrigin, AssetProvenanceV1, BlobState, MediaAsset, MediaBlob, MediaKind,
        RetentionClass,
    };
    use lettuce_types::{MediaBlobId, Revision, TimestampMillis};

    use super::*;

    fn snapshot() -> CanonicalMediaAsset {
        let blob_id = MediaBlobId::new();
        CanonicalMediaAsset {
            asset: MediaAsset {
                id: AssetId::new(),
                blob_id,
                kind: AssetKind::AvatarOriginal,
                origin: AssetOrigin::Upload,
                retention: RetentionClass::Persistent,
                provenance: AssetProvenanceV1::default(),
                revision: Revision::INITIAL,
                created_at: TimestampMillis::new(1),
                updated_at: TimestampMillis::new(1),
            },
            blob: MediaBlob {
                id: blob_id,
                content_hash: ContentHash::parse("ab".repeat(32)).expect("hash"),
                kind: MediaKind::Image,
                mime_type: "image/png".into(),
                byte_size: 10,
                width: Some(1),
                height: Some(1),
                duration_ms: None,
                validation_version: 1,
                state: BlobState::Ready,
                created_at: TimestampMillis::new(1),
                updated_at: TimestampMillis::new(1),
            },
        }
    }

    #[test]
    fn media_payload_and_catalog_are_canonical_and_bounded() {
        let value = snapshot();
        let payload = canonical_media_asset_payload(&value).expect("payload");
        assert_eq!(payload.schema(), MEDIA_ASSET_SYNC_SCHEMA);
        assert_eq!(
            serde_json::from_slice::<CanonicalMediaAsset>(payload.bytes()).expect("decode"),
            value
        );
        assert!(SyncMediaCatalog::new(vec![value.clone(), value]).is_err());
    }
}
