use lettuce_media::{BlobState, IngestedMedia, MediaStoreError};
use lettuce_types::AssetId;

impl crate::ApiOperationTransaction<'_, '_> {
    pub fn require_library_image(&self, id: AssetId) -> Result<(), MediaStoreError> {
        use rusqlite::OptionalExtension;
        let row: Option<(String, String)> = self.transaction.query_row(
            "SELECT asset.blob_kind,blob.state FROM media_assets asset JOIN media_blobs blob ON blob.id=asset.blob_id WHERE asset.id=?1",
            [id.to_string()], |row| Ok((row.get(0)?, row.get(1)?)))
            .optional().map_err(|error| { tracing::warn!(%error, "require library image failed"); MediaStoreError::RepositoryData { reason: "require library image: a value could not be read or converted" } })?;
        match row {
            None => Err(MediaStoreError::AssetNotFound),
            Some((kind, _)) if kind != "image" => Err(MediaStoreError::KindMismatch),
            Some((_, state)) if state != "ready" => Err(MediaStoreError::NotReady),
            Some(_) => Ok(()),
        }
    }

    pub fn install_library_upload(
        &self,
        mut upload: IngestedMedia,
    ) -> Result<AssetId, MediaStoreError> {
        if upload.blob.state != BlobState::Ready || upload.asset.blob_id != upload.blob.id {
            return Err(MediaStoreError::InvalidMetadata {
                reason: "install library upload: a value failed validation",
            });
        }
        upload.blob.state = BlobState::Staged;
        let mut blob =
            crate::register_media_blob_in(self.transaction, upload.blob).map_err(|error| {
                tracing::warn!(%error, "install library upload failed");
                MediaStoreError::RepositoryData {
                    reason: "install library upload: a value could not be read or converted",
                }
            })?;
        match blob.state {
            BlobState::Ready => {}
            BlobState::Staged | BlobState::Missing => {
                blob.updated_at = blob.updated_at.max(upload.asset.created_at);
                self.transaction
                    .execute(
                        "UPDATE media_blobs SET state='ready',updated_at=?2 WHERE id=?1",
                        rusqlite::params![blob.id.to_string(), blob.updated_at.get()],
                    )
                    .map_err(|error| {
                        tracing::warn!(%error, "install library upload failed");
                        MediaStoreError::CatalogFailure {
                            reason: "install library upload: a storage step failed",
                        }
                    })?;
            }
            BlobState::Quarantined => return Err(MediaStoreError::NotReady),
        }
        upload.asset.blob_id = blob.id;
        crate::create_media_asset_in(self.transaction, upload.asset)
            .map(|asset| asset.id)
            .map_err(|error| {
                tracing::warn!(%error, "install library upload failed");
                MediaStoreError::CatalogFailure {
                    reason: "install library upload: a storage step failed",
                }
            })
    }
}
