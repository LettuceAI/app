use super::*;
use lettuce_types::PersonaId;

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
    assert!(matches!(
        database.purge_notices(),
        Err(PurgeError::Storage { .. })
    ));
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
        database.purge_library_operation(crate::LibraryKind::Persona, &id.to_string(),
            Revision::new(1),
            "delete-persona",
            "digest",
            TimestampMillis::new(2),
        )
    };
    assert!(matches!(delete(), Err(PurgeError::Storage { .. })));
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
        database.purge_library_operation(crate::LibraryKind::Persona, &id.to_string(),
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
        .purge_library_operation(crate::LibraryKind::Persona, &id.to_string(),
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
    let id: PersonaId = std::env::var("LETTUCE_PERSONA_PURGE_CRASH_ID")
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
        .purge_library_operation(crate::LibraryKind::Persona, &id.to_string(),
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
        database.purge_library_operation(crate::LibraryKind::Persona, &id.to_string(), Revision::INITIAL, "delete", "digest", TimestampMillis::new(now)).expect("purge");
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

#[test]
fn character_purge_removes_group_companion_checkpoints_and_relationships_toward_it() {
    use lettuce_companions::{
        GroupCompanionCheckpoint, GroupCompanionEffect, GroupCompanionEffectReason,
        GroupCompanionMessageSource, GroupCompanionRepository, GroupMemberRelationship,
        RelationshipState,
    };
    use lettuce_types::{
        ConversationBranchId, ConversationId, ConversationParticipantId, MessageId,
        MessageRevisionId,
    };

    let database = Database::open_in_memory().expect("database");
    let deleted = CharacterId::new();
    let survivor = CharacterId::new();
    let conversation_id = ConversationId::new();
    let branch_id = ConversationBranchId::new();
    let user_participant = ConversationParticipantId::new();
    let message_id = MessageId::new();
    let revision_id = MessageRevisionId::new();
    let relationship = |target_character_id| GroupMemberRelationship {
        target_character_id,
        state: RelationshipState {
            closeness: 0.0,
            trust: 0.1,
            affection: 0.0,
            tension: 0.0,
            stability: 0.0,
            interaction_count: 0,
            last_interaction_at: TimestampMillis::new(10),
        },
    };
    let effect = |signal: &str| GroupCompanionEffect {
        reason: GroupCompanionEffectReason::Witnessed,
        social_event: None,
        emotion_delta: lettuce_companions::CompanionEmotionDelta::default(),
        relationship_change: None,
        signal_changes: lettuce_companions::CompanionSignalChanges {
            added: vec![signal.into()],
            removed: Vec::new(),
        },
        by_user: false,
    };
    let checkpoint = |character_id, target_character_id, signal: &str| GroupCompanionCheckpoint {
        message_id,
        character_id,
        source: GroupCompanionMessageSource {
            revision_id: Some(revision_id),
            candidate_id: None,
        },
        emotional_state: lettuce_companions::EmotionalState {
            felt: lettuce_companions::EmotionVector::default(),
            expressed: lettuce_companions::EmotionVector::default(),
            blocked: lettuce_companions::EmotionVector::default(),
            momentum: lettuce_companions::EmotionVector::default(),
            active_drivers: Vec::new(),
            confidence: 0.5,
            updated_at: TimestampMillis::new(10),
        },
        member_relationships: vec![relationship(target_character_id)],
        effects: vec![effect(signal)],
        created_at: TimestampMillis::new(10),
    };
    {
        let mut connection = database.connection().expect("lock");
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("transaction");
        for (character_id, name) in [(deleted, "Deleted"), (survivor, "Survivor")] {
            transaction
                .execute(
                    "INSERT INTO characters (
                       id, status, name, normalized_name, profile_json,
                       provenance_json, defaults_json, interaction_mode,
                       memory_policy, voice_autoplay, presentation_json, revision,
                       created_at, updated_at
                     ) VALUES (?1, 'active', ?2, ?3, '{}', '{}', '{}',
                               'companion', 'manual', 0, '{}', 1, 1, 1)",
                    params![character_id.to_string(), name, name.to_ascii_lowercase()],
                )
                .expect("character");
        }
        transaction
            .execute(
                "INSERT INTO conversations (
                   id, kind, lifecycle, title, active_branch_id, kind_json,
                   revision, created_at, updated_at
                 ) VALUES (?1, 'group', 'active', 'Group', ?2,
                           '{\"format_version\":1,\"value\":null}', 1, 1, 1)",
                params![conversation_id.to_string(), branch_id.to_string()],
            )
            .expect("conversation");
        transaction
            .execute(
                "INSERT INTO conversation_branches (
                   conversation_id, id, status, revision, created_at, updated_at
                 ) VALUES (?1, ?2, 'active', 1, 1, 1)",
                params![conversation_id.to_string(), branch_id.to_string()],
            )
            .expect("branch");
        transaction
            .execute(
                "INSERT INTO conversation_participants (
                   conversation_id, id, role, ordinal, source_kind, source_id,
                   enabled, muted, display_name, model_selection_json, revision,
                   created_at, updated_at
                 ) VALUES (?1, ?2, 'user', 0, 'user', NULL, 1, 0, 'User',
                           '{\"format_version\":1}', 1, 1, 1)",
                params![conversation_id.to_string(), user_participant.to_string()],
            )
            .expect("user participant");
        transaction
            .execute(
                "INSERT INTO conversation_messages (
                   conversation_id, id, branch_id, author_participant_id, role,
                   timeline_ordinal, logical_time, effective_time, visibility, pinned,
                   scene_edited, active_revision_id, revision, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, 'user', 1, 1, 1, 'visible', 0, 0, ?5, 1, 1, 1)",
                params![
                    conversation_id.to_string(),
                    message_id.to_string(),
                    branch_id.to_string(),
                    user_participant.to_string(),
                    revision_id.to_string()
                ],
            )
            .expect("message");
        transaction
            .execute(
                "INSERT INTO conversation_message_revisions (
                   conversation_id, id, message_id, branch_id, sequence, parts_json,
                   authored_at
                 ) VALUES (?1, ?2, ?3, ?4, 1,
                           '{\"format_version\":1,\"value\":null}', 1)",
                params![
                    conversation_id.to_string(),
                    revision_id.to_string(),
                    message_id.to_string(),
                    branch_id.to_string()
                ],
            )
            .expect("revision");
        crate::conversation::group_companion_adapter::replace_message_checkpoints_in(
            &transaction,
            conversation_id,
            message_id,
            &[
                checkpoint(deleted, survivor, "deleted"),
                checkpoint(survivor, deleted, "survivor"),
            ],
        )
        .expect("checkpoints");
        crate::conversation::group_companion_adapter::record_persona_application_in(
            &transaction,
            conversation_id,
            message_id,
            deleted,
            "__default__",
            TimestampMillis::new(20),
        )
        .expect("persona application");
        transaction.commit().expect("commit");
    }
    database
        .purge_character(deleted, TimestampMillis::new(30))
        .expect("purge character");
    let checkpoints = database
        .checkpoints_for_message(conversation_id, message_id)
        .expect("checkpoints");
    assert_eq!(checkpoints.len(), 1);
    assert_eq!(checkpoints[0].character_id, survivor);
    assert!(checkpoints[0].member_relationships.is_empty());
    assert_eq!(checkpoints[0].effects.len(), 1);
    assert_eq!(
        database
            .recorded_persona_applications(conversation_id, &[message_id], None)
            .expect("persona applications"),
        Vec::new()
    );
}

#[test]
fn perspective_memory_spaces_are_purged_with_their_conversation_and_character() {
    use lettuce_memory::{MemoryCategory, MemoryItem, MemorySpaceSnapshot, Score};
    use lettuce_types::{
        ConversationBranchId, ConversationId, MemoryId, MemorySpaceId, Revision,
    };

    let database = Database::open_in_memory().expect("database");
    let deleted = CharacterId::new();
    let survivor = CharacterId::new();
    let conversation_id = ConversationId::new();
    let branch_id = ConversationBranchId::new();
    let deleted_space = MemorySpaceId::new();
    let survivor_space = MemorySpaceId::new();
    let item = |text: &str| {
        let id = MemoryId::new();
        MemoryItem {
            id,
            short_id: lettuce_memory::MemoryShortId::derived(id),
            text: text.to_owned(),
            origin: lettuce_memory::MemoryOrigin::Model,
            category: Some(MemoryCategory::Other),
            source_message_id: None,
            source_role: None,
            observed_at: None,
            observed_time_precision: None,
            superseded_by: None,
            superseded_at: None,
            supersedes: Vec::new(),
            token_count: Some(3),
            is_cold: false,
            is_pinned: false,
            importance: Score::FULL,
            persistence_importance: Score::FULL,
            prompt_importance: Score::FULL,
            volatility: Score::LEGACY_VOLATILITY,
            access_count: 0,
            created_at: TimestampMillis::new(1),
            last_accessed_at: TimestampMillis::new(1),
        }
    };
    {
        let mut connection = database.connection().expect("lock");
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("transaction");
        for (character_id, name) in [(deleted, "Deleted"), (survivor, "Survivor")] {
            transaction
                .execute(
                    "INSERT INTO characters (
                       id, status, name, normalized_name, profile_json,
                       provenance_json, defaults_json, interaction_mode,
                       memory_policy, voice_autoplay, presentation_json, revision,
                       created_at, updated_at
                     ) VALUES (?1, 'active', ?2, ?3, '{}', '{}', '{}',
                               'companion', 'manual', 0, '{}', 1, 1, 1)",
                    params![character_id.to_string(), name, name.to_ascii_lowercase()],
                )
                .expect("character");
        }
        transaction
            .execute(
                "INSERT INTO conversations (
                   id, kind, lifecycle, title, active_branch_id, kind_json,
                   revision, created_at, updated_at
                 ) VALUES (?1, 'group', 'active', 'Group', ?2,
                           '{\"format_version\":1,\"value\":null}', 1, 1, 1)",
                params![conversation_id.to_string(), branch_id.to_string()],
            )
            .expect("conversation");
        transaction
            .execute(
                "INSERT INTO conversation_branches (
                   conversation_id, id, status, revision, created_at, updated_at
                 ) VALUES (?1, ?2, 'active', 1, 1, 1)",
                params![conversation_id.to_string(), branch_id.to_string()],
            )
            .expect("branch");
        for (character_id, space_id, text) in [
            (deleted, deleted_space, "deleted perspective"),
            (survivor, survivor_space, "survivor perspective"),
        ] {
            crate::memory::memory_adapter::insert_perspective_space_in(
                &transaction,
                conversation_id,
                branch_id,
                character_id,
                &MemorySpaceSnapshot {
                    id: space_id,
                    revision: Revision::INITIAL,
                    items: vec![item(text)],
                },
            )
            .expect("perspective space");
        }
        transaction.commit().expect("commit");
    }
    database
        .purge_character(deleted, TimestampMillis::new(30))
        .expect("purge character");
    {
        let connection = database.connection().expect("lock");
        let remaining: i64 = connection
            .query_row(
                "SELECT count(*) FROM group_companion_memory_spaces WHERE character_id = ?1",
                [deleted.to_string()],
                |row| row.get(0),
            )
            .expect("deleted bindings");
        assert_eq!(remaining, 0);
        let spaces: i64 = connection
            .query_row(
                "SELECT count(*) FROM memory_spaces WHERE id = ?1",
                [deleted_space.to_string()],
                |row| row.get(0),
            )
            .expect("deleted space");
        assert_eq!(spaces, 0);
        let survivor_bindings: i64 = connection
            .query_row(
                "SELECT count(*) FROM group_companion_memory_spaces WHERE character_id = ?1",
                [survivor.to_string()],
                |row| row.get(0),
            )
            .expect("survivor bindings");
        assert_eq!(survivor_bindings, 1);
    }
    database
        .purge_conversation(conversation_id, TimestampMillis::new(40))
        .expect("purge conversation");
    let connection = database.connection().expect("lock");
    let bindings: i64 = connection
        .query_row(
            "SELECT count(*) FROM group_companion_memory_spaces WHERE conversation_id = ?1",
            [conversation_id.to_string()],
            |row| row.get(0),
        )
        .expect("conversation bindings");
    assert_eq!(bindings, 0);
    let spaces: i64 = connection
        .query_row(
            "SELECT count(*) FROM memory_spaces WHERE id IN (?1, ?2)",
            params![deleted_space.to_string(), survivor_space.to_string()],
            |row| row.get(0),
        )
        .expect("remaining spaces");
    assert_eq!(spaces, 0);
}
