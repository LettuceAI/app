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

#[derive(Default)]
struct ReadTokenizerModels {
    installed: std::sync::atomic::AtomicBool,
    counts: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

#[async_trait::async_trait]
impl ModelLoader for ReadTokenizerModels {
    fn installed(&self, _: &ApiContext, model: dto::RequiredModel) -> bool {
        model == dto::RequiredModel::Embedding
            && self.installed.load(std::sync::atomic::Ordering::SeqCst)
    }
    async fn prepare(&self, _: &ApiContext) -> bool {
        true
    }
    fn embedding(
        &self,
        _: &ApiContext,
    ) -> ModelLoad<std::sync::Arc<dyn crate::MemoryEmbeddingEngine>> {
        if self.installed.load(std::sync::atomic::Ordering::SeqCst) {
            ModelLoad::Loaded(std::sync::Arc::new(ReadTokenizer(self.counts.clone())))
        } else {
            ModelLoad::NotInstalled
        }
    }
    fn emotion(
        &self,
        _: &ApiContext,
    ) -> ModelLoad<std::sync::Arc<dyn crate::CompanionEmotionEngine>> {
        ModelLoad::NotInstalled
    }
}

struct ReadTokenizer(std::sync::Arc<std::sync::atomic::AtomicUsize>);
impl crate::MemoryEmbeddingEngine for ReadTokenizer {
    fn source_revision(&self) -> &str {
        "read-tokenizer"
    }
    fn dimensions(&self) -> lettuce_embeddings::EmbeddingDimensions {
        lettuce_embeddings::EmbeddingDimensions::D64
    }
    fn count_tokens(&self, text: &str) -> Result<u32, crate::EmbeddingGenerationError> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(u32::try_from(text.split_whitespace().count()).expect("fixture count"))
    }
    fn embed_memory(
        &self,
        _: &lettuce_embeddings::EmbeddingRequest,
        _: &lettuce_jobs::handle::CancellationToken,
    ) -> Result<lettuce_embeddings::EmbeddingVector, crate::EmbeddingGenerationError> {
        panic!("manual memory reads and writes must not embed")
    }
}

#[tokio::test]
async fn memory_get_recounts_unknown_items_and_summary_once_without_embedding_or_revision_changes()
{
    let models = std::sync::Arc::new(ReadTokenizerModels::default());
    let harness = super::tests::harness_in(
        Reply::Text("reply"),
        std::sync::Arc::new(lettuce_jobs::SystemClock),
        None,
        None,
        models.clone(),
    );
    let conversation_id = launch(&harness, "memory-get-launch").await;
    memory_add(
        &harness.context,
        dto::MemoryAddRequest {
            conversation_id: conversation_id.clone(),
            text: "Four token manual memory".into(),
            category: None,
            observed_at: None,
            expected_revision: 1,
            client_operation_id: "read-add".into(),
        },
    )
    .await
    .expect("add unknown");
    memory_summary_update(
        &harness.context,
        dto::MemorySummaryUpdateRequest {
            conversation_id: conversation_id.clone(),
            summary: dto::MemorySummaryEdit::Set {
                text: "Three token summary".into(),
            },
            expected_revision: 2,
            client_operation_id: "read-summary".into(),
        },
    )
    .await
    .expect("summary unknown");
    let request = dto::ConversationRequest { conversation_id };
    let unknown = memory_get(&harness.context, request.clone())
        .await
        .expect("unknown read");
    assert_eq!(unknown.revision, 3);
    assert_eq!(unknown.items[0].token_count, None);
    assert_eq!(unknown.summary.expect("summary").token_count, None);
    models
        .installed
        .store(true, std::sync::atomic::Ordering::SeqCst);
    harness.context.models_changed();
    let counted = memory_get(&harness.context, request.clone())
        .await
        .expect("counted read");
    assert_eq!(counted.revision, 3);
    assert_eq!(counted.items[0].token_count, Some(4));
    assert_eq!(
        counted.summary.as_ref().expect("summary").token_count,
        Some(3)
    );
    assert_eq!(counted.items[0].origin, dto::MemoryOrigin::User);
    assert_eq!(
        counted.summary.as_ref().expect("summary").origin,
        dto::MemoryOrigin::User
    );
    assert_eq!(models.counts.load(std::sync::atomic::Ordering::SeqCst), 2);
    assert_eq!(
        memory_get(&harness.context, request)
            .await
            .expect("repeat read"),
        counted
    );
    assert_eq!(models.counts.load(std::sync::atomic::Ordering::SeqCst), 2);
}

#[tokio::test]
async fn clear_own_user_summary_resolves_the_original_inherited_summary_after_parent_changes() {
    let harness = harness(Reply::Text("reply"));
    let conversation_id = launch(&harness, "inherited-summary-launch").await;
    let message = conversation_add_user_message(
        &harness.context,
        dto::ConversationAddUserMessageRequest {
            conversation_id: conversation_id.clone(),
            text: "Fork here".into(),
            expected_revision: 1,
            client_operation_id: "inherited-message".into(),
        },
    )
    .await
    .expect("message");
    memory_summary_update(
        &harness.context,
        dto::MemorySummaryUpdateRequest {
            conversation_id: conversation_id.clone(),
            summary: dto::MemorySummaryEdit::Set {
                text: "Parent as of fork".into(),
            },
            expected_revision: 1,
            client_operation_id: "parent-summary".into(),
        },
    )
    .await
    .expect("parent summary");
    let database = harness.context.backend().database();
    let root = ConversationReader::get(database, conversation_id.parse().expect("conversation"))
        .expect("conversation")
        .conversation
        .active_branch_id;
    let fork = conversation_branch_fork(
        &harness.context,
        dto::ConversationBranchForkRequest {
            conversation_id: conversation_id.clone(),
            message_id: message.message.id,
            expected_revision: message.revision,
            client_operation_id: "summary-fork".into(),
        },
    )
    .await
    .expect("fork");
    memory_summary_update(
        &harness.context,
        dto::MemorySummaryUpdateRequest {
            conversation_id: conversation_id.clone(),
            summary: dto::MemorySummaryEdit::Set {
                text: "Child override".into(),
            },
            expected_revision: 1,
            client_operation_id: "child-summary".into(),
        },
    )
    .await
    .expect("child summary");
    let selected = conversation_branch_select(
        &harness.context,
        dto::ConversationBranchMutationRequest {
            branch_id: root.to_string(),
            expected_revision: fork.revision,
            client_operation_id: "select-summary-parent".into(),
        },
    )
    .await
    .expect("parent");
    memory_summary_update(
        &harness.context,
        dto::MemorySummaryUpdateRequest {
            conversation_id: conversation_id.clone(),
            summary: dto::MemorySummaryEdit::Set {
                text: "Parent changed later".into(),
            },
            expected_revision: 2,
            client_operation_id: "parent-later-summary".into(),
        },
    )
    .await
    .expect("parent change");
    conversation_branch_select(
        &harness.context,
        dto::ConversationBranchMutationRequest {
            branch_id: fork.branch_id.clone(),
            expected_revision: selected.revision,
            client_operation_id: "select-summary-child".into(),
        },
    )
    .await
    .expect("child");
    memory_summary_update(
        &harness.context,
        dto::MemorySummaryUpdateRequest {
            conversation_id: conversation_id.clone(),
            summary: dto::MemorySummaryEdit::Clear,
            expected_revision: 2,
            client_operation_id: "child-clear-summary".into(),
        },
    )
    .await
    .expect("clear child");
    let conversation =
        ConversationReader::get(database, conversation_id.parse().expect("conversation"))
            .expect("conversation")
            .conversation;
    let memory =
        MemoryRepository::get_for_branch(database, conversation.id, conversation.active_branch_id)
            .expect("memory")
            .expect("space");
    assert!(
        database
            .get_summary(memory.id)
            .expect("own summary")
            .is_none()
    );
    let inherited = database
        .get_summary_for_branch(memory.id, conversation.id, conversation.active_branch_id)
        .expect("resolved summary")
        .expect("inherited summary");
    assert_eq!(inherited.text, "Parent as of fork");
    let view = memory_get(
        &harness.context,
        dto::ConversationRequest { conversation_id },
    )
    .await
    .expect("read actual summary");
    assert_eq!(view.summary.expect("summary").text, inherited.text);
}

#[tokio::test]
async fn a_source_free_user_summary_is_used_by_the_manual_chat_prompt() {
    let harness = harness(Reply::Text("reply"));
    let conversation_id = launch(&harness, "manual-summary-prompt-launch").await;
    memory_summary_update(
        &harness.context,
        dto::MemorySummaryUpdateRequest {
            conversation_id: conversation_id.clone(),
            summary: dto::MemorySummaryEdit::Set {
                text: "The user chose this durable summary".into(),
            },
            expected_revision: 1,
            client_operation_id: "manual-summary-prompt".into(),
        },
    )
    .await
    .expect("summary");
    super::tests::send(
        &harness,
        &conversation_id,
        "manual-summary-turn",
        "Hello",
        std::sync::Arc::new(super::tests::RecordingStream::default()),
    )
    .await
    .expect("send");
    super::turns_tests::run_generation(&harness).await;
    let requests = harness.provider.requests.lock().expect("requests");
    let context = serde_json::to_string(&requests.last().expect("generation request").context)
        .expect("context");
    assert!(context.contains("The user chose this durable summary"));
}

#[tokio::test]
async fn memory_status_reports_real_model_and_embedding_failures_and_lease_pause() {
    use lettuce_characters::CharacterRepository;
    use lettuce_jobs::{CancellationReason, JobStore, ResourceAvailability, WorkerId};
    use lettuce_settings::GlobalSettingsStore;
    use lettuce_types::TimestampMillis;
    for case in ["model", "embedding", "lease"] {
        let harness = harness(Reply::Text("reply"));
        let (conversation_id, _) =
            super::turns_tests::replied_chat(&harness, &format!("status-{case}")).await;
        if case == "embedding" {
            memory_add(
                &harness.context,
                dto::MemoryAddRequest {
                    conversation_id: conversation_id.clone(),
                    text: "Unknown manual count".into(),
                    category: None,
                    observed_at: None,
                    expected_revision: 1,
                    client_operation_id: "status-add".into(),
                },
            )
            .await
            .expect("unknown item");
        }
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
            harness.context.now(),
        )
        .expect("dynamic character");
        let stored = GlobalSettingsStore::load(database).expect("settings");
        let mut settings = stored.settings;
        settings.dynamic_memory.enabled = true;
        settings.dynamic_memory.summary_message_interval = 2;
        GlobalSettingsStore::save(
            database,
            settings,
            if case == "model" {
                None
            } else {
                stored.default_model_profile_id
            },
            stored.revision,
        )
        .expect("dynamic settings");
        let engine = if case == "embedding" {
            harness.context.embedding()
        } else {
            std::sync::Arc::new(ReadTokenizer(std::sync::Arc::default()))
                as std::sync::Arc<dyn crate::MemoryEmbeddingEngine>
        };
        let host = harness
            .context
            .backend()
            .companion_memory_host(engine.as_ref(), harness.context.inference());
        let now = harness.context.now();
        let work = host
            .after_turn(
                conversation_id.parse().expect("id"),
                lettuce_conversations::GenerationOperation::Send,
                WorkerId::new(),
                now,
                std::time::Duration::from_secs(30),
                &ResourceAvailability::all(),
            )
            .expect("admit")
            .into_iter()
            .next()
            .expect("work");
        let job_id = work.job.id;
        if case == "lease" {
            let expired = database
                .orphaned_claims(TimestampMillis::new(now.get() + 1), 100)
                .expect("recover claim");
            assert!(expired.iter().any(|claim| claim.job_id == job_id));
            crate::jobs::job_recovery::fail_interrupted_job(
                database,
                &JobStore::get(database, job_id)
                    .expect("job")
                    .expect("present"),
                TimestampMillis::new(now.get() + 2),
            )
            .expect("pause repeated crashes");
        } else {
            assert!(matches!(
                host.run_claimed(
                    work,
                    CancellationReason::User,
                    TimestampMillis::new(now.get() + 1)
                )
                .await
                .expect("settle"),
                crate::CompanionMemorySettledWork::Failed { .. }
            ));
        }
        let view = memory_get(
            &harness.context,
            dto::ConversationRequest { conversation_id },
        )
        .await
        .expect("status");
        assert_eq!(view.status.latest_job_id, Some(job_id.to_string()));
        assert_eq!(
            view.status.latest_cycle_status,
            Some(dto::MemoryCycleStatus::Failed)
        );
        assert_eq!(
            view.status.failure,
            Some(match case {
                "model" => dto::MemoryFailureCode::ModelMissing,
                "embedding" => dto::MemoryFailureCode::EmbeddingUnavailable,
                _ => dto::MemoryFailureCode::LeaseLost,
            })
        );
        assert_eq!(
            view.status.paused_reason,
            (case == "lease").then_some(dto::MemoryPausedReason::LeaseLost)
        );
    }
}

#[tokio::test]
async fn memory_user_edits_are_cut_locally_by_fork_and_delete_after() {
    for fork in [false, true] {
        let harness = harness(Reply::Text("reply"));
        let conversation_id = launch(&harness, "undo-launch").await;
        let anchor = conversation_add_user_message(
            &harness.context,
            dto::ConversationAddUserMessageRequest {
                conversation_id: conversation_id.clone(),
                text: "Kept anchor".into(),
                expected_revision: 1,
                client_operation_id: "undo-anchor".into(),
            },
        )
        .await
        .expect("anchor");
        let added = memory_add(
            &harness.context,
            dto::MemoryAddRequest {
                conversation_id: conversation_id.clone(),
                text: "Before cut".into(),
                category: None,
                observed_at: None,
                expected_revision: 1,
                client_operation_id: "undo-add".into(),
            },
        )
        .await
        .expect("add");
        let later = conversation_add_user_message(
            &harness.context,
            dto::ConversationAddUserMessageRequest {
                conversation_id: conversation_id.clone(),
                text: "Removed suffix".into(),
                expected_revision: anchor.revision,
                client_operation_id: "undo-later".into(),
            },
        )
        .await
        .expect("later");
        memory_update(
            &harness.context,
            dto::MemoryUpdateRequest {
                conversation_id: conversation_id.clone(),
                memory_id: added.memory_id.expect("id"),
                text: Some("After cut".into()),
                category: dto::MemoryCategoryChange::Keep,
                observed_at: dto::MemoryObservedAtChange::Keep,
                expected_revision: 2,
                client_operation_id: "undo-update".into(),
            },
        )
        .await
        .expect("update");
        let database = harness.context.backend().database();
        let root = ConversationReader::get(database, conversation_id.parse().expect("id"))
            .expect("conversation")
            .conversation
            .active_branch_id;
        if fork {
            conversation_branch_fork(
                &harness.context,
                dto::ConversationBranchForkRequest {
                    conversation_id: conversation_id.clone(),
                    message_id: anchor.message.id,
                    expected_revision: later.revision,
                    client_operation_id: "undo-fork".into(),
                },
            )
            .await
            .expect("fork");
        } else {
            let request = dto::MessageDeleteRequest {
                conversation_id: conversation_id.clone(),
                message_id: anchor.message.id,
                expected_revision: later.revision,
                client_operation_id: "undo-delete-after".into(),
            };
            let deleted = messages_delete_after(&harness.context, request.clone())
                .await
                .expect("delete");
            assert_eq!(
                messages_delete_after(&harness.context, request)
                    .await
                    .expect("replay"),
                deleted
            );
        }
        let view = memory_get(
            &harness.context,
            dto::ConversationRequest {
                conversation_id: conversation_id.clone(),
            },
        )
        .await
        .expect("memory");
        assert_eq!(view.items.len(), 1);
        assert_eq!(view.items[0].text, "Before cut");
        if fork {
            assert_eq!(
                MemoryRepository::get_for_branch(
                    database,
                    conversation_id.parse().expect("id"),
                    root
                )
                .expect("parent")
                .expect("space")
                .items[0]
                    .text,
                "After cut"
            );
        } else {
            use lettuce_transfer::ProviderBackupSource;
            let history = database
                .read_provider_backup_graph()
                .expect("history")
                .memory
                .manual_edits;
            assert!(
                history
                    .iter()
                    .find(|record| matches!(
                        record.history.edit.mutation,
                        lettuce_memory::MemoryManualMutation::Update { .. }
                    ))
                    .expect("update")
                    .undone_at
                    .is_some()
            );
            assert!(
                history
                    .iter()
                    .find(|record| matches!(
                        record.history.edit.mutation,
                        lettuce_memory::MemoryManualMutation::Add { .. }
                    ))
                    .expect("add")
                    .undone_at
                    .is_none()
            );
        }
    }
}

#[tokio::test]
async fn delete_after_undoes_every_manual_setter_and_summary_at_the_removed_anchor() {
    let harness = harness(Reply::Text("reply"));
    let conversation_id = launch(&harness, "all-manual-undo").await;
    let anchor = conversation_add_user_message(
        &harness.context,
        dto::ConversationAddUserMessageRequest {
            conversation_id: conversation_id.clone(),
            text: "Keep".into(),
            expected_revision: 1,
            client_operation_id: "all-undo-anchor".into(),
        },
    )
    .await
    .expect("anchor");
    let item = memory_add(
        &harness.context,
        dto::MemoryAddRequest {
            conversation_id: conversation_id.clone(),
            text: "Original".into(),
            category: Some(dto::MemoryCategory::Preference),
            observed_at: Some(5),
            expected_revision: 1,
            client_operation_id: "all-undo-item".into(),
        },
    )
    .await
    .expect("item")
    .memory_id
    .expect("id");
    memory_summary_update(
        &harness.context,
        dto::MemorySummaryUpdateRequest {
            conversation_id: conversation_id.clone(),
            summary: dto::MemorySummaryEdit::Set {
                text: "Original summary".into(),
            },
            expected_revision: 2,
            client_operation_id: "all-undo-summary".into(),
        },
    )
    .await
    .expect("summary");
    let later = conversation_add_user_message(
        &harness.context,
        dto::ConversationAddUserMessageRequest {
            conversation_id: conversation_id.clone(),
            text: "Remove".into(),
            expected_revision: anchor.revision,
            client_operation_id: "all-undo-later".into(),
        },
    )
    .await
    .expect("later");
    memory_update(
        &harness.context,
        dto::MemoryUpdateRequest {
            conversation_id: conversation_id.clone(),
            memory_id: item.clone(),
            text: Some("Changed".into()),
            category: dto::MemoryCategoryChange::Set(None),
            observed_at: dto::MemoryObservedAtChange::Set(None),
            expected_revision: 3,
            client_operation_id: "all-undo-update".into(),
        },
    )
    .await
    .expect("update");
    memory_pin(
        &harness.context,
        dto::MemoryPinRequest {
            conversation_id: conversation_id.clone(),
            memory_id: item.clone(),
            pinned: true,
            expected_revision: 4,
            client_operation_id: "all-undo-pin".into(),
        },
    )
    .await
    .expect("pin");
    memory_pin(
        &harness.context,
        dto::MemoryPinRequest {
            conversation_id: conversation_id.clone(),
            memory_id: item.clone(),
            pinned: false,
            expected_revision: 5,
            client_operation_id: "all-undo-unpin".into(),
        },
    )
    .await
    .expect("unpin");
    memory_set_temperature(
        &harness.context,
        dto::MemoryTemperatureRequest {
            conversation_id: conversation_id.clone(),
            memory_id: item.clone(),
            temperature: dto::MemoryTemperature::Cold,
            expected_revision: 6,
            client_operation_id: "all-undo-cold".into(),
        },
    )
    .await
    .expect("cold");
    memory_delete(
        &harness.context,
        dto::MemoryDeleteRequest {
            conversation_id: conversation_id.clone(),
            memory_id: item.clone(),
            expected_revision: 7,
            client_operation_id: "all-undo-remove".into(),
        },
    )
    .await
    .expect("remove");
    memory_add(
        &harness.context,
        dto::MemoryAddRequest {
            conversation_id: conversation_id.clone(),
            text: "Late addition".into(),
            category: None,
            observed_at: None,
            expected_revision: 8,
            client_operation_id: "all-undo-add".into(),
        },
    )
    .await
    .expect("late addition");
    memory_summary_update(
        &harness.context,
        dto::MemorySummaryUpdateRequest {
            conversation_id: conversation_id.clone(),
            summary: dto::MemorySummaryEdit::Set {
                text: "Late summary".into(),
            },
            expected_revision: 9,
            client_operation_id: "all-undo-late-summary".into(),
        },
    )
    .await
    .expect("late summary");
    messages_delete_after(
        &harness.context,
        dto::MessageDeleteRequest {
            conversation_id: conversation_id.clone(),
            message_id: anchor.message.id,
            expected_revision: later.revision,
            client_operation_id: "all-undo-delete".into(),
        },
    )
    .await
    .expect("delete after");
    let view = memory_get(
        &harness.context,
        dto::ConversationRequest { conversation_id },
    )
    .await
    .expect("view");
    assert_eq!(view.revision, 11);
    assert_eq!(view.items.len(), 1);
    assert_eq!(view.items[0].id, item);
    assert_eq!(view.items[0].text, "Original");
    assert_eq!(
        view.items[0].category,
        Some(dto::MemoryCategory::Preference)
    );
    assert_eq!(view.items[0].observed_at, Some(5));
    assert_eq!(view.items[0].temperature, dto::MemoryTemperature::Hot);
    assert!(!view.items[0].pinned);
    assert_eq!(view.summary.expect("summary").text, "Original summary");
    use lettuce_transfer::ProviderBackupSource;
    let mut graph = harness
        .context
        .backend()
        .database()
        .read_provider_backup_graph()
        .expect("backup");
    lettuce_transfer::canonicalize_and_validate(&mut graph)
        .expect("manual-only undo is valid backup state");
    let mut missing_evidence = graph.clone();
    missing_evidence
        .memory
        .manual_edits
        .retain(|record| record.undone_at.is_none());
    assert!(lettuce_transfer::canonicalize_and_validate(&mut missing_evidence).is_err());
}

#[tokio::test]
async fn memory_ids_of_a_deleted_branch_are_refused_and_nothing_is_written() {
    let harness = harness(Reply::Text("reply"));
    let conversation_id = launch(&harness, "tombstone-launch").await;
    let anchor = conversation_add_user_message(
        &harness.context,
        dto::ConversationAddUserMessageRequest {
            conversation_id: conversation_id.clone(),
            text: "Fork here".into(),
            expected_revision: 1,
            client_operation_id: "tombstone-anchor".into(),
        },
    )
    .await
    .expect("anchor");
    let database = harness.context.backend().database();
    let root = ConversationReader::get(database, conversation_id.parse().expect("id"))
        .expect("conversation")
        .conversation
        .active_branch_id;
    let fork = conversation_branch_fork(
        &harness.context,
        dto::ConversationBranchForkRequest {
            conversation_id: conversation_id.clone(),
            message_id: anchor.message.id,
            expected_revision: anchor.revision,
            client_operation_id: "tombstone-fork".into(),
        },
    )
    .await
    .expect("fork");
    let child_item = memory_add(
        &harness.context,
        dto::MemoryAddRequest {
            conversation_id: conversation_id.clone(),
            text: "Only on the fork".into(),
            category: None,
            observed_at: None,
            expected_revision: 1,
            client_operation_id: "tombstone-add".into(),
        },
    )
    .await
    .expect("fork memory")
    .memory_id
    .expect("id");
    let selected = conversation_branch_select(
        &harness.context,
        dto::ConversationBranchMutationRequest {
            branch_id: root.to_string(),
            expected_revision: fork.revision,
            client_operation_id: "tombstone-select".into(),
        },
    )
    .await
    .expect("select root");
    conversation_branch_delete(
        &harness.context,
        dto::ConversationBranchMutationRequest {
            branch_id: fork.branch_id.clone(),
            expected_revision: selected.revision,
            client_operation_id: "tombstone-delete".into(),
        },
    )
    .await
    .expect("delete fork");
    let before =
        MemoryRepository::get_for_branch(database, conversation_id.parse().expect("id"), root)
            .expect("parent memory")
            .expect("space");
    let refused = memory_pin(
        &harness.context,
        dto::MemoryPinRequest {
            conversation_id: conversation_id.clone(),
            memory_id: child_item,
            pinned: true,
            expected_revision: before.revision.get(),
            client_operation_id: "tombstone-pin".into(),
        },
    )
    .await
    .expect_err("memory of a deleted branch");
    assert_eq!(refused.code, ApiErrorCode::NotFound);
    assert_eq!(
        MemoryRepository::get_for_branch(database, conversation_id.parse().expect("id"), root)
            .expect("parent memory")
            .expect("space"),
        before
    );
}
