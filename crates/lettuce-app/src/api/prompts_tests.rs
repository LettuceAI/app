use std::sync::Arc;

use lettuce_characters::{CharacterDefaults, CharacterRepository};
use lettuce_context::PromptRepository;
use lettuce_contracts::{self as dto, ApiErrorCode, ApiErrorDetails};
use lettuce_settings::GlobalSettingsStore;
use lettuce_transfer::ProviderBackupSource;
use lettuce_types::{ConversationId, PromptDocumentId};

use super::lorebooks_tests::backup_round_trip;
use super::tests::{Harness, RecordingStream, Reply, create_character, harness, launch, send};
use super::turns_tests::run_generation;
use super::*;

const DIRECT_PLACEHOLDERS: &str = "{{char.name}} is {{char.desc}}. {{persona.name}}: {{persona.desc}}. {{scene}} {{scene_direction}} {{context_summary}} {{key_memories}}";

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
            client_operation_id: key.into(),
            prompt: input(name, DIRECT_PLACEHOLDERS),
        },
    )
    .await
    .expect("create prompt")
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
    let own = super::turns_tests::conversation(&harness, &chat)
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
            client_operation_id: "preview".into(),
            prompt: preview_input,
        },
    )
    .await
    .expect("preview prompt");
    let sample = prompt_preview(
        &harness.context,
        dto::PromptPreviewRequest {
            prompt_id: prompt.prompt.id.clone(),
            conversation_id: None,
            character_id: Some(harness.character_id.to_string()),
            persona_id: None,
        },
    )
    .await
    .expect("sample preview");
    let text = &sample.entries[0].text;
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
    assert!(!live.entries[0].text.contains("Memory 1 (Preview)"));

    let cast = super::turns_tests::group_cast(&harness, "preview-group").await;
    let group_prompt = prompt_create(
        &harness.context,
        dto::PromptCreateRequest {
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
