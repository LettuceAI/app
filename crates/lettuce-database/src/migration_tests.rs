use lettuce_characters::{ChatAppearanceV1, FontSize, GroupRepository};
use lettuce_conversations::{
    BranchStatus, CharacterLaunchSnapshot, Conversation, ConversationBranch, ConversationKind,
    ConversationParticipant, ConversationReader, CurrentConversationSettings,
    GroupChatModeSnapshot, GroupConversationDetails, GroupLaunchSnapshot,
    GroupMemberLaunchSnapshot, GroupParticipantPolicyDocument, GroupParticipantPolicySnapshot,
    GroupSpeakerSelectionSnapshot, ParticipantRole, ParticipantSource, ProtectedSnapshotRef,
    SnapshotSelection, SnapshotSource, TrustedArtifactDescriptor,
};
use lettuce_types::{
    CharacterId, ContentHash, ConversationBranchId, ConversationId, ConversationParticipantId,
    GroupId, Revision, SnapshotArtifactId, TimestampMillis,
};
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
fn migration_30_makes_stored_groups_roleplay_only() {
    let path = std::env::temp_dir().join(format!(
        "lettuce-migration-30-{}.db",
        GroupId::new()
    ));
    let group_id = GroupId::new();
    let conversation_id = ConversationId::new();
    let branch_id = ConversationBranchId::new();
    let first = CharacterId::new();
    let second = CharacterId::new();
    let user_participant = ConversationParticipantId::new();
    let first_participant = ConversationParticipantId::new();
    let second_participant = ConversationParticipantId::new();
    {
        let mut connection = Connection::open(&path).expect("old file");
        configure(&connection, false).expect("configure");
        let old: Vec<_> = MIGRATIONS
            .iter()
            .copied()
            .filter(|migration| migration.id <= 31)
            .collect();
        apply_migrations(&mut connection, &old).expect("old schema");
        let transaction = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .expect("transaction");
        let artifact = |source: SnapshotSource| -> ProtectedSnapshotRef {
            let payload = format!("{source:?}").into_bytes();
            let digest = ContentHash::parse(blake3::hash(&payload).to_hex().to_string())
                .expect("digest");
            let reference = ProtectedSnapshotRef {
                source,
                source_revision: Revision::INITIAL,
                artifact_id: SnapshotArtifactId::new(),
                digest,
                schema_version: 1,
                byte_size: u64::try_from(payload.len()).expect("size"),
            };
            crate::conversation::conversation_artifact_adapter::insert_trusted_artifact_in(
                &transaction,
                &TrustedArtifactDescriptor::Snapshot(reference.clone()),
                &payload,
                TimestampMillis::new(0),
            )
            .expect("artifact");
            reference
        };
        let group_ref = artifact(SnapshotSource::Group(group_id));
        let first_ref = artifact(SnapshotSource::Character(first));
        let second_ref = artifact(SnapshotSource::Character(second));
        transaction
            .execute(
                "INSERT INTO groups (id,status,name,normalized_name,chat_mode,persona_selection_kind,speaker_selection,memory_policy,disable_character_lorebooks,presentation_json,revision,created_at,updated_at) VALUES (?1,'active','Group','group','conversation','inherit','round_robin','manual',0,'{}',1,0,0)",
                params![group_id.to_string()],
            )
            .expect("group");
        let member = |character: CharacterId, snapshot_ref: ProtectedSnapshotRef, ordinal: u32| {
            GroupMemberLaunchSnapshot {
                character: CharacterLaunchSnapshot {
                    snapshot_ref,
                    source_id: character,
                    source_revision: Revision::INITIAL,
                    name: format!("Member {ordinal}"),
                    nickname: None,
                },
                ordinal,
                enabled: true,
                muted: false,
                model_override: SnapshotSelection::Disabled,
                lorebooks: SnapshotSelection::Explicit(Vec::new()),
                prompt: SnapshotSelection::Disabled,
            }
        };
        let kind = ConversationKind::Group(GroupConversationDetails {
            format_version: 1,
            group: GroupLaunchSnapshot {
                snapshot_ref: group_ref,
                source_id: group_id,
                source_revision: Revision::INITIAL,
                name: "Group".into(),
                members: vec![
                    member(first, first_ref, 0),
                    member(second, second_ref, 1),
                ],
                chat_mode: GroupChatModeSnapshot::Conversation,
                speaker_selection: GroupSpeakerSelectionSnapshot::RoundRobin,
                speaker_selection_model: None,
                memory: SnapshotSelection::Disabled,
                disable_character_lorebook: false,
                persona: SnapshotSelection::Disabled,
                scene: SnapshotSelection::Disabled,
                prompt: SnapshotSelection::Disabled,
                lorebooks: SnapshotSelection::Explicit(Vec::new()),
                model: SnapshotSelection::Disabled,
            },
            initial_participant_policy: GroupParticipantPolicyDocument {
                members: vec![
                    GroupParticipantPolicySnapshot {
                        participant_id: first_participant,
                        enabled: true,
                        muted: false,
                        model_override: SnapshotSelection::Disabled,
                    },
                    GroupParticipantPolicySnapshot {
                        participant_id: second_participant,
                        enabled: true,
                        muted: false,
                        model_override: SnapshotSelection::Disabled,
                    },
                ],
                revision: Revision::INITIAL,
                created_at: TimestampMillis::new(0),
                updated_at: TimestampMillis::new(0),
            },
        });
        let participant = |id, ordinal, role, source, name: &str| ConversationParticipant {
            id,
            role,
            ordinal,
            enabled: true,
            muted: false,
            source,
            display_name: name.to_owned(),
            authored_description: None,
            model_selection: SnapshotSelection::Disabled,
            member_snapshot: None,
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(0),
            updated_at: TimestampMillis::new(0),
        };
        let conversation = Conversation {
            id: conversation_id,
            lifecycle: lettuce_conversations::ConversationLifecycle::Active,
            title: "Group launch".into(),
            origin_conversation_id: None,
            origin_message_id: None,
            kind,
            active_branch_id: branch_id,
            participants: vec![
                participant(
                    user_participant,
                    0,
                    ParticipantRole::User,
                    ParticipantSource::User,
                    "User",
                ),
                participant(
                    first_participant,
                    1,
                    ParticipantRole::Character,
                    ParticipantSource::Character(first),
                    "Member 0",
                ),
                participant(
                    second_participant,
                    2,
                    ParticipantRole::Character,
                    ParticipantSource::Character(second),
                    "Member 1",
                ),
            ],
            current_settings: Some(CurrentConversationSettings::inherited(Revision::INITIAL)),
            revision: Revision::INITIAL,
            created_at: TimestampMillis::new(0),
            updated_at: TimestampMillis::new(0),
        };
        crate::conversation::conversation_vertical_slice::save_conversation(
            &transaction,
            &conversation,
        )
        .expect("conversation");
        crate::conversation::conversation_vertical_slice::save_branch(
            &transaction,
            &ConversationBranch {
                label: None,
                label_updated_at: None,
                id: branch_id,
                conversation_id,
                parent_branch_id: None,
                fork_message_id: None,
                head_message_id: None,
                status: BranchStatus::Active,
                revision: Revision::INITIAL,
                created_at: TimestampMillis::new(0),
                updated_at: TimestampMillis::new(0),
            },
        )
        .expect("branch");
        for reference in lettuce_conversations::conversation_snapshot_references(&conversation.kind)
        {
            transaction
                .execute(
                    "INSERT OR IGNORE INTO conversation_snapshot_refs (conversation_id, artifact_id) VALUES (?1, ?2)",
                    params![conversation_id.to_string(), reference.artifact_id.to_string()],
                )
                .expect("snapshot ref");
        }
        transaction
            .execute(
                "UPDATE conversation_settings SET chat_mode='conversation' WHERE conversation_id=?1",
                [conversation_id.to_string()],
            )
            .expect("legacy override");
        transaction.commit().expect("commit");
        connection
            .execute_batch(
                MIGRATIONS
                    .iter()
                    .find(|migration| migration.id == 30)
                    .expect("migration 30")
                    .sql,
            )
            .expect("migration 30");
    }
    let database = Database::open(&path).expect("upgrade");
    let connection = database.connection().expect("connection");
    assert_eq!(
        connection
            .query_row(
                "SELECT chat_mode FROM groups WHERE id=?1",
                [group_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .expect("group mode"),
        "roleplay"
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT chat_mode FROM conversation_settings WHERE conversation_id=?1",
                [conversation_id.to_string()],
                |row| row.get::<_, Option<String>>(0),
            )
            .expect("settings mode"),
        None
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT json_extract(kind_json,'$.value.details.group.chat_mode') FROM conversations WHERE id=?1",
                [conversation_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .expect("launch mode"),
        "roleplay"
    );
    drop(connection);
    let aggregate = ConversationReader::get(&database, conversation_id).expect("load");
    let ConversationKind::Group(details) = &aggregate.conversation.kind else {
        panic!("group conversation");
    };
    assert_eq!(details.group.chat_mode, GroupChatModeSnapshot::Roleplay);
    drop(database);
    std::fs::remove_file(path).expect("remove test database");
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

#[test]
fn migration_31_defaults_existing_group_companion_mode_off() {
    let mut connection = Connection::open_in_memory().expect("database");
    configure(&connection, false).expect("configure");
    let old: Vec<_> = MIGRATIONS
        .iter()
        .copied()
        .filter(|migration| migration.id <= 30)
        .collect();
    apply_migrations(&mut connection, &old).expect("schema through migration 30");
    let group_id = GroupId::new();
    connection
        .execute(
            "INSERT INTO groups (id,status,name,normalized_name,chat_mode,persona_selection_kind,speaker_selection,memory_policy,disable_character_lorebooks,presentation_json,revision,created_at,updated_at) VALUES (?1,'active','Existing group','existing group','roleplay','inherit','heuristic','manual',1,'{\"format_version\":1,\"value\":{}}',17,100,200)",
            params![group_id.to_string()],
        )
        .expect("existing group");
    apply_migrations(&mut connection, &MIGRATIONS[..31]).expect("schema through migration 31");
    let companion_mode: i64 = connection
        .query_row(
            "SELECT companion_mode FROM groups WHERE id = ?1",
            [group_id.to_string()],
            |row| row.get(0),
        )
        .expect("existing group companion mode");
    assert_eq!(companion_mode, 0);
    let override_column: i64 = connection
        .query_row(
            "SELECT count(*) FROM pragma_table_info('conversation_settings') WHERE name = 'companion_mode'",
            [],
            |row| row.get(0),
        )
        .expect("override column");
    assert_eq!(override_column, 1);
    let upgraded = recorded_migrations(&connection);
    assert_eq!(upgraded.iter().filter(|entry| entry.0 == 31).count(), 1);
}
