use lettuce_contracts::{self as dto, ApiErrorCode};

use crate::api::tests::{Reply, harness};
use crate::api::*;

fn draft() -> dto::PersonaDraft {
    dto::PersonaDraft {
        title: "Writer".into(),
        description: "A writer".into(),
        nickname: Some("W".into()),
        design_description: None,
        avatar_crop: None,
        image_recommendation: Some(dto::LibraryImageRecommendation {
            artifact_id: None,
            unresolved_legacy_name: Some("authored-lora".into()),
            strength: 0.75,
        }),
        media: vec![],
    }
}

#[tokio::test]
async fn persona_save_creates_edits_replays_and_rejects_stale_or_changed_requests() {
    let harness = harness(Reply::Text("reply"));
    let request = dto::PersonaSaveRequest {
        persona_id: None,
        draft: draft(),
        expected_revision: None,
        client_operation_id: "create-persona".into(),
    };
    let created = persona_save(&harness.context, request.clone())
        .await
        .expect("create");
    assert_eq!(created.revision, 1);
    assert_eq!(created.draft, draft());
    assert_eq!(
        persona_save(&harness.context, request.clone())
            .await
            .expect("replay"),
        created
    );
    assert_eq!(
        persona_get(
            &harness.context,
            dto::PersonaGetRequest {
                persona_id: created.id.clone()
            }
        )
        .await
        .expect("get"),
        created
    );
    let mut changed = request;
    changed.draft.title = "Another".into();
    assert_eq!(
        persona_save(&harness.context, changed)
            .await
            .expect_err("changed digest")
            .code,
        ApiErrorCode::Conflict
    );
    let mut update = dto::PersonaSaveRequest {
        persona_id: Some(created.id.clone()),
        draft: created.draft.clone(),
        expected_revision: Some(created.revision),
        client_operation_id: "update-persona".into(),
    };
    update.draft.description = "Edited".into();
    let edited = persona_save(&harness.context, update.clone())
        .await
        .expect("edit");
    assert_eq!(edited.revision, 2);
    assert_eq!(edited.draft.description, "Edited");
    assert_eq!(
        edited.draft.image_recommendation,
        created.draft.image_recommendation
    );
    update.client_operation_id = "stale-persona".into();
    assert_eq!(
        persona_save(&harness.context, update)
            .await
            .expect_err("stale")
            .code,
        ApiErrorCode::Conflict
    );
}

#[tokio::test]
async fn persona_save_rejects_shutdown_before_writing() {
    let harness = harness(Reply::Text("reply"));
    harness.context.begin_shutdown();
    let error = persona_save(
        &harness.context,
        dto::PersonaSaveRequest {
            persona_id: None,
            draft: draft(),
            expected_revision: None,
            client_operation_id: "shutdown-persona".into(),
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
            .lookup_api_operation("persona_save", "shutdown-persona")
            .expect("receipt")
            .is_none()
    );
}
