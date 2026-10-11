use lettuce_characters::CharacterRepository;
use lettuce_contracts::{self as dto, ApiErrorCode};
use lettuce_types::CharacterId;

use super::tests::{Reply, harness, launch};
use super::*;

#[tokio::test]
async fn character_duplicate_is_an_authored_copy_with_a_stable_replay_result() {
    let harness = harness(Reply::Text("reply"));
    launch(&harness, "source-chat").await;
    let request = dto::CharacterDuplicateRequest {
        character_id: harness.character_id.to_string(),
        client_operation_id: "duplicate".into(),
    };
    let copied = character_duplicate(&harness.context, request.clone())
        .await
        .expect("duplicate");
    assert_eq!(
        character_duplicate(&harness.context, request.clone())
            .await
            .expect("replay"),
        copied
    );
    assert_ne!(copied.id, request.character_id);
    assert!(
        conversations_list(
            &harness.context,
            dto::ConversationsListRequest {
                character_id: Some(copied.id.clone()),
                ..Default::default()
            }
        )
        .await
        .expect("copy chats")
        .items
        .is_empty()
    );
    let database = harness.context.backend().database();
    let source = CharacterRepository::get(database, harness.character_id)
        .expect("source")
        .expect("exists");
    let copy = CharacterRepository::get(database, copied.id.parse::<CharacterId>().expect("id"))
        .expect("copy")
        .expect("exists");
    assert_eq!(
        copy.character.profile.name,
        format!("{} (Copy)", source.character.profile.name)
    );
    assert_eq!(
        copy.character.image_recommendation,
        source.character.image_recommendation
    );
    let mut changed = request;
    changed.character_id = CharacterId::new().to_string();
    assert_eq!(
        character_duplicate(&harness.context, changed)
            .await
            .expect_err("changed digest")
            .code,
        ApiErrorCode::Conflict
    );
}

#[tokio::test]
async fn companion_duplication_reports_both_missing_models_without_a_receipt() {
    let harness = harness(Reply::Text("reply"));
    let database = harness.context.backend().database();
    let character = CharacterRepository::get(database, harness.character_id)
        .expect("source")
        .expect("exists")
        .character;
    let mut defaults = character.defaults;
    defaults.interaction_mode = lettuce_characters::InteractionMode::Companion;
    CharacterRepository::update_defaults(
        database,
        harness.character_id,
        character.revision,
        defaults,
        harness.context.now(),
    )
    .expect("companion");
    let error = character_duplicate(
        &harness.context,
        dto::CharacterDuplicateRequest {
            character_id: harness.character_id.to_string(),
            client_operation_id: "missing-models".into(),
        },
    )
    .await
    .expect_err("missing models");
    assert_eq!(error.code, ApiErrorCode::ModelRequired);
    assert_eq!(
        error.details,
        Some(dto::ApiErrorDetails::MissingModels {
            missing_models: vec![dto::RequiredModel::Embedding, dto::RequiredModel::Emotion]
        })
    );
    assert!(
        database
            .lookup_api_operation("character_duplicate", "missing-models")
            .expect("receipt")
            .is_none()
    );
}

#[tokio::test]
async fn character_duplicate_rejects_shutdown_without_writing() {
    let harness = harness(Reply::Text("reply"));
    harness.context.begin_shutdown();
    let error = character_duplicate(
        &harness.context,
        dto::CharacterDuplicateRequest {
            character_id: harness.character_id.to_string(),
            client_operation_id: "shutdown-duplicate".into(),
        },
    )
    .await
    .expect_err("shutdown");
    assert_eq!(error.code, ApiErrorCode::Cancelled);
    assert!(
        harness
            .context
            .backend()
            .database()
            .lookup_api_operation("character_duplicate", "shutdown-duplicate")
            .expect("receipt")
            .is_none()
    );
}
