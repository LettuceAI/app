use std::str::FromStr;

use lettuce_transfer::{
    LEGACY_DATABASE_SCHEMA_VERSION, LegacyImportAdmission, LegacyImportAdmissionRequest,
    LegacyImportAssignment, LegacyImportRepository, LegacyImportRepositoryError,
    LegacyImportRunStatus, LegacyImportSources,
};
use lettuce_types::{
    AssetId, ContentHash, LegacyImportRunId, LorebookEntryId, LorebookId, PersonaId,
    TimestampMillis,
};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use crate::Database;

const MAX_MEDIA_PATH_BYTES: usize = 1_024;

impl LegacyImportRepository for Database {
    fn admit(
        &self,
        mut request: LegacyImportAdmissionRequest,
    ) -> Result<LegacyImportAdmission, LegacyImportRepositoryError> {
        normalize_sources(&mut request.sources)?;
        if request.source_schema_version != LEGACY_DATABASE_SCHEMA_VERSION {
            return Err(LegacyImportRepositoryError::InvalidInput);
        }
        let mut connection = self
            .connection()
            .map_err(|_| LegacyImportRepositoryError::Storage)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| LegacyImportRepositoryError::Storage)?;
        if let Some(mut existing) = load_admission(&transaction, request.run_id)? {
            if existing.source_schema_version != request.source_schema_version
                || existing.inventory_fingerprint != request.inventory_fingerprint
                || existing.plan_fingerprint != request.plan_fingerprint
                || assignment_sources(&existing.assignments) != request.sources
            {
                return Err(LegacyImportRepositoryError::Conflict);
            }
            existing.replayed = true;
            transaction
                .commit()
                .map_err(|_| LegacyImportRepositoryError::Storage)?;
            return Ok(existing);
        }

        transaction
            .execute(
                "INSERT INTO legacy_import_runs (id,source_schema_version,inventory_fingerprint,plan_fingerprint,status,admitted_at,updated_at) VALUES (?1,?2,?3,?4,'admitting',?5,?5)",
                params![
                    request.run_id.to_string(),
                    request.source_schema_version,
                    request.inventory_fingerprint.as_str(),
                    request.plan_fingerprint.as_str(),
                    request.admitted_at.get(),
                ],
            )
            .map_err(|_| LegacyImportRepositoryError::Storage)?;
        insert_assignments(&transaction, request.run_id, &request.sources)?;
        transaction
            .execute(
                "UPDATE legacy_import_runs SET status='admitted' WHERE id=?1 AND status='admitting'",
                [request.run_id.to_string()],
            )
            .map_err(|_| LegacyImportRepositoryError::Storage)?;
        let admission = load_admission(&transaction, request.run_id)?
            .ok_or(LegacyImportRepositoryError::Storage)?;
        transaction
            .commit()
            .map_err(|_| LegacyImportRepositoryError::Storage)?;
        Ok(admission)
    }
}

fn normalize_sources(sources: &mut LegacyImportSources) -> Result<(), LegacyImportRepositoryError> {
    sources.persona_ids.sort_unstable();
    sources.lorebook_ids.sort_unstable();
    sources.lorebook_entry_ids.sort_unstable();
    sources.media_paths.sort();
    if has_duplicates(&sources.persona_ids)
        || has_duplicates(&sources.lorebook_ids)
        || has_duplicates(&sources.lorebook_entry_ids)
        || has_duplicates(&sources.media_paths)
        || sources
            .media_paths
            .iter()
            .any(|path| !valid_media_path(path))
    {
        return Err(LegacyImportRepositoryError::InvalidInput);
    }
    Ok(())
}

fn has_duplicates<T: PartialEq>(values: &[T]) -> bool {
    values.windows(2).any(|pair| pair[0] == pair[1])
}

fn valid_media_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= MAX_MEDIA_PATH_BYTES
        && !path.starts_with('/')
        && !path.contains('\\')
        && path
            .split('/')
            .all(|component| !component.is_empty() && component != "." && component != "..")
}

fn insert_assignments(
    transaction: &Transaction<'_>,
    run_id: LegacyImportRunId,
    sources: &LegacyImportSources,
) -> Result<(), LegacyImportRepositoryError> {
    for source_id in &sources.persona_ids {
        insert_assignment(
            transaction,
            run_id,
            "persona",
            &source_id.to_string(),
            PersonaId::new().to_string(),
        )?;
    }
    for source_id in &sources.lorebook_ids {
        insert_assignment(
            transaction,
            run_id,
            "lorebook",
            &source_id.to_string(),
            LorebookId::new().to_string(),
        )?;
    }
    for source_id in &sources.lorebook_entry_ids {
        insert_assignment(
            transaction,
            run_id,
            "lorebook_entry",
            &source_id.to_string(),
            LorebookEntryId::new().to_string(),
        )?;
    }
    for source_path in &sources.media_paths {
        insert_assignment(
            transaction,
            run_id,
            "media",
            source_path,
            AssetId::new().to_string(),
        )?;
    }
    Ok(())
}

fn insert_assignment(
    transaction: &Transaction<'_>,
    run_id: LegacyImportRunId,
    source_kind: &str,
    source_key: &str,
    destination_id: String,
) -> Result<(), LegacyImportRepositoryError> {
    transaction
        .execute(
            "INSERT INTO legacy_import_assignments (run_id,source_kind,source_key,destination_id) VALUES (?1,?2,?3,?4)",
            params![run_id.to_string(), source_kind, source_key, destination_id],
        )
        .map_err(|_| LegacyImportRepositoryError::Storage)?;
    Ok(())
}

fn load_admission(
    transaction: &Transaction<'_>,
    run_id: LegacyImportRunId,
) -> Result<Option<LegacyImportAdmission>, LegacyImportRepositoryError> {
    let row = transaction
        .query_row(
            "SELECT source_schema_version,inventory_fingerprint,plan_fingerprint,status,admitted_at FROM legacy_import_runs WHERE id=?1",
            [run_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, u32>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            },
        )
        .optional()
        .map_err(|_| LegacyImportRepositoryError::Storage)?;
    let Some((source_schema_version, inventory, plan, status, admitted_at)) = row else {
        return Ok(None);
    };
    Ok(Some(LegacyImportAdmission {
        run_id,
        source_schema_version,
        inventory_fingerprint: ContentHash::parse(inventory)
            .map_err(|_| LegacyImportRepositoryError::Storage)?,
        plan_fingerprint: ContentHash::parse(plan)
            .map_err(|_| LegacyImportRepositoryError::Storage)?,
        status: parse_status(&status)?,
        assignments: load_assignments(transaction, run_id)?,
        admitted_at: TimestampMillis::new(admitted_at),
        replayed: false,
    }))
}

fn parse_status(value: &str) -> Result<LegacyImportRunStatus, LegacyImportRepositoryError> {
    match value {
        "admitted" => Ok(LegacyImportRunStatus::Admitted),
        "importing" => Ok(LegacyImportRunStatus::Importing),
        "completed" => Ok(LegacyImportRunStatus::Completed),
        "failed" => Ok(LegacyImportRunStatus::Failed),
        _ => Err(LegacyImportRepositoryError::Storage),
    }
}

fn load_assignments(
    transaction: &Transaction<'_>,
    run_id: LegacyImportRunId,
) -> Result<Vec<LegacyImportAssignment>, LegacyImportRepositoryError> {
    let mut statement = transaction
        .prepare(
            "SELECT source_kind,source_key,destination_id FROM legacy_import_assignments WHERE run_id=?1 ORDER BY CASE source_kind WHEN 'persona' THEN 1 WHEN 'lorebook' THEN 2 WHEN 'lorebook_entry' THEN 3 ELSE 4 END,source_key",
        )
        .map_err(|_| LegacyImportRepositoryError::Storage)?;
    statement
        .query_map([run_id.to_string()], |row| {
            let source_kind: String = row.get(0)?;
            let source_key: String = row.get(1)?;
            let destination_id: String = row.get(2)?;
            parse_assignment(&source_kind, source_key, destination_id)
                .map_err(|_| rusqlite::Error::InvalidQuery)
        })
        .map_err(|_| LegacyImportRepositoryError::Storage)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| LegacyImportRepositoryError::Storage)
}

fn parse_assignment(
    source_kind: &str,
    source_key: String,
    destination_id: String,
) -> Result<LegacyImportAssignment, LegacyImportRepositoryError> {
    match source_kind {
        "persona" => Ok(LegacyImportAssignment::Persona {
            legacy_id: PersonaId::from_str(&source_key)
                .map_err(|_| LegacyImportRepositoryError::Storage)?,
            destination_id: PersonaId::from_str(&destination_id)
                .map_err(|_| LegacyImportRepositoryError::Storage)?,
        }),
        "lorebook" => Ok(LegacyImportAssignment::Lorebook {
            legacy_id: LorebookId::from_str(&source_key)
                .map_err(|_| LegacyImportRepositoryError::Storage)?,
            destination_id: LorebookId::from_str(&destination_id)
                .map_err(|_| LegacyImportRepositoryError::Storage)?,
        }),
        "lorebook_entry" => Ok(LegacyImportAssignment::LorebookEntry {
            legacy_id: LorebookEntryId::from_str(&source_key)
                .map_err(|_| LegacyImportRepositoryError::Storage)?,
            destination_id: LorebookEntryId::from_str(&destination_id)
                .map_err(|_| LegacyImportRepositoryError::Storage)?,
        }),
        "media" => Ok(LegacyImportAssignment::Media {
            relative_path: source_key,
            destination_id: AssetId::from_str(&destination_id)
                .map_err(|_| LegacyImportRepositoryError::Storage)?,
        }),
        _ => Err(LegacyImportRepositoryError::Storage),
    }
}

fn assignment_sources(assignments: &[LegacyImportAssignment]) -> LegacyImportSources {
    let mut sources = LegacyImportSources {
        persona_ids: Vec::new(),
        lorebook_ids: Vec::new(),
        lorebook_entry_ids: Vec::new(),
        media_paths: Vec::new(),
    };
    for assignment in assignments {
        match assignment {
            LegacyImportAssignment::Persona { legacy_id, .. } => {
                sources.persona_ids.push(*legacy_id);
            }
            LegacyImportAssignment::Lorebook { legacy_id, .. } => {
                sources.lorebook_ids.push(*legacy_id);
            }
            LegacyImportAssignment::LorebookEntry { legacy_id, .. } => {
                sources.lorebook_entry_ids.push(*legacy_id);
            }
            LegacyImportAssignment::Media { relative_path, .. } => {
                sources.media_paths.push(relative_path.clone());
            }
        }
    }
    sources
}

#[cfg(test)]
mod tests {
    use std::fs;

    use lettuce_transfer::{
        LEGACY_DATABASE_SCHEMA_VERSION, LegacyImportAdmissionRequest, LegacyImportRepository,
        LegacyImportRepositoryError, LegacyImportRunStatus, LegacyImportSources,
    };
    use lettuce_types::{
        ContentHash, LegacyImportRunId, LorebookEntryId, LorebookId, PersonaId, TimestampMillis,
    };

    use crate::Database;

    fn database_path(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "lettuce-legacy-import-{label}-{}.sqlite3",
            LegacyImportRunId::new()
        ))
    }

    fn request(run_id: LegacyImportRunId) -> LegacyImportAdmissionRequest {
        LegacyImportAdmissionRequest {
            run_id,
            source_schema_version: LEGACY_DATABASE_SCHEMA_VERSION,
            inventory_fingerprint: ContentHash::parse("ab".repeat(32)).expect("inventory hash"),
            plan_fingerprint: ContentHash::parse("cd".repeat(32)).expect("plan hash"),
            sources: LegacyImportSources {
                persona_ids: vec![PersonaId::new()],
                lorebook_ids: vec![LorebookId::new()],
                lorebook_entry_ids: vec![LorebookEntryId::new()],
                media_paths: vec!["images/avatar.png".to_owned()],
            },
            admitted_at: TimestampMillis::new(100),
        }
    }

    #[test]
    fn admission_replays_assignments_and_conflicts_after_reopen() {
        let path = database_path("replay");
        let run_id = LegacyImportRunId::new();
        let original = request(run_id);
        let first_database = Database::open(&path).expect("open database");
        let first = first_database
            .admit(original.clone())
            .expect("admit import");
        assert_eq!(first.status, LegacyImportRunStatus::Admitted);
        assert!(!first.replayed);
        assert_eq!(first.assignments.len(), 4);
        let replay = first_database
            .admit(original.clone())
            .expect("replay import");
        assert!(replay.replayed);
        assert_eq!(replay.assignments, first.assignments);
        assert_eq!(replay.admitted_at, first.admitted_at);
        drop(first_database);

        let reopened = Database::open(&path).expect("reopen database");
        let reopened_replay = reopened
            .admit(original.clone())
            .expect("replay after reopen");
        assert!(reopened_replay.replayed);
        assert_eq!(reopened_replay.assignments, first.assignments);

        let mut changed_source = original.clone();
        changed_source.sources.persona_ids = vec![PersonaId::new()];
        assert_eq!(
            reopened.admit(changed_source),
            Err(LegacyImportRepositoryError::Conflict)
        );
        let mut changed_plan = original;
        changed_plan.plan_fingerprint =
            ContentHash::parse("ef".repeat(32)).expect("changed plan hash");
        assert_eq!(
            reopened.admit(changed_plan),
            Err(LegacyImportRepositoryError::Conflict)
        );
        let connection = reopened.connection().expect("database lock");
        assert!(
            connection
                .execute(
                    "UPDATE legacy_import_runs SET plan_fingerprint=?2 WHERE id=?1",
                    rusqlite::params![run_id.to_string(), "11".repeat(32)],
                )
                .is_err()
        );
        assert!(
            connection
                .execute(
                    "INSERT INTO legacy_import_assignments (run_id,source_kind,source_key,destination_id) VALUES (?1,'media','images/late.png',?2)",
                    rusqlite::params![run_id.to_string(), lettuce_types::AssetId::new().to_string()],
                )
                .is_err()
        );
        let domain_rows: u32 = connection
            .query_row(
                "SELECT (SELECT count(*) FROM personas) + (SELECT count(*) FROM lorebooks) + (SELECT count(*) FROM lorebook_entries) + (SELECT count(*) FROM media_assets)",
                [],
                |row| row.get(0),
            )
            .expect("domain row count");
        assert_eq!(domain_rows, 0);
        drop(connection);
        drop(reopened);
        fs::remove_file(path).expect("remove database");
    }

    #[test]
    fn assignment_failure_rolls_back_the_entire_admission() {
        let path = database_path("rollback");
        let run_id = LegacyImportRunId::new();
        let database = Database::open(&path).expect("open database");
        database
            .connection()
            .expect("database lock")
            .execute_batch(
                "CREATE TRIGGER reject_legacy_lorebook_assignment BEFORE INSERT ON legacy_import_assignments WHEN NEW.source_kind='lorebook' BEGIN SELECT RAISE(ABORT, 'test rollback'); END;",
            )
            .expect("install rollback trigger");
        assert_eq!(
            database.admit(request(run_id)),
            Err(LegacyImportRepositoryError::Storage)
        );
        let connection = database.connection().expect("database lock");
        let run_count: u32 = connection
            .query_row(
                "SELECT count(*) FROM legacy_import_runs WHERE id=?1",
                [run_id.to_string()],
                |row| row.get(0),
            )
            .expect("run count");
        let assignment_count: u32 = connection
            .query_row(
                "SELECT count(*) FROM legacy_import_assignments WHERE run_id=?1",
                [run_id.to_string()],
                |row| row.get(0),
            )
            .expect("assignment count");
        assert_eq!((run_count, assignment_count), (0, 0));
        drop(connection);
        drop(database);
        fs::remove_file(path).expect("remove database");
    }
}
