use std::sync::Arc;

use lettuce_characters::GroupRepository;
use lettuce_contracts::{self as dto, ApiErrorCode};
use lettuce_conversations::{ConversationReader, ConversationRepositoryError};
use lettuce_types::ConversationId;

use super::tests::{RecordingStream, Reply, harness, send};
use super::*;
use crate::launch::tests::{member, seed_group, seed_named_character};

#[tokio::test(flavor = "multi_thread")]
async fn group_delete_waits_for_streaming_work_and_replays_without_partial_deletion() {
    let harness = harness(Reply::UntilCancelled);
    let database = harness.context.backend().database();
    let first = harness.character_id;
    let second = seed_named_character(database, "Second");
    let group = seed_group(
        database,
        vec![member(first, 0), member(second, 1)],
        None,
        |group| {
            group.speaker_selection = lettuce_characters::SpeakerSelection::RoundRobin;
        },
    );
    let chat = conversation_launch_group(
        &harness.context,
        dto::LaunchGroupRequest {
            group_id: group.to_string(),
            client_operation_id: "group-delete-launch".into(),
        },
    )
    .await
    .expect("launch")
    .conversation_id;
    send(
        &harness,
        &chat,
        "group-delete-send",
        "Wait",
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect("send");
    let request = dto::GroupDeleteRequest {
        group_id: group.to_string(),
        expected_revision: 1,
        client_operation_id: "group-delete".into(),
    };
    let worker = ConversationGenerationWorker::new(harness.context.clone());
    let (ran, deleted) = tokio::join!(worker.run_once(), async {
        harness.provider.entered.notified().await;
        group_delete(&harness.context, request.clone()).await
    });
    assert!(ran.expect("worker"));
    deleted.expect("delete settles work");
    assert!(
        GroupRepository::get(database, group)
            .expect("group read")
            .is_none()
    );
    assert!(matches!(
        ConversationReader::get(database, chat.parse::<ConversationId>().expect("id")),
        Err(ConversationRepositoryError::NotFound)
    ));
    group_delete(&harness.context, request.clone())
        .await
        .expect("replay");
    assert_eq!(
        group_delete(
            &harness.context,
            dto::GroupDeleteRequest {
                expected_revision: 2,
                ..request
            }
        )
        .await
        .expect_err("changed digest")
        .code,
        ApiErrorCode::Conflict
    );
}

#[tokio::test]
async fn deleting_a_persona_moves_explicit_groups_to_inherit_and_replays() {
    let harness = harness(Reply::Text("reply".into()));
    let database = harness.context.backend().database();
    let persona = crate::launch::tests::seed_persona(database, "Deleted");
    let second = seed_named_character(database, "Second");
    let group = seed_group(
        database,
        vec![member(harness.character_id, 0), member(second, 1)],
        None,
        |group| group.persona = lettuce_characters::Selection::Explicit(persona),
    );
    let request = dto::PersonaDeleteRequest {
        persona_id: persona.to_string(),
        expected_revision: 1,
        client_operation_id: "persona-delete".into(),
    };
    persona_delete(&harness.context, request.clone())
        .await
        .expect("delete");
    persona_delete(&harness.context, request)
        .await
        .expect("replay");
    assert_eq!(
        GroupRepository::get(database, group)
            .expect("group")
            .expect("exists")
            .group
            .persona,
        lettuce_characters::Selection::Inherit
    );
}
