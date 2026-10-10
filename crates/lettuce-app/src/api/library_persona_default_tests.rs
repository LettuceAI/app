use lettuce_characters::PersonaRepository;
use lettuce_contracts::{self as dto, ApiErrorCode};

use super::tests::{Reply, harness};
use super::*;

#[tokio::test]
async fn persona_defaults_replay_compare_the_singleton_and_preserve_persona_revisions() {
    let harness = harness(Reply::Text("reply"));
    let created = persona_save(
        &harness.context,
        dto::PersonaSaveRequest {
            persona_id: None,
            expected_revision: None,
            client_operation_id: "persona".into(),
            draft: dto::PersonaDraft {
                title: "Writer".into(),
                description: "Description".into(),
                nickname: None,
                design_description: None,
                avatar_crop: None,
                image_recommendation: None,
                media: vec![],
            },
        },
    )
    .await
    .expect("create");
    let state = PersonaRepository::get_default_snapshot(harness.context.backend().database())
        .expect("default")
        .state;
    let request = dto::PersonaDefaultSetRequest {
        persona_id: created.id.clone(),
        expected_revision: state.revision.get(),
        client_operation_id: "set-default".into(),
    };
    let selected = persona_default_set(&harness.context, request.clone())
        .await
        .expect("set");
    assert_eq!(selected.persona_id, Some(created.id.clone()));
    assert_eq!(selected.revision, state.revision.get() + 1);
    assert_eq!(
        persona_default_set(&harness.context, request.clone())
            .await
            .expect("replay"),
        selected
    );
    let mut changed = request.clone();
    changed.persona_id = lettuce_types::PersonaId::new().to_string();
    assert_eq!(
        persona_default_set(&harness.context, changed)
            .await
            .expect_err("changed digest")
            .code,
        ApiErrorCode::Conflict
    );
    let mut stale = request;
    stale.client_operation_id = "stale".into();
    assert_eq!(
        persona_default_set(&harness.context, stale)
            .await
            .expect_err("stale revision")
            .code,
        ApiErrorCode::Conflict
    );
    let clear = dto::PersonaDefaultClearRequest {
        expected_revision: selected.revision,
        client_operation_id: "clear-default".into(),
    };
    let cleared = persona_default_clear(&harness.context, clear.clone())
        .await
        .expect("clear");
    assert_eq!(cleared.persona_id, None);
    assert_eq!(
        persona_default_clear(&harness.context, clear)
            .await
            .expect("replay"),
        cleared
    );
    assert_eq!(
        persona_get(
            &harness.context,
            dto::PersonaGetRequest {
                persona_id: created.id
            }
        )
        .await
        .expect("get")
        .revision,
        created.revision
    );
}

#[tokio::test]
async fn persona_default_writes_reject_shutdown() {
    let harness = harness(Reply::Text("reply"));
    harness.context.begin_shutdown();
    assert_eq!(
        persona_default_clear(
            &harness.context,
            dto::PersonaDefaultClearRequest {
                expected_revision: 1,
                client_operation_id: "shutdown".into(),
            }
        )
        .await
        .expect_err("shutdown")
        .code,
        ApiErrorCode::Cancelled
    );
}
