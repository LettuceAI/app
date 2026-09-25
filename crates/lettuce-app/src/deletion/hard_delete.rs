//! Deleting conversations and characters for good, and deleting the media
//! files they leave unused.

use std::collections::BTreeSet;
use std::path::Path;

use lettuce_database::{Database, PurgeError, PurgeReceipt};
use lettuce_media::{
    LocalMediaBlobStore, MediaAssetRepository, MediaBlobRepository, MediaObjectRemoval,
    MediaStoreError,
};
use lettuce_types::{CharacterId, ContentHash, ConversationId, TimestampMillis};

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

/// The media store, the app's database directory and the database file this
/// process uses. Every collection lists the other database files in the
/// directory at that moment (a database a restore kept, or one a restore is
/// writing), and an object any of them catalogs is never deleted.
#[derive(Debug)]
pub struct MediaGarbageScope<'a, BR, AR> {
    pub store: &'a LocalMediaBlobStore<BR, AR>,
    pub location: &'a crate::AppDatabaseLocation,
    pub open_database: &'a Path,
}

impl<BR, AR> MediaGarbageScope<'_, BR, AR> {
    /// The objects the other database files catalog, or `None` when one of
    /// them cannot be read: collection then does nothing this run and the
    /// user gets a notice naming the file.
    fn kept_by_other_databases(
        &self,
        database: &Database,
        now: TimestampMillis,
    ) -> Result<Option<BTreeSet<ContentHash>>, HardDeleteError> {
        let files = self
            .location
            .other_database_files(self.open_database)
            .map_err(|_| HardDeleteError::Purge(PurgeError::Storage))?;
        let mut kept = BTreeSet::new();
        for path in files {
            match Database::media_objects_in_file(&path) {
                Ok(objects) => kept.extend(objects),
                Err(error) => {
                    let name = path
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    tracing::warn!(%error, file = %name, "media collection skipped: a database file cannot be read");
                    database.record_media_collection_skipped(&name, now)?;
                    return Ok(None);
                }
            }
        }
        Ok(Some(kept))
    }
}

/// Deletes a conversation (direct or group) and everything recorded for it,
/// then the media files it alone used. A failed media collection leaves the
/// candidates queued for the next collection.
pub fn delete_conversation<BR, AR>(
    database: &Database,
    media: &MediaGarbageScope<'_, BR, AR>,
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
    media: &MediaGarbageScope<'_, BR, AR>,
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
    media: &MediaGarbageScope<'_, BR, AR>,
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
/// commit, except files another database file still catalogs or a backup
/// holds pinned. A file left behind is for `sweep_orphan_media_files`.
pub fn collect_media_garbage<BR, AR>(
    database: &Database,
    media: &MediaGarbageScope<'_, BR, AR>,
    now: TimestampMillis,
) -> Result<MediaObjectRemoval, HardDeleteError>
where
    BR: MediaBlobRepository,
    AR: MediaAssetRepository,
{
    database.run_queued_purges(now)?;
    let Some(kept) = media.kept_by_other_databases(database, now)? else {
        return Ok(MediaObjectRemoval::default());
    };
    let mut purge_error = None;
    let removal = media
        .store
        .remove_released_objects(|| {
            database
                .collect_media_garbage(now)
                .map(|released| {
                    released
                        .into_iter()
                        .filter(|object| !kept.contains(&object.content_hash))
                        .collect()
                })
                .map_err(|error| {
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

/// Deletes media files in the managed media directory that neither this
/// catalog (a blob in any state) nor another database file names, such as
/// the files of a collection interrupted between its commit and the
/// deletion. Pinned objects and everything outside the media object tree
/// are left alone.
pub fn sweep_orphan_media_files<BR, AR>(
    database: &Database,
    media: &MediaGarbageScope<'_, BR, AR>,
    now: TimestampMillis,
) -> Result<MediaObjectRemoval, HardDeleteError>
where
    BR: MediaBlobRepository,
    AR: MediaAssetRepository,
{
    let Some(kept) = media.kept_by_other_databases(database, now)? else {
        return Ok(MediaObjectRemoval::default());
    };
    let mut purge_error = None;
    media
        .store
        .sweep_orphan_objects(|hash| {
            if kept.contains(hash) {
                return Ok(true);
            }
            database.media_object_retained(hash).map_err(|error| {
                purge_error = Some(error);
                MediaStoreError::CatalogFailure
            })
        })
        .map_err(|error| purge_error.map_or(HardDeleteError::Media(error), HardDeleteError::Purge))
}
