use lettuce_media::{
    MediaAsset, MediaAssetRepository, MediaAssetRepositoryError, MediaBlob, MediaBlobRepository,
    MediaBlobRepositoryError, RetentionClass,
};
use lettuce_types::{
    AssetId, ContentHash, MediaBlobId, Page, PageRequest, Revision, TimestampMillis,
};

use crate::Database;

impl MediaBlobRepository for &Database {
    fn register(&self, blob: MediaBlob) -> Result<MediaBlob, MediaBlobRepositoryError> {
        MediaBlobRepository::register(*self, blob)
    }
    fn finalize_staged_to_ready(
        &self,
        id: MediaBlobId,
        at: TimestampMillis,
    ) -> Result<MediaBlob, MediaBlobRepositoryError> {
        MediaBlobRepository::finalize_staged_to_ready(*self, id, at)
    }
    fn restore_missing_to_ready(
        &self,
        id: MediaBlobId,
        at: TimestampMillis,
    ) -> Result<MediaBlob, MediaBlobRepositoryError> {
        MediaBlobRepository::restore_missing_to_ready(*self, id, at)
    }
    fn get(&self, id: MediaBlobId) -> Result<Option<MediaBlob>, MediaBlobRepositoryError> {
        MediaBlobRepository::get(*self, id)
    }
    fn find_by_hash(
        &self,
        hash: &ContentHash,
    ) -> Result<Option<MediaBlob>, MediaBlobRepositoryError> {
        MediaBlobRepository::find_by_hash(*self, hash)
    }
}

impl MediaAssetRepository for &Database {
    fn create(&self, asset: MediaAsset) -> Result<MediaAsset, MediaAssetRepositoryError> {
        MediaAssetRepository::create(*self, asset)
    }
    fn get(&self, id: AssetId) -> Result<Option<MediaAsset>, MediaAssetRepositoryError> {
        MediaAssetRepository::get(*self, id)
    }
    fn update_retention(
        &self,
        id: AssetId,
        revision: Revision,
        retention: RetentionClass,
        at: TimestampMillis,
    ) -> Result<MediaAsset, MediaAssetRepositoryError> {
        MediaAssetRepository::update_retention(*self, id, revision, retention, at)
    }
    fn list_library(
        &self,
        request: PageRequest,
    ) -> Result<Page<MediaAsset>, MediaAssetRepositoryError> {
        MediaAssetRepository::list_library(*self, request)
    }
}
