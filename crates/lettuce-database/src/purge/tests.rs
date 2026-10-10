use super::*;

/// The tables that still hold rows whose `column` is `id`, with their counts.
pub(crate) fn remaining_rows(database: &Database, column: &str, id: &str) -> Vec<(String, i64)> {
    let connection = database.connection().expect("database lock");
    let tables: Vec<String> = connection
        .prepare(
            "SELECT m.name FROM sqlite_schema AS m JOIN pragma_table_info(m.name) AS c
             WHERE m.type = 'table' AND c.name = ?1 ORDER BY m.name",
        )
        .and_then(|mut statement| statement.query_map([column], |row| row.get(0))?.collect())
        .expect("tables");
    tables
        .into_iter()
        .filter_map(|table| {
            let count: i64 = connection
                .query_row(
                    &format!("SELECT count(*) FROM \"{table}\" WHERE \"{column}\" = ?1"),
                    [id],
                    |row| row.get(0),
                )
                .expect("count");
            (count > 0).then_some((table, count))
        })
        .collect()
}

pub(crate) fn execute(database: &Database, sql: &str, values: &[&str]) -> rusqlite::Result<usize> {
    database
        .connection()
        .expect("database lock")
        .execute(sql, rusqlite::params_from_iter(values))
}

#[test]
fn uuids_are_found_anywhere_in_text_and_lowercased() {
    let mut found = BTreeSet::new();
    collect_uuids(
        b"{\"a\":\"0F8FAD5B-D9CB-469F-A165-70867728950E\",\"b\":[\"7c9e6679-7425-40de-944b-e07fc1f90ae7x\"]}",
        &mut found,
    );
    collect_uuids(b"0f8fad5b-d9cb-469f-a165-70867728950", &mut found);
    assert_eq!(
        found.into_iter().collect::<Vec<_>>(),
        vec![
            "0f8fad5b-d9cb-469f-a165-70867728950e".to_owned(),
            "7c9e6679-7425-40de-944b-e07fc1f90ae7".to_owned(),
        ]
    );
}

#[test]
fn missing_entities_and_unknown_queue_rows_are_reported_and_dropped() {
    let database = Database::open_in_memory().expect("database");
    assert_eq!(
        database.purge_conversation(ConversationId::new(), TimestampMillis::new(1)),
        Err(PurgeError::NotFound)
    );
    assert_eq!(
        database.purge_character(CharacterId::new(), TimestampMillis::new(1)),
        Err(PurgeError::NotFound)
    );
    {
        let connection = database.connection().expect("lock");
        connection
            .execute(
                "INSERT INTO purge_queue (entity_kind, entity_id, change_id, queued_at)
                 VALUES ('conversation', ?1, ?2, 1)",
                [
                    ConversationId::new().to_string(),
                    uuid::Uuid::new_v4().to_string(),
                ],
            )
            .expect("queue");
    }
    assert_eq!(
        database.run_queued_purges(TimestampMillis::new(2)),
        Ok(Vec::new())
    );
    let queued: i64 = database
        .connection()
        .expect("lock")
        .query_row("SELECT count(*) FROM purge_queue", [], |row| row.get(0))
        .expect("queue");
    assert_eq!(queued, 0);
    let foreign_keys: i64 = database
        .connection()
        .expect("lock")
        .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
        .expect("pragma");
    assert_eq!(foreign_keys, 1, "enforcement is restored after a purge");
}

#[test]
fn a_database_that_lost_foreign_key_enforcement_refuses_every_use() {
    let database = Database::open_in_memory().expect("database");
    database
        .foreign_keys_lost
        .store(true, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(database.purge_notices(), Err(PurgeError::Storage));
    assert!(matches!(
        database.connection(),
        Err(crate::DatabaseError::ForeignKeysLost)
    ));
}

#[test]
fn persona_purge_retains_helper_receipts_and_rolls_back_late_failure() {
    let database = Database::open_in_memory().expect("database");
    let id = lettuce_types::PersonaId::new();
    {
        let connection = database.connection().expect("lock");
        connection.execute("INSERT INTO personas (id,status,title,normalized_title,description,revision,created_at,updated_at) VALUES (?1,'active','Persona','persona','Description',1,1,1)", [id.to_string()]).expect("persona");
        connection.execute("UPDATE persona_defaults SET default_persona_id=?1,revision=revision+1,updated_at=1", [id.to_string()]).expect("default");
        connection.execute_batch("INSERT INTO creation_workflows VALUES ('workflow','{}','drafting',NULL,1,1,1); INSERT INTO creation_proposals VALUES ('proposal','workflow',NULL,NULL,0,'drafting','{}','[]',1);").expect("helper");
        connection
            .execute(
                "INSERT INTO creation_apply_receipts VALUES ('workflow',1,'proposal',?1,1,1)",
                [id.to_string()],
            )
            .expect("receipt");
        connection.execute_batch("CREATE TEMP TRIGGER fail_persona_receipt BEFORE INSERT ON api_operation_receipts BEGIN SELECT RAISE(ABORT,'late failure'); END;").expect("fault");
    }
    let delete = || {
        database.purge_persona_operation(
            id,
            Revision::new(1),
            "delete-persona",
            "digest",
            TimestampMillis::new(2),
        )
    };
    assert_eq!(delete(), Err(PurgeError::Storage));
    {
        let connection = database.connection().expect("lock");
        assert_eq!(
            connection
                .query_row(
                    "SELECT default_persona_id FROM persona_defaults",
                    [],
                    |row| row.get::<_, String>(0)
                )
                .expect("default"),
            id.to_string()
        );
        assert_eq!(
            connection
                .query_row("SELECT count(*) FROM personas", [], |row| row
                    .get::<_, i64>(0))
                .expect("persona"),
            1
        );
        connection
            .execute_batch("DROP TRIGGER fail_persona_receipt;")
            .expect("remove fault");
    }
    delete().expect("purge");
    delete().expect("replay");
    assert_eq!(
        database.purge_persona_operation(
            id,
            Revision::new(1),
            "delete-persona",
            "changed",
            TimestampMillis::new(2)
        ),
        Err(PurgeError::Conflict)
    );
    let connection = database.connection().expect("lock");
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM personas", [], |row| row
                .get::<_, i64>(0))
            .expect("persona"),
        0
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT default_persona_id FROM persona_defaults",
                [],
                |row| row.get::<_, Option<String>>(0)
            )
            .expect("default"),
        None
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT persona_id FROM creation_apply_receipts",
                [],
                |row| row.get::<_, String>(0)
            )
            .expect("receipt"),
        id.to_string()
    );
}

#[test]
fn persona_purge_process_exit_preserves_the_default_and_replay_state() {
    let root = std::env::temp_dir().join(format!("slice5-persona-crash-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).expect("directory");
    let path = root.join("library.sqlite3");
    let id = lettuce_types::PersonaId::new();
    let database = Database::open(&path).expect("database");
    {
        let connection = database.connection().expect("lock");
        connection.execute("INSERT INTO personas (id,status,title,normalized_title,description,revision,created_at,updated_at) VALUES (?1,'active','Persona','persona','Description',1,1,1)", [id.to_string()]).expect("persona");
        connection.execute("UPDATE persona_defaults SET default_persona_id=?1,revision=revision+1,updated_at=1", [id.to_string()]).expect("default");
    }
    drop(database);
    let output = std::process::Command::new(std::env::current_exe().expect("executable"))
        .args(["--exact", "purge::tests::persona_purge_crash_child"])
        .env("LETTUCE_PERSONA_PURGE_CRASH_DATABASE", &path)
        .env("LETTUCE_PERSONA_PURGE_CRASH_ID", id.to_string())
        .output()
        .expect("child");
    assert_eq!(
        output.status.code(),
        Some(77),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let database = Database::open(&path).expect("reopen");
    assert!(
        database
            .lookup_api_operation("persona_delete", "crash")
            .expect("receipt")
            .is_none()
    );
    assert_eq!(
        database
            .connection()
            .expect("lock")
            .query_row(
                "SELECT default_persona_id FROM persona_defaults",
                [],
                |row| row.get::<_, String>(0)
            )
            .expect("default"),
        id.to_string()
    );
    database
        .purge_persona_operation(
            id,
            Revision::INITIAL,
            "crash",
            "digest",
            TimestampMillis::new(20),
        )
        .expect("retry");
    drop(database);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn persona_purge_crash_child() {
    let Some(path) = std::env::var_os("LETTUCE_PERSONA_PURGE_CRASH_DATABASE") else {
        return;
    };
    let id = std::env::var("LETTUCE_PERSONA_PURGE_CRASH_ID")
        .expect("id")
        .parse()
        .expect("persona id");
    let database = Database::open(path).expect("database");
    {
        let connection = database.connection().expect("lock");
        connection
            .create_scalar_function(
                "crash_persona_purge",
                0,
                rusqlite::functions::FunctionFlags::SQLITE_UTF8,
                |_| -> rusqlite::Result<i64> {
                    std::process::exit(77);
                },
            )
            .expect("crash function");
        connection.execute_batch("CREATE TEMP TRIGGER crash_persona_receipt BEFORE INSERT ON api_operation_receipts BEGIN SELECT crash_persona_purge(); END;").expect("crash trigger");
    }
    database
        .purge_persona_operation(
            id,
            Revision::INITIAL,
            "crash",
            "digest",
            TimestampMillis::new(20),
        )
        .expect("purge");
    panic!("crash hook was not reached");
}

#[test]
fn persona_purge_preserves_replica_timestamps() {
    let id = PersonaId::new();
    let apply = |now| {
        let database = Database::open_in_memory().expect("database");
        {
            let connection = database.connection().expect("lock");
            connection.execute("INSERT INTO personas(id,status,title,normalized_title,description,revision,created_at,updated_at) VALUES (?1,'active','Persona','persona','Description',1,1,1)", [id.to_string()]).expect("persona");
            connection.execute("UPDATE persona_defaults SET default_persona_id=?1,revision=revision+1,updated_at=1", [id.to_string()]).expect("default");
        }
        database.purge_persona_operation(id, Revision::INITIAL, "delete", "digest", TimestampMillis::new(now)).expect("purge");
        database.connection().expect("lock").query_row("SELECT default_persona_id,revision,updated_at FROM persona_defaults", [], |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?))).expect("row")
    };
    assert_eq!(apply(20), apply(30));
}

#[test]
fn persona_purge_failure_notice_is_a_sync_entity() {
    let database = Database::open_in_memory().expect("database");
    let id = PersonaId::new().to_string();
    {
        let connection = database.connection().expect("lock");
        connection.execute("INSERT INTO purge_queue(entity_kind,entity_id,change_id,queued_at) VALUES ('persona',?1,?2,1)", params![id, uuid::Uuid::new_v4().to_string()]).expect("queue");
        for _ in 0..MAX_QUEUED_PURGE_FAILURES {
            give_up_after_failures(&connection, "persona", &id, TimestampMillis::new(20)).expect("failure");
        }
    }
    let notices = database.purge_notices().expect("notices");
    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0].entity, PurgeNoticeEntity::SyncEntity);
    assert_eq!(notices[0].entity_id, id);
    assert_eq!(notices[0].reason, PurgeNoticeReason::DroppedAfterFailures);
}
