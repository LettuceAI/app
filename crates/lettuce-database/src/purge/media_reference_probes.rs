use std::collections::BTreeSet;

use lettuce_media::{MediaReference, MediaReferenceKind};
use rusqlite::Connection;

use super::{FINISHED_JOB, scanned};
use crate::purge::{PurgeError, storage, text_columns};

pub(super) struct Probe {
    pub sql: String,
    kind: MediaReferenceKind,
    identity: String,
}

fn reference_kind(table: &str) -> Result<MediaReferenceKind, PurgeError> {
    use MediaReferenceKind as Kind;
    let families = [
        ("legacy_import_", Kind::LegacyImport),
        ("sync_", Kind::Sync),
        ("character", Kind::Character),
        ("persona", Kind::Persona),
        ("group", Kind::Group),
        ("scene", Kind::Scene),
        ("conversation", Kind::Conversation),
        ("generation_", Kind::Conversation),
        ("revision_media_", Kind::Conversation),
        ("candidate_media_", Kind::Conversation),
        ("message_", Kind::Conversation),
        ("tool_", Kind::Conversation),
        ("turn_", Kind::Conversation),
        ("starter_", Kind::Character),
        ("creation_", Kind::Creation),
        ("speech_", Kind::Speech),
        ("asr_", Kind::Speech),
        ("audio_", Kind::Speech),
        ("user_voices", Kind::Speech),
        ("discovered_tts_", Kind::Speech),
        ("installed_whisper_", Kind::Model),
        ("image_", Kind::Image),
        ("playground_", Kind::Image),
        ("memory_", Kind::Memory),
        ("dynamic_memory_", Kind::Memory),
        ("companion_", Kind::Companion),
        ("model_", Kind::Model),
        ("local_model_", Kind::Model),
        ("llama_", Kind::Model),
        ("llm_", Kind::Model),
        ("provider_", Kind::Model),
        ("app_settings", Kind::Settings),
        ("device_", Kind::Settings),
        ("app_usage_", Kind::Usage),
        ("usage_", Kind::Usage),
        ("legacy_usage_", Kind::Usage),
        ("lorebook", Kind::Lorebook),
        ("prompt_", Kind::Prompt),
        ("job", Kind::Job),
        ("hugging_face_job_", Kind::Job),
        ("backup_", Kind::Transfer),
        ("purge_", Kind::Transfer),
    ];
    families
        .into_iter()
        .find_map(|(prefix, kind)| table.starts_with(prefix).then_some(kind))
        .ok_or(PurgeError::Integrity)
}

fn key_expression(connection: &Connection, table: &str) -> Result<String, PurgeError> {
    if !table
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err(PurgeError::Integrity);
    }
    let columns: Vec<String> = connection
        .prepare("SELECT name FROM pragma_table_info(?1) WHERE pk > 0 ORDER BY pk")
        .and_then(|mut statement| statement.query_map([table], |row| row.get(0))?.collect())
        .map_err(storage)?;
    if columns.is_empty() {
        return Err(PurgeError::Integrity);
    }
    Ok(format!(
        "json_array({})",
        columns
            .iter()
            .map(|column| format!("\"{column}\""))
            .collect::<Vec<_>>()
            .join(",")
    ))
}

fn table_probe(connection: &Connection, table: &str, predicate: &str) -> Result<Probe, PurgeError> {
    Ok(Probe {
        sql: format!(
            "SELECT {} AS reference_key FROM \"{table}\" WHERE {predicate}",
            key_expression(connection, table)?
        ),
        kind: reference_kind(table)?,
        identity: table.into(),
    })
}

pub(super) fn probes(connection: &Connection) -> Result<Vec<Probe>, PurgeError> {
    let foreign_keys: Vec<(String, String)> = connection.prepare("SELECT m.name,f.\"from\" FROM sqlite_schema m JOIN pragma_foreign_key_list(m.name) f WHERE m.type='table' AND f.\"table\"='media_assets' AND (f.\"to\" IS NULL OR f.\"to\"='id') ORDER BY m.name,f.\"from\"")
        .and_then(|mut statement| statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?.collect()).map_err(storage)?;
    let mut result = Vec::new();
    for (table, column) in foreign_keys {
        if table == "legacy_import_media_completions" || table == "media_gc_candidates" {
            continue;
        }
        result.push(table_probe(
            connection,
            &table,
            &format!("\"{column}\"=c.value"),
        )?);
    }
    result.push(Probe {
        sql: format!("SELECT json_array(run.id) AS reference_key FROM legacy_import_assignments assignment JOIN legacy_import_runs run ON run.id=assignment.run_id WHERE assignment.source_kind='media' AND assignment.destination_id=c.value AND (run.status IN ('admitting','admitted','importing') OR (run.status='partial' AND (SELECT count(*) FROM legacy_import_stage_results receipt WHERE receipt.run_id=run.id)<{}))", lettuce_transfer::LegacyImportStage::ALL.len()),
        kind: MediaReferenceKind::LegacyImport,
        identity: "legacy_import_runs".into(),
    });
    result.push(Probe {
        sql: "SELECT json_array(batch.batch_id) AS reference_key FROM sync_incoming_changes change JOIN sync_incoming_batches batch ON batch.batch_id=change.batch_id WHERE batch.state<>'committed' AND instr(CAST(change.payload_bytes AS TEXT),c.value)>0".into(),
        kind: MediaReferenceKind::Sync,
        identity: "sync_incoming_batches".into(),
    });
    for column in ["current_payload", "incoming_payload"] {
        result.push(table_probe(
            connection,
            "sync_conflicts",
            &format!("instr(CAST({column} AS TEXT),c.value)>0"),
        )?);
    }
    result.push(Probe {
        sql: "SELECT json_array(deferred.change_id) AS reference_key FROM sync_deferred_changes deferred JOIN sync_changes change ON change.change_id=deferred.change_id WHERE instr(CAST(change.payload_bytes AS TEXT),c.value)>0".into(),
        kind: MediaReferenceKind::Sync,
        identity: "sync_deferred_changes".into(),
    });
    for column in text_columns(connection, "jobs")? {
        result.push(table_probe(
            connection,
            "jobs",
            &format!("state NOT IN {FINISHED_JOB} AND instr(CAST(\"{column}\" AS TEXT),c.value)>0"),
        )?);
    }
    result.push(Probe {
        sql: format!("SELECT json_array(job.id) AS reference_key FROM job_events event JOIN jobs job ON job.id=event.job_id WHERE job.state NOT IN {FINISHED_JOB} AND instr(event.event_json,c.value)>0"),
        kind: MediaReferenceKind::Job,
        identity: "jobs".into(),
    });
    let tables: Vec<String> = connection
        .prepare("SELECT name FROM sqlite_schema WHERE type='table' ORDER BY name")
        .and_then(|mut statement| statement.query_map([], |row| row.get(0))?.collect())
        .map_err(storage)?;
    for table in tables.into_iter().filter(|table| scanned(table)) {
        for column in text_columns(connection, &table)? {
            result.push(table_probe(
                connection,
                &table,
                &format!("instr(CAST(\"{column}\" AS TEXT),c.value)>0"),
            )?);
        }
    }
    Ok(result)
}

pub(super) fn owners(
    connection: &Connection,
    asset: &str,
) -> Result<Vec<MediaReference>, PurgeError> {
    owners_using(connection, asset, &probes(connection)?)
}

pub(super) fn owners_using(
    connection: &Connection,
    asset: &str,
    probes: &[Probe],
) -> Result<Vec<MediaReference>, PurgeError> {
    let mut result = BTreeSet::new();
    for probe in probes {
        let keys: Vec<String> = connection
            .prepare(&probe.sql.replace("c.value", "?1"))
            .and_then(|mut statement| statement.query_map([asset], |row| row.get(0))?.collect())
            .map_err(storage)?;
        for key in keys {
            let components: Vec<serde_json::Value> = serde_json::from_str(&key).map_err(storage)?;
            let owner_id = components
                .first()
                .and_then(serde_json::Value::as_str)
                .filter(|value| value.parse::<uuid::Uuid>().is_ok())
                .map(str::to_owned)
                .unwrap_or_else(|| {
                    blake3::hash(format!("{}:{key}", probe.identity).as_bytes())
                        .to_hex()
                        .to_string()
                });
            result.insert(MediaReference {
                kind: probe.kind,
                owner_id,
            });
        }
    }
    Ok(result.into_iter().collect())
}
