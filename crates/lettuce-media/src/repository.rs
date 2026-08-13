//! Synchronous ports for logical asset persistence.

use lettuce_types::{AssetId, Page, PageRequest, Revision, TimestampMillis};

use crate::{AssetState, MediaAsset, RetentionClass};

/// Errors exposed by a logical asset adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum MediaAssetRepositoryError {
    #[error("media asset was not found")]
    NotFound,
    #[error("a media asset with this identity already exists")]
    AlreadyExists,
    #[error("the media asset update has a stale revision")]
    StaleRevision,
    #[error("media asset data are invalid")]
    InvalidData,
    #[error("the referenced media blob was not found")]
    BlobMissing,
    #[error("media asset storage failed")]
    Storage,
}

/// Persistence port for logical media records.
pub trait MediaAssetRepository: Send + Sync {
    /// Creates an asset. The adapter must verify that `blob_id` exists and
    /// that its physical kind matches `asset.kind` before committing.
    fn create(&self, asset: MediaAsset) -> Result<MediaAsset, MediaAssetRepositoryError>;

    fn get(&self, id: AssetId) -> Result<Option<MediaAsset>, MediaAssetRepositoryError>;

    /// Changes only retention, using the supplied revision as a CAS token.
    /// A successful update increments the stored revision exactly once.
    fn update_retention(
        &self,
        id: AssetId,
        expected_revision: Revision,
        retention: RetentionClass,
        updated_at: TimestampMillis,
    ) -> Result<MediaAsset, MediaAssetRepositoryError>;

    /// Marks an asset missing or corrupt, using the supplied revision as a CAS
    /// token. Adapters must reject every other state for this operation.
    fn mark_missing_or_corrupt(
        &self,
        id: AssetId,
        expected_revision: Revision,
        state: AssetState,
        updated_at: TimestampMillis,
    ) -> Result<MediaAsset, MediaAssetRepositoryError>;

    /// Lists only assets with `RetentionClass::Library`; cursor semantics are
    /// adapter-owned and must remain opaque to callers.
    fn list_library(
        &self,
        request: PageRequest,
    ) -> Result<Page<MediaAsset>, MediaAssetRepositoryError>;
}
