use std::sync::Arc;

use lettuce_contracts::{self as dto, ApiErrorCode};
use lettuce_models::{ChatParameterProfile, ModelSettingsLayer};

use crate::api::app::settings::convert;
use crate::api::tests::{Harness, RecordingStream, Reply, harness, launch, send};
use crate::api::*;

fn layer(temperature: Option<f64>) -> dto::SettingsModelSettingsLayer {
    convert(
        ModelSettingsLayer {
            chat_parameters: ChatParameterProfile {
                temperature,
                ..ChatParameterProfile::default()
            },
            ..ModelSettingsLayer::default()
        },
        "layer",
    )
    .expect("layer")
}

async fn update(
    harness: &Harness,
    conversation_id: &str,
    expected: Option<u64>,
    patch: dto::ConversationSettingsPatch,
) -> Result<dto::ConversationSettingsView, dto::ApiError> {
    conversation_settings_update(
        &harness.context,
        dto::ConversationSettingsUpdateRequest {
            conversation_id: conversation_id.into(),
            expected_settings_revision: expected,
            patch,
        },
    )
    .await
}

fn set_layer(layer: dto::SettingsModelSettingsLayer) -> dto::ConversationSettingsPatch {
    dto::ConversationSettingsPatch {
        model_settings: Some(layer),
        ..dto::ConversationSettingsPatch::default()
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_chat_sets_and_clears_its_own_model_settings_under_the_settings_revision() {
    let harness = harness(Reply::Text("Hello."));
    let conversation_id = launch(&harness, "layer-launch").await;
    let view = conversation_settings_get(
        &harness.context,
        dto::ConversationSettingsGetRequest {
            conversation_id: conversation_id.clone(),
        },
    )
    .await
    .expect("view");
    assert_eq!(view.settings_revision, None);
    assert_eq!(view.model_settings, Some(layer(None)));
    let set = update(
        &harness,
        &conversation_id,
        view.settings_revision,
        set_layer(layer(Some(0.31))),
    )
    .await
    .expect("set");
    assert_eq!(set.model_settings, Some(layer(Some(0.31))));
    let revision = set
        .settings_revision
        .expect("the first settings write creates a revision");
    let stale = update(
        &harness,
        &conversation_id,
        view.settings_revision,
        set_layer(layer(Some(0.5))),
    )
    .await
    .expect_err("stale revision");
    assert_eq!(stale.code, ApiErrorCode::Conflict);
    let kept = update(
        &harness,
        &conversation_id,
        Some(revision),
        dto::ConversationSettingsPatch {
            author_note: Some("Note".into()),
            ..dto::ConversationSettingsPatch::default()
        },
    )
    .await
    .expect("another field keeps the layer");
    assert_eq!(kept.model_settings, Some(layer(Some(0.31))));
    let cleared = update(
        &harness,
        &conversation_id,
        kept.settings_revision,
        set_layer(layer(None)),
    )
    .await
    .expect("clear");
    assert_eq!(cleared.model_settings, Some(layer(None)));
    let mut not_finite = layer(None);
    not_finite.chat_parameters.temperature = Some(f64::NAN);
    let invalid = update(
        &harness,
        &conversation_id,
        cleared.settings_revision,
        set_layer(not_finite),
    )
    .await
    .expect_err("not finite");
    assert_eq!(invalid.code, ApiErrorCode::InvalidInput);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_turn_uses_the_chats_own_model_settings() {
    let harness = harness(Reply::Text("Hello."));
    let conversation_id = launch(&harness, "layer-turn-launch").await;
    update(
        &harness,
        &conversation_id,
        None,
        set_layer(layer(Some(0.31))),
    )
    .await
    .expect("set");
    send(
        &harness,
        &conversation_id,
        "layer-turn-send",
        "Hi",
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect("send");
    let worker = ConversationGenerationWorker::new(harness.context.clone());
    assert!(worker.run_once().await.expect("run generation"));
    let requests = harness.provider.requests.lock().expect("requests");
    assert_eq!(
        requests[0].profile.chat_profile.parameters.temperature,
        Some(0.31)
    );
}

fn runtime_layer(threads: Option<u32>) -> dto::SettingsModelSettingsLayer {
    convert(
        ModelSettingsLayer {
            llama_cpp: lettuce_models::LlamaCppSettings {
                threads,
                ..lettuce_models::LlamaCppSettings::default()
            },
            ..ModelSettingsLayer::default()
        },
        "layer",
    )
    .expect("layer")
}

#[tokio::test(flavor = "multi_thread")]
async fn the_llama_cpp_side_of_the_layer_round_trips() {
    let harness = harness(Reply::Text("Hello."));
    let conversation_id = launch(&harness, "runtime-launch").await;
    let set = update(
        &harness,
        &conversation_id,
        None,
        set_layer(runtime_layer(Some(3))),
    )
    .await
    .expect("set");
    assert_eq!(set.model_settings, Some(runtime_layer(Some(3))));
    let view = conversation_settings_get(
        &harness.context,
        dto::ConversationSettingsGetRequest { conversation_id },
    )
    .await
    .expect("view");
    assert_eq!(view.model_settings, Some(runtime_layer(Some(3))));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_group_chat_has_no_model_settings_and_refuses_a_patch() {
    let harness = harness(Reply::Text("Hello."));
    let database = harness.context.backend().database();
    let first = crate::launch::tests::seed_named_character(database, "Ada");
    let second = crate::launch::tests::seed_named_character(database, "Bea");
    let group_id = crate::launch::tests::seed_group(
        database,
        vec![
            crate::launch::tests::member(first, 0),
            crate::launch::tests::member(second, 1),
        ],
        None,
        |_| {},
    );
    let conversation_id = conversation_launch_group(
        &harness.context,
        dto::LaunchGroupRequest {
            group_id: group_id.to_string(),
            client_operation_id: "layer-group-launch".into(),
        },
    )
    .await
    .expect("launch group")
    .conversation_id;
    let view = conversation_settings_get(
        &harness.context,
        dto::ConversationSettingsGetRequest {
            conversation_id: conversation_id.clone(),
        },
    )
    .await
    .expect("view");
    assert_eq!(view.model_settings, None);
    let refused = update(
        &harness,
        &conversation_id,
        view.settings_revision,
        set_layer(layer(Some(0.31))),
    )
    .await
    .expect_err("a group has no model settings");
    assert_eq!(refused.code, ApiErrorCode::InvalidInput);
    assert!(matches!(
        refused.details,
        Some(dto::ApiErrorDetails::InvalidField { ref field }) if field == "model_settings"
    ));
    let after = conversation_settings_get(
        &harness.context,
        dto::ConversationSettingsGetRequest { conversation_id },
    )
    .await
    .expect("view after");
    assert_eq!(after.settings_revision, view.settings_revision);
}
