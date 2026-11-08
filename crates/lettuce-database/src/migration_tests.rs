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
fn migration_26_preserves_existing_content_and_database_guards() {
    let mut connection = Connection::open_in_memory().expect("database");
    configure(&connection, false).expect("configure");
    let old: Vec<_> = MIGRATIONS.iter().copied().filter(|migration| migration.id <= 25).collect();
    apply_migrations(&mut connection, &old).expect("old schema");
    connection.execute("INSERT INTO device_ui_state VALUES (1, '{\"theme\":\"dark\"}', 42)", []).expect("existing settings");
    let history = recorded_migrations(&connection);
    let objects = |connection: &Connection| -> Vec<(String, String)> {
        connection.prepare("SELECT type,name FROM sqlite_master WHERE name NOT LIKE 'sqlite_%' ORDER BY type,name").expect("schema")
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?))).expect("objects")
            .collect::<Result<_, _>>().expect("schema objects")
    };
    let before = objects(&connection);
    let through_26 = &MIGRATIONS[..26];
    apply_migrations(&mut connection, through_26).expect("upgrade");
    assert_eq!(objects(&connection), before);
    assert_eq!(&recorded_migrations(&connection)[..history.len()], history.as_slice());
    let settings: (String, i64) = connection.query_row("SELECT state_json,updated_at FROM device_ui_state WHERE id=1", [], |row| Ok((row.get(0)?, row.get(1)?))).expect("settings");
    assert_eq!(settings, ("{\"theme\":\"dark\"}".into(), 42));
    let large = serde_json::json!({"saved_presets": "x".repeat(512 * 1024)}).to_string();
    connection.execute("UPDATE device_ui_state SET state_json=?1 WHERE id=1", [&large]).expect("preferences above the former limit");
    assert!(connection.execute("UPDATE device_ui_state SET state_json='[]' WHERE id=1", []).is_err());
    assert!(connection.execute("INSERT INTO user_voices(id,provider_id,name,model_id,voice_id,revision,created_at,updated_at) VALUES ('00000000-0000-0000-0000-000000000001','00000000-0000-0000-0000-000000000002','Voice','model','voice',1,0,0)", []).is_err());
    assert_eq!(connection.query_row("PRAGMA foreign_keys", [], |row| row.get::<_, i64>(0)).expect("enforcement"), 1);
    assert!(connection.prepare("PRAGMA foreign_key_check").expect("check").query([]).expect("rows").next().expect("next").is_none());
    assert_eq!(connection.query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0)).expect("integrity"), "ok");
    let upgraded = recorded_migrations(&connection);
    apply_migrations(&mut connection, through_26).expect("repeat startup");
    assert_eq!(recorded_migrations(&connection), upgraded);
}

#[test]
fn migration_29_consolidates_group_prompts() {
    let mut connection = Connection::open_in_memory().expect("database");
    configure(&connection, false).expect("configure");
    let old: Vec<_> = MIGRATIONS
        .iter()
        .copied()
        .filter(|migration| migration.id <= 28)
        .collect();
    apply_migrations(&mut connection, &old).expect("old schema");
    connection
        .execute_batch(
            "BEGIN;
             INSERT INTO prompt_documents (id,status,name,purpose,condense,behavior_version,provenance_kind,built_in_key,provenance_json,revision,created_at,updated_at) VALUES
                ('conv-builtin','active','Group Chat (Conversation)','group_chat_conversational',0,'legacy_v1','built_in','prompt_app_group_chat','{}',1,0,0),
                ('roleplay-builtin','active','Group Chat (Roleplay)','group_chat_roleplay',0,'legacy_v1','built_in','prompt_app_group_chat_roleplay','{}',1,0,0),
                ('user-conv','active','Custom group prompt','group_chat_conversational',0,'legacy_v1','user',NULL,'{}',1,0,0);
             INSERT INTO prompt_entries (id,prompt_id,name,role,content,enabled,injection_position,depth,system_prompt,ordinal,revision,created_at,updated_at) VALUES
                ('entry-conv','conv-builtin','Task','system','hello',1,'relative',0,1,0,1,0,0);
             INSERT INTO characters (id,status,name,nickname,normalized_name,normalized_nickname,profile_json,provenance_json,defaults_json,interaction_mode,memory_policy,model_profile_id,default_scene_id,default_starter_id,direct_prompt_id,group_conversation_prompt_id,group_roleplay_prompt_id,voice_profile_id,voice_legacy_locator,voice_autoplay,presentation_json,image_recommendation_json,revision,created_at,updated_at) VALUES
                ('char-1','active','Character',NULL,'character',NULL,'{}','{}','{\"format_version\":1,\"value\":{\"interaction_mode\":\"roleplay\",\"memory_policy\":\"manual\",\"model_profile_id\":null,\"default_scene_id\":null,\"default_starter_id\":null,\"direct_prompt_id\":\"conv-builtin\",\"group_conversation_prompt_id\":\"user-conv\",\"group_roleplay_prompt_id\":null,\"voice\":null,\"voice_autoplay\":false}}','roleplay','manual',NULL,NULL,NULL,'conv-builtin','user-conv',NULL,NULL,NULL,0,'{}',NULL,1,0,0);
             INSERT INTO groups (id,status,name,normalized_name,chat_mode,persona_selection_kind,persona_id,speaker_selection,memory_policy,disable_character_lorebooks,group_conversation_prompt_id,group_roleplay_prompt_id,presentation_json,background_asset_id,background_blob_kind,starting_scene_id,revision,created_at,updated_at) VALUES
                ('group-1','active','Group','group','conversation','inherit',NULL,'llm','manual',0,'conv-builtin',NULL,'{}',NULL,'image',NULL,1,0,0);
             INSERT INTO conversations (id,kind,lifecycle,title,active_branch_id,kind_json,revision,created_at,updated_at) VALUES
                ('conv-group','group','active','Group','branch-group','{\"format_version\":1,\"value\":{}}',1,0,0);
             INSERT INTO conversation_branches (conversation_id,id,parent_branch_id,status,revision,created_at,updated_at) VALUES
                ('conv-group','branch-group',NULL,'active',1,0,0);
             INSERT INTO conversation_settings (conversation_id,revision,author_note_provenance,memory_provenance,model_provenance,voice_provenance,prompt_json,prompt_provenance,lorebooks_provenance,persona_provenance,scene_provenance,roleplay_prompt_json,roleplay_prompt_provenance,created_at,updated_at) VALUES
                ('conv-group',1,'launch_inherited','launch_inherited','launch_inherited','launch_inherited','{\"format_version\":1,\"value\":{\"purpose\":\"group_conversational\"}}','current_override','launch_inherited','launch_inherited','launch_inherited','{\"format_version\":1,\"value\":{\"purpose\":\"group_roleplay\"}}','current_override',0,0);
             COMMIT;",
        )
        .expect("old content");
    apply_migrations(&mut connection, MIGRATIONS).expect("upgrade");
    assert_eq!(
        connection
            .query_row(
                "SELECT purpose FROM prompt_documents WHERE id='user-conv'",
                [],
                |row| row.get::<_, String>(0)
            )
            .expect("user document"),
        "group_chat_roleplay"
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT count(*) FROM prompt_documents WHERE built_in_key='prompt_app_group_chat'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .expect("conversational built-in"),
        0
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT count(*) FROM prompt_entries WHERE prompt_id='conv-builtin'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .expect("dependent entries"),
        0
    );
    let character: (Option<String>, Option<String>, String) = connection
        .query_row(
            "SELECT direct_prompt_id,group_roleplay_prompt_id,defaults_json FROM characters WHERE id='char-1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("character");
    assert_eq!(character.0.as_deref(), Some("roleplay-builtin"));
    assert_eq!(character.1.as_deref(), Some("user-conv"));
    assert!(!character.2.contains("group_conversation_prompt_id"));
    assert_eq!(
        connection
            .query_row("SELECT group_conversation_prompt_id,group_roleplay_prompt_id FROM groups WHERE id='group-1'", [], |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, Option<String>>(1)?)))
            .expect("group"),
        (None, Some("roleplay-builtin".to_owned()))
    );
    let settings: (Option<String>, String, Option<String>, String) = connection
        .query_row(
            "SELECT prompt_json,prompt_provenance,roleplay_prompt_json,roleplay_prompt_provenance FROM conversation_settings WHERE conversation_id='conv-group'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("settings");
    assert_eq!(
        settings,
        (
            None,
            "launch_inherited".into(),
            None,
            "launch_inherited".into()
        )
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT revision FROM conversations WHERE id='conv-group'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .expect("change feed revision"),
        2
    );
    assert_eq!(
        connection
            .query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
            .expect("integrity"),
        "ok"
    );
}

#[test]
fn failed_table_rebuild_rolls_back_and_restores_foreign_keys() {
    let mut connection = Connection::open_in_memory().expect("database");
    configure(&connection, false).expect("configure");
    let old: Vec<_> = MIGRATIONS.iter().copied().filter(|migration| migration.id <= 25).collect();
    apply_migrations(&mut connection, &old).expect("old schema");
    connection.execute("INSERT INTO device_ui_state VALUES (1, '{}', 42)", []).expect("settings");
    let failed = super::Migration { id: 26, sql: "DROP TABLE device_ui_state; SELECT * FROM missing_table;" };
    assert!(apply_migrations(&mut connection, &[failed]).is_err());
    assert_eq!(connection.query_row("SELECT updated_at FROM device_ui_state WHERE id=1", [], |row| row.get::<_, i64>(0)).expect("rolled back row"), 42);
    assert_eq!(connection.query_row("PRAGMA foreign_keys", [], |row| row.get::<_, i64>(0)).expect("enforcement restored"), 1);
    assert_eq!(recorded_migrations(&connection).len(), 25);
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

#[test]
fn migration_25_upgrades_a_file_without_rewriting_existing_groups_or_checksums() {
    let path = std::env::temp_dir().join(format!("lettuce-migration-25-{}.db", GroupId::new()));
    let group_id = GroupId::new();
    let history = {
        let mut connection = Connection::open(&path).expect("old file");
        configure(&connection, true).expect("configure old file");
        let old: Vec<_> = MIGRATIONS.iter().copied().filter(|migration| migration.id <= 24).collect();
        apply_migrations(&mut connection, &old).expect("schema through migration 24");
        connection.execute(
            "INSERT INTO groups (id,status,name,normalized_name,chat_mode,persona_selection_kind,speaker_selection,memory_policy,disable_character_lorebooks,presentation_json,revision,created_at,updated_at) VALUES (?1,'active','Existing group','existing group','roleplay','inherit','heuristic','manual',1,'{\"format_version\":1,\"value\":{}}',17,100,200)",
            params![group_id.to_string()],
        ).expect("existing group");
        recorded_migrations(&connection)
    };
    let database = Database::open(&path).expect("automatically upgrade existing file");
    {
        let connection = database.connection().expect("connection");
        let row: (i64, i64, i64, i64, i64, i64) = connection
            .query_row(
                "SELECT time_awareness, disable_character_notes, disable_character_lorebooks, revision, created_at, updated_at FROM groups WHERE id = ?1",
                [group_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
            )
            .expect("existing group row");
        assert_eq!(row, (0, 0, 1, 17, 100, 200));
        let notes: i64 = connection
            .query_row("SELECT count(*) FROM group_scheduled_notes", [], |row| row.get(0))
            .expect("notes table");
        assert_eq!(notes, 0);
        let override_column: i64 = connection
            .query_row(
                "SELECT count(*) FROM pragma_table_info('conversation_settings') WHERE name = 'disable_character_notes'",
                [],
                |row| row.get(0),
            )
            .expect("override column");
        assert_eq!(override_column, 1);
        let upgraded = recorded_migrations(&connection);
        assert_eq!(&upgraded[..history.len()], history.as_slice());
        assert_eq!(upgraded.iter().filter(|entry| entry.0 == 25).count(), 1);
    }
    drop(database);
    std::fs::remove_file(path).expect("remove test database");
}
