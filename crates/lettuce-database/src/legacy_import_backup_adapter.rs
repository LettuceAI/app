use lettuce_transfer::{
    BackupLegacyImportRun, BackupSqlRow, BackupSqlValue, LegacyImportBackup, backup_sql_text,
};
use rusqlite::{Transaction, params, types::Value};

const RUN_COLUMNS: &[&str] = &[
    "id",
    "source_schema_version",
    "inventory_fingerprint",
    "plan_fingerprint",
    "source_fingerprint",
    "status",
    "admitted_at",
    "updated_at",
];
const ASSIGNMENT_COLUMNS: &[&str] = &[
    "run_id",
    "source_kind",
    "source_key",
    "source_detail",
    "destination_id",
    "auxiliary_id",
    "expected_byte_len",
    "expected_content_hash",
];
const SKIP_COLUMNS: &[&str] = &["run_id", "source_kind", "source_key", "reason"];
const SECRET_COMPLETION_COLUMNS: &[&str] = &[
    "run_id",
    "source_kind",
    "source_key",
    "source_detail",
    "destination_ref",
    "generation",
    "completed_at",
];
const MEDIA_COMPLETION_COLUMNS: &[&str] = &[
    "run_id",
    "relative_path",
    "destination_asset_id",
    "blob_id",
    "byte_len",
    "content_hash",
    "completed_at",
];
const STAGE_RESULT_COLUMNS: &[&str] = &["run_id", "stage", "record_count", "completed_at"];
const RESULT_COLUMNS: &[&str] = &[
    "run_id",
    "plan_fingerprint",
    "persona_count",
    "lorebook_count",
    "lorebook_entry_count",
    "completed_at",
];
const PROVIDER_MODEL_RESULT_COLUMNS: &[&str] = &[
    "run_id",
    "plan_fingerprint",
    "provider_account_count",
    "model_profile_count",
    "prompt_count",
    "completed_at",
];
const ASR_RESULT_COLUMNS: &[&str] = &[
    "run_id",
    "plan_fingerprint",
    "vocabulary_count",
    "correction_count",
    "ignored_suggestion_count",
    "voice_example_count",
    "completed_at",
];
const USAGE_RECORD_COLUMNS: &[&str] = &[
    "run_id",
    "source_id",
    "recorded_at",
    "session_source_id",
    "character_source_id",
    "character_name",
    "model_source_id",
    "model_profile_id",
    "model_name",
    "provider_source_id",
    "provider_label",
    "operation_type",
    "finish_reason",
    "prompt_tokens",
    "completion_tokens",
    "total_tokens",
    "memory_tokens",
    "summary_tokens",
    "reasoning_tokens",
    "image_tokens",
    "audio_tokens",
    "prompt_cost",
    "completion_cost",
    "total_cost",
    "success",
    "error_message",
    "metadata_json",
];

fn read_rows(
    transaction: &Transaction<'_>,
    table: &str,
    columns: &[&str],
    filter: Option<(&str, &str)>,
) -> rusqlite::Result<Vec<BackupSqlRow>> {
    let listed = columns.join(", ");
    let sql = match filter {
        Some((column, _)) => {
            format!("SELECT {listed} FROM {table} WHERE {column} = ?1 ORDER BY {listed}")
        }
        None => format!("SELECT {listed} FROM {table} ORDER BY {listed}"),
    };
    let mut statement = transaction.prepare(&sql)?;
    let map_row = |row: &rusqlite::Row<'_>| {
        columns
            .iter()
            .enumerate()
            .map(|(index, column)| {
                let value = match row.get::<_, Value>(index)? {
                    Value::Null => BackupSqlValue::Null,
                    Value::Integer(value) => BackupSqlValue::Integer(value),
                    Value::Real(value) => BackupSqlValue::Real(value),
                    Value::Text(value) => BackupSqlValue::Text(value),
                    Value::Blob(_) => {
                        return Err(rusqlite::Error::InvalidColumnType(
                            index,
                            (*column).to_owned(),
                            rusqlite::types::Type::Blob,
                        ));
                    }
                };
                Ok(((*column).to_owned(), value))
            })
            .collect::<rusqlite::Result<BackupSqlRow>>()
    };
    match filter {
        Some((_, value)) => statement.query_map([value], map_row)?.collect(),
        None => statement.query_map([], map_row)?.collect(),
    }
}

pub(crate) fn read_in(transaction: &Transaction<'_>) -> rusqlite::Result<LegacyImportBackup> {
    let runs = read_rows(transaction, "legacy_import_runs", RUN_COLUMNS, None)?
        .into_iter()
        .map(|run| {
            let run_id = backup_sql_text(&run, "id")
                .ok_or(rusqlite::Error::InvalidQuery)?
                .to_owned();
            let rows = |table, columns| {
                read_rows(
                    transaction,
                    table,
                    columns,
                    Some(("run_id", run_id.as_str())),
                )
            };
            let single = |table, columns| rows(table, columns).map(|rows| rows.into_iter().next());
            Ok(BackupLegacyImportRun {
                assignments: rows("legacy_import_assignments", ASSIGNMENT_COLUMNS)?,
                skips: rows("legacy_import_skips", SKIP_COLUMNS)?,
                secret_completions: rows(
                    "legacy_import_secret_completions",
                    SECRET_COMPLETION_COLUMNS,
                )?,
                media_completions: rows(
                    "legacy_import_media_completions",
                    MEDIA_COMPLETION_COLUMNS,
                )?,
                stage_results: rows("legacy_import_stage_results", STAGE_RESULT_COLUMNS)?,
                results: single("legacy_import_results", RESULT_COLUMNS)?,
                provider_model_results: single(
                    "legacy_import_provider_model_results",
                    PROVIDER_MODEL_RESULT_COLUMNS,
                )?,
                asr_results: single("legacy_import_asr_results", ASR_RESULT_COLUMNS)?,
                run,
            })
        })
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(LegacyImportBackup {
        runs,
        usage_records: read_rows(
            transaction,
            "legacy_usage_records",
            USAGE_RECORD_COLUMNS,
            None,
        )?,
    })
}

fn insert_rows<'a>(
    transaction: &Transaction<'_>,
    table: &str,
    columns: &[&str],
    rows: impl IntoIterator<Item = &'a BackupSqlRow>,
) -> Result<(), rusqlite::Error> {
    let sql = format!(
        "INSERT INTO {table} ({}) VALUES ({})",
        columns.join(", "),
        (1..=columns.len())
            .map(|index| format!("?{index}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    for row in rows {
        if row.len() != columns.len() {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let values = columns
            .iter()
            .map(|column| match row.get(*column) {
                Some(BackupSqlValue::Null) => Ok(Value::Null),
                Some(BackupSqlValue::Integer(value)) => Ok(Value::Integer(*value)),
                Some(BackupSqlValue::Real(value)) => Ok(Value::Real(*value)),
                Some(BackupSqlValue::Text(value)) => Ok(Value::Text(value.clone())),
                None => Err(rusqlite::Error::InvalidQuery),
            })
            .collect::<rusqlite::Result<Vec<_>>>()?;
        transaction.execute(&sql, rusqlite::params_from_iter(values))?;
    }
    Ok(())
}

fn advance(transaction: &Transaction<'_>, run_id: &str, status: &str) -> rusqlite::Result<()> {
    transaction.execute(
        "UPDATE legacy_import_runs SET status = ?2 WHERE id = ?1",
        params![run_id, status],
    )?;
    Ok(())
}

fn restore_run(
    transaction: &Transaction<'_>,
    entry: &BackupLegacyImportRun,
) -> rusqlite::Result<()> {
    let run_id = backup_sql_text(&entry.run, "id").ok_or(rusqlite::Error::InvalidQuery)?;
    let status = backup_sql_text(&entry.run, "status").ok_or(rusqlite::Error::InvalidQuery)?;
    let mut admitting = entry.run.clone();
    admitting.insert("status".into(), BackupSqlValue::Text("admitting".into()));
    insert_rows(transaction, "legacy_import_runs", RUN_COLUMNS, [&admitting])?;
    insert_rows(
        transaction,
        "legacy_import_assignments",
        ASSIGNMENT_COLUMNS,
        &entry.assignments,
    )?;
    insert_rows(
        transaction,
        "legacy_import_skips",
        SKIP_COLUMNS,
        &entry.skips,
    )?;
    if status != "admitting" {
        advance(transaction, run_id, "admitted")?;
        insert_rows(
            transaction,
            "legacy_import_media_completions",
            MEDIA_COMPLETION_COLUMNS,
            &entry.media_completions,
        )?;
        insert_rows(
            transaction,
            "legacy_import_secret_completions",
            SECRET_COMPLETION_COLUMNS,
            &entry.secret_completions,
        )?;
        insert_rows(
            transaction,
            "legacy_import_asr_results",
            ASR_RESULT_COLUMNS,
            &entry.asr_results,
        )?;
        let partial = status == "partial"
            || !entry.stage_results.is_empty()
            || entry.provider_model_results.is_some();
        if partial || matches!(status, "importing" | "completed") || entry.results.is_some() {
            advance(transaction, run_id, "importing")?;
            insert_rows(
                transaction,
                "legacy_import_results",
                RESULT_COLUMNS,
                &entry.results,
            )?;
            insert_rows(
                transaction,
                "legacy_import_provider_model_results",
                PROVIDER_MODEL_RESULT_COLUMNS,
                &entry.provider_model_results,
            )?;
            if partial {
                advance(transaction, run_id, "partial")?;
                insert_rows(
                    transaction,
                    "legacy_import_stage_results",
                    STAGE_RESULT_COLUMNS,
                    &entry.stage_results,
                )?;
            }
        }
        if matches!(status, "completed" | "failed") {
            advance(transaction, run_id, status)?;
        }
    }
    let updated_at = match entry.run.get("updated_at") {
        Some(BackupSqlValue::Integer(value)) => *value,
        _ => return Err(rusqlite::Error::InvalidQuery),
    };
    transaction.execute(
        "UPDATE legacy_import_runs SET updated_at = ?2 WHERE id = ?1",
        params![run_id, updated_at],
    )?;
    if read_rows(
        transaction,
        "legacy_import_runs",
        RUN_COLUMNS,
        Some(("id", run_id)),
    )? != [entry.run.clone()]
    {
        return Err(rusqlite::Error::InvalidQuery);
    }
    Ok(())
}

pub(crate) fn insert_restored_in(
    transaction: &Transaction<'_>,
    backup: &LegacyImportBackup,
) -> rusqlite::Result<()> {
    let mut runs = backup.runs.iter().collect::<Vec<_>>();
    runs.sort_by_key(|entry| {
        (
            match entry.run.get("admitted_at") {
                Some(BackupSqlValue::Integer(value)) => *value,
                _ => i64::MIN,
            },
            backup_sql_text(&entry.run, "id"),
        )
    });
    for entry in runs {
        restore_run(transaction, entry)?;
    }
    insert_rows(
        transaction,
        "legacy_usage_records",
        USAGE_RECORD_COLUMNS,
        &backup.usage_records,
    )
}
