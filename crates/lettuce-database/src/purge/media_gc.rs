//! Reference-counted collection of the media a purge stopped referencing.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use lettuce_media::ReleasedMediaObject;
use lettuce_types::{ContentHash, TimestampMillis};
use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior, params};

use super::{PurgeError, storage, text_columns};
use crate::Database;

/// Tables whose text is bookkeeping about media or history, not a use of it:
/// the media catalog itself, purge and sync journals, legacy import
/// evidence, job logs and provider replay caches. Sync changes still waiting
/// to apply and unfinished jobs are probed separately.
const UNSCANNED_TABLES: [&str; 9] = [
    "media_assets",
    "media_blobs",
    "media_gc_candidates",
    "purge_authorizations",
    "purge_queue",
    "schema_migrations",
    "conversation_replay_artifacts",
    "jobs",
    "job_events",
];
const UNSCANNED_PREFIXES: [&str; 2] = ["sync_", "legacy_import_"];

/// A legacy import only needs its destination assets while it can still
/// attach them.
const OPEN_IMPORT_COMPLETION: &str = "SELECT completion.destination_asset_id
     FROM legacy_import_media_completions completion
     JOIN legacy_import_runs run ON run.id = completion.run_id
     WHERE run.status NOT IN ('completed', 'partial', 'failed')";

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
    let mut probes: Vec<String> = connection
        .prepare(
            "SELECT m.name, f.\"from\" FROM sqlite_schema AS m
             JOIN pragma_foreign_key_list(m.name) AS f
             WHERE m.type = 'table' AND f.\"table\" = 'media_assets'
               AND (f.\"to\" IS NULL OR f.\"to\" = 'id')
             ORDER BY m.name, f.\"from\"",
        )
        .and_then(|mut statement| {
            statement
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .map_err(storage)?
        .into_iter()
        .filter(|(table, _)| table != "legacy_import_media_completions")
        .map(|(table, column)| {
            format!("SELECT \"{column}\" FROM \"{table}\" WHERE \"{column}\" = c.value")
        })
        .collect();
    probes.push(format!(
        "{OPEN_IMPORT_COMPLETION} AND completion.destination_asset_id = c.value"
    ));
    probes.push(
        "SELECT 1 FROM sync_incoming_changes change
         JOIN sync_incoming_batches batch ON batch.batch_id = change.batch_id
         WHERE batch.state <> 'committed'
           AND instr(CAST(change.payload_bytes AS TEXT), c.value) > 0"
            .to_owned(),
    );
    probes.push(
        "SELECT 1 FROM sync_deferred_changes deferred
         JOIN sync_changes change ON change.change_id = deferred.change_id
         WHERE instr(CAST(change.payload_bytes AS TEXT), c.value) > 0"
            .to_owned(),
    );
    for column in text_columns(connection, "jobs")? {
        probes.push(format!(
            "SELECT 1 FROM jobs WHERE state NOT IN {FINISHED_JOB}
               AND instr(CAST(\"{column}\" AS TEXT), c.value) > 0"
        ));
    }
    probes.push(format!(
        "SELECT 1 FROM job_events event JOIN jobs job ON job.id = event.job_id
         WHERE job.state NOT IN {FINISHED_JOB} AND instr(event.event_json, c.value) > 0"
    ));
    let tables: Vec<String> = connection
        .prepare("SELECT name FROM sqlite_schema WHERE type = 'table' ORDER BY name")
        .and_then(|mut statement| statement.query_map([], |row| row.get(0))?.collect())
        .map_err(storage)?;
    for table in tables.into_iter().filter(|table| scanned(table)) {
        for column in text_columns(connection, &table)? {
            probes.push(format!(
                "SELECT 1 FROM \"{table}\" WHERE instr(CAST(\"{column}\" AS TEXT), c.value) > 0"
            ));
        }
    }
    for probe in probes {
        if candidates.is_empty() {
            break;
        }
        let ids = serde_json::to_string(&*candidates).map_err(storage)?;
        let referenced: Vec<String> = connection
            .prepare(&format!(
                "SELECT c.value FROM json_each(?1) AS c WHERE EXISTS ({probe})"
            ))
            .and_then(|mut statement| statement.query_map([ids], |row| row.get(0))?.collect())
            .map_err(storage)?;
        for id in referenced {
            candidates.remove(&id);
        }
    }
    Ok(())
}

fn collect(
    connection: &mut Connection,
    now: TimestampMillis,
) -> Result<Vec<ReleasedMediaObject>, PurgeError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(storage)?;
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
        if let (Some(blob), Some(retention)) = (blob, retention)
            && retention != "library"
        {
            blobs.insert(asset.clone(), blob);
            unused.insert(asset);
        }
    }
    drop_referenced(&transaction, &mut unused)?;
    let mut released_blobs = BTreeSet::new();
    for asset in &unused {
        let evidence: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM legacy_import_media_completions WHERE destination_asset_id = ?1)",
                [asset],
                |row| row.get(0),
            )
            .map_err(storage)?;
        if !evidence {
            transaction
                .execute("DELETE FROM media_assets WHERE id = ?1", [asset])
                .map_err(storage)?;
        }
        if let Some(blob) = blobs.get(asset) {
            released_blobs.insert(blob.clone());
        }
    }
    let unused_json = serde_json::to_string(&unused).map_err(storage)?;
    let mut released = Vec::new();
    for blob in released_blobs {
        let still_used: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM media_assets
                               WHERE blob_id = ?1 AND id NOT IN (SELECT value FROM json_each(?2)))",
                params![blob, unused_json],
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
        let held: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM media_assets WHERE blob_id = ?1)
                     OR EXISTS(SELECT 1 FROM legacy_import_media_completions WHERE blob_id = ?1)",
                [&blob],
                |row| row.get(0),
            )
            .map_err(storage)?;
        if held {
            transaction
                .execute(
                    "UPDATE media_blobs SET state = 'missing', updated_at = ?2 WHERE id = ?1",
                    params![blob, now.get()],
                )
                .map_err(storage)?;
        } else {
            transaction
                .execute("DELETE FROM media_blobs WHERE id = ?1", [&blob])
                .map_err(storage)?;
        }
        if state == "ready" {
            released.push(ReleasedMediaObject {
                content_hash: ContentHash::parse(hash).map_err(storage)?,
                byte_size: u64::try_from(size).map_err(storage)?,
            });
        }
    }
    transaction.commit().map_err(storage)?;
    Ok(released)
}

impl Database {
    /// Collects the assets purges queued: an asset nothing references any
    /// more (no foreign key, and its id in no stored text outside
    /// bookkeeping tables) is deleted unless it is library media, and a
    /// blob none of whose assets is still used leaves the catalog (kept as
    /// `missing` while an asset or import record names it). Returns the
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

    /// The content of every blob another database file catalogs, in any
    /// state, read without writing to that file.
    pub fn media_objects_in_file(path: &Path) -> Result<BTreeSet<ContentHash>, PurgeError> {
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(storage)?;
        let hashes: Vec<String> = connection
            .prepare("SELECT content_hash FROM media_blobs")
            .and_then(|mut statement| statement.query_map([], |row| row.get(0))?.collect())
            .map_err(storage)?;
        hashes
            .into_iter()
            .map(|hash| ContentHash::parse(hash).map_err(storage))
            .collect()
    }
}
