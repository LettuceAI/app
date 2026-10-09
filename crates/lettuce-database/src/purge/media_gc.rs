//! Reference-counted collection of the media a purge stopped referencing.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use lettuce_media::ReleasedMediaObject;
use lettuce_types::{ContentHash, TimestampMillis};
use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior};

use super::{PurgeError, storage};

#[path = "media_reference_probes.rs"]
mod references;
use crate::Database;

/// Tables whose text is bookkeeping about media or history, not a use of it:
/// the media catalog itself, purge and sync journals, legacy import
/// evidence, job logs and provider replay caches. Sync changes still waiting
/// to apply, conflict evidence (a user may choose its side) and unfinished
/// jobs are probed separately.
const UNSCANNED_TABLES: [&str; 10] = [
    "media_assets",
    "media_blobs",
    "media_gc_candidates",
    "purge_authorizations",
    "purge_queue",
    "schema_migrations",
    "conversation_replay_artifacts",
    "jobs",
    "job_events",
    "api_operation_receipts",
];
const UNSCANNED_PREFIXES: [&str; 2] = ["sync_", "legacy_import_"];

const FINISHED_JOB: &str = "('succeeded', 'failed', 'cancelled', 'interrupted')";

fn scanned(table: &str) -> bool {
    !table.starts_with("sqlite_")
        && !UNSCANNED_TABLES.contains(&table)
        && !UNSCANNED_PREFIXES
            .iter()
            .any(|prefix| table.starts_with(prefix))
}

/// Removes from `candidates` every asset something still uses: a foreign
/// key, or its id anywhere in the text of a table that is not bookkeeping.
fn drop_referenced(
    connection: &Connection,
    candidates: &mut BTreeSet<String>,
) -> Result<(), PurgeError> {
    let probes = references::probes(connection)?;
    for probe in probes {
        if candidates.is_empty() {
            break;
        }
        let ids = serde_json::to_string(&*candidates).map_err(storage)?;
        let referenced: Vec<String> = connection
            .prepare(&format!(
                "SELECT c.value FROM json_each(?1) AS c WHERE EXISTS ({})",
                probe.sql
            ))
            .and_then(|mut statement| statement.query_map([ids], |row| row.get(0))?.collect())
            .map_err(storage)?;
        for id in referenced {
            candidates.remove(&id);
        }
    }
    Ok(())
}

fn drop_retained(
    connection: &Connection,
    candidates: &mut BTreeSet<String>,
) -> Result<(), PurgeError> {
    drop_referenced(connection, candidates)?;
    let ids = serde_json::to_string(&*candidates).map_err(storage)?;
    let library: Vec<String> = connection.prepare("SELECT id FROM media_assets WHERE retention='library' AND id IN (SELECT value FROM json_each(?1))")
        .and_then(|mut statement| statement.query_map([ids], |row| row.get(0))?.collect()).map_err(storage)?;
    for id in library {
        candidates.remove(&id);
    }
    Ok(())
}

fn retained_objects(connection: &Connection) -> Result<BTreeSet<ContentHash>, PurgeError> {
    let assets: BTreeSet<String> = connection
        .prepare("SELECT id FROM media_assets")
        .and_then(|mut statement| statement.query_map([], |row| row.get(0))?.collect())
        .map_err(storage)?;
    let mut garbage = assets.clone();
    drop_retained(connection, &mut garbage)?;
    let live = assets.difference(&garbage).collect::<Vec<_>>();
    let ids = serde_json::to_string(&live).map_err(storage)?;
    let hashes: Vec<String> = connection.prepare("SELECT DISTINCT blob.content_hash FROM media_blobs blob JOIN media_assets asset ON asset.blob_id=blob.id WHERE asset.id IN (SELECT value FROM json_each(?1))")
        .and_then(|mut statement| statement.query_map([ids], |row| row.get(0))?.collect()).map_err(storage)?;
    hashes
        .into_iter()
        .map(|hash| ContentHash::parse(hash).map_err(storage))
        .collect()
}

fn collect(
    connection: &mut Connection,
    now: TimestampMillis,
) -> Result<Vec<ReleasedMediaObject>, PurgeError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(storage)?;
    let released = collect_in(&transaction, now)?;
    transaction.commit().map_err(storage)?;
    Ok(released)
}

fn collect_in(
    transaction: &Connection,
    now: TimestampMillis,
) -> Result<Vec<ReleasedMediaObject>, PurgeError> {
    transaction.execute("INSERT OR IGNORE INTO media_gc_candidates(asset_id,queued_at) SELECT id,?1 FROM media_assets WHERE retention='temporary' AND expires_at<=?1", [now.get()]).map_err(storage)?;
    let queued: Vec<(String, Option<String>, Option<String>)> = transaction
        .prepare(
            "SELECT candidate.asset_id, asset.blob_id, asset.retention
             FROM media_gc_candidates candidate
             LEFT JOIN media_assets asset ON asset.id = candidate.asset_id
             ORDER BY candidate.asset_id",
        )
        .and_then(|mut statement| {
            statement
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
                .collect()
        })
        .map_err(storage)?;
    transaction
        .execute("DELETE FROM media_gc_candidates", [])
        .map_err(storage)?;
    let mut blobs = BTreeMap::new();
    let mut unused = BTreeSet::new();
    for (asset, blob, retention) in queued {
        if let (Some(blob), Some(_)) = (blob, retention) {
            blobs.insert(asset.clone(), blob);
            unused.insert(asset);
        }
    }
    drop_retained(transaction, &mut unused)?;
    let mut released_blobs = BTreeSet::new();
    for asset in &unused {
        transaction
            .execute("DELETE FROM media_assets WHERE id = ?1", [asset])
            .map_err(storage)?;
        if let Some(blob) = blobs.get(asset) {
            released_blobs.insert(blob.clone());
        }
    }
    let mut released = Vec::new();
    for blob in released_blobs {
        let still_used: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM media_assets WHERE blob_id = ?1)",
                [&blob],
                |row| row.get(0),
            )
            .map_err(storage)?;
        if still_used {
            continue;
        }
        let Some((hash, size, state)) = transaction
            .query_row(
                "SELECT content_hash, byte_size, state FROM media_blobs WHERE id = ?1",
                [&blob],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(storage)?
        else {
            continue;
        };
        transaction
            .execute("DELETE FROM media_blobs WHERE id = ?1", [&blob])
            .map_err(storage)?;
        if state == "ready" {
            released.push(ReleasedMediaObject {
                content_hash: ContentHash::parse(hash).map_err(storage)?,
                byte_size: u64::try_from(size).map_err(storage)?,
            });
        }
    }
    Ok(released)
}

impl Database {
    pub fn media_library_page(
        &self,
        role: Option<lettuce_media::MediaKind>,
        request: lettuce_types::PageRequest,
    ) -> Result<
        lettuce_types::Page<lettuce_media::MediaLibraryEntry>,
        lettuce_media::MediaLibraryError,
    > {
        use lettuce_media::{MediaKind, MediaLibraryEntry, MediaLibraryError};
        if role.is_some_and(|kind| !matches!(kind, MediaKind::Image | MediaKind::Audio)) {
            return Err(MediaLibraryError::InvalidData);
        }
        let cursor = request
            .cursor
            .as_deref()
            .map(|value| {
                let bytes =
                    crate::hex_decode(value).map_err(|_| MediaLibraryError::InvalidCursor)?;
                let cursor: MediaLibraryCursor =
                    serde_json::from_slice(&bytes).map_err(|_| MediaLibraryError::InvalidCursor)?;
                if cursor.version != 1
                    || cursor.role != role
                    || cursor.id.parse::<lettuce_types::AssetId>().is_err()
                {
                    return Err(MediaLibraryError::InvalidCursor);
                }
                Ok(cursor)
            })
            .transpose()?;
        let role_name = role.map(|role| match role {
            MediaKind::Image => "image",
            MediaKind::Audio => "audio",
            _ => unreachable!(),
        });
        let limit = usize::from(lettuce_types::PageLimit::new(request.limit.get()).get());
        let mut connection = self.connection().map_err(|_| MediaLibraryError::Storage)?;
        let transaction = connection
            .transaction()
            .map_err(|_| MediaLibraryError::Storage)?;
        let ids: Vec<String> = transaction.prepare("SELECT id FROM media_assets WHERE blob_kind IN ('image','audio') AND (?1 IS NULL OR blob_kind=?1) AND (?2 IS NULL OR updated_at<?2 OR (updated_at=?2 AND id>?3)) ORDER BY updated_at DESC,id ASC LIMIT ?4")
            .and_then(|mut statement| statement.query_map(rusqlite::params![role_name,cursor.as_ref().map(|cursor| cursor.updated_at),cursor.as_ref().map(|cursor| cursor.id.as_str()),i64::try_from(limit+1).map_err(|_| rusqlite::Error::InvalidQuery)?], |row| row.get(0))?.collect()).map_err(library_sql_error)?;
        let has_more = ids.len() > limit;
        let probes = references::probes(&transaction).map_err(media_library_error)?;
        let mut items = Vec::with_capacity(limit);
        for id in ids.into_iter().take(limit) {
            let asset_id = id.parse().map_err(|_| MediaLibraryError::InvalidData)?;
            let asset = crate::load_asset_with_blob(&transaction, asset_id)
                .map_err(library_sql_error)?
                .ok_or(MediaLibraryError::InvalidData)?;
            let blob = transaction
                .query_row(
                    &format!(
                        "SELECT {} FROM media_blobs WHERE id=?1",
                        crate::MEDIA_BLOB_COLUMNS
                    ),
                    [asset.blob_id.to_string()],
                    crate::media_from_row,
                )
                .map_err(library_sql_error)?;
            let owners = references::owners_using(&transaction, &id, &probes)
                .map_err(media_library_error)?;
            items.push(MediaLibraryEntry {
                asset,
                blob,
                references: owners,
            });
        }
        let next_cursor = if has_more {
            let last = items.last().ok_or(MediaLibraryError::InvalidData)?;
            Some(crate::hex_encode(
                &serde_json::to_vec(&MediaLibraryCursor {
                    version: 1,
                    role,
                    updated_at: last.asset.updated_at.get(),
                    id: last.asset.id.to_string(),
                })
                .map_err(|_| MediaLibraryError::InvalidData)?,
            ))
        } else {
            None
        };
        transaction
            .commit()
            .map_err(|_| MediaLibraryError::Storage)?;
        Ok(lettuce_types::Page { items, next_cursor })
    }

    pub fn media_references(
        &self,
        id: lettuce_types::AssetId,
    ) -> Result<Vec<lettuce_media::MediaReference>, lettuce_media::MediaLibraryError> {
        let mut connection = self
            .connection()
            .map_err(|_| lettuce_media::MediaLibraryError::Storage)?;
        let transaction = connection
            .transaction()
            .map_err(|_| lettuce_media::MediaLibraryError::Storage)?;
        let exists: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM media_assets WHERE id=?1)",
                [id.to_string()],
                |row| row.get(0),
            )
            .map_err(|_| lettuce_media::MediaLibraryError::Storage)?;
        if !exists {
            return Err(lettuce_media::MediaLibraryError::NotFound);
        }
        references::owners(&transaction, &id.to_string()).map_err(media_library_error)
    }

    pub fn remove_media_library_asset(
        &self,
        id: lettuce_types::AssetId,
        key: lettuce_types::RequestId,
        digest: &str,
        now: TimestampMillis,
    ) -> Result<Vec<ReleasedMediaObject>, lettuce_media::MediaLibraryError> {
        self.commit_api_operation(
            "media_library_remove",
            &key.to_string(),
            digest,
            now,
            |operation| operation.remove_media_library_asset(id, now),
        )
    }

    pub(crate) fn unreferenced_temporary_assets(
        connection: &Connection,
    ) -> Result<BTreeSet<String>, PurgeError> {
        let mut unused = connection
            .prepare("SELECT id FROM media_assets WHERE retention='temporary'")
            .and_then(|mut statement| {
                statement
                    .query_map([], |row| row.get(0))?
                    .collect::<rusqlite::Result<BTreeSet<String>>>()
            })
            .map_err(storage)?;
        drop_referenced(connection, &mut unused)?;
        Ok(unused)
    }

    /// Collects the assets purges queued: an asset nothing references any
    /// more (no foreign key, and its id in no stored text outside
    /// bookkeeping tables) is deleted unless it is library media, and a
    /// blob none of whose assets is still used leaves the catalog. Returns the
    /// released objects whose bytes the caller deletes after this commit.
    pub fn collect_media_garbage(
        &self,
        now: TimestampMillis,
    ) -> Result<Vec<ReleasedMediaObject>, PurgeError> {
        let mut connection = self.connection().map_err(storage)?;
        collect(&mut connection, now)
    }

    /// Whether the catalog has a blob with this content in any state; a
    /// `missing` blob whose file is still present becomes ready again when
    /// the same bytes are ingested, so its file is kept.
    pub fn media_object_retained(&self, content_hash: &ContentHash) -> Result<bool, PurgeError> {
        self.connection()
            .map_err(storage)?
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM media_blobs WHERE content_hash = ?1)",
                [content_hash.as_str()],
                |row| row.get(0),
            )
            .map_err(storage)
    }

    pub fn media_objects_in_file(path: &Path) -> Result<BTreeSet<ContentHash>, PurgeError> {
        let mut connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(storage)?;
        let transaction = connection.transaction().map_err(storage)?;
        retained_objects(&transaction)
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct MediaLibraryCursor {
    version: u32,
    role: Option<lettuce_media::MediaKind>,
    updated_at: i64,
    id: String,
}

fn library_sql_error(error: rusqlite::Error) -> lettuce_media::MediaLibraryError {
    match error {
        rusqlite::Error::InvalidQuery => lettuce_media::MediaLibraryError::InvalidData,
        _ => lettuce_media::MediaLibraryError::Storage,
    }
}

impl lettuce_media::MediaLibraryRepository for Database {
    fn library_page(
        &self,
        role: Option<lettuce_media::MediaKind>,
        request: lettuce_types::PageRequest,
    ) -> Result<
        lettuce_types::Page<lettuce_media::MediaLibraryEntry>,
        lettuce_media::MediaLibraryError,
    > {
        self.media_library_page(role, request)
    }

    fn retaining_references(
        &self,
        asset: lettuce_types::AssetId,
    ) -> Result<Vec<lettuce_media::MediaReference>, lettuce_media::MediaLibraryError> {
        self.media_references(asset)
    }

    fn remove_library_asset(
        &self,
        asset: lettuce_types::AssetId,
        key: lettuce_types::RequestId,
        digest: &str,
        now: TimestampMillis,
    ) -> Result<Vec<ReleasedMediaObject>, lettuce_media::MediaLibraryError> {
        self.remove_media_library_asset(asset, key, digest, now)
    }
}

fn media_library_error(error: PurgeError) -> lettuce_media::MediaLibraryError {
    match error {
        PurgeError::Storage => lettuce_media::MediaLibraryError::Storage,
        _ => lettuce_media::MediaLibraryError::InvalidData,
    }
}

impl From<crate::ApiOperationError> for lettuce_media::MediaLibraryError {
    fn from(error: crate::ApiOperationError) -> Self {
        match error {
            crate::ApiOperationError::Conflict => Self::Conflict,
            crate::ApiOperationError::InvalidData => Self::InvalidData,
            crate::ApiOperationError::Storage => Self::Storage,
        }
    }
}

impl crate::ApiOperationTransaction<'_, '_> {
    pub fn remove_media_library_asset(
        &self,
        id: lettuce_types::AssetId,
        now: TimestampMillis,
    ) -> Result<Vec<ReleasedMediaObject>, lettuce_media::MediaLibraryError> {
        use lettuce_media::MediaLibraryError;
        let asset = id.to_string();
        let exists: bool = self
            .transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM media_assets WHERE id=?1)",
                [&asset],
                |row| row.get(0),
            )
            .map_err(|_| MediaLibraryError::Storage)?;
        if !exists {
            return Err(MediaLibraryError::NotFound);
        }
        let owners = references::owners(self.transaction, &asset).map_err(media_library_error)?;
        if !owners.is_empty() {
            return Err(MediaLibraryError::InUse(owners));
        }
        self.transaction.execute("UPDATE media_assets SET retention='persistent',expires_at=NULL,revision=revision+1,updated_at=?2 WHERE id=?1", rusqlite::params![asset,now.get()]).map_err(|_| MediaLibraryError::Storage)?;
        self.transaction
            .execute(
                "INSERT OR IGNORE INTO media_gc_candidates(asset_id,queued_at) VALUES (?1,?2)",
                rusqlite::params![asset, now.get()],
            )
            .map_err(|_| MediaLibraryError::Storage)?;
        collect_in(self.transaction, now).map_err(media_library_error)
    }
}

#[cfg(test)]
#[path = "media_gc_tests.rs"]
mod tests;
