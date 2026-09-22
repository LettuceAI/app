use rusqlite::Transaction;

use lettuce_transfer::PlaygroundHistoryBackup;

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
