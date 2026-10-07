use super::tests::{Reply, harness, launch};
use super::*;
use lettuce_contracts::{self as dto, ApiErrorCode};
use lettuce_conversations::ConversationReader;
use lettuce_memory::{MemoryRepository, MemorySummaryRepository};

#[tokio::test]
async fn manual_memory_without_tokenizer_replays_and_uses_conversation_membership() {
    let harness = harness(Reply::Text("reply"));
    let conversation_id = launch(&harness, "manual-memory-launch").await;
    let request = dto::MemoryAddRequest {
        conversation_id: conversation_id.clone(),
        text: "A manual memory".into(),
        category: None,
        observed_at: Some(5),
        expected_revision: 1,
        client_operation_id: "manual-add".into(),
    };
    let added = memory_add(&harness.context, request.clone())
        .await
        .expect("manual add without tokenizer");
    assert_eq!(
        added,
        memory_add(&harness.context, request.clone())
            .await
            .expect("replay")
    );
    let mut changed = request;
    changed.text = "Different memory".into();
    assert_eq!(
        memory_add(&harness.context, changed)
            .await
            .expect_err("changed request")
            .code,
        ApiErrorCode::Conflict
    );
    let database = harness.context.backend().database();
    let conversation =
        ConversationReader::get(database, conversation_id.parse().expect("conversation"))
            .expect("conversation")
            .conversation;
    let before =
        MemoryRepository::get_for_branch(database, conversation.id, conversation.active_branch_id)
            .expect("memory")
            .expect("space");
    assert_eq!(before.items[0].token_count, None);
    assert_eq!(
        before.items[0].observed_time_precision.as_deref(),
        Some("user")
    );
    let other = launch(&harness, "other-memory-launch").await;
    let foreign = dto::MemoryPinRequest {
        conversation_id: other,
        memory_id: added.memory_id.expect("item"),
        pinned: true,
        expected_revision: 1,
        client_operation_id: "foreign-pin".into(),
    };
    assert_eq!(
        memory_pin(&harness.context, foreign)
            .await
            .expect_err("foreign memory")
            .code,
        ApiErrorCode::NotFound
    );
    assert_eq!(
        MemoryRepository::get(database, before.id).expect("memory"),
        Some(before)
    );
}

#[tokio::test]
async fn user_summary_without_tokenizer_and_clear_commit_replayable_revisions() {
    let harness = harness(Reply::Text("reply"));
    let conversation_id = launch(&harness, "user-summary-launch").await;
    let request = dto::MemorySummaryUpdateRequest {
        conversation_id: conversation_id.clone(),
        summary: dto::MemorySummaryEdit::Set {
            text: "User summary".into(),
        },
        expected_revision: 1,
        client_operation_id: "summary-set".into(),
    };
    let result = memory_summary_update(&harness.context, request.clone())
        .await
        .expect("summary without tokenizer");
    assert_eq!(result.revision, 2);
    assert_eq!(
        result,
        memory_summary_update(&harness.context, request)
            .await
            .expect("replay")
    );
    let database = harness.context.backend().database();
    let conversation =
        ConversationReader::get(database, conversation_id.parse().expect("conversation"))
            .expect("conversation")
            .conversation;
    let space =
        MemoryRepository::get_for_branch(database, conversation.id, conversation.active_branch_id)
            .expect("memory")
            .expect("space");
    let summary = database
        .get_summary(space.id)
        .expect("summary")
        .expect("present");
    assert_eq!(summary.origin, lettuce_memory::MemoryOrigin::User);
    assert_eq!(summary.token_count, None);
    let clear = dto::MemorySummaryUpdateRequest {
        conversation_id,
        summary: dto::MemorySummaryEdit::Clear,
        expected_revision: 2,
        client_operation_id: "summary-clear".into(),
    };
    let cleared = memory_summary_update(&harness.context, clear.clone())
        .await
        .expect("clear");
    assert_eq!(cleared.revision, 3);
    assert_eq!(
        cleared,
        memory_summary_update(&harness.context, clear)
            .await
            .expect("clear replay")
    );
    assert!(database.get_summary(space.id).expect("summary").is_none());
}

#[tokio::test]
async fn manual_setters_replay_conflict_and_preserve_legacy_pin_temperature_rules() {
    let harness = harness(Reply::Text("reply"));
    let conversation_id = launch(&harness, "manual-setters-launch").await;
    let added = memory_add(
        &harness.context,
        dto::MemoryAddRequest {
            conversation_id: conversation_id.clone(),
            text: "Original memory".into(),
            category: Some(dto::MemoryCategory::Other),
            observed_at: Some(1),
            expected_revision: 1,
            client_operation_id: "setters-add".into(),
        },
    )
    .await
    .expect("add");
    let id = added.memory_id.expect("id");
    let update = dto::MemoryUpdateRequest {
        conversation_id: conversation_id.clone(),
        memory_id: id.clone(),
        text: Some("Changed memory".into()),
        category: dto::MemoryCategoryChange::Set(None),
        observed_at: dto::MemoryObservedAtChange::Set(None),
        expected_revision: 2,
        client_operation_id: "setter-update".into(),
    };
    let updated = memory_update(&harness.context, update.clone())
        .await
        .expect("update");
    assert_eq!(
        updated,
        memory_update(&harness.context, update.clone())
            .await
            .expect("update replay")
    );
    let mut changed = update;
    changed.text = Some("Another memory".into());
    assert_eq!(
        memory_update(&harness.context, changed)
            .await
            .expect_err("update digest")
            .code,
        ApiErrorCode::Conflict
    );
    let cold = dto::MemoryTemperatureRequest {
        conversation_id: conversation_id.clone(),
        memory_id: id.clone(),
        temperature: dto::MemoryTemperature::Cold,
        expected_revision: 3,
        client_operation_id: "setter-cold".into(),
    };
    let cooled = memory_set_temperature(&harness.context, cold.clone())
        .await
        .expect("cold");
    assert_eq!(
        cooled,
        memory_set_temperature(&harness.context, cold.clone())
            .await
            .expect("cold replay")
    );
    let mut changed = cold.clone();
    changed.temperature = dto::MemoryTemperature::Hot;
    assert_eq!(
        memory_set_temperature(&harness.context, changed)
            .await
            .expect_err("temperature digest")
            .code,
        ApiErrorCode::Conflict
    );
    let pin = dto::MemoryPinRequest {
        conversation_id: conversation_id.clone(),
        memory_id: id.clone(),
        pinned: true,
        expected_revision: 4,
        client_operation_id: "setter-pin".into(),
    };
    let pinned = memory_pin(&harness.context, pin.clone())
        .await
        .expect("pin");
    assert_eq!(
        pinned,
        memory_pin(&harness.context, pin.clone())
            .await
            .expect("pin replay")
    );
    let mut changed = pin;
    changed.pinned = false;
    assert_eq!(
        memory_pin(&harness.context, changed)
            .await
            .expect_err("pin digest")
            .code,
        ApiErrorCode::Conflict
    );
    let mut refused = cold;
    refused.expected_revision = 5;
    refused.client_operation_id = "pinned-cold".into();
    assert_eq!(
        memory_set_temperature(&harness.context, refused)
            .await
            .expect_err("pinned cold")
            .code,
        ApiErrorCode::InvalidInput
    );
    let database = harness.context.backend().database();
    let conversation =
        ConversationReader::get(database, conversation_id.parse().expect("conversation"))
            .expect("conversation")
            .conversation;
    let snapshot =
        MemoryRepository::get_for_branch(database, conversation.id, conversation.active_branch_id)
            .expect("memory")
            .expect("present");
    let item = &snapshot.items[0];
    assert!(item.is_pinned);
    assert!(!item.is_cold);
    assert_eq!(item.importance, lettuce_memory::Score::FULL);
    assert_eq!(item.text, "Changed memory");
    assert_eq!(item.category, None);
    assert_eq!(item.observed_at, None);
    let delete = dto::MemoryDeleteRequest {
        conversation_id,
        memory_id: id,
        expected_revision: 5,
        client_operation_id: "setter-delete".into(),
    };
    let deleted = memory_delete(&harness.context, delete.clone())
        .await
        .expect("delete");
    assert_eq!(
        deleted,
        memory_delete(&harness.context, delete.clone())
            .await
            .expect("delete replay")
    );
    let mut changed = delete;
    changed.expected_revision = 6;
    assert_eq!(
        memory_delete(&harness.context, changed)
            .await
            .expect_err("delete digest")
            .code,
        ApiErrorCode::Conflict
    );
    assert!(
        MemoryRepository::get(database, snapshot.id)
            .expect("memory")
            .expect("present")
            .items
            .is_empty()
    );
}

#[tokio::test]
async fn dynamic_add_and_metadata_update_require_embedding_without_writing() {
    use lettuce_characters::CharacterRepository;
    use lettuce_settings::GlobalSettingsStore;
    let harness = harness(Reply::Text("reply"));
    let conversation_id = launch(&harness, "dynamic-manual-launch").await;
    let added = memory_add(
        &harness.context,
        dto::MemoryAddRequest {
            conversation_id: conversation_id.clone(),
            text: "Manual before dynamic".into(),
            category: None,
            observed_at: None,
            expected_revision: 1,
            client_operation_id: "before-dynamic-add".into(),
        },
    )
    .await
    .expect("manual add");
    let database = harness.context.backend().database();
    let character = CharacterRepository::get(database, harness.character_id)
        .expect("character")
        .expect("present")
        .character;
    let mut defaults = character.defaults;
    defaults.memory_policy = lettuce_characters::MemoryPolicy::Dynamic;
    CharacterRepository::update_defaults(
        database,
        harness.character_id,
        character.revision,
        defaults,
        lettuce_types::TimestampMillis::new(5),
    )
    .expect("dynamic character");
    let stored = GlobalSettingsStore::load(database).expect("settings");
    let mut settings = stored.settings;
    settings.dynamic_memory.enabled = true;
    GlobalSettingsStore::save(
        database,
        settings,
        stored.default_model_profile_id,
        stored.revision,
    )
    .expect("dynamic enabled");
    let conversation =
        ConversationReader::get(database, conversation_id.parse().expect("conversation"))
            .expect("conversation")
            .conversation;
    let before =
        MemoryRepository::get_for_branch(database, conversation.id, conversation.active_branch_id)
            .expect("memory")
            .expect("present");
    let add = dto::MemoryAddRequest {
        conversation_id: conversation_id.clone(),
        text: "Dynamic add".into(),
        category: None,
        observed_at: None,
        expected_revision: 2,
        client_operation_id: "dynamic-add".into(),
    };
    assert_eq!(
        memory_add(&harness.context, add)
            .await
            .expect_err("required embedding")
            .code,
        ApiErrorCode::ModelRequired
    );
    let update = dto::MemoryUpdateRequest {
        conversation_id,
        memory_id: added.memory_id.expect("id"),
        text: None,
        category: dto::MemoryCategoryChange::Set(None),
        observed_at: dto::MemoryObservedAtChange::Keep,
        expected_revision: 2,
        client_operation_id: "dynamic-update".into(),
    };
    assert_eq!(
        memory_update(&harness.context, update)
            .await
            .expect_err("required embedding")
            .code,
        ApiErrorCode::ModelRequired
    );
    assert_eq!(
        MemoryRepository::get(database, before.id).expect("memory"),
        Some(before)
    );
    assert!(
        database
            .lookup_api_operation("memory_add", "dynamic-add")
            .expect("receipt")
            .is_none()
    );
    assert!(
        database
            .lookup_api_operation("memory_update", "dynamic-update")
            .expect("receipt")
            .is_none()
    );
}

#[tokio::test]
async fn manual_edit_history_is_carried_by_backup_and_restore() {
    use lettuce_transfer::{ProviderBackupRestoreWriter, ProviderBackupSource};
    let harness = harness(Reply::Text("reply"));
    let conversation_id = launch(&harness, "manual-backup-launch").await;
    memory_add(
        &harness.context,
        dto::MemoryAddRequest {
            conversation_id,
            text: "Backed up manual memory".into(),
            category: None,
            observed_at: None,
            expected_revision: 1,
            client_operation_id: "backup-manual-add".into(),
        },
    )
    .await
    .expect("add");
    let mut graph = harness
        .context
        .backend()
        .database()
        .read_provider_backup_graph()
        .expect("backup");
    assert_eq!(graph.memory.manual_edits.len(), 1);
    lettuce_transfer::canonicalize_and_validate(&mut graph).expect("valid backup");
    let restored = lettuce_database::Database::open_in_memory().expect("destination");
    use lettuce_conversations::{ConversationArtifactTransferPort, TrustedArtifactDescriptor};
    let database = harness.context.backend().database();
    let artifacts = lettuce_transfer::provider_backup_artifact_requirements(&graph)
        .expect("requirements")
        .into_iter()
        .map(|descriptor| {
            let mut sink = MemoryArtifactBytes(Vec::new());
            match &descriptor {
                TrustedArtifactDescriptor::Snapshot(reference) => {
                    database.export_snapshot(reference.artifact_id, &mut sink)
                }
                TrustedArtifactDescriptor::Replay(reference) => {
                    database.export_replay(reference.artifact_id, &mut sink)
                }
            }
            .expect("export artifact");
            lettuce_transfer::BackupConversationArtifact {
                descriptor,
                bytes: zeroize::Zeroizing::new(sink.0),
            }
        })
        .collect::<Vec<_>>();
    restored
        .restore_provider_backup_graph(&graph, &artifacts)
        .expect("restore");
    let backup = restored
        .read_provider_backup_graph()
        .expect("restored backup");
    assert_eq!(backup.memory.manual_edits, graph.memory.manual_edits);
    let receipt = restored
        .lookup_api_operation("memory_add", "backup-manual-add")
        .expect("receipt")
        .expect("present");
    assert_eq!(receipt.command, "memory_add");
}

#[tokio::test]
async fn manual_edit_history_syncs_into_the_peers_resolved_space() {
    use lettuce_sync::{IncomingChangeRepository, LocalChangeJournal, SyncDeviceId};
    use lettuce_transfer::ProviderBackupSource;
    use lettuce_types::{OperationId, TimestampMillis};
    let harness = harness(Reply::Text("reply"));
    let conversation_id = launch(&harness, "manual-sync-launch").await;
    let added = memory_add(
        &harness.context,
        dto::MemoryAddRequest {
            conversation_id: conversation_id.clone(),
            text: "Synced manual memory".into(),
            category: None,
            observed_at: None,
            expected_revision: 1,
            client_operation_id: "sync-manual-add".into(),
        },
    )
    .await
    .expect("add");
    let database = harness.context.backend().database();
    database
        .journal_current_state(TimestampMillis::new(1000))
        .expect("journal");
    let peer = crate::AppBackend::open_in_memory(TimestampMillis::new(1)).expect("peer");
    let changes = database
        .outbound_changes(
            &peer.database().local_frontier().expect("frontier"),
            lettuce_sync::MAX_OUTBOUND_CHANGES,
            lettuce_sync::MAX_OUTBOUND_PAYLOAD_BYTES,
        )
        .expect("outbound")
        .changes;
    let batch = OperationId::new();
    peer.database()
        .stage_incoming_batch(
            SyncDeviceId::new(),
            batch,
            &lettuce_sync::canonical_batch_hash(&changes),
            &changes,
            TimestampMillis::new(1001),
        )
        .expect("stage");
    let applied = peer
        .database()
        .apply_incoming_batch(batch, TimestampMillis::new(1001))
        .expect("apply");
    assert_eq!(applied.state, lettuce_sync::IncomingBatchState::Committed);
    let backup = peer
        .database()
        .read_provider_backup_graph()
        .expect("backup");
    assert_eq!(backup.memory.manual_edits.len(), 1);
    let record = &backup.memory.manual_edits[0];
    assert_eq!(
        record
            .history
            .after_item
            .as_ref()
            .expect("after item")
            .id
            .to_string(),
        added.memory_id.expect("id")
    );
    assert_eq!(
        record.history.edit.conversation_id.to_string(),
        conversation_id
    );
    assert!(
        backup
            .memory
            .spaces
            .iter()
            .any(|space| space.snapshot.id == record.history.space_id)
    );
    assert_eq!(record.history.edit.space_id, record.history.space_id);
}

struct MemoryArtifactBytes(Vec<u8>);
impl lettuce_conversations::TrustedArtifactSink for MemoryArtifactBytes {
    fn begin(
        &mut self,
        _: &lettuce_conversations::TrustedArtifactDescriptor,
    ) -> Result<(), lettuce_conversations::ArtifactTransferError> {
        Ok(())
    }
    fn chunk(&mut self, bytes: &[u8]) -> Result<(), lettuce_conversations::ArtifactTransferError> {
        self.0.extend_from_slice(bytes);
        Ok(())
    }
    fn finish(&mut self) -> Result<(), lettuce_conversations::ArtifactTransferError> {
        Ok(())
    }
}
