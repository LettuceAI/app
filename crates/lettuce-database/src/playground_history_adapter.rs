use std::str::FromStr;

use lettuce_image_generation::{
    ImageGenerationRecord, ImageGenerationState, PlaygroundHistoryEntry, PlaygroundHistoryError,
    PlaygroundHistoryImage, PlaygroundHistoryRepository, PlaygroundOrigin,
};
use lettuce_transfer::PlaygroundHistoryBackup;
use lettuce_types::{AssetId, JobId, TimestampMillis};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use crate::Database;
use crate::legacy_import_backup_adapter::{insert_rows, read_rows};

const ENTRY_COLUMNS: &[&str] = &[
    "id",
    "origin",
    "job_id",
    "import_run_id",
    "source_id",
    "created_at",
    "provider_kind",
    "source_model_id",
    "model_profile_id",
    "model_name",
    "prompt",
    "negative_prompt",
    "seed",
    "params_json",
    "status",
    "error",
];

const IMAGE_COLUMNS: &[&str] = &[
    "history_id",
    "ordinal",
    "asset_id",
    "source_asset_id",
    "mime_type",
    "url",
    "width",
    "height",
];

pub(crate) fn read_in(transaction: &Transaction<'_>) -> rusqlite::Result<PlaygroundHistoryBackup> {
    Ok(PlaygroundHistoryBackup {
        entries: read_rows(transaction, "playground_history", ENTRY_COLUMNS, None)?,
        images: read_rows(
            transaction,
            "playground_history_images",
            IMAGE_COLUMNS,
            None,
        )?,
    })
}

pub(crate) fn insert_restored_in(
    transaction: &Transaction<'_>,
    backup: &PlaygroundHistoryBackup,
) -> rusqlite::Result<()> {
    insert_rows(
        transaction,
        "playground_history",
        ENTRY_COLUMNS,
        &backup.entries,
    )?;
    insert_rows(
        transaction,
        "playground_history_images",
        IMAGE_COLUMNS,
        &backup.images,
    )
}

/// A playground generation enters the history when it is admitted, pending,
/// with its model as it was then.
pub(crate) fn record_admission_in(
    transaction: &Transaction<'_>,
    record: &ImageGenerationRecord,
) -> rusqlite::Result<()> {
    let request = &record.request;
    let (provider_kind, model_name) = transaction
        .query_row(
            "SELECT account.provider_kind, model.display_name
               FROM model_profiles model
               JOIN provider_accounts account ON account.id = model.provider_account_id
              WHERE model.id = ?1",
            [request.model_profile_id.to_string()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?
        .unwrap_or_default();
    let params_json = serde_json::json!({
        "format": "generated-v1",
        "settings": request.settings,
        "loras": request.loras,
        "size": request.size,
        "quality": request.quality,
        "style": request.style,
        "count": request.count,
        "inputImages": request.input_images,
        "maskImage": request.mask_image,
    })
    .to_string();
    transaction.execute(
        "INSERT INTO playground_history (
            id, origin, job_id, created_at, provider_kind, model_profile_id, model_name, prompt,
            negative_prompt, seed, params_json, status
         ) VALUES (?1, 'generated', ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'pending')",
        params![
            record.job_id.to_string(),
            request.created_at.get(),
            provider_kind,
            request.model_profile_id.to_string(),
            model_name,
            request.prompt,
            request.settings.negative_prompt,
            request.settings.seed.map(i64::from),
            params_json,
        ],
    )?;
    Ok(())
}

/// A settled playground generation records its outcome and images.
pub(crate) fn record_settlement_in(
    transaction: &Transaction<'_>,
    job_id: JobId,
    state: &ImageGenerationState,
) -> rusqlite::Result<()> {
    let (status, error) = match state {
        ImageGenerationState::Pending => return Ok(()),
        ImageGenerationState::Succeeded { .. } => ("complete", None),
        ImageGenerationState::Failed { message, .. } => ("failed", Some(message.as_str())),
        ImageGenerationState::Cancelled { .. } => ("cancelled", None),
    };
    let changed = transaction.execute(
        "UPDATE playground_history SET status = ?2, error = ?3 WHERE job_id = ?1",
        params![job_id.to_string(), status, error],
    )?;
    if changed == 1
        && let ImageGenerationState::Succeeded { result } = state
    {
        for (ordinal, image) in result.images.iter().enumerate() {
            transaction.execute(
                "INSERT INTO playground_history_images (
                    history_id, ordinal, asset_id, mime_type, width, height
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    job_id.to_string(),
                    i64::try_from(ordinal).map_err(|_| rusqlite::Error::InvalidQuery)?,
                    image.asset_id.to_string(),
                    image.mime_type,
                    image.width,
                    image.height,
                ],
            )?;
        }
    }
    Ok(())
}

fn storage(_: rusqlite::Error) -> PlaygroundHistoryError {
    PlaygroundHistoryError::Storage
}

fn images_in(
    transaction: &Transaction<'_>,
    id: &str,
) -> rusqlite::Result<Vec<PlaygroundHistoryImage>> {
    let mut statement = transaction.prepare(
        "SELECT asset_id, source_asset_id, mime_type, url, width, height
           FROM playground_history_images WHERE history_id = ?1 ORDER BY ordinal",
    )?;
    statement
        .query_map([id], |row| {
            Ok(PlaygroundHistoryImage {
                asset_id: row
                    .get::<_, Option<String>>(0)?
                    .map(|value| AssetId::from_str(&value))
                    .transpose()
                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                source_asset_id: row.get(1)?,
                mime_type: row.get(2)?,
                url: row.get(3)?,
                width: row.get(4)?,
                height: row.get(5)?,
            })
        })?
        .collect()
}

impl PlaygroundHistoryRepository for Database {
    fn list_playground_history(
        &self,
        limit: u32,
        before: Option<TimestampMillis>,
    ) -> Result<Vec<PlaygroundHistoryEntry>, PlaygroundHistoryError> {
        let mut connection = self
            .connection()
            .map_err(|_| PlaygroundHistoryError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(storage)?;
        let entries = {
            let mut statement = transaction
                .prepare(
                    "SELECT id, origin, job_id, created_at, provider_kind, model_profile_id,
                            model_name, prompt, negative_prompt, seed, params_json, status, error,
                            source_model_id
                       FROM playground_history
                      WHERE ?1 IS NULL OR created_at < ?1
                      ORDER BY created_at DESC, id DESC
                      LIMIT ?2",
                )
                .map_err(storage)?;
            statement
                .query_map(
                    params![before.map(TimestampMillis::get), i64::from(limit)],
                    |row| {
                        Ok(PlaygroundHistoryEntry {
                            id: row.get(0)?,
                            origin: match row.get::<_, String>(1)?.as_str() {
                                "generated" => PlaygroundOrigin::Generated,
                                _ => PlaygroundOrigin::Imported,
                            },
                            job_id: row
                                .get::<_, Option<String>>(2)?
                                .map(|value| JobId::from_str(&value))
                                .transpose()
                                .map_err(|_| rusqlite::Error::InvalidQuery)?,
                            created_at: TimestampMillis::new(row.get(3)?),
                            provider_kind: row.get(4)?,
                            source_model_id: row.get(13)?,
                            model_profile_id: row
                                .get::<_, Option<String>>(5)?
                                .map(|value| value.parse())
                                .transpose()
                                .map_err(|_| rusqlite::Error::InvalidQuery)?,
                            model_name: row.get(6)?,
                            prompt: row.get(7)?,
                            negative_prompt: row.get(8)?,
                            seed: row.get(9)?,
                            params_json: row.get(10)?,
                            status: row.get(11)?,
                            error: row.get(12)?,
                            images: Vec::new(),
                        })
                    },
                )
                .and_then(Iterator::collect::<rusqlite::Result<Vec<_>>>)
                .map_err(storage)?
        };
        let entries = entries
            .into_iter()
            .map(|mut entry| {
                entry.images = images_in(&transaction, &entry.id)?;
                Ok(entry)
            })
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(storage)?;
        transaction.commit().map_err(storage)?;
        Ok(entries)
    }

    fn delete_playground_history(
        &self,
        id: &str,
        delete_images: bool,
    ) -> Result<Vec<AssetId>, PlaygroundHistoryError> {
        let mut connection = self
            .connection()
            .map_err(|_| PlaygroundHistoryError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let job_id = transaction
            .query_row(
                "SELECT job_id FROM playground_history WHERE id = ?1",
                [id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()
            .map_err(storage)?;
        let Some(job_id) = job_id else {
            transaction.commit().map_err(storage)?;
            return Ok(Vec::new());
        };
        let assets = images_in(&transaction, id)
            .map_err(storage)?
            .into_iter()
            .filter_map(|image| image.asset_id)
            .collect::<Vec<_>>();
        transaction
            .execute("DELETE FROM playground_history WHERE id = ?1", [id])
            .map_err(storage)?;
        let mut deleted = Vec::new();
        if delete_images {
            if let Some(job_id) = job_id {
                transaction
                    .execute(
                        "DELETE FROM image_generations WHERE job_id = ?1 AND state != 'pending'",
                        [&job_id],
                    )
                    .map_err(storage)?;
            }
            for asset_id in assets {
                transaction
                    .execute_batch("SAVEPOINT release_image")
                    .map_err(storage)?;
                let removed = transaction.execute(
                    "DELETE FROM media_assets WHERE id = ?1 AND retention != 'library'",
                    [asset_id.to_string()],
                );
                match removed {
                    Ok(1) => {
                        transaction
                            .execute_batch("RELEASE release_image")
                            .map_err(storage)?;
                        deleted.push(asset_id);
                    }
                    _ => transaction
                        .execute_batch("ROLLBACK TO release_image; RELEASE release_image")
                        .map_err(storage)?,
                }
            }
        }
        transaction.commit().map_err(storage)?;
        Ok(deleted)
    }
}
