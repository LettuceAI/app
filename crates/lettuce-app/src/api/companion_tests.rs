use std::sync::{Arc, Mutex};

use super::jobs::JobFeed;
use super::tests::{Harness, Reply, harness};
use super::*;
use lettuce_contracts::{self as dto, ApiErrorCode};
use lettuce_types::CharacterId;

fn companion(harness: &Harness, name: &str, share_soul_growth: bool) -> CharacterId {
    let fact = lettuce_companions::SoulFact {
        id: "authored-fact".into(),
        category: lettuce_companions::SoulCategory::Traits,
        value: "Dry humor".into(),
        kind: lettuce_companions::SoulFactKind::Authored,
        policy: lettuce_companions::SoulFactPolicy::Current,
        slot: "traits".into(),
        confidence: 1.0,
        evidence_count: 1,
        weight: 1.0,
        valid_from: lettuce_types::TimestampMillis::new(1),
        valid_until: None,
        locked: false,
        source_memory_ids: vec![],
        created_at: lettuce_types::TimestampMillis::new(1),
        supersedes: vec![],
        superseded_by: None,
        superseded_at: None,
    };
    super::tests::create_character(
        harness.context.backend().database(),
        name,
        lettuce_characters::CharacterDefaults {
            interaction_mode: lettuce_characters::InteractionMode::Companion,
            companion_soul: Some(lettuce_companions::CompanionSoulConfig {
                authored_facts: vec![fact],
                share_soul_growth_across_chats: share_soul_growth,
                share_memory_across_chats: false,
                ..lettuce_companions::CompanionSoulConfig::default()
            }),
            memory_policy: lettuce_characters::MemoryPolicy::Dynamic,
            ..lettuce_characters::CharacterDefaults::default()
        },
    )
}

async fn launch_companion(harness: &Harness, character: CharacterId, key: &str) -> String {
    conversation_launch_direct(
        &harness.context,
        dto::LaunchDirectRequest {
            character_id: character.to_string(),
            title: None,
            scene_id: None,
            starter_id: None,
            client_operation_id: key.into(),
        },
    )
    .await
    .expect("launch companion")
    .conversation_id
}

fn note(character: CharacterId, key: &str) -> dto::CompanionNoteUpsertRequest {
    dto::CompanionNoteUpsertRequest {
        character_id: character.to_string(),
        note_id: None,
        label: "  Birthday  ".into(),
        content: "  Her birthday is today.  ".into(),
        available_at: 1_000,
        expires_at: Some(5_000),
        recurrence: dto::CompanionNoteRecurrence::Yearly,
        recurrence_window_ms: Some(2_000),
        enabled: true,
        client_operation_id: key.into(),
    }
}

#[tokio::test]
async fn notes_get_api_assigned_ids_and_timestamps_and_distinct_errors() {
    let harness = harness(Reply::Text("reply"));
    let character = companion(&harness, "Noted", true);
    let created = companion_notes_upsert(&harness.context, note(character, "note-create"))
        .await
        .expect("create");
    assert!(uuid::Uuid::parse_str(&created.id).is_ok());
    assert_eq!(created.label, "Birthday");
    assert_eq!(created.content, "Her birthday is today.");
    assert_eq!(created.created_at, created.updated_at);
    assert!(created.created_at > 0);
    assert_eq!(
        companion_notes_upsert(&harness.context, note(character, "note-create"))
            .await
            .expect("replay"),
        created
    );
    let mut changed = note(character, "note-create");
    changed.content = "Another note".into();
    assert_eq!(
        companion_notes_upsert(&harness.context, changed)
            .await
            .expect_err("changed request")
            .code,
        ApiErrorCode::Conflict
    );
    let mut update = note(character, "note-update");
    update.note_id = Some(created.id.clone());
    update.content = "Updated content".into();
    let updated = companion_notes_upsert(&harness.context, update)
        .await
        .expect("update");
    assert_eq!(updated.id, created.id);
    assert_eq!(updated.created_at, created.created_at);
    assert!(updated.updated_at >= created.updated_at);
    assert_eq!(updated.content, "Updated content");
    let listed = companion_notes_list(
        &harness.context,
        dto::CompanionNotesRequest {
            character_id: character.to_string(),
        },
    )
    .await
    .expect("list");
    assert_eq!(listed, vec![updated.clone()]);

    let mut unknown = note(character, "note-unknown");
    unknown.note_id = Some(uuid::Uuid::new_v4().to_string());
    assert_eq!(
        companion_notes_upsert(&harness.context, unknown)
            .await
            .expect_err("the API assigns ids")
            .code,
        ApiErrorCode::NotFound
    );
    let missing = companion_notes_upsert(
        &harness.context,
        note(CharacterId::new(), "note-no-character"),
    )
    .await
    .expect_err("character not found");
    assert_eq!(missing.code, ApiErrorCode::NotFound);
    let roleplay = companion_notes_upsert(
        &harness.context,
        note(harness.character_id, "note-roleplay"),
    )
    .await
    .expect_err("not a companion");
    assert_eq!(roleplay.code, ApiErrorCode::Unsupported);
    assert_ne!(missing.message, roleplay.message);
    let mut backwards = note(character, "note-backwards");
    backwards.expires_at = Some(500);
    assert_eq!(
        companion_notes_upsert(&harness.context, backwards)
            .await
            .expect_err("expires before it starts")
            .code,
        ApiErrorCode::InvalidInput
    );
    let mut empty = note(character, "note-empty");
    empty.content = "   ".into();
    assert_eq!(
        companion_notes_upsert(&harness.context, empty)
            .await
            .expect_err("empty content")
            .code,
        ApiErrorCode::InvalidInput
    );
    let preview = |as_of| {
        let context = harness.context.clone();
        async move {
            companion_notes_active_preview(
                &context,
                dto::CompanionNotesActivePreviewRequest {
                    character_id: character.to_string(),
                    as_of,
                },
            )
            .await
            .expect("preview")
        }
    };
    assert_eq!(preview(1_500).await.len(), 1);
    assert!(preview(500).await.is_empty());
    assert!(preview(6_000).await.is_empty());
    let delete = dto::CompanionNoteDeleteRequest {
        note_id: created.id.clone(),
        client_operation_id: "note-delete".into(),
    };
    companion_notes_delete(&harness.context, delete.clone())
        .await
        .expect("delete");
    companion_notes_delete(&harness.context, delete)
        .await
        .expect("replay");
    companion_notes_delete(
        &harness.context,
        dto::CompanionNoteDeleteRequest {
            note_id: created.id,
            client_operation_id: "note-delete-missing".into(),
        },
    )
    .await
    .expect("a missing note deletes silently");
    assert!(
        companion_notes_list(
            &harness.context,
            dto::CompanionNotesRequest {
                character_id: character.to_string(),
            },
        )
        .await
        .expect("list")
        .is_empty()
    );
}

#[tokio::test]
async fn soul_commands_act_on_the_conversation_soul_or_the_shared_character_soul() {
    for shared in [false, true] {
        let harness = harness(Reply::Text("reply"));
        let character = companion(&harness, "Soulful", shared);
        let source = launch_companion(&harness, character, "soul-source").await;
        let copy = conversation_duplicate(
            &harness.context,
            dto::ConversationDuplicateRequest {
                conversation_id: source.clone(),
                title: None,
                with_messages: false,
                client_operation_id: "soul-copy".into(),
            },
        )
        .await
        .expect("duplicate")
        .conversation_id;
        let view = |conversation: Option<&str>| {
            let context = harness.context.clone();
            let conversation = conversation.map(str::to_owned);
            async move {
                companion_soul_get(
                    &context,
                    dto::CompanionSoulGetRequest {
                        character_id: character.to_string(),
                        conversation_id: conversation,
                    },
                )
                .await
                .expect("soul")
            }
        };
        let in_copy = view(Some(&copy)).await;
        assert_eq!(
            in_copy.growth.owner,
            if shared {
                dto::SoulOwnerKind::Character
            } else {
                dto::SoulOwnerKind::Conversation
            }
        );
        assert_eq!(in_copy.config.soul.essence, "");
        assert_eq!(in_copy.config.authored_facts[0].id, "authored-fact");
        assert_eq!(in_copy.growth.facts.len(), 1);
        assert_eq!(in_copy.growth.active_count, 1);
        assert_eq!(in_copy.growth.superseded_count, 0);
        let fact = in_copy.growth.facts[0].id.clone();
        let lock = dto::CompanionSoulGrowthLockRequest {
            character_id: character.to_string(),
            conversation_id: Some(copy.clone()),
            fact_id: fact.clone(),
            locked: true,
            client_operation_id: "soul-lock".into(),
        };
        assert!(
            companion_soul_growth_lock(&harness.context, lock.clone())
                .await
                .expect("lock")
        );
        assert!(
            companion_soul_growth_lock(&harness.context, lock)
                .await
                .expect("replay")
        );
        assert!(view(Some(&copy)).await.growth.facts[0].locked);
        let in_source = view(Some(&source)).await;
        assert_eq!(in_source.growth.facts[0].locked, shared);
        assert_eq!(
            view(None).await.growth.facts.iter().any(|f| f.locked),
            shared
        );
        assert!(
            !companion_soul_growth_remove(
                &harness.context,
                dto::CompanionSoulGrowthRemoveRequest {
                    character_id: character.to_string(),
                    conversation_id: Some(copy.clone()),
                    fact_id: "no-such-fact".into(),
                    client_operation_id: "soul-remove-missing".into(),
                },
            )
            .await
            .expect("remove missing")
        );
        assert!(
            companion_soul_growth_remove(
                &harness.context,
                dto::CompanionSoulGrowthRemoveRequest {
                    character_id: character.to_string(),
                    conversation_id: Some(copy.clone()),
                    fact_id: fact,
                    client_operation_id: "soul-remove".into(),
                },
            )
            .await
            .expect("remove locked entry too")
        );
        assert!(view(Some(&copy)).await.growth.facts.is_empty());
        assert_eq!(
            view(Some(&source)).await.growth.facts.len(),
            usize::from(!shared)
        );
        let clear = dto::CompanionSoulGrowthClearRequest {
            character_id: character.to_string(),
            conversation_id: Some(source.clone()),
            client_operation_id: "soul-clear".into(),
        };
        let cleared = companion_soul_growth_clear(&harness.context, clear.clone())
            .await
            .expect("clear");
        assert_eq!(cleared, u32::from(!shared));
        assert_eq!(
            companion_soul_growth_clear(&harness.context, clear)
                .await
                .expect("replay"),
            cleared
        );
        assert!(view(Some(&source)).await.growth.facts.is_empty());
    }
}

#[tokio::test]
async fn soul_commands_refuse_unknown_non_companion_and_foreign_targets() {
    let harness = harness(Reply::Text("reply"));
    let character = companion(&harness, "Soulful", true);
    let other = companion(&harness, "Other", true);
    let chat = launch_companion(&harness, other, "soul-foreign").await;
    let get = |character: String, conversation: Option<String>| {
        let context = harness.context.clone();
        async move {
            companion_soul_get(
                &context,
                dto::CompanionSoulGetRequest {
                    character_id: character,
                    conversation_id: conversation,
                },
            )
            .await
        }
    };
    assert_eq!(
        get(CharacterId::new().to_string(), None)
            .await
            .expect_err("unknown character")
            .code,
        ApiErrorCode::NotFound
    );
    assert_eq!(
        get(harness.character_id.to_string(), None)
            .await
            .expect_err("roleplay character")
            .code,
        ApiErrorCode::Unsupported
    );
    assert_eq!(
        get(character.to_string(), Some(chat))
            .await
            .expect_err("another character's chat")
            .code,
        ApiErrorCode::InvalidInput
    );
}

#[derive(Default)]
struct RecordingJob(Mutex<Vec<dto::JobEvent>>, tokio::sync::Notify);

impl JobEventSink for RecordingJob {
    fn emit(&self, event: dto::JobEvent) -> bool {
        self.0.lock().expect("job events").push(event);
        self.1.notify_one();
        true
    }
}

const SOUL_REPLY: &str = r#"{"operations":[{"name":"set_identity","arguments":{"traits":"Patient and observant"}},{"name":"set_authored_facts","arguments":{"facts":[{"category":"backstory","value":"Raised by the sea","policy":"historical","slot":"origin","confidence":1.0}]}},{"name":"done","arguments":{}}]}"#;

async fn base_draft(harness: &Harness) -> dto::CompanionSoulDraft {
    let character = companion(harness, "Draft base", true);
    let view = companion_soul_get(
        &harness.context,
        dto::CompanionSoulGetRequest {
            character_id: character.to_string(),
            conversation_id: None,
        },
    )
    .await
    .expect("soul");
    let mut soul = view.config.soul;
    soul.traits = "Careful".into();
    dto::CompanionSoulDraft {
        soul,
        authored_facts: vec![],
        relationship_defaults: view.config.relationship_defaults,
    }
}

fn writer(
    key: &str,
    current_soul: Option<dto::CompanionSoulDraft>,
) -> dto::CompanionSoulWriterRunRequest {
    dto::CompanionSoulWriterRunRequest {
        character_name: "Mira".into(),
        character_definition: Some("A careful traveller".into()),
        character_description: None,
        opening_context: Some("At the station".into()),
        current_soul,
        user_notes: None,
        model_profile_id: None,
        client_operation_id: key.into(),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_soul_writer_streams_deltas_and_completes_with_its_draft() {
    let harness = harness(Reply::Text(SOUL_REPLY));
    let draft = Some(base_draft(&harness).await);
    let mut feed = JobFeed::start(&harness.context).await.expect("feed");
    let accepted =
        companion_soul_writer_run(&harness.context, writer("soul-writer", draft.clone()))
            .await
            .expect("accepted");
    assert_eq!(
        companion_soul_writer_run(&harness.context, writer("soul-writer", draft.clone()))
            .await
            .expect("replay"),
        accepted
    );
    let mut changed = writer("soul-writer", draft.clone());
    changed.character_name = "Someone else".into();
    assert_eq!(
        companion_soul_writer_run(&harness.context, changed)
            .await
            .expect_err("changed request")
            .code,
        ApiErrorCode::Conflict
    );
    let sink = Arc::new(RecordingJob::default());
    job_watch(
        &harness.context,
        dto::JobWatchRequest {
            job_id: accepted.job_id.clone(),
        },
        sink.clone(),
    )
    .await
    .expect("watch");
    let runner = JobRunner::new(harness.context.clone(), JobHandlers::standard());
    while runner.run_once().await.expect("runner") {}
    runner.wait_idle().await;
    feed.publish(&harness.context).await.expect("publish");
    let events = sink.0.lock().expect("events").clone();
    let deltas = events
        .iter()
        .filter_map(|event| match event {
            dto::JobEvent::TextDelta { text, .. } => text.clone(),
            _ => None,
        })
        .collect::<String>();
    assert!(deltas.contains("Hel"));
    match events.last().expect("a last event") {
        dto::JobEvent::Completed { job } => match job.result.as_ref().expect("a result") {
            dto::JobResultDto::CompanionSoulDraft { draft } => {
                assert_eq!(draft.soul.traits, "Patient and observant");
                assert_eq!(draft.authored_facts.len(), 1);
                assert_eq!(draft.authored_facts[0].value, "Raised by the sea");
                assert_eq!(draft.authored_facts[0].kind, dto::SoulFactKind::Authored);
            }
            other => panic!("expected a Soul draft, got {other:?}"),
        },
        other => panic!("expected completion, got {other:?}"),
    }
    let view = job_get(
        &harness.context,
        dto::JobGetRequest {
            job_id: accepted.job_id,
        },
    )
    .await
    .expect("job");
    assert!(matches!(
        view.result,
        Some(dto::JobResultDto::CompanionSoulDraft { .. })
    ));
}

#[tokio::test]
async fn the_soul_writer_names_a_missing_model_and_a_blank_name() {
    let harness = harness(Reply::Text(SOUL_REPLY));
    let database = harness.context.backend().database();
    let stored = lettuce_settings::GlobalSettingsStore::load(database).expect("settings");
    lettuce_settings::GlobalSettingsStore::save(database, stored.settings, None, stored.revision)
        .expect("no default model");
    let error = companion_soul_writer_run(&harness.context, writer("soul-no-model", None))
        .await
        .expect_err("no model");
    assert_eq!(error.code, ApiErrorCode::InvalidInput);
    assert_eq!(
        error.details,
        Some(dto::ApiErrorDetails::InvalidField {
            field: "model_profile_id".into()
        })
    );
    let mut blank = writer("soul-blank", None);
    blank.character_name = "  ".into();
    assert_eq!(
        companion_soul_writer_run(&harness.context, blank)
            .await
            .expect_err("blank name")
            .details,
        Some(dto::ApiErrorDetails::InvalidField {
            field: "character_name".into()
        })
    );
}

#[tokio::test]
async fn delete_after_in_one_pooled_chat_keeps_the_other_chats_later_edit() {
    let harness = super::tests::harness_in(
        Reply::Text("reply"),
        Arc::new(lettuce_jobs::SystemClock),
        None,
        None,
        Arc::new(super::inspect_tests::AllModels),
    );
    let character = super::tests::create_character(
        harness.context.backend().database(),
        "Pooled",
        lettuce_characters::CharacterDefaults {
            interaction_mode: lettuce_characters::InteractionMode::Companion,
            companion_soul: Some(lettuce_companions::CompanionSoulConfig::default()),
            memory_policy: lettuce_characters::MemoryPolicy::Dynamic,
            ..lettuce_characters::CharacterDefaults::default()
        },
    );
    let a = launch_companion(&harness, character, "pool-a").await;
    let first = conversation_add_user_message(
        &harness.context,
        dto::ConversationAddUserMessageRequest {
            conversation_id: a.clone(),
            text: "Keep".into(),
            expected_revision: 1,
            client_operation_id: "pool-a-first".into(),
        },
    )
    .await
    .expect("first message");
    let b = conversation_duplicate(
        &harness.context,
        dto::ConversationDuplicateRequest {
            conversation_id: a.clone(),
            title: None,
            with_messages: true,
            client_operation_id: "pool-b".into(),
        },
    )
    .await
    .expect("duplicate")
    .conversation_id;
    let revision = |chat: String| {
        let context = harness.context.clone();
        async move {
            memory_get(
                &context,
                dto::ConversationRequest {
                    conversation_id: chat,
                },
            )
            .await
            .expect("memory")
        }
    };
    let added = memory_add(
        &harness.context,
        dto::MemoryAddRequest {
            conversation_id: a.clone(),
            text: "Tea v1".into(),
            category: None,
            observed_at: None,
            expected_revision: revision(a.clone()).await.revision,
            client_operation_id: "pool-add".into(),
        },
    )
    .await
    .expect("add")
    .memory_id
    .expect("id");
    let later = conversation_add_user_message(
        &harness.context,
        dto::ConversationAddUserMessageRequest {
            conversation_id: a.clone(),
            text: "Removed".into(),
            expected_revision: first.revision,
            client_operation_id: "pool-a-later".into(),
        },
    )
    .await
    .expect("later message");
    let edit = |chat: &str, text: &str, expected: u64, key: &str| dto::MemoryUpdateRequest {
        conversation_id: chat.into(),
        memory_id: added.clone(),
        text: Some(text.into()),
        category: dto::MemoryCategoryChange::Keep,
        observed_at: dto::MemoryObservedAtChange::Keep,
        expected_revision: expected,
        client_operation_id: key.into(),
    };
    memory_update(
        &harness.context,
        edit(
            &a,
            "Tea v2",
            revision(a.clone()).await.revision,
            "pool-a-edit",
        ),
    )
    .await
    .expect("edit in a");
    memory_update(
        &harness.context,
        edit(
            &b,
            "Tea v3",
            revision(b.clone()).await.revision,
            "pool-b-edit",
        ),
    )
    .await
    .expect("edit in b");
    messages_delete_after(
        &harness.context,
        dto::MessageDeleteRequest {
            conversation_id: a.clone(),
            message_id: first.message.id,
            expected_revision: later.revision,
            client_operation_id: "pool-a-delete-after".into(),
        },
    )
    .await
    .expect("delete after in a");
    let texts = revision(b)
        .await
        .items
        .into_iter()
        .map(|item| item.text)
        .collect::<Vec<_>>();
    assert_eq!(texts, vec!["Tea v3".to_owned()]);
}

#[tokio::test]
async fn source_free_user_summaries_sync_to_fresh_direct_and_pooled_peers() {
    use lettuce_memory::{MemoryRepository, MemorySummaryRepository};
    use lettuce_sync::{IncomingBatchState, IncomingChangeRepository, LocalChangeJournal};
    for pooled in [false, true] {
        let harness = harness(Reply::Text("reply"));
        let chat = if pooled {
            let character = super::tests::create_character(
                harness.context.backend().database(),
                "Summary pool",
                lettuce_characters::CharacterDefaults {
                    interaction_mode: lettuce_characters::InteractionMode::Companion,
                    companion_soul: Some(lettuce_companions::CompanionSoulConfig::default()),
                    memory_policy: lettuce_characters::MemoryPolicy::Dynamic,
                    ..lettuce_characters::CharacterDefaults::default()
                },
            );
            launch_companion(&harness, character, "summary-pool").await
        } else {
            super::tests::launch(&harness, "summary-direct").await
        };
        memory_summary_update(
            &harness.context,
            dto::MemorySummaryUpdateRequest {
                conversation_id: chat.clone(),
                summary: dto::MemorySummaryEdit::Set { text: "My source-free summary".into() },
                expected_revision: 1,
                client_operation_id: "source-free-summary".into(),
            },
        ).await.expect("summary edit");
        let source = harness.context.backend().database();
        let peer = lettuce_database::Database::open_in_memory().expect("fresh peer");
        source.journal_current_state(harness.context.now()).expect("journal");
        loop {
            let batch = source.outbound_changes(
                &peer.local_frontier().expect("frontier"),
                lettuce_sync::MAX_OUTBOUND_CHANGES,
                lettuce_sync::MAX_OUTBOUND_PAYLOAD_BYTES,
            ).expect("batch");
            if batch.changes.is_empty() { break; }
            let id = lettuce_types::OperationId::new();
            peer.stage_incoming_batch(
                lettuce_sync::SyncDeviceId::new(), id,
                &lettuce_sync::canonical_batch_hash(&batch.changes), &batch.changes,
                harness.context.now(),
            ).expect("stage");
            assert_eq!(peer.apply_incoming_batch(id, harness.context.now()).expect("apply").state,
                IncomingBatchState::Committed);
        }
        let conversation = lettuce_conversations::ConversationReader::get(&peer, chat.parse().expect("chat id")).expect("conversation").conversation;
        let space = peer.get_for_branch(conversation.id, conversation.active_branch_id).expect("memory").expect("space");
        let summary = peer.get_summary(space.id).expect("summary").expect("synced");
        assert_eq!(summary.text, "My source-free summary");
        assert_eq!(summary.origin, lettuce_memory::MemoryOrigin::User);
        assert!(summary.source_message_ids.is_empty());
    }
}
