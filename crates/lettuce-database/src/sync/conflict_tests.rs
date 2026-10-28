use super::*;
use lettuce_characters::{
    Character, CharacterDefaults, CharacterDetails, CharacterMedia, CharacterPresentationV1,
    CharacterProfile, CharacterProvenance, Scene, SceneDocumentV1, SceneOwner, ScenePart,
};
use lettuce_context::{
    LorebookMetadataDraft, LorebookRepository, PromptMetadataDraft, PromptPurpose, PromptRepository,
};
use lettuce_types::{CharacterId, Revision};

fn fixture(database: &Database, kind: &str) -> (String, CanonicalPayload) {
    match kind {
        "character" => {
            let character = Character::new(
                CharacterId::new(),
                CharacterProfile {
                    name: "Initial".into(),
                    nickname: None,
                    description: None,
                    definition: None,
                    design_description: None,
                    scenario: None,
                },
                CharacterProvenance::default(),
                CharacterDefaults::default(),
                CharacterPresentationV1::default(),
                None,
                CharacterMedia::default(),
                TimestampMillis::new(1),
            )
            .expect("character");
            let id = character.id.to_string();
            let details = CharacterDetails {
                character: character.clone(),
                scenes: vec![Scene::new(
                    lettuce_types::SceneId::new(),
                    SceneOwner::Character(character.id),
                    0,
                    SceneDocumentV1::new(vec![ScenePart::Text {
                        text: "Initial child".into(),
                    }]).expect("document"),
                    TimestampMillis::new(1),
                ).expect("scene")],
                variants: Vec::new(),
                starters: Vec::new(),
            };
            (
                id,
                lettuce_sync::canonical_character_payload(&details).expect("payload"),
            )
        }
        "lorebook" => {
            let details = LorebookRepository::create(
                database,
                LorebookMetadataDraft {
                    name: "Initial".into(),
                    detection_policy: Default::default(),
                    icon_asset_id: None,
                    behavior_version: Default::default(),
                },
                vec![lettuce_context::LorebookEntryDraft {
                    title: "Initial child".into(),
                    enabled: true,
                    always_active: false,
                    keywords: vec!["initial".into()],
                    case_sensitive: false,
                    match_mode: lettuce_context::KeywordMatchMode::Literal,
                    content: "Initial content".into(),
                    priority: 0,
                }],
                TimestampMillis::new(1),
            )
            .expect("lorebook");
            (
                details.book.id.to_string(),
                lettuce_sync::canonical_lorebook_payload(&details).expect("payload"),
            )
        }
        "prompt" => {
            let document = PromptRepository::create_user_draft(
                database,
                PromptMetadataDraft {
                    name: "Initial".into(),
                    purpose: PromptPurpose::DirectChat,
                    condense: false,
                    behavior_version: Default::default(),
                },
                vec![lettuce_context::PromptEntryDraft {
                    built_in_entry_key: None,
                    name: "Initial child".into(),
                    role: lettuce_context::PromptEntryRole::System,
                    content: "Initial content".into(),
                    enabled: true,
                    injection_position: lettuce_context::PromptEntryPosition::Relative,
                    depth: 0,
                    conditional_min_messages: None,
                    interval_turns: None,
                    system_prompt: false,
                    conditions: None,
                    payload: None,
                }],
                TimestampMillis::new(1),
            )
            .expect("prompt");
            (
                document.id.to_string(),
                lettuce_sync::canonical_prompt_payload(&document).expect("payload"),
            )
        }
        _ => panic!("fixture kind"),
    }
}

fn edit(database: &Database, kind: &str, id: &str, name: &str, at: i64) {
    let codec = snapshot_codec(kind).expect("codec");
    let mut connection = database.connection().expect("connection");
    let tx = connection.transaction().expect("transaction");
    let payload = (codec.current)(&tx, id)
        .expect("current")
        .expect("present");
    let mut value: serde_json::Value = serde_json::from_slice(payload.bytes()).expect("value");
    let root = match kind {
        "character" => &mut value["character"],
        "lorebook" => &mut value["book"],
        _ => &mut value,
    };
    if kind == "character" {
        root["profile"]["name"] = name.into();
    } else {
        root["name"] = name.into();
    }
    let revision: Revision = serde_json::from_value(root["revision"].clone()).expect("revision");
    root["revision"] =
        serde_json::to_value(revision.next().expect("next revision")).expect("revision");
    root["updated_at"] = serde_json::to_value(TimestampMillis::new(at)).expect("timestamp");
    let key = if kind == "character" { "scenes" } else { "entries" };
    let template = value[key][0].clone();
    value[key] = serde_json::Value::Array((0..if name == "Source" { 2 } else { 1 }).map(|ordinal| {
        let mut child = template.clone();
        child["id"] = lettuce_types::OperationId::new().to_string().into();
        if kind == "character" {
            child["content"]["parts"][0]["text"] = format!("{name} child {ordinal}").into();
        } else {
            child["content"] = format!("{name} child {ordinal}").into();
        }
        if kind != "prompt" {
            child["ordinal"] = ordinal.into();
        }
        child
    }).collect());
    (codec.materialize)(&tx, id, &serde_json::to_vec(&value).expect("payload"))
        .expect("edit");
    tx.commit().expect("commit");
}

fn replicate(from: &Database, to: &Database, at: i64) {
    from.journal_current_state(TimestampMillis::new(at))
        .expect("journal");
    let frontier = to.local_frontier().expect("frontier");
    let batch = from
        .outbound_changes(&frontier, MAX_OUTBOUND_CHANGES, MAX_OUTBOUND_PAYLOAD_BYTES)
        .expect("outbound");
    if batch.changes.is_empty() {
        return;
    }
    let id = OperationId::new();
    let peer = batch.changes[0].origin_device();
    to.stage_incoming_batch(
        peer,
        id,
        &canonical_batch_hash(&batch.changes),
        &batch.changes,
        TimestampMillis::new(at),
    )
    .expect("stage");
    assert_eq!(
        to.apply_incoming_batch(id, TimestampMillis::new(at))
            .expect("apply")
            .state,
        IncomingBatchState::Committed
    );
}

#[test]
fn character_lorebook_and_prompt_resolutions_propagate_replay_and_reject_stale() {
    for kind in ["character", "lorebook", "prompt"] {
        let source = Database::open_in_memory().expect("source");
        let target = Database::open_in_memory().expect("target");
        let (id, payload) = fixture(&source, kind);
        {
            let mut connection = source.connection().expect("connection");
            let tx = connection.transaction().expect("transaction");
            (snapshot_codec(kind).expect("codec").materialize)(&tx, &id, payload.bytes())
                .expect("initial");
            tx.commit().expect("commit");
        }
        replicate(&source, &target, 2);
        edit(&source, kind, &id, "Source", 3);
        edit(&target, kind, &id, "Target", 4);
        source
            .journal_current_state(TimestampMillis::new(3))
            .expect("journal source");
        target
            .journal_current_state(TimestampMillis::new(4))
            .expect("journal target");
        replicate(&source, &target, 5);
        replicate(&target, &source, 6);
        let conflicts = target.unresolved_sync_conflicts().expect("conflicts");
        assert_eq!(conflicts.len(), 1, "{kind}");
        let conflict = &conflicts[0];
        let operation = OperationId::new();
        assert_eq!(
            target.resolve_sync_conflict(
                conflict.id,
                None,
                ConflictChoice::Other,
                operation,
                TimestampMillis::new(7)
            ),
            Err(ConflictRepositoryError::Conflict)
        );
        let resolution = target
            .resolve_sync_conflict(
                conflict.id,
                conflict.current.change_id,
                ConflictChoice::Other,
                operation,
                TimestampMillis::new(7),
            )
            .expect("resolve");
        assert_eq!(
            target
                .resolve_sync_conflict(
                    conflict.id,
                    conflict.current.change_id,
                    ConflictChoice::Other,
                    operation,
                    TimestampMillis::new(8)
                )
                .expect("replay"),
            resolution
        );
        assert_eq!(
            target.resolve_sync_conflict(
                conflict.id,
                conflict.current.change_id,
                ConflictChoice::Current,
                operation,
                TimestampMillis::new(8)
            ),
            Err(ConflictRepositoryError::Conflict)
        );
        assert!(
            target
                .unresolved_sync_conflicts()
                .expect("resolved")
                .is_empty()
        );
        replicate(&target, &source, 9);
        assert!(
            source
                .unresolved_sync_conflicts()
                .expect("superseded")
                .is_empty()
        );
        let codec = snapshot_codec(kind).expect("codec");
        let value = |database: &Database| {
            let mut connection = database.connection().expect("connection");
            let tx = connection.transaction().expect("transaction");
            (codec.current)(&tx, &id).expect("current")
        };
        assert_eq!(value(&source), value(&target));
    }
}

#[test]
fn deleted_current_is_acknowledged_and_other_cannot_recreate_the_character() {
    let source = Database::open_in_memory().expect("source");
    let target = Database::open_in_memory().expect("target");
    let (id, payload) = fixture(&source, "character");
    {
        let mut connection = source.connection().expect("connection");
        let tx = connection.transaction().expect("transaction");
        (CHARACTER_CODEC.materialize)(&tx, &id, payload.bytes())
            .expect("initial");
        tx.commit().expect("commit");
    }
    replicate(&source, &target, 2);
    target
        .purge_character(id.parse().expect("id"), TimestampMillis::new(3))
        .expect("delete");
    target
        .journal_current_state(TimestampMillis::new(3))
        .expect("journal delete");
    edit(&source, "character", &id, "Unobserved peer edit", 4);
    replicate(&source, &target, 5);
    let conflicts = target.unresolved_sync_conflicts().expect("conflicts");
    assert_eq!(conflicts.len(), 1);
    let conflict = &conflicts[0];
    assert_eq!(conflict.current.value, SyncConflictValue::Deleted);
    assert!(matches!(
        target.resolve_sync_conflict(
            conflict.id,
            conflict.current.change_id,
            ConflictChoice::Other,
            OperationId::new(),
            TimestampMillis::new(6)
        ),
        Err(ConflictRepositoryError::Unsupported { .. })
    ));
    let operation = OperationId::new();
    let resolution = target
        .resolve_sync_conflict(
            conflict.id,
            conflict.current.change_id,
            ConflictChoice::Current,
            operation,
            TimestampMillis::new(7),
        )
        .expect("acknowledge delete");
    assert_eq!(resolution.operation(), ChangeOperation::Delete);
    assert_eq!(
        target
            .resolve_sync_conflict(
                conflict.id,
                conflict.current.change_id,
                ConflictChoice::Current,
                operation,
                TimestampMillis::new(8)
            )
            .expect("replay"),
        resolution
    );
    assert!(
        target
            .unresolved_sync_conflicts()
            .expect("resolved")
            .is_empty()
    );
    assert!(
        crate::catalog::character_adapter::load_character_details(
            &target.connection().expect("connection"),
            id.parse().expect("id")
        )
        .expect("read")
        .is_none()
    );
}

#[test]
fn resolution_receipt_failure_rolls_back_entity_journal_and_inbox() {
    let source = Database::open_in_memory().expect("source");
    let path = std::env::temp_dir().join(format!(
        "sync-resolution-restart-{}.sqlite3",
        OperationId::new()
    ));
    let target = Database::open(&path).expect("target");
    let (id, _) = fixture(&source, "prompt");
    replicate(&source, &target, 2);
    edit(&source, "prompt", &id, "Source", 3);
    edit(&target, "prompt", &id, "Target", 4);
    source
        .journal_current_state(TimestampMillis::new(3))
        .expect("source journal");
    target
        .journal_current_state(TimestampMillis::new(4))
        .expect("target journal");
    replicate(&source, &target, 5);
    let conflict = target
        .unresolved_sync_conflicts()
        .expect("conflicts")
        .remove(0);
    target.connection().expect("connection").execute_batch("CREATE TEMP TRIGGER fail_sync_receipt BEFORE INSERT ON api_operation_receipts BEGIN SELECT RAISE(ABORT, 'receipt fault'); END;").expect("fault");
    let operation = OperationId::new();
    let resolve = || {
        target.commit_api_operation::<String, crate::ApiOperationError>(
            "sync_conflict_resolve",
            "decision",
            "digest",
            TimestampMillis::new(6),
            |tx| {
                tx.resolve_sync_conflict(
                    conflict.id,
                    conflict.current.change_id,
                    ConflictChoice::Other,
                    operation,
                    TimestampMillis::new(6),
                )
                .map(|change| change.id().as_uuid().to_string())
                .map_err(|error| { tracing::warn!(%error, "resolution receipt failure rolls back entity journal and inbox failed"); crate::ApiOperationError::Storage { reason: "injected failure" } })
            },
        )
    };
    assert!(matches!(
        resolve(),
        Err(crate::ApiOperationError::Storage { .. })
    ));
    assert!(
        target
            .local_change_for_operation(operation)
            .expect("journal")
            .is_none()
    );
    assert_eq!(
        target.unresolved_sync_conflicts().expect("inbox"),
        vec![conflict.clone()]
    );
    target
        .connection()
        .expect("connection")
        .execute_batch("DROP TRIGGER fail_sync_receipt")
        .expect("remove fault");
    let result = resolve().expect("retry");
    assert_eq!(resolve().expect("replay"), result);
    assert_eq!(
        target.commit_api_operation::<String, crate::ApiOperationError>(
            "sync_conflict_resolve",
            "decision",
            "changed",
            TimestampMillis::new(7),
            |_| panic!("changed digest must not execute")
        ),
        Err(crate::ApiOperationError::Conflict)
    );
    drop(target);
    let reopened = Database::open(&path).expect("restart");
    assert_eq!(
        reopened.commit_api_operation::<String, crate::ApiOperationError>(
            "sync_conflict_resolve",
            "decision",
            "digest",
            TimestampMillis::new(8),
            |_| panic!("restart must replay")
        ),
        Ok(result)
    );
    assert!(
        reopened
            .unresolved_sync_conflicts()
            .expect("closed inbox")
            .is_empty()
    );
    assert!(
        reopened
            .local_change_for_operation(operation)
            .expect("resolution survived")
            .is_some()
    );
    drop(reopened);
    std::fs::remove_file(path).expect("cleanup");
}

#[test]
fn legacy_dismiss_marker_is_the_only_mutable_evidence_field() {
    let database = Database::open_in_memory().expect("database");
    let connection = database.connection().expect("connection");
    connection.execute("INSERT INTO legacy_import_runs(id,source_schema_version,inventory_fingerprint,plan_fingerprint,status,admitted_at,updated_at) VALUES ('run',1,?1,?1,'admitting',1,1)", ["a".repeat(64)]).expect("run");
    connection.execute("INSERT INTO legacy_import_preserved_rows(run_id,source_table,source_key,row_json) VALUES ('run','sync_v2_conflicts','conflict','{}')", []).expect("evidence");
    connection
        .execute(
            "UPDATE legacy_import_preserved_rows SET dismissed_at=2 WHERE run_id='run'",
            [],
        )
        .expect("dismiss marker");
    assert!(
        connection
            .execute("UPDATE legacy_import_preserved_rows SET dismissed_at=3", [])
            .is_err()
    );
    assert!(
        connection
            .execute(
                "UPDATE legacy_import_preserved_rows SET dismissed_at=NULL",
                []
            )
            .is_err()
    );
    assert!(
        connection
            .execute(
                "UPDATE legacy_import_preserved_rows SET row_json='{\"changed\":true}'",
                []
            )
            .is_err()
    );
    assert!(
        connection
            .execute(
                "UPDATE legacy_import_preserved_rows SET source_key='changed'",
                []
            )
            .is_err()
    );
    assert!(
        connection
            .execute("DELETE FROM legacy_import_preserved_rows", [])
            .is_err()
    );
    let preserved: (String, i64) = connection
        .query_row(
            "SELECT row_json,dismissed_at FROM legacy_import_preserved_rows",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("preserved");
    assert_eq!(preserved, ("{}".into(), 2));
}

fn legacy_snapshot(id: &str, name: &str) -> String {
    legacy_snapshot_field(id, "name", name)
}

fn legacy_snapshot_field(id: &str, key: &str, name: &str) -> String {
    let mut bytes = Vec::new();
    let text = |bytes: &mut Vec<u8>, value: &str| {
        bytes.extend_from_slice(&(value.len() as u64).to_le_bytes());
        bytes.extend_from_slice(value.as_bytes());
    };
    bytes.extend_from_slice(&2_u64.to_le_bytes());
    text(&mut bytes, "id");
    text(&mut bytes, key);
    bytes.extend_from_slice(&1_u64.to_le_bytes());
    bytes.extend_from_slice(&3_u32.to_le_bytes());
    text(&mut bytes, id);
    bytes.push(1);
    bytes.extend_from_slice(&2_u64.to_le_bytes());
    for value in [id, name] {
        bytes.extend_from_slice(&3_u32.to_le_bytes());
        text(&mut bytes, value);
    }
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn legacy_inbox_has_three_values_and_dismiss_only_changes_the_marker() {
    let database = Database::open_in_memory().expect("database");
    let source_id = CharacterId::new().to_string();
    let destination = CharacterId::from_uuid(
        lettuce_transfer::LegacyIdScope::new().source("character", &source_id),
    );
    let (_, payload) = fixture(&database, "character");
    let mut details: CharacterDetails = serde_json::from_slice(payload.bytes()).expect("details");
    details.character.id = destination;
    for scene in &mut details.scenes {
        scene.owner = SceneOwner::Character(destination);
    }
    let run_id = lettuce_types::LegacyImportRunId::new();
    let row_json = serde_json::json!({
        "conflict_id": "legacy-conflict", "status": "unresolved", "table_name": "characters", "detected_at": 10,
        "local_row": {"hex": legacy_snapshot(&source_id, "Local")},
        "incoming_row": {"hex": legacy_snapshot(&source_id, "Incoming")},
    }).to_string();
    {
        let mut connection = database.connection().expect("connection");
        let tx = connection.transaction().expect("transaction");
        crate::catalog::character_adapter::sync_replace_character(&tx, &details)
            .expect("character");
        tx.execute("INSERT INTO legacy_import_runs(id,source_schema_version,inventory_fingerprint,plan_fingerprint,status,admitted_at,updated_at) VALUES (?1,1,?2,?2,'admitting',1,1)", params![run_id.to_string(), "a".repeat(64)]).expect("run");
        tx.execute("INSERT INTO legacy_import_preserved_rows(run_id,source_table,source_key,row_json) VALUES (?1,'sync_v2_conflicts','legacy-conflict',?2)", params![run_id.to_string(), row_json]).expect("evidence");
        tx.commit().expect("commit");
    }
    let conflicts = database.unresolved_legacy_conflicts().expect("inbox");
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].fields.len(), 1);
    assert_eq!(conflicts[0].fields[0].key, "name");
    assert_eq!(
        conflicts[0].fields[0].current,
        crate::LegacyConflictCurrent::Value(serde_json::Value::from("Initial"))
    );
    assert_eq!(conflicts[0].fields[0].local, "Local");
    assert_eq!(conflicts[0].fields[0].incoming, "Incoming");
    database
        .commit_api_operation::<(), crate::ApiOperationError>(
            "legacy_conflict_dismiss",
            "dismiss",
            "digest",
            TimestampMillis::new(11),
            |tx| {
                tx.dismiss_legacy_conflict(run_id, "legacy-conflict", TimestampMillis::new(11))
                    .map_err(|error| { tracing::warn!(%error, "legacy inbox has three values and dismiss only changes the marker failed"); crate::ApiOperationError::Storage { reason: "injected failure" } })
            },
        )
        .expect("dismiss");
    database
        .commit_api_operation::<(), crate::ApiOperationError>(
            "legacy_conflict_dismiss",
            "dismiss",
            "digest",
            TimestampMillis::new(12),
            |_| panic!("same digest must replay"),
        )
        .expect("dismiss replay");
    assert_eq!(
        database.commit_api_operation::<(), crate::ApiOperationError>(
            "legacy_conflict_dismiss",
            "dismiss",
            "changed",
            TimestampMillis::new(12),
            |_| panic!("changed digest must not execute")
        ),
        Err(crate::ApiOperationError::Conflict)
    );
    assert!(
        database
            .unresolved_legacy_conflicts()
            .expect("closed inbox")
            .is_empty()
    );
    let preserved: String = database
        .connection()
        .expect("connection")
        .query_row(
            "SELECT row_json FROM legacy_import_preserved_rows",
            [],
            |row| row.get(0),
        )
        .expect("preserved");
    assert_eq!(preserved, row_json);
    assert_eq!(
        crate::catalog::character_adapter::load_character_details(
            &database.connection().expect("connection"),
            destination
        )
        .expect("current"),
        Some(details)
    );
}

#[test]
fn legacy_conflict_without_incoming_snapshot_stays_only_in_provenance() {
    let database = Database::open_in_memory().expect("database");
    let run = lettuce_types::LegacyImportRunId::new();
    let source = CharacterId::new().to_string();
    let row_json = serde_json::json!({"status":"unresolved","table_name":"characters","detected_at":10,"local_row":{"hex":legacy_snapshot(&source,"Local")},"incoming_row":null}).to_string();
    {
        let connection = database.connection().expect("connection");
        connection.execute("INSERT INTO legacy_import_runs(id,source_schema_version,inventory_fingerprint,plan_fingerprint,status,admitted_at,updated_at) VALUES (?1,1,?2,?2,'admitting',1,1)", params![run.to_string(), "a".repeat(64)]).expect("run");
        connection.execute("INSERT INTO legacy_import_preserved_rows(run_id,source_table,source_key,row_json) VALUES (?1,'sync_v2_conflicts','missing',?2)", params![run.to_string(), row_json]).expect("evidence");
    }
    assert!(
        database
            .unresolved_legacy_conflicts()
            .expect("inbox")
            .is_empty()
    );
    let connection = database.connection().expect("connection");
    let preserved: (String, Option<i64>) = connection
        .query_row(
            "SELECT row_json,dismissed_at FROM legacy_import_preserved_rows",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("evidence");
    assert_eq!(preserved, (row_json, None));
}

#[test]
fn resolution_racing_incoming_apply_never_overwrites_an_unobserved_current_snapshot() {
    for kind in ["character", "lorebook", "prompt"] {
        let source = Database::open_in_memory().expect("source");
        let target = std::sync::Arc::new(Database::open_in_memory().expect("target"));
        let (id, payload) = fixture(&source, kind);
        {
            let mut connection = source.connection().expect("connection");
            let tx = connection.transaction().expect("transaction");
            (snapshot_codec(kind).expect("codec").materialize)(&tx, &id, payload.bytes())
                .expect("initial");
            tx.commit().expect("commit");
        }
        replicate(&source, &target, 2);
        edit(&source, kind, &id, "Source", 3);
        edit(&target, kind, &id, "Target", 4);
        source
            .journal_current_state(TimestampMillis::new(3))
            .expect("source journal");
        target
            .journal_current_state(TimestampMillis::new(4))
            .expect("target journal");
        replicate(&source, &target, 5);
        let conflict = target.unresolved_sync_conflicts().expect("inbox").remove(0);
        edit(&source, kind, &id, "Later source", 1_000);
        source
            .journal_current_state(TimestampMillis::new(1_000))
            .expect("later journal");
        let batch = source
            .outbound_changes(
                &target.local_frontier().expect("frontier"),
                MAX_OUTBOUND_CHANGES,
                MAX_OUTBOUND_PAYLOAD_BYTES,
            )
            .expect("batch");
        let batch_id = OperationId::new();
        target
            .stage_incoming_batch(
                batch.changes[0].origin_device(),
                batch_id,
                &canonical_batch_hash(&batch.changes),
                &batch.changes,
                TimestampMillis::new(1_001),
            )
            .expect("stage");
        let operation = OperationId::new();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let apply_target = target.clone();
        let apply_barrier = barrier.clone();
        let apply = std::thread::spawn(move || {
            apply_barrier.wait();
            apply_target
                .apply_incoming_batch(batch_id, TimestampMillis::new(1_001))
                .expect("apply")
        });
        barrier.wait();
        let resolution = target.resolve_sync_conflict(
            conflict.id,
            conflict.current.change_id,
            ConflictChoice::Other,
            operation,
            TimestampMillis::new(1_002),
        );
        assert_eq!(
            apply.join().expect("worker").state,
            IncomingBatchState::Committed
        );
        match resolution {
            Ok(change) => assert_eq!(
                target
                    .local_change_for_operation(operation)
                    .expect("journal"),
                Some(change)
            ),
            Err(error) => {
                assert_eq!(error, ConflictRepositoryError::Conflict);
                assert!(
                    target
                        .local_change_for_operation(operation)
                        .expect("no overwrite")
                        .is_none()
                );
            }
        }
    }
}

#[test]
fn fork_acknowledgment_is_atomic_with_its_receipt_and_keeps_the_branch() {
    let database = Database::open_in_memory().expect("database");
    let conversation = lettuce_types::ConversationId::new();
    let branch = lettuce_types::ConversationBranchId::new();
    {
        let mut connection = database.connection().expect("connection");
        let tx = connection.transaction().expect("transaction");
        tx.execute("INSERT INTO conversations(id,kind,lifecycle,title,active_branch_id,kind_json,revision,created_at,updated_at) VALUES (?1,'direct','active','Fork receipt',?2,'{\"format_version\":1}',1,1,1)", params![conversation.to_string(),branch.to_string()]).expect("conversation");
        tx.execute("INSERT INTO conversation_branches(conversation_id,id,status,revision,created_at,updated_at) VALUES (?1,?2,'active',1,1,1)", params![conversation.to_string(),branch.to_string()]).expect("branch");
        tx.execute("INSERT INTO sync_conversation_forks(conversation_id,branch_id,holds_local,detected_at) VALUES (?1,?2,1,1)", params![conversation.to_string(),branch.to_string()]).expect("notice");
        tx.commit().expect("commit");
        connection.execute_batch("CREATE TEMP TRIGGER fail_fork_receipt BEFORE INSERT ON api_operation_receipts BEGIN SELECT RAISE(ABORT, 'receipt fault'); END;").expect("fault");
    }
    let acknowledge = || {
        database.commit_api_operation::<(), crate::ApiOperationError>(
            "sync_fork_resolve",
            "fork",
            "digest",
            TimestampMillis::new(2),
            |tx| {
                tx.resolve_conversation_fork(conversation, branch, TimestampMillis::new(2))
                    .map_err(|error| { tracing::warn!(%error, "fork acknowledgment is atomic with its receipt and keeps the branch failed"); crate::ApiOperationError::Storage { reason: "injected failure" } })
            },
        )
    };
    assert!(matches!(
        acknowledge(),
        Err(crate::ApiOperationError::Storage { .. })
    ));
    assert_eq!(
        lettuce_sync::ConversationForkRepository::unresolved_conversation_forks(&database)
            .expect("notice retained")
            .len(),
        1
    );
    database
        .connection()
        .expect("connection")
        .execute_batch("DROP TRIGGER fail_fork_receipt")
        .expect("remove fault");
    acknowledge().expect("acknowledge");
    acknowledge().expect("replay");
    assert_eq!(
        database.commit_api_operation::<(), crate::ApiOperationError>(
            "sync_fork_resolve",
            "fork",
            "changed",
            TimestampMillis::new(3),
            |_| panic!("changed digest must not execute")
        ),
        Err(crate::ApiOperationError::Conflict)
    );
    assert!(
        lettuce_sync::ConversationForkRepository::unresolved_conversation_forks(&database)
            .expect("closed")
            .is_empty()
    );
    assert_eq!(
        database
            .connection()
            .expect("connection")
            .query_row(
                "SELECT status FROM conversation_branches WHERE conversation_id=?1 AND id=?2",
                params![conversation.to_string(), branch.to_string()],
                |row| row.get::<_, String>(0)
            )
            .expect("kept branch"),
        "active"
    );
}

#[test]
fn missing_legacy_entity_fields_are_not_mapped() {
    let database = Database::open_in_memory().expect("database");
    for table in ["characters", "personas", "lorebooks", "lorebook_entries", "prompt_templates"] {
        let run = lettuce_types::LegacyImportRunId::new();
        let source = CharacterId::new().to_string();
        let kind = match table {
            "personas" => "persona",
            "lorebooks" => "lorebook",
            "lorebook_entries" => "lorebook_entry",
            "prompt_templates" => "prompt",
            _ => "character",
        };
        let key = if matches!(table, "personas" | "lorebook_entries") { "title" } else { "name" };
        let row = serde_json::json!({"status":"unresolved","table_name":table,"detected_at":10,
            "local_row":{"hex":legacy_snapshot_field(&source,key,"Local")},
            "incoming_row":{"hex":legacy_snapshot_field(&source,key,"Incoming")}}).to_string();
        let connection = database.connection().expect("connection");
        connection.execute("INSERT INTO legacy_import_runs(id,source_schema_version,inventory_fingerprint,plan_fingerprint,status,admitted_at,updated_at) VALUES (?1,1,?2,?2,'admitting',1,1)", params![run.to_string(), "a".repeat(64)]).expect("run");
        if kind != "character" {
            connection.execute("INSERT INTO legacy_import_assignments(run_id,source_kind,source_key,destination_id) VALUES (?1,?2,?3,?4)", params![run.to_string(), kind, source, lettuce_types::OperationId::new().to_string()]).expect("imported identity of missing entity");
        }
        connection.execute("INSERT INTO legacy_import_preserved_rows(run_id,source_table,source_key,row_json) VALUES (?1,'sync_v2_conflicts',?2,?3)", params![run.to_string(), table, row]).expect("evidence");
    }
    let conflicts = database.unresolved_legacy_conflicts().expect("inbox");
    assert_eq!(conflicts.len(), 5);
    for conflict in conflicts {
        assert!(!conflict.fields.is_empty());
        assert!(conflict.fields.iter().all(|field| field.current == crate::LegacyConflictCurrent::NotMapped), "{}", conflict.entity_kind);
    }
}

#[test]
fn aggregate_resolutions_replace_or_keep_the_chosen_children() {
    for kind in ["character", "lorebook", "prompt"] {
        for choice in [ConflictChoice::Current, ConflictChoice::Other] {
            let source = Database::open_in_memory().expect("source");
            let target = Database::open_in_memory().expect("target");
            let (id, payload) = fixture(&source, kind);
            {
                let mut connection = source.connection().expect("connection");
                let tx = connection.transaction().expect("transaction");
                (snapshot_codec(kind).expect("codec").materialize)(&tx, &id, payload.bytes()).expect("initial");
                tx.commit().expect("commit");
            }
            replicate(&source, &target, 2);
            edit(&source, kind, &id, "Source", 3);
            edit(&target, kind, &id, "Target", 4);
            target.journal_current_state(TimestampMillis::new(4)).expect("journal");
            replicate(&source, &target, 5);
            let conflict = target.unresolved_sync_conflicts().expect("inbox").remove(0);
            let children = |database: &Database| {
                let mut connection = database.connection().expect("connection");
                let tx = connection.transaction().expect("transaction");
                let payload = (snapshot_codec(kind).expect("codec").current)(&tx, &id).expect("current").expect("present");
                let value: serde_json::Value = serde_json::from_slice(payload.bytes()).expect("snapshot");
                value[if kind == "character" { "scenes" } else { "entries" }].clone()
            };
            let source_children = children(&source);
            let current_children = children(&target);
            assert_ne!(source_children, current_children, "{kind}");
            assert_eq!(source_children.as_array().expect("children").len(), 2);
            assert_eq!(current_children.as_array().expect("children").len(), 1);
            target.resolve_sync_conflict(conflict.id, conflict.current.change_id, choice, OperationId::new(), TimestampMillis::new(6)).expect("resolve");
            assert_eq!(children(&target), if choice == ConflictChoice::Other { source_children } else { current_children }, "{kind} {choice:?}");
        }
    }
}
