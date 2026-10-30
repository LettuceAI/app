use lettuce_characters::{ChatAppearanceV1, FontSize, GroupRepository};
use lettuce_types::GroupId;
use rusqlite::{Connection, params};

use super::{Database, MIGRATIONS, apply_migrations, configure, migration_checksum};

#[test]
fn migration_checksums_match_append_only_history() {
    let actual: String = MIGRATIONS
        .iter()
        .map(|migration| format!("{} {}\n", migration.id, migration_checksum(migration.sql)))
        .collect();
    assert_eq!(
        actual,
        include_str!("../migrations/checksums.txt"),
        "existing migration SQL and checksum entries are immutable; append a new migration and its checksum entry"
    );
}

fn recorded_migrations(connection: &Connection) -> Vec<(u32, String, i64)> {
    connection
        .prepare("SELECT id,checksum,applied_at FROM schema_migrations ORDER BY id")
        .expect("prepare history")
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .expect("read history")
        .collect::<Result<_, _>>()
        .expect("migration rows")
}

#[test]
fn migration_23_upgrades_a_file_without_rewriting_existing_groups_or_checksums() {
    let path = std::env::temp_dir().join(format!("lettuce-migration-23-{}.db", GroupId::new()));
    let group_id = GroupId::new();
    let presentation = ChatAppearanceV1 { font_size: FontSize::Large, ..Default::default() };
    let encoded = serde_json::json!({ "format_version": 1, "value": presentation }).to_string();
    let history = {
        let mut connection = Connection::open(&path).expect("old file");
        configure(&connection, true).expect("configure old file");
        let old: Vec<_> = MIGRATIONS.iter().copied().filter(|migration| migration.id <= 22).collect();
        apply_migrations(&mut connection, &old).expect("schema through migration 22");
        connection.execute(
            "INSERT INTO groups (id,status,name,normalized_name,chat_mode,persona_selection_kind,speaker_selection,memory_policy,disable_character_lorebooks,presentation_json,revision,created_at,updated_at) VALUES (?1,'active','Existing group','existing group','roleplay','inherit','heuristic','manual',0,?2,17,100,200)",
            params![group_id.to_string(), encoded],
        ).expect("existing group with historical appearance");
        recorded_migrations(&connection)
    };
    let database = Database::open(&path).expect("automatically upgrade existing file");
    let existing = GroupRepository::get(&database, group_id).expect("read group").expect("preserved group");
    assert_eq!(existing.group.name, "Existing group");
    assert_eq!(existing.group.revision.get(), 17);
    assert_eq!(existing.group.created_at.get(), 100);
    assert_eq!(existing.group.updated_at.get(), 200);
    assert_eq!(existing.group.presentation, presentation);
    assert_eq!(existing.group.presentation_overrides, None);
    let upgraded = {
        let connection = database.connection().expect("connection");
        let upgraded = recorded_migrations(&connection);
        assert_eq!(&upgraded[..history.len()], history.as_slice());
        assert_eq!(upgraded.iter().filter(|entry| entry.0 == 23).count(), 1);
        let integrity: String = connection.query_row("PRAGMA integrity_check", [], |row| row.get(0)).expect("integrity");
        assert_eq!(integrity, "ok");
        upgraded
    };
    drop(database);
    let reopened = Database::open(&path).expect("repeat startup");
    assert_eq!(recorded_migrations(&reopened.connection().expect("connection")), upgraded);
    assert_eq!(GroupRepository::get(&reopened, group_id).expect("read again"), Some(existing));
    drop(reopened);
    std::fs::remove_file(path).expect("remove test database");
}
