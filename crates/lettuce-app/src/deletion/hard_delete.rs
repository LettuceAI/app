//! Deleting conversations and characters for good, and deleting the media
//! files they leave unused.

use lettuce_database::{Database, PurgeError, PurgeReceipt};
use lettuce_media::{
    LocalMediaBlobStore, MediaAssetRepository, MediaBlobRepository, MediaObjectRemoval,
    MediaStoreError,
};
use lettuce_types::{CharacterId, ConversationId, TimestampMillis};

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum HardDeleteError {
    #[error(transparent)]
    Purge(#[from] PurgeError),
    #[error("media collection failed")]
    Media(MediaStoreError),
}

/// What a deletion removed: the rows, and the media files nothing else used.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HardDeletion {
    pub receipt: PurgeReceipt,
    pub media: MediaObjectRemoval,
}

/// Deletes a conversation (direct or group) and everything recorded for it,
/// then the media files it alone used. A failed media collection leaves the
/// candidates queued for the next collection.
pub fn delete_conversation<BR, AR>(
    database: &Database,
    media: &LocalMediaBlobStore<BR, AR>,
    id: ConversationId,
    now: TimestampMillis,
) -> Result<HardDeletion, HardDeleteError>
where
    BR: MediaBlobRepository,
    AR: MediaAssetRepository,
{
    let receipt = database.purge_conversation(id, now)?;
    Ok(HardDeletion {
        receipt,
        media: collect_after_purge(database, media, now),
    })
}

/// Deletes a character with its direct conversations and companion memory,
/// then the media files nothing else uses.
pub fn delete_character<BR, AR>(
    database: &Database,
    media: &LocalMediaBlobStore<BR, AR>,
    id: CharacterId,
    now: TimestampMillis,
) -> Result<HardDeletion, HardDeleteError>
where
    BR: MediaBlobRepository,
    AR: MediaAssetRepository,
{
    let receipt = database.purge_character(id, now)?;
    Ok(HardDeletion {
        receipt,
        media: collect_after_purge(database, media, now),
    })
}

fn collect_after_purge<BR, AR>(
    database: &Database,
    media: &LocalMediaBlobStore<BR, AR>,
    now: TimestampMillis,
) -> MediaObjectRemoval
where
    BR: MediaBlobRepository,
    AR: MediaAssetRepository,
{
    match collect_media_garbage(database, media, now) {
        Ok(removal) => removal,
        Err(error) => {
            tracing::warn!(%error, "media left unused by a deletion stays queued");
            MediaObjectRemoval::default()
        }
    }
}

/// Runs the deletes received through sync, then collects the media every
/// purge so far left unused: catalog rows go first, files after that
/// commit. A file that cannot be deleted stays for `sweep_orphan_media_files`.
pub fn collect_media_garbage<BR, AR>(
    database: &Database,
    media: &LocalMediaBlobStore<BR, AR>,
    now: TimestampMillis,
) -> Result<MediaObjectRemoval, HardDeleteError>
where
    BR: MediaBlobRepository,
    AR: MediaAssetRepository,
{
    database.run_queued_purges(now)?;
    let mut purge_error = None;
    let removal = media
        .remove_released_objects(|| {
            database.collect_media_garbage(now).map_err(|error| {
                purge_error = Some(error);
                MediaStoreError::CatalogFailure
            })
        })
        .map_err(|error| {
            purge_error.map_or(HardDeleteError::Media(error), HardDeleteError::Purge)
        })?;
    if removal.failed > 0 {
        tracing::warn!(
            failed = removal.failed,
            "some unused media files were not deleted"
        );
    }
    Ok(removal)
}

/// Deletes media files in the managed media directory that no catalog row
/// keeps, such as the files of a collection interrupted between its commit
/// and the deletion. Nothing outside the media object tree is touched.
pub fn sweep_orphan_media_files<BR, AR>(
    database: &Database,
    media: &LocalMediaBlobStore<BR, AR>,
) -> Result<MediaObjectRemoval, HardDeleteError>
where
    BR: MediaBlobRepository,
    AR: MediaAssetRepository,
{
    let mut purge_error = None;
    media
        .sweep_orphan_objects(|hash| {
            database.media_object_retained(hash).map_err(|error| {
                purge_error = Some(error);
                MediaStoreError::CatalogFailure
            })
        })
        .map_err(|error| purge_error.map_or(HardDeleteError::Media(error), HardDeleteError::Purge))
}
