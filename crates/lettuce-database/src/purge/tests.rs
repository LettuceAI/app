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
        queue_purge(
            &connection,
            PurgeKind::Conversation,
            &ConversationId::new().to_string(),
            TimestampMillis::new(1),
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
