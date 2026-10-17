use lettuce_contracts::{self as dto, ApiErrorCode};

use crate::api::tests::{Reply, harness};
use crate::api::*;

#[tokio::test]
async fn character_editor_get_save_preserves_full_draft_and_replays() {
    let harness = harness(Reply::Text("reply"));
    let before = character_get(
        &harness.context,
        dto::CharacterGetRequest {
            character_id: harness.character_id.to_string(),
        },
    )
    .await
    .expect("get");
    let mut draft = before.draft.clone();
    draft.profile.name = "Edited character".into();
    draft.profile.scenario = Some("authored-scenario".into());
    draft.provenance.sources = vec!["authored-source".into()];
    draft.image_recommendation = Some(dto::LibraryImageRecommendation {
        artifact_id: None,
        unresolved_legacy_name: Some("authored-lora".into()),
        strength: 0.75,
    });
    let request = dto::CharacterSaveRequest {
        character_id: Some(before.id.clone()),
        draft: draft.clone(),
        expected_revision: Some(before.revision),
        client_operation_id: "save-character".into(),
    };
    let saved = character_save(&harness.context, request.clone())
        .await
        .expect("save");
    assert_eq!(saved.draft, draft);
    assert_eq!(saved.revision, before.revision + 1);
    assert_eq!(saved.created_at, before.created_at);
    assert_eq!(
        character_save(&harness.context, request.clone())
            .await
            .expect("replay"),
        saved
    );
    let mut changed = request.clone();
    changed.draft.profile.name = "Changed".into();
    assert_eq!(
        character_save(&harness.context, changed)
            .await
            .expect_err("changed")
            .code,
        ApiErrorCode::Conflict
    );
    let mut stale = request;
    stale.client_operation_id = "stale-character".into();
    assert_eq!(
        character_save(&harness.context, stale)
            .await
            .expect_err("stale")
            .code,
        ApiErrorCode::Conflict
    );
}

#[tokio::test]
async fn character_draft_default_creates_a_character_once_named() {
    let harness = harness(Reply::Text("reply"));
    let mut draft = character_draft_default(None).await.expect("default draft");
    assert_eq!(draft.profile.name, "");
    assert_eq!(
        draft.defaults.interaction_mode,
        dto::LibraryInteractionMode::Roleplay
    );
    assert!(draft.media.is_empty() && draft.scenes.is_empty() && draft.starters.is_empty());
    draft.profile.name = "Fresh character".into();
    draft.profile.definition = Some("A new definition".into());
    let created = character_save(
        &harness.context,
        dto::CharacterSaveRequest {
            character_id: None,
            draft: draft.clone(),
            expected_revision: None,
            client_operation_id: "create-from-default".into(),
        },
    )
    .await
    .expect("create");
    assert_eq!(created.draft, draft);
    assert_eq!(created.revision, 1);
}

#[tokio::test]
async fn character_draft_default_companion_uses_domain_defaults_without_creating_a_character() {
    let draft = character_draft_default(Some(dto::LibraryInteractionMode::Companion))
        .await
        .expect("companion draft");
    assert_eq!(
        draft.defaults.interaction_mode,
        dto::LibraryInteractionMode::Companion
    );
    let fixture: dto::CharacterDraft = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../apps/ui/src/features/characters/fixtures/default-companion-draft.json",
    )))
    .expect("companion fixture");
    assert_eq!(draft, fixture);
    let config = draft.defaults.companion_soul.expect("companion config");
    assert_eq!(
        config,
        crate::api::companion::config_view(&lettuce_companions::CompanionSoulConfig::default()),
    );
    assert!(draft.profile.name.is_empty());
    assert!(draft.scenes.is_empty());
    assert!(
        character_draft_default(Some(dto::LibraryInteractionMode::Roleplay))
            .await
            .expect("roleplay draft")
            .defaults
            .companion_soul
            .is_none()
    );
}

#[tokio::test]
async fn character_editor_companion_switch_requires_models_without_partial_writes() {
    let harness = harness(Reply::Text("reply"));
    let before = character_get(
        &harness.context,
        dto::CharacterGetRequest {
            character_id: harness.character_id.to_string(),
        },
    )
    .await
    .expect("get");
    let mut draft = before.draft.clone();
    draft.defaults.interaction_mode = dto::LibraryInteractionMode::Companion;
    let request = dto::CharacterSaveRequest {
        character_id: Some(before.id.clone()),
        draft,
        expected_revision: Some(before.revision),
        client_operation_id: "switch-companion".into(),
    };
    let error = character_save(&harness.context, request)
        .await
        .expect_err("models required");
    assert_eq!(error.code, ApiErrorCode::ModelRequired);
    assert!(matches!(
        error.details,
        Some(dto::ApiErrorDetails::MissingModels { .. })
    ));
    assert_eq!(
        character_get(
            &harness.context,
            dto::CharacterGetRequest {
                character_id: before.id.clone()
            }
        )
        .await
        .expect("get"),
        before
    );
    assert!(
        harness
            .context
            .backend()
            .database()
            .lookup_api_operation("character_save", "switch-companion")
            .expect("receipt")
            .is_none()
    );
}

#[tokio::test]
async fn character_editor_shutdown_does_not_commit() {
    let harness = harness(Reply::Text("reply"));
    let before = character_get(
        &harness.context,
        dto::CharacterGetRequest {
            character_id: harness.character_id.to_string(),
        },
    )
    .await
    .expect("get");
    harness.context.begin_shutdown();
    assert_eq!(
        character_save(
            &harness.context,
            dto::CharacterSaveRequest {
                character_id: Some(before.id),
                draft: before.draft,
                expected_revision: Some(before.revision),
                client_operation_id: "shutdown-character".into()
            }
        )
        .await
        .expect_err("shutdown")
        .code,
        ApiErrorCode::Cancelled
    );
    assert!(
        harness
            .context
            .backend()
            .database()
            .lookup_api_operation("character_save", "shutdown-character")
            .expect("receipt")
            .is_none()
    );
}

#[tokio::test]
async fn character_editor_draft_requires_nullable_fields_and_preserves_companion_config() {
    use lettuce_characters::CharacterRepository;
    let harness = harness(Reply::Text("reply"));
    let database = harness.context.backend().database();
    let character = CharacterRepository::get(database, harness.character_id)
        .expect("get")
        .expect("exists")
        .character;
    let mut defaults = character.defaults;
    defaults.interaction_mode = lettuce_characters::InteractionMode::Companion;
    defaults.companion_soul = Some(lettuce_companions::CompanionSoulConfig::default());
    CharacterRepository::update_defaults(
        database,
        harness.character_id,
        character.revision,
        defaults,
        harness.context.now(),
    )
    .expect("set companion");
    let before = character_get(
        &harness.context,
        dto::CharacterGetRequest {
            character_id: harness.character_id.to_string(),
        },
    )
    .await
    .expect("get");
    let mut partial = serde_json::to_value(&before.draft).expect("serialize");
    partial["profile"]
        .as_object_mut()
        .expect("object")
        .remove("scenario");
    assert!(serde_json::from_value::<dto::CharacterDraft>(partial).is_err());
    let mut partial = serde_json::to_value(&before.draft).expect("serialize");
    partial
        .as_object_mut()
        .expect("object")
        .remove("image_recommendation");
    assert!(serde_json::from_value::<dto::CharacterDraft>(partial).is_err());
    let mut draft = before.draft.clone();
    draft.profile.name = "Edited companion".into();
    draft
        .defaults
        .companion_soul
        .as_mut()
        .expect("config")
        .prompting
        .style_notes = "authored style".into();
    let after = character_save(
        &harness.context,
        dto::CharacterSaveRequest {
            character_id: Some(before.id),
            draft: draft.clone(),
            expected_revision: Some(before.revision),
            client_operation_id: "edit-companion".into(),
        },
    )
    .await
    .expect("edit without creation gate");
    assert_eq!(after.draft, draft);
}

#[tokio::test]
async fn character_editor_round_trips_scenes_starters_and_recursive_widgets() {
    let harness = harness(Reply::Text("reply"));
    let before = character_get(
        &harness.context,
        dto::CharacterGetRequest {
            character_id: harness.character_id.to_string(),
        },
    )
    .await
    .expect("get");
    let mut draft = before.draft;
    let scene_id = lettuce_types::SceneId::new().to_string();
    let variant_id = lettuce_types::SceneVariantId::new().to_string();
    let starter_id = lettuce_types::ConversationStarterId::new().to_string();
    let document = dto::LibrarySceneDocumentV1 {
        format_version: 1,
        parts: vec![dto::LibraryScenePart::Text {
            text: "Authored scene".into(),
        }],
    };
    draft.scenes = vec![dto::LibrarySceneDraft {
        id: scene_id.clone(),
        ordinal: 0,
        content: document.clone(),
        direction: Some("Scene direction".into()),
        selected_variant_id: Some(variant_id.clone()),
        assets: vec![],
    }];
    draft.variants = vec![dto::LibrarySceneVariantDraft {
        id: variant_id,
        scene_id: scene_id.clone(),
        ordinal: 0,
        content: document,
        direction: Some("Variant direction".into()),
    }];
    draft.starters = vec![dto::LibraryStarterDraft {
        id: starter_id.clone(),
        name: "Opening".into(),
        ordinal: 0,
        messages: vec![dto::LibraryStarterMessage {
            id: lettuce_types::StarterMessageId::new().to_string(),
            role: dto::LibraryStarterRole::Assistant,
            content: "Hello".into(),
        }],
        scene_id: Some(scene_id.clone()),
        prompt_id: None,
        lorebooks: dto::LibraryLorebookSelection::Explicit(vec![]),
    }];
    draft.defaults.default_scene_id = Some(scene_id);
    draft.defaults.default_starter_id = Some(starter_id);
    draft.presentation.chat_appearance.chat_widget_slots.left = vec![dto::LibraryWidgetNode::Box {
        id: "box".into(),
        design: Some(dto::LibraryWidgetDesign::Outline),
        variant: Some(dto::LibraryBoxVariant::Info),
        title: Some("Title".into()),
        description: None,
        children: vec![dto::LibraryWidgetNode::Time {
            id: "clock".into(),
            design: None,
            title: None,
            hour_format: Some(dto::LibraryHourFormat::H24),
            show_seconds: Some(true),
            show_date: Some(false),
        }],
    }];
    let saved = character_save(
        &harness.context,
        dto::CharacterSaveRequest {
            character_id: Some(before.id.clone()),
            draft: draft.clone(),
            expected_revision: Some(before.revision),
            client_operation_id: "graph-editor".into(),
        },
    )
    .await
    .expect("save graph");
    assert_eq!(saved.draft, draft);
    assert_eq!(
        character_get(
            &harness.context,
            dto::CharacterGetRequest {
                character_id: before.id
            }
        )
        .await
        .expect("get"),
        saved
    );
}

#[tokio::test]
async fn character_lifecycle_replays_original_view_and_enforces_revision() {
    let harness = harness(Reply::Text("reply"));
    let request = dto::CharacterLifecycleRequest {
        character_id: harness.character_id.to_string(),
        expected_revision: 1,
        client_operation_id: "archive-character".into(),
    };
    let archived = character_archive(&harness.context, request.clone())
        .await
        .expect("archive");
    assert!(archived.archived);
    assert_eq!(archived.revision, 2);
    let restored = character_restore(
        &harness.context,
        dto::CharacterLifecycleRequest {
            expected_revision: archived.revision,
            client_operation_id: "restore-character".into(),
            ..request.clone()
        },
    )
    .await
    .expect("restore");
    assert!(!restored.archived);
    assert_eq!(restored.revision, 3);
    assert_eq!(
        character_archive(&harness.context, request.clone())
            .await
            .expect("replay"),
        archived
    );
    let mut changed = request.clone();
    changed.expected_revision = 3;
    assert_eq!(
        character_archive(&harness.context, changed)
            .await
            .expect_err("digest")
            .code,
        ApiErrorCode::Conflict
    );
    let mut stale = request;
    stale.client_operation_id = "stale-archive".into();
    assert_eq!(
        character_archive(&harness.context, stale)
            .await
            .expect_err("revision")
            .code,
        ApiErrorCode::Conflict
    );
    harness.context.shutdown_token().cancel();
    assert_eq!(
        character_restore(
            &harness.context,
            dto::CharacterLifecycleRequest {
                character_id: harness.character_id.to_string(),
                expected_revision: restored.revision,
                client_operation_id: "shutdown-restore".into(),
            }
        )
        .await
        .expect_err("shutdown")
        .code,
        ApiErrorCode::Cancelled
    );
}
