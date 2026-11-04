use std::sync::Arc;

use lettuce_characters::{CharacterDefaults, CharacterRepository};
use lettuce_context::PromptRepository;
use lettuce_contracts::{self as dto, ApiErrorCode, ApiErrorDetails};
use lettuce_settings::GlobalSettingsStore;
use lettuce_transfer::ProviderBackupSource;
use lettuce_types::{ConversationId, PromptDocumentId};

use crate::api::conversations::turns_tests::run_generation;
use crate::api::lorebooks::tests::backup_round_trip;
use crate::api::tests::{Harness, RecordingStream, Reply, create_character, harness, launch, send};
use crate::api::*;

const DIRECT_PLACEHOLDERS: &str = "{{char.name}} is {{char.desc}}. {{persona.name}}: {{persona.desc}}. {{scene}} {{scene_direction}} {{context_summary}} {{key_memories}}";

#[tokio::test]
async fn prompt_copy_preserves_its_source_and_replays_after_source_deletion() {
    let harness = harness(Reply::Text("unused"));
    let legacy = dto::PromptCreateRequest { client_operation_id: "legacy".into(), source_prompt_id: None, prompt: input("Legacy", DIRECT_PLACEHOLDERS) };
    assert_eq!(serde_json::to_string(&legacy).expect("legacy request"), format!("{{\"client_operation_id\":\"legacy\",\"prompt\":{}}}", serde_json::to_string(&legacy.prompt).expect("legacy input")));
    let source = create(&harness, "copy-source", "Source").await;
    let request: dto::PromptCreateRequest = serde_json::from_value(serde_json::json!({
        "client_operation_id": "copy", "source_prompt_id": source.prompt.id,
        "prompt": input("Copy", DIRECT_PLACEHOLDERS),
    })).expect("copy request");
    let copy = prompt_create(&harness.context, request.clone()).await.expect("copy");
    assert!(matches!(&copy.prompt.origin, dto::PromptOrigin::Derived { source_id, source_name, source_deleted: false } if source_id == &source.prompt.id && source_name == "Source"));
    assert_ne!(copy.entries[0].id, source.entries[0].id);
    prompt_delete(&harness.context, dto::PromptDeleteRequest { client_operation_id: "delete-copy-source".into(), prompt_id: source.prompt.id, expected_revision: source.prompt.revision }).await.expect("delete source");
    assert_eq!(prompt_create(&harness.context, request).await.expect("replay"), copy);
    let current = prompt_get(&harness.context, dto::PromptGetRequest { prompt_id: copy.prompt.id }).await.expect("copy remains");
    assert!(matches!(current.prompt.origin, dto::PromptOrigin::Derived { source_deleted: true, .. }));
}

#[tokio::test]
async fn prompt_preview_renders_an_unsaved_draft_without_writes_or_events() {
    let harness = harness(Reply::Text("unused"));
    let before = harness.context.backend().database().read_provider_backup_graph().expect("before");
    let events = harness.events.events().len();
    let id = PromptDocumentId::new().to_string();
    let request: dto::PromptPreviewRequest = serde_json::from_value(serde_json::json!({
        "prompt_id": id, "draft": input("Unsaved", &format!("Draft marker {DIRECT_PLACEHOLDERS}")),
        "conversation_id": null, "character_id": harness.character_id.to_string(), "persona_id": null,
    })).expect("draft preview request");
    let preview = prompt_preview(&harness.context, request).await.expect("draft preview");
    assert!(preview.entries.iter().any(|entry| entry.text.contains("Draft marker")));
    assert!(preview.entries.iter().any(|entry| entry.text.contains("A meticulous engineer")));
    assert_eq!(prompt_get(&harness.context, dto::PromptGetRequest { prompt_id: id }).await.expect_err("not stored").code, ApiErrorCode::NotFound);
    assert_eq!(harness.context.backend().database().read_provider_backup_graph().expect("after"), before);
    assert_eq!(harness.events.events().len(), events);
}

fn entry(content: &str) -> dto::PromptEntryInput {
    dto::PromptEntryInput {
        entry_id: None,
        name: "Main".into(),
        role: dto::PromptEntryRole::System,
        content: content.into(),
        enabled: true,
        position: dto::PromptEntryPosition::Relative,
        depth: 0,
        conditional_min_messages: None,
        interval_turns: None,
        system_prompt: false,
        condition: None,
        image_slot: None,
    }
}

fn input(name: &str, content: &str) -> dto::PromptInput {
    dto::PromptInput {
        name: name.into(),
        kind: dto::PromptKind::DirectChat,
        condense: false,
        behavior: dto::PromptBehavior::LegacyV1,
        entries: vec![entry(content)],
    }
}

async fn create(harness: &Harness, key: &str, name: &str) -> dto::PromptView {
    prompt_create(
        &harness.context,
        dto::PromptCreateRequest {
            source_prompt_id: None,
            client_operation_id: key.into(),
            prompt: input(name, DIRECT_PLACEHOLDERS),
        },
    )
    .await
    .expect("create prompt")
}

#[tokio::test]
async fn catalog_feed_prompt_create_delete_and_replay_emit_only_commits() {
    let harness = harness(Reply::Text("unused"));
    let before = harness
        .events
        .events()
        .iter()
        .filter(|event| **event == dto::ApiEvent::PromptsChanged)
        .count();
    let prompt = create(&harness, "feed-prompt", "Prompt").await;
    create(&harness, "feed-prompt", "Prompt").await;
    assert_eq!(
        harness
            .events
            .events()
            .iter()
            .filter(|event| **event == dto::ApiEvent::PromptsChanged)
            .count(),
        before + 1
    );
    let request = dto::PromptDeleteRequest {
        client_operation_id: "feed-delete".into(),
        prompt_id: prompt.prompt.id,
        expected_revision: prompt.prompt.revision,
    };
    prompt_delete(&harness.context, request.clone())
        .await
        .expect("delete");
    prompt_delete(&harness.context, request)
        .await
        .expect("replay delete");
    assert_eq!(
        harness
            .events
            .events()
            .iter()
            .filter(|event| **event == dto::ApiEvent::PromptsChanged)
            .count(),
        before + 2
    );
}

#[tokio::test]
async fn catalog_feed_sync_apply_refreshes_lorebooks_and_prompts_once() {
    let source = harness(Reply::Text("unused"));
    let target = harness(Reply::Text("unused"));
    create(&source, "sync-prompt", "Shared prompt").await;
    crate::api::lorebooks::tests::create(
        &source,
        "sync-book",
        "Shared book",
        vec![crate::api::lorebooks::tests::entry(
            "Entry", "Content", None,
        )],
    )
    .await;
    let count = |kind: dto::ApiEvent| {
        target
            .events
            .events()
            .iter()
            .filter(|event| **event == kind)
            .count()
    };
    let before = (
        count(dto::ApiEvent::LorebooksChanged),
        count(dto::ApiEvent::PromptsChanged),
    );
    crate::launch::tests::sync_prompts(
        source.context.backend().database(),
        target.context.backend().database(),
        10_000,
    );
    let after = (
        count(dto::ApiEvent::LorebooksChanged),
        count(dto::ApiEvent::PromptsChanged),
    );
    assert_eq!(after, (before.0 + 1, before.1 + 1));
    crate::launch::tests::sync_prompts(
        source.context.backend().database(),
        target.context.backend().database(),
        10_001,
    );
    assert_eq!(
        (
            count(dto::ApiEvent::LorebooksChanged),
            count(dto::ApiEvent::PromptsChanged)
        ),
        after
    );
}

fn turn_prompts(
    database: &lettuce_database::Database,
    chat: &str,
) -> Vec<(PromptDocumentId, String)> {
    let id = chat.parse::<ConversationId>().expect("id");
    database
        .read_provider_backup_graph()
        .expect("graph")
        .conversation_runtime
        .conversations
        .iter()
        .filter(|runtime| runtime.conversation_id == id)
        .flat_map(|runtime| runtime.turns.iter())
        .filter_map(|turn| turn.turn.prompt.as_ref())
        .map(|prompt| (prompt.document_id, prompt.name.clone()))
        .collect()
}

#[tokio::test]
async fn writes_require_the_kinds_placeholders_and_replay_by_key() {
    let harness = harness(Reply::Text("ok"));
    let refused = prompt_create(
        &harness.context,
        dto::PromptCreateRequest {
            source_prompt_id: None,
            client_operation_id: "short".into(),
            prompt: input("Short", "{{char.name}} only"),
        },
    )
    .await
    .expect_err("missing placeholders");
    assert_eq!(refused.code, ApiErrorCode::InvalidInput);
    let Some(ApiErrorDetails::PromptMissingPlaceholders { placeholders }) = refused.details else {
        panic!("no placeholder details: {refused:?}");
    };
    assert_eq!(
        placeholders,
        vec![
            "{{scene}}",
            "{{scene_direction}}",
            "{{char.desc}}",
            "{{persona.name}}",
            "{{persona.desc}}",
            "{{context_summary}}",
            "{{key_memories}}"
        ]
    );
    let validation = prompt_validate(
        &harness.context,
        dto::PromptValidateRequest {
            prompt: dto::PromptInput {
                kind: dto::PromptKind::LorebookGeneratorCoherence,
                ..input("Coherence", "no placeholders")
            },
        },
    )
    .await
    .expect("validate");
    assert_eq!(validation.missing_placeholders, vec!["{{drafted_entries}}"]);
    let registry = prompt_placeholders(
        &harness.context,
        dto::PromptPlaceholdersRequest {
            kind: dto::PromptKind::LorebookGeneratorPlanner,
        },
    )
    .await
    .expect("registry");
    assert_eq!(
        registry.required,
        vec!["{{brief}}", "{{target_count}}", "{{source_excerpts}}"]
    );

    let created = create(&harness, "full", "Full").await;
    assert_eq!(create(&harness, "full", "Full").await, created);
    let conflict = prompt_create(
        &harness.context,
        dto::PromptCreateRequest {
            source_prompt_id: None,
            client_operation_id: "full".into(),
            prompt: input("Other", DIRECT_PLACEHOLDERS),
        },
    )
    .await
    .expect_err("changed request");
    assert_eq!(conflict.code, ApiErrorCode::Conflict);

    let mut kept = input("Renamed", DIRECT_PLACEHOLDERS);
    kept.entries[0].entry_id = Some(created.entries[0].id.clone());
    let updated = prompt_update(
        &harness.context,
        dto::PromptUpdateRequest {
            client_operation_id: "PromptUpdateRequest-140".into(),
            prompt_id: created.prompt.id.clone(),
            expected_revision: created.prompt.revision,
            prompt: kept.clone(),
        },
    )
    .await
    .expect("update");
    assert_eq!(updated.entries[0].id, created.entries[0].id);
    assert_eq!(updated.prompt.name, "Renamed");
    let stale = prompt_update(
        &harness.context,
        dto::PromptUpdateRequest {
            client_operation_id: "PromptUpdateRequest-152".into(),
            prompt_id: created.prompt.id.clone(),
            expected_revision: created.prompt.revision,
            prompt: kept,
        },
    )
    .await
    .expect_err("stale");
    assert_eq!(stale.code, ApiErrorCode::Conflict);
    let disabled = prompt_update(
        &harness.context,
        dto::PromptUpdateRequest {
            client_operation_id: "PromptUpdateRequest-163".into(),
            prompt_id: created.prompt.id.clone(),
            expected_revision: updated.prompt.revision,
            prompt: dto::PromptInput {
                entries: vec![dto::PromptEntryInput {
                    enabled: false,
                    ..entry(DIRECT_PLACEHOLDERS)
                }],
                ..input("Disabled", "")
            },
        },
    )
    .await
    .expect_err("disabled entries do not count");
    assert_eq!(disabled.code, ApiErrorCode::InvalidInput);
}

#[tokio::test]
async fn every_built_in_prompt_carries_its_required_placeholders() {
    let harness = harness(Reply::Text("ok"));
    let database = harness.context.backend().database();
    for id in crate::BuiltInPromptId::ALL {
        let document = PromptRepository::get(
            database,
            harness.context.backend().built_in_prompt_ids().get(id),
        )
        .expect("read")
        .expect("seeded");
        assert_eq!(
            lettuce_context::missing_required_placeholders(document.purpose, &document.entries),
            Vec::<&str>::new(),
            "{id:?}"
        );
    }
}

#[tokio::test]
async fn reset_keeps_the_users_name_and_restores_content() {
    let harness = harness(Reply::Text("ok"));
    let id = harness
        .context
        .backend()
        .built_in_prompt_ids()
        .reply_helper
        .to_string();
    let original = prompt_get(
        &harness.context,
        dto::PromptGetRequest {
            prompt_id: id.clone(),
        },
    )
    .await
    .expect("get");
    let mut edited = dto::PromptInput {
        name: "My helper".into(),
        kind: original.prompt.kind,
        condense: original.condense,
        behavior: original.behavior,
        entries: original
            .entries
            .iter()
            .map(|entry| dto::PromptEntryInput {
                entry_id: Some(entry.id.clone()),
                name: entry.name.clone(),
                role: entry.role,
                content: entry.content.clone(),
                enabled: entry.enabled,
                position: entry.position,
                depth: entry.depth,
                conditional_min_messages: entry.conditional_min_messages,
                interval_turns: entry.interval_turns,
                system_prompt: entry.system_prompt,
                condition: entry.condition.clone(),
                image_slot: entry.image_slot,
            })
            .collect(),
    };
    edited.entries[0].content.push_str(" Extra words.");
    let changed = prompt_update(
        &harness.context,
        dto::PromptUpdateRequest {
            client_operation_id: "PromptUpdateRequest-238".into(),
            prompt_id: id.clone(),
            expected_revision: original.prompt.revision,
            prompt: edited,
        },
    )
    .await
    .expect("edit built-in");
    assert!(matches!(
        changed.prompt.origin,
        dto::PromptOrigin::BuiltIn { edited: true, .. }
    ));
    let stale = prompt_builtin_reset(
        &harness.context,
        dto::PromptBuiltinResetRequest::One {
            client_operation_id: "reset-one".into(),
            prompt_id: id.clone(),
            expected_revision: original.prompt.revision,
        },
    )
    .await
    .expect_err("stale reset");
    assert_eq!(stale.code, ApiErrorCode::Conflict);
    let reset = prompt_builtin_reset(
        &harness.context,
        dto::PromptBuiltinResetRequest::One {
            client_operation_id: "reset-one".into(),
            prompt_id: id.clone(),
            expected_revision: changed.prompt.revision,
        },
    )
    .await
    .expect("reset");
    assert_eq!(reset.prompts[0].prompt.name, "My helper");
    assert_eq!(reset.prompts[0].entries, original.entries);
    let all = prompt_builtin_reset(
        &harness.context,
        dto::PromptBuiltinResetRequest::All {
            client_operation_id: "reset-all".into(),
        },
    )
    .await
    .expect("reset all");
    assert_eq!(all.prompts.len(), crate::BuiltInPromptId::ALL.len());
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_resets_selections_keeps_history_and_refuses_protected_prompts() {
    let harness = harness(Reply::Text("A reply."));
    let database = harness.context.backend().database();
    let deleted = create(&harness, "deleted", "Doomed prompt").await;
    let deleted_id: PromptDocumentId = deleted.prompt.id.parse().expect("id");
    let chat = launch(&harness, "prompt-chat").await;
    conversation_settings_update(
        &harness.context,
        dto::ConversationSettingsUpdateRequest {
            conversation_id: chat.clone(),
            expected_settings_revision: None,
            patch: dto::ConversationSettingsPatch {
                prompt: Some(dto::ChoiceChange::Set {
                    id: deleted.prompt.id.clone(),
                }),
                ..dto::ConversationSettingsPatch::default()
            },
        },
    )
    .await
    .expect("prompt override");
    send(
        &harness,
        &chat,
        "prompt-send",
        "Hello",
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect("send");
    run_generation(&harness).await;
    assert_eq!(
        turn_prompts(database, &chat),
        vec![(deleted_id, "Doomed prompt".to_owned())]
    );
    let character = create_character(
        database,
        "Selector",
        CharacterDefaults {
            direct_prompt_id: Some(deleted_id),
            ..CharacterDefaults::default()
        },
    );
    let stored = database.load().expect("settings");
    let mut settings = stored.settings.clone();
    settings.lorebook_entry_generator.entry_prompt_id = Some(deleted_id);
    database
        .save(settings, stored.default_model_profile_id, stored.revision)
        .expect("feature setting");

    let removed = prompt_delete(
        &harness.context,
        dto::PromptDeleteRequest {
            client_operation_id: "PromptDeleteRequest-328".into(),
            prompt_id: deleted.prompt.id.clone(),
            expected_revision: deleted.prompt.revision,
        },
    )
    .await
    .expect("delete");
    let events = harness.events.events();
    assert!(events.contains(&dto::ApiEvent::CharacterChanged {
        character_id: character.to_string()
    }));
    assert!(events.contains(&dto::ApiEvent::ConversationChanged {
        conversation_id: chat.clone()
    }));
    assert!(events.contains(&dto::ApiEvent::PromptsChanged));
    let open = conversation_open(
        &harness.context,
        dto::ConversationOpenRequest {
            conversation_id: chat.clone(),
        },
    )
    .await
    .expect("history chat");
    let reply = open
        .messages
        .items
        .iter()
        .find(|message| message.role == dto::MessageRole::Assistant)
        .expect("reply");
    let history = message_prompt_snapshot(
        &harness.context,
        dto::MessagePromptSnapshotRequest {
            message_id: reply.id.clone(),
        },
    )
    .await
    .expect("source history");
    assert_eq!(
        history.prompt,
        Some(dto::HistoricalSourceView {
            id: deleted.prompt.id.clone(),
            name: "Doomed prompt".into(),
            deleted: true
        })
    );
    assert_eq!(removed.character_ids, vec![character.to_string()]);
    assert_eq!(removed.conversation_ids, vec![chat.clone()]);
    assert!(removed.settings_changed);
    assert_eq!(
        CharacterRepository::get(database, character)
            .expect("character")
            .expect("exists")
            .character
            .defaults
            .direct_prompt_id,
        None
    );
    assert_eq!(
        database
            .load()
            .expect("settings")
            .settings
            .lorebook_entry_generator
            .entry_prompt_id,
        None
    );
    let own = crate::api::conversations::turns_tests::conversation(&harness, &chat)
        .current_settings
        .expect("settings");
    assert_eq!(
        own.prompt_provenance,
        lettuce_conversations::SettingProvenance::LaunchInherited
    );
    let history = vec![(deleted_id, "Doomed prompt".to_owned())];
    assert_eq!(turn_prompts(database, &chat), history);
    assert_eq!(turn_prompts(&backup_round_trip(database), &chat), history);
    let peer = lettuce_database::Database::open_in_memory().expect("peer");
    crate::sync::sync_exchange::tests::exchange(
        database,
        &peer,
        900,
        lettuce_types::TimestampMillis::now().expect("clock"),
    )
    .await;
    assert_eq!(turn_prompts(&peer, &chat), history);

    let protected = harness.context.backend().built_in_prompt_ids().app_default;
    let current = PromptRepository::get(database, protected)
        .expect("read")
        .expect("exists");
    let refused = prompt_delete(
        &harness.context,
        dto::PromptDeleteRequest {
            client_operation_id: "PromptDeleteRequest-382".into(),
            prompt_id: protected.to_string(),
            expected_revision: current.revision.get(),
        },
    )
    .await
    .expect_err("protected");
    assert_eq!(refused.code, ApiErrorCode::Conflict);
    assert_eq!(refused.details, Some(ApiErrorDetails::PromptProtected));
}

#[tokio::test(flavor = "multi_thread")]
async fn preview_renders_live_conversation_values_samples_and_group_values() {
    let harness = harness(Reply::Text("A reply."));
    let character = character_get(
        &harness.context,
        dto::CharacterGetRequest {
            character_id: harness.character_id.to_string(),
        },
    )
    .await
    .expect("character");
    let mut draft = character.draft;
    draft.profile.description = Some("A short card intro".into());
    draft.profile.definition = Some("  Patient and precise with every answer  ".into());
    character_save(
        &harness.context,
        dto::CharacterSaveRequest {
            character_id: Some(character.id),
            draft,
            expected_revision: Some(character.revision),
            client_operation_id: "preview-character-definition".into(),
        },
    )
    .await
    .expect("save character description and intro");
    let mut preview_input = input(
        "Preview",
        "{{char.name}} is {{char.desc}}. {{persona.name}}: {{persona.desc}}. {{context_summary}} {{key_memories}}",
    );
    preview_input
        .entries
        .push(entry("{{scene}} {{scene_direction}}"));
    let prompt = prompt_create(
        &harness.context,
        dto::PromptCreateRequest {
            source_prompt_id: None,
            client_operation_id: "preview".into(),
            prompt: preview_input,
        },
    )
    .await
    .expect("preview prompt");
    let sample = prompt_preview(
        &harness.context,
        dto::PromptPreviewRequest {
            draft: None,
            prompt_id: prompt.prompt.id.clone(),
            conversation_id: None,
            character_id: Some(harness.character_id.to_string()),
            persona_id: None,
        },
    )
    .await
    .expect("sample preview");
    let text = &sample.entries[0].text;
    assert!(text.contains(" is Patient and precise with every answer. "), "{text}");
    assert!(!text.contains("A short card intro"), "{text}");
    assert!(text.contains("This is a placeholder for the context summary"));
    assert!(text.contains("Memory 1 (Preview)"));

    let chat = launch(&harness, "preview-chat").await;
    send(
        &harness,
        &chat,
        "preview-send",
        "Hello",
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect("send");
    run_generation(&harness).await;
    let live = prompt_preview(
        &harness.context,
        dto::PromptPreviewRequest {
            draft: None,
            prompt_id: prompt.prompt.id.clone(),
            conversation_id: Some(chat),
            character_id: None,
            persona_id: None,
        },
    )
    .await
    .expect("live preview");
    let character_name =
        CharacterRepository::get(harness.context.backend().database(), harness.character_id)
            .expect("character")
            .expect("exists")
            .character
            .profile
            .name;
    assert!(live.entries[0].text.contains(&character_name));
    assert!(live.entries[0].text.contains(" is Patient and precise with every answer. "));
    assert!(!live.entries[0].text.contains("A short card intro"));
    assert!(!live.entries[0].text.contains("Memory 1 (Preview)"));

    let cast = crate::api::conversations::turns_tests::group_cast(&harness, "preview-group").await;
    let group_prompt = prompt_create(
        &harness.context,
        dto::PromptCreateRequest {
            source_prompt_id: None,
            client_operation_id: "group-prompt".into(),
            prompt: dto::PromptInput {
                kind: dto::PromptKind::GroupChatConversational,
                ..input(
                    "Group",
                    "{{char.name}} {{char.desc}} {{persona.name}} {{persona.desc}} Cast: {{group_characters}}",
                )
            },
        },
    )
    .await
    .expect("group prompt");
    let group = prompt_preview(
        &harness.context,
        dto::PromptPreviewRequest {
            draft: None,
            prompt_id: group_prompt.prompt.id,
            conversation_id: Some(cast.chat),
            character_id: Some(cast.ada_character.to_string()),
            persona_id: None,
        },
    )
    .await
    .expect("group preview");
    let text = &group.entries[0].text;
    assert!(text.starts_with("Ada"), "{text}");
    assert!(text.contains("Bea") && text.contains("Cleo"), "{text}");
}

#[tokio::test]
async fn live_preview_uses_provider_and_model_conditions_without_writing_snapshots() {
    let harness = harness(Reply::Text("ok"));
    let chat = launch(&harness, "preview-model").await;
    let prompt = prompt_create(
        &harness.context,
        dto::PromptCreateRequest {
            source_prompt_id: None,
            client_operation_id: "provider-preview".into(),
            prompt: dto::PromptInput {
                entries: vec![
                    entry(DIRECT_PLACEHOLDERS),
                    dto::PromptEntryInput {
                        condition: Some(dto::PromptCondition::All {
                            conditions: vec![
                                dto::PromptCondition::ProviderIdAny {
                                    values: vec!["ollama".into()],
                                },
                                dto::PromptCondition::InputScopeAny {
                                    values: vec!["text".into()],
                                },
                            ],
                        }),
                        ..entry("Provider-visible")
                    },
                ],
                ..input("Preview conditions", DIRECT_PLACEHOLDERS)
            },
        },
    )
    .await
    .expect("prompt");
    let db = harness.context.backend().database();
    let before = db.read_provider_backup_graph().expect("before");
    let preview = prompt_preview(
        &harness.context,
        dto::PromptPreviewRequest {
            draft: None,
            prompt_id: prompt.prompt.id,
            conversation_id: Some(chat),
            character_id: None,
            persona_id: None,
        },
    )
    .await
    .expect("preview");
    assert!(
        preview
            .entries
            .iter()
            .any(|entry| entry.text == "Provider-visible")
    );
    assert_eq!(db.read_provider_backup_graph().expect("after"), before);
}

#[tokio::test]
async fn prompt_update_replays_its_original_view_after_default_changes() {
    let harness = harness(Reply::Text("ok"));
    let prompt = create(&harness, "replay-view-create", "Original").await;
    let request = dto::PromptUpdateRequest {
        client_operation_id: "replay-view-update".into(),
        prompt_id: prompt.prompt.id.clone(),
        expected_revision: prompt.prompt.revision,
        prompt: input("Renamed", DIRECT_PLACEHOLDERS),
    };
    let updated = prompt_update(&harness.context, request.clone())
        .await
        .expect("update");
    let settings =
        GlobalSettingsStore::load(harness.context.backend().database()).expect("settings");
    prompt_app_default_set(
        &harness.context,
        dto::PromptAppDefaultSetRequest {
            client_operation_id: "replay-view-default".into(),
            prompt_id: Some(prompt.prompt.id),
            expected_settings_revision: settings.revision.get(),
        },
    )
    .await
    .expect("default");
    assert_eq!(
        prompt_update(&harness.context, request)
            .await
            .expect("replay"),
        updated
    );
}

#[tokio::test]
async fn runtime_rules_stay_loadable_because_the_runtime_text_is_protected() {
    let harness = harness(Reply::Text("ok"));
    let db = harness.context.backend().database();
    let ids = harness.context.backend().built_in_prompt_ids();
    let runtime = PromptRepository::get(db, ids.get(crate::BuiltInPromptId::ChatRuntime))
        .expect("runtime")
        .expect("exists");
    assert_eq!(
        PromptRepository::archive(
            db,
            runtime.id,
            runtime.revision,
            lettuce_types::TimestampMillis::now().expect("now"),
        ),
        Err(lettuce_context::PromptRepositoryError::Protected)
    );
}

async fn listed(harness: &Harness, id: &str) -> dto::PromptSummary {
    let mut cursor = None;
    loop {
        let page = prompts_list(
            &harness.context,
            dto::PromptsListRequest {
                kind: None,
                lifecycle: None,
                cursor,
                limit: None,
            },
        )
        .await
        .expect("list");
        if let Some(found) = page.items.into_iter().find(|item| item.id == id) {
            return found;
        }
        cursor = Some(page.next_cursor.expect("prompt is listed"));
    }
}

fn outdated_copy(harness: &Harness) -> lettuce_context::PromptDocument {
    use lettuce_context::{LifecycleStatus, PromptDocument, PromptEntry, PromptProvenance};
    let database = harness.context.backend().database();
    let source = PromptRepository::get(
        database,
        harness.context.backend().built_in_prompt_ids().reply_helper,
    )
    .expect("get")
    .expect("source");
    let catalog = crate::BuiltInPromptCatalog::bundled().expect("catalog");
    let seed = catalog.seed(crate::BuiltInPromptId::ReplyHelper);
    let mut entries: Vec<PromptEntry> = source
        .entries
        .iter()
        .map(|entry| PromptEntry {
            id: lettuce_types::PromptEntryId::new(),
            built_in_entry_key: None,
            ..entry.clone()
        })
        .collect();
    entries[0].content.push_str(" Mine.");
    let mut base = seed.seed_base();
    for (entry, held) in base.entries.iter_mut().zip(&entries) {
        entry.entry_id = Some(held.id);
    }
    base.entries[0].seed_entry_digest =
        lettuce_types::ContentHash::parse("b".repeat(64)).expect("hash");
    let now = harness.context.now();
    PromptDocument {
        id: PromptDocumentId::new(),
        status: LifecycleStatus::Active,
        name: "My helper copy".into(),
        purpose: source.purpose,
        entries,
        condense: source.condense,
        behavior_version: source.behavior_version,
        provenance: PromptProvenance::Derived {
            source: source.id,
            source_name: source.name.clone(),
            seed_base: Some(base),
        },
        revision: lettuce_types::Revision::INITIAL,
        created_at: now,
        updated_at: now,
    }
}

fn store(harness: &Harness, document: &lettuce_context::PromptDocument, key: &str) {
    harness
        .context
        .backend()
        .database()
        .commit_api_operation("fixture", key, "digest", harness.context.now(), |scope| {
            scope.import_prompt_file(document).expect("write");
            Ok::<_, lettuce_database::ApiOperationError>(())
        })
        .expect("fixture");
}

#[tokio::test]
async fn built_in_updates_are_flagged_reviewed_applied_and_replayed() {
    let harness = harness(Reply::Text("unused"));
    let pending = prompts_builtin_updates_pending(&harness.context)
        .await
        .expect("pending");
    assert_eq!(pending.count, 0);
    let copy = outdated_copy(&harness);
    store(&harness, &copy, "outdated-copy");
    let id = copy.id.to_string();
    assert!(listed(&harness, &id).await.built_in_update);
    let helper = harness
        .context
        .backend()
        .built_in_prompt_ids()
        .reply_helper
        .to_string();
    assert!(!listed(&harness, &helper).await.built_in_update);
    let pending = prompts_builtin_updates_pending(&harness.context)
        .await
        .expect("pending");
    assert_eq!((pending.count, pending.prompt_ids), (1, vec![id.clone()]));

    let catalog = crate::BuiltInPromptCatalog::bundled().expect("catalog");
    let seed = catalog.seed(crate::BuiltInPromptId::ReplyHelper);
    let review = prompt_builtin_update_get(
        &harness.context,
        dto::PromptBuiltinUpdateGetRequest {
            prompt_id: id.clone(),
        },
    )
    .await
    .expect("review");
    assert_eq!(review.revision, 1);
    assert_eq!(review.built_in_name, seed.metadata.name);
    assert_eq!(review.changes.len(), 1);
    let change = &review.changes[0];
    assert_eq!(change.kind, dto::PromptBuiltinChangeKind::Conflict);
    assert_eq!(
        Some(change.entry_key.as_str()),
        seed.entries[0].built_in_entry_key.as_deref()
    );
    assert_eq!(
        change.new.as_ref().map(|entry| entry.content.as_str()),
        Some(seed.entries[0].content.as_str())
    );
    assert!(
        change
            .current
            .as_ref()
            .is_some_and(|entry| entry.content.ends_with(" Mine."))
    );

    let request = dto::PromptBuiltinUpdateApplyRequest {
        client_operation_id: "apply-update".into(),
        prompt_id: id.clone(),
        expected_revision: review.revision,
        choices: vec![dto::PromptBuiltinUpdateChoice {
            entry_key: change.entry_key.clone(),
            take_new: true,
        }],
    };
    let applied = prompt_builtin_update_apply(&harness.context, request.clone())
        .await
        .expect("apply");
    assert_eq!(applied.entries[0].content, seed.entries[0].content);
    assert_eq!(applied.entries[0].id, copy.entries[0].id.to_string());
    assert!(!applied.prompt.built_in_update);
    assert_eq!(applied.prompt.revision, 2);
    assert_eq!(
        prompt_builtin_update_apply(&harness.context, request)
            .await
            .expect("replay"),
        applied
    );
    assert!(!listed(&harness, &id).await.built_in_update);
    let pending = prompts_builtin_updates_pending(&harness.context)
        .await
        .expect("pending");
    assert_eq!(pending.count, 0);
}

#[tokio::test]
async fn keeping_mine_clears_the_update_and_bad_requests_are_typed() {
    let harness = harness(Reply::Text("unused"));
    let copy = outdated_copy(&harness);
    store(&harness, &copy, "outdated-copy");
    let id = copy.id.to_string();
    let review = prompt_builtin_update_get(
        &harness.context,
        dto::PromptBuiltinUpdateGetRequest {
            prompt_id: id.clone(),
        },
    )
    .await
    .expect("review");
    let key = review.changes[0].entry_key.clone();
    let request = |name: &str, choices: Vec<(&str, bool)>| dto::PromptBuiltinUpdateApplyRequest {
        client_operation_id: name.into(),
        prompt_id: id.clone(),
        expected_revision: review.revision,
        choices: choices
            .into_iter()
            .map(|(entry_key, take_new)| dto::PromptBuiltinUpdateChoice {
                entry_key: entry_key.into(),
                take_new,
            })
            .collect(),
    };
    let missing = prompt_builtin_update_apply(&harness.context, request("missing", vec![]))
        .await
        .expect_err("a pending entry needs a choice");
    assert_eq!(missing.code, ApiErrorCode::InvalidInput);
    let stale = prompt_builtin_update_apply(
        &harness.context,
        request("stale", vec![(key.as_str(), false), ("not-pending", true)]),
    )
    .await
    .expect_err("stale choice");
    assert_eq!(stale.code, ApiErrorCode::Conflict);
    let kept = prompt_builtin_update_apply(
        &harness.context,
        request("keep", vec![(key.as_str(), false)]),
    )
    .await
    .expect("keep");
    assert!(kept.entries[0].content.ends_with(" Mine."));
    assert!(!kept.prompt.built_in_update);
    let repeated = prompt_builtin_update_apply(
        &harness.context,
        dto::PromptBuiltinUpdateApplyRequest {
            expected_revision: kept.prompt.revision,
            ..request("again", vec![(key.as_str(), false)])
        },
    )
    .await
    .expect_err("nothing is pending any more");
    assert_eq!(repeated.code, ApiErrorCode::Conflict);

    let user = create(&harness, "plain", "Plain").await;
    let refused = prompt_builtin_update_get(
        &harness.context,
        dto::PromptBuiltinUpdateGetRequest {
            prompt_id: user.prompt.id,
        },
    )
    .await
    .expect_err("not based on a built-in");
    assert_eq!(refused.code, ApiErrorCode::InvalidInput);
}
