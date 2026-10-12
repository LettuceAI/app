use std::sync::Arc;

use lettuce_characters::GroupRepository;
use lettuce_contracts::{self as dto, ApiErrorCode};
use lettuce_conversations::{ConversationReader, ConversationRepositoryError};
use lettuce_types::ConversationId;

use crate::api::tests::{RecordingStream, Reply, harness, launch, send};
use crate::api::*;
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
    let harness = harness(Reply::Text("reply"));
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

#[tokio::test(flavor = "multi_thread")]
async fn character_delete_cancels_its_stream_and_replays() {
    let harness = harness(Reply::UntilCancelled);
    let chat = launch(&harness, "character-delete-launch").await;
    send(
        &harness,
        &chat,
        "delete-character-send",
        "Wait",
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect("send");
    let request = dto::CharacterDeleteRequest {
        character_id: harness.character_id.to_string(),
        expected_revision: 1,
        client_operation_id: "delete-character".into(),
    };
    let worker = ConversationGenerationWorker::new(harness.context.clone());
    let (ran, deleted) = tokio::join!(worker.run_once(), async {
        harness.provider.entered.notified().await;
        character_delete(&harness.context, request.clone()).await
    });
    assert!(ran.expect("worker"));
    deleted.expect("delete");
    assert!(
        lettuce_characters::CharacterRepository::get(
            harness.context.backend().database(),
            harness.character_id
        )
        .expect("character")
        .is_none()
    );
    assert!(matches!(
        ConversationReader::get(
            harness.context.backend().database(),
            chat.parse().expect("id")
        ),
        Err(ConversationRepositoryError::NotFound)
    ));
    character_delete(&harness.context, request.clone())
        .await
        .expect("replay");
    assert_eq!(
        character_delete(
            &harness.context,
            dto::CharacterDeleteRequest {
                expected_revision: 2,
                ..request
            }
        )
        .await
        .expect_err("digest")
        .code,
        ApiErrorCode::Conflict
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn character_delete_preserves_another_members_running_turn() {
    let harness = harness(Reply::UntilCancelled);
    let database = harness.context.backend().database();
    let other = seed_named_character(database, "Other");
    let group = seed_group(
        database,
        vec![member(other, 0), member(harness.character_id, 1)],
        None,
        |group| group.speaker_selection = lettuce_characters::SpeakerSelection::RoundRobin,
    );
    let chat = conversation_launch_group(
        &harness.context,
        dto::LaunchGroupRequest {
            group_id: group.to_string(),
            client_operation_id: "other-member-launch".into(),
        },
    )
    .await
    .expect("launch")
    .conversation_id;
    send(
        &harness,
        &chat,
        "other-member-send",
        "Wait",
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect("send");
    let worker = ConversationGenerationWorker::new(harness.context.clone());
    let (ran, ()) = tokio::join!(worker.run_once(), async {
        harness.provider.entered.notified().await;
        character_delete(
            &harness.context,
            dto::CharacterDeleteRequest {
                character_id: harness.character_id.to_string(),
                expected_revision: 1,
                client_operation_id: "delete-other-member".into(),
            },
        )
        .await
        .expect("delete");
        let id = chat.parse().expect("id");
        assert!(
            lettuce_conversations::ConversationOverviewReader::live_turn(database, id)
                .expect("turn")
                .is_some()
        );
        assert_eq!(
            GroupRepository::get(database, group)
                .expect("get")
                .expect("group")
                .group
                .members
                .len(),
            1
        );
        crate::api::conversations::delete::cancel_conversation_work(&harness.context, id)
            .expect("cleanup cancel");
    });
    assert!(ran.expect("worker"));
    assert!(ConversationReader::get(database, chat.parse().expect("id")).is_ok());
}

#[tokio::test]
async fn character_delete_settles_unselected_group_turn_and_preserves_history() {
    let harness = harness(Reply::Text("reply"));
    let database = harness.context.backend().database();
    let other = seed_named_character(database, "Other");
    let group = seed_group(
        database,
        vec![member(harness.character_id, 0), member(other, 1)],
        None,
        |_| {},
    );
    let chat = conversation_launch_group(
        &harness.context,
        dto::LaunchGroupRequest {
            group_id: group.to_string(),
            client_operation_id: "unselected-launch".into(),
        },
    )
    .await
    .expect("launch")
    .conversation_id;
    send(
        &harness,
        &chat,
        "unselected-send",
        "History",
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect("send");
    character_delete(
        &harness.context,
        dto::CharacterDeleteRequest {
            character_id: harness.character_id.to_string(),
            expected_revision: 1,
            client_operation_id: "delete-unselected".into(),
        },
    )
    .await
    .expect("delete");
    let id = chat.parse().expect("id");
    assert!(ConversationReader::get(database, id).is_ok());
    assert!(
        lettuce_conversations::ConversationOverviewReader::live_turn(database, id)
            .expect("turn")
            .is_none()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn character_delete_does_not_cancel_a_different_speaker_selected_after_its_read() {
    let h = harness(Reply::UntilCancelled);
    let database = h.context.backend().database();
    let other = seed_named_character(database, "Other");
    let group = seed_group(
        database,
        vec![member(other, 0), member(h.character_id, 1)],
        None,
        |group| group.speaker_selection = lettuce_characters::SpeakerSelection::RoundRobin,
    );
    let chat = conversation_launch_group(
        &h.context,
        dto::LaunchGroupRequest {
            group_id: group.to_string(),
            client_operation_id: "selection-race-launch".into(),
        },
    )
    .await
    .expect("launch")
    .conversation_id;
    send(
        &h,
        &chat,
        "selection-race-send",
        "Wait",
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect("send");
    let id = chat.parse().expect("id");
    let worker_thread = Arc::new(std::sync::Mutex::new(None));
    let worker_slot = Arc::clone(&worker_thread);
    let provider = Arc::clone(&h.provider);
    let hook = Arc::new(move |context: &ApiContext| {
        let context = context.clone();
        let handle = std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("worker runtime")
                .block_on(ConversationGenerationWorker::new(context).run_once())
        });
        *worker_slot.lock().expect("worker slot") = Some(handle);
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("wait runtime")
            .block_on(provider.entered.notified());
    }) as crate::api::conversations::delete::BeforeSettle;
    let character = h.character_id;
    h.context
        .blocking(move |context| {
            super::delete::cancel_character_group_turn(context, id, character, &hook)
        })
        .await
        .expect("stale cancellation skipped");
    let turn = lettuce_conversations::ConversationOverviewReader::live_turn(database, id)
        .expect("live")
        .expect("other member still streaming");
    assert_eq!(
        ConversationReader::get_turn(database, turn)
            .expect("turn")
            .status,
        lettuce_conversations::GenerationTurnStatus::Running
    );
    character_delete(
        &h.context,
        dto::CharacterDeleteRequest {
            character_id: h.character_id.to_string(),
            expected_revision: 1,
            client_operation_id: "selection-race-delete".into(),
        },
    )
    .await
    .expect("delete leaves other speaker running");
    assert!(
        lettuce_conversations::ConversationOverviewReader::live_turn(database, id)
            .expect("live")
            .is_some()
    );
    crate::api::conversations::delete::cancel_conversation_work(&h.context, id)
        .expect("cleanup cancel");
    assert!(
        worker_thread
            .lock()
            .expect("worker slot")
            .take()
            .expect("worker")
            .join()
            .expect("thread")
            .expect("worker result")
    );
}
