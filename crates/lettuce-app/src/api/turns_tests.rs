use std::sync::Arc;

use lettuce_contracts::{self as dto, ApiErrorCode, ApiErrorDetails, GenerationEvent};
use lettuce_conversations::{
    ConversationLifecycle, ConversationReader, ConversationRepository, GenerationTurnStatus,
    MessageDraft, MessagePart, MessageRole, MessageVisibility, ParticipantRole, SendConversation,
    TombstoneMessage,
};
use lettuce_types::ConversationId;

use super::tests::{Harness, RecordingStream, Reply, harness, launch, send};
use super::*;

pub(super) fn conversation(harness: &Harness, id: &str) -> lettuce_conversations::Conversation {
    ConversationReader::get(
        harness.context.backend().database(),
        id.parse::<ConversationId>().expect("id"),
    )
    .expect("conversation")
    .conversation
}

pub(super) async fn open(harness: &Harness, id: &str) -> dto::ConversationView {
    conversation_open(
        &harness.context,
        dto::ConversationOpenRequest {
            conversation_id: id.into(),
        },
    )
    .await
    .expect("open")
}

pub(super) async fn run_generation(harness: &Harness) {
    let worker = ConversationGenerationWorker::new(harness.context.clone());
    assert!(worker.run_once().await.expect("worker ran"));
}

/// A chat with one user message and its settled reply; returns the
/// conversation and the reply's message id.
pub(super) async fn replied_chat(harness: &Harness, key: &str) -> (String, String) {
    let chat = launch(harness, &format!("{key}-launch")).await;
    send(
        harness,
        &chat,
        &format!("{key}-send"),
        "Hi there",
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect("send");
    run_generation(harness).await;
    let view = open(harness, &chat).await;
    let reply = view
        .messages
        .items
        .iter()
        .find(|message| message.role == dto::MessageRole::Assistant)
        .expect("reply")
        .id
        .clone();
    (chat, reply)
}

pub(super) fn regenerate_request(
    harness: &Harness,
    chat: &str,
    message: &str,
    key: &str,
) -> dto::ConversationRegenerateRequest {
    dto::ConversationRegenerateRequest {
        conversation_id: chat.into(),
        message_id: message.into(),
        expected_revision: conversation(harness, chat).revision.get(),
        client_operation_id: key.into(),
        guidance: None,
        model_profile_id: None,
        forced_speaker_participant_id: None,
        swap_places: false,
    }
}

pub(super) fn continue_request(
    harness: &Harness,
    chat: &str,
    key: &str,
) -> dto::ConversationContinueRequest {
    dto::ConversationContinueRequest {
        conversation_id: chat.into(),
        expected_revision: conversation(harness, chat).revision.get(),
        client_operation_id: key.into(),
        forced_speaker_participant_id: None,
        swap_places: false,
    }
}

fn last_event(stream: &RecordingStream) -> GenerationEvent {
    stream.events().last().expect("an event").clone()
}

#[tokio::test(flavor = "multi_thread")]
async fn regenerate_adds_a_variant_replays_its_key_and_conflicts_on_another_request_or_a_stale_revision()
 {
    let harness = harness(Reply::Text("Hello."));
    let (chat, reply) = replied_chat(&harness, "regen").await;
    let stale = conversation(&harness, &chat).revision.get();
    let stream = Arc::new(RecordingStream::default());
    let mut request = regenerate_request(&harness, &chat, &reply, "regen-1");
    request.guidance = Some("  shorter ".into());
    let accepted = conversation_regenerate(&harness.context, request.clone(), stream.clone())
        .await
        .expect("regenerate");
    let turn = ConversationReader::get_turn(
        harness.context.backend().database(),
        accepted.turn_id.parse().expect("turn id"),
    )
    .expect("turn");
    assert_eq!(turn.guidance.as_deref(), Some("shorter"));
    run_generation(&harness).await;
    assert_eq!(
        last_event(&stream),
        GenerationEvent::Completed {
            turn_id: accepted.turn_id.clone(),
            message_id: reply.clone()
        }
    );
    let view = open(&harness, &chat).await;
    let shown = view
        .messages
        .items
        .iter()
        .find(|message| message.id == reply)
        .expect("reply");
    assert_eq!(shown.candidate_count, 2);
    assert_eq!(shown.candidate_index, Some(1));
    assert_eq!(view.messages.items.len(), 2);

    let replay_stream = Arc::new(RecordingStream::default());
    let replayed =
        conversation_regenerate(&harness.context, request.clone(), replay_stream.clone())
            .await
            .expect("replayed");
    assert_eq!(replayed, accepted);
    assert_eq!(
        last_event(&replay_stream),
        GenerationEvent::Completed {
            turn_id: accepted.turn_id.clone(),
            message_id: reply.clone()
        }
    );
    assert_eq!(harness.provider.requests.lock().expect("requests").len(), 2);

    let mut other = request.clone();
    other.guidance = Some("longer".into());
    let conflict = conversation_regenerate(
        &harness.context,
        other,
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect_err("another request under the key");
    assert_eq!(conflict.code, ApiErrorCode::Conflict);

    let mut old = regenerate_request(&harness, &chat, &reply, "regen-2");
    old.expected_revision = stale;
    let error =
        conversation_regenerate(&harness.context, old, Arc::new(RecordingStream::default()))
            .await
            .expect_err("stale revision");
    assert_eq!(error.code, ApiErrorCode::Conflict);
    assert!(
        error.details.is_none(),
        "a stale revision is a plain conflict, not busy"
    );
    conversation_regenerate(
        &harness.context,
        regenerate_request(&harness, &chat, &reply, "regen-3"),
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect("with the current revision");
}

#[tokio::test(flavor = "multi_thread")]
async fn every_turn_operation_is_busy_while_a_turn_is_live() {
    let harness = harness(Reply::Text("Hello."));
    let (chat, reply) = replied_chat(&harness, "busy").await;
    conversation_regenerate(
        &harness.context,
        regenerate_request(&harness, &chat, &reply, "busy-regen"),
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect("regenerate");
    let stream = || Arc::new(RecordingStream::default());
    let revision = conversation(&harness, &chat).revision.get();
    let regenerate = conversation_regenerate(
        &harness.context,
        regenerate_request(&harness, &chat, &reply, "busy-regen-2"),
        stream(),
    )
    .await
    .expect_err("regenerate");
    let continued = conversation_continue(
        &harness.context,
        continue_request(&harness, &chat, "busy-continue"),
        stream(),
    )
    .await
    .expect_err("continue");
    let retried = conversation_retry(
        &harness.context,
        dto::ConversationRetryRequest {
            conversation_id: chat.clone(),
            turn_id: uuid::Uuid::new_v4().to_string(),
            client_operation_id: "busy-retry".into(),
        },
        stream(),
    )
    .await
    .expect_err("retry");
    let added = conversation_add_user_message(
        &harness.context,
        dto::ConversationAddUserMessageRequest {
            conversation_id: chat.clone(),
            text: "One more".into(),
            expected_revision: revision,
            client_operation_id: "busy-add".into(),
        },
    )
    .await
    .expect_err("add user message");
    for error in [regenerate, continued, retried, added] {
        assert_eq!(error.code, ApiErrorCode::Busy, "{error:?}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn regenerate_refuses_a_missing_deleted_or_wrong_target_with_typed_errors() {
    let harness = harness(Reply::Text("Hello."));
    let (chat, reply) = replied_chat(&harness, "targets").await;
    let view = open(&harness, &chat).await;
    let user = view
        .messages
        .items
        .iter()
        .find(|message| message.role == dto::MessageRole::User)
        .expect("user message")
        .id
        .clone();
    let stream = || Arc::new(RecordingStream::default());

    let not_a_reply = conversation_regenerate(
        &harness.context,
        regenerate_request(&harness, &chat, &user, "targets-user"),
        stream(),
    )
    .await
    .expect_err("a user message");
    assert_eq!(not_a_reply.code, ApiErrorCode::InvalidInput);
    assert_eq!(
        not_a_reply.details,
        Some(ApiErrorDetails::InvalidField {
            field: "message_id".into()
        })
    );

    let unknown = conversation_regenerate(
        &harness.context,
        regenerate_request(
            &harness,
            &chat,
            &uuid::Uuid::new_v4().to_string(),
            "targets-unknown",
        ),
        stream(),
    )
    .await
    .expect_err("a message the branch does not show");
    assert_eq!(unknown.code, ApiErrorCode::NotFound);

    let database = harness.context.backend().database();
    let stored = conversation(&harness, &chat);
    ConversationRepository::delete_message(
        database,
        &TombstoneMessage {
            conversation_id: stored.id,
            message_id: reply.parse().expect("message id"),
            expected_revision: stored.revision,
            operation: crate::conversation::edit_operation("targets-delete".into(), &[b"delete"])
                .expect("token"),
            descendants: lettuce_conversations::DescendantPolicy::Preserve,
        },
        harness.context.now(),
    )
    .expect("delete the reply");
    let deleted = conversation_regenerate(
        &harness.context,
        regenerate_request(&harness, &chat, &reply, "targets-deleted"),
        stream(),
    )
    .await
    .expect_err("a deleted reply");
    assert_eq!(deleted.code, ApiErrorCode::NotFound);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_deleted_conversation_is_not_found() {
    let harness = harness(Reply::Text("Hello."));
    let (chat, _reply) = replied_chat(&harness, "gone").await;
    let mut stored = conversation(&harness, &chat);
    assert!(super::turns::refuse_deleted(stored.clone()).is_ok());
    stored.lifecycle = ConversationLifecycle::Tombstoned;
    let error = super::turns::refuse_deleted(stored).expect_err("deleted");
    assert_eq!(error.code, ApiErrorCode::NotFound);
}

#[tokio::test(flavor = "multi_thread")]
async fn continue_adds_a_new_reply_and_replays_by_key() {
    let harness = harness(Reply::Text("Hello."));
    let (chat, _reply) = replied_chat(&harness, "continue").await;
    let stream = Arc::new(RecordingStream::default());
    let request = continue_request(&harness, &chat, "continue-1");
    let accepted = conversation_continue(&harness.context, request.clone(), stream.clone())
        .await
        .expect("continue");
    run_generation(&harness).await;
    let GenerationEvent::Completed { message_id, .. } = last_event(&stream) else {
        panic!("the reply did not complete: {:?}", stream.events());
    };
    let view = open(&harness, &chat).await;
    assert_eq!(view.messages.items.len(), 3);
    assert_eq!(view.messages.items[2].id, message_id);
    assert_eq!(view.messages.items[2].role, dto::MessageRole::Assistant);
    let replay = Arc::new(RecordingStream::default());
    let replayed = conversation_continue(&harness.context, request.clone(), replay.clone())
        .await
        .expect("replayed");
    assert_eq!(replayed, accepted);
    assert_eq!(
        last_event(&replay),
        GenerationEvent::Completed {
            turn_id: accepted.turn_id,
            message_id
        }
    );
    let mut different = request;
    different.swap_places = true;
    let conflict = conversation_continue(
        &harness.context,
        different,
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect_err("another request under the key");
    assert_eq!(conflict.code, ApiErrorCode::Conflict);
}

#[tokio::test(flavor = "multi_thread")]
async fn continue_needs_a_message_to_continue_from() {
    let harness = harness(Reply::Text("Hello."));
    let chat = launch(&harness, "empty-launch").await;
    let error = conversation_continue(
        &harness.context,
        continue_request(&harness, &chat, "empty-continue"),
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect_err("nothing to continue");
    assert_eq!(error.code, ApiErrorCode::InvalidInput);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_cancelled_turn_retries_once_and_a_settled_one_cannot() {
    let harness = harness(Reply::Text("Hello."));
    let chat = launch(&harness, "retry-launch").await;
    let accepted = send(
        &harness,
        &chat,
        "retry-send",
        "Stop me",
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect("send");
    generation_cancel(
        &harness.context,
        dto::GenerationCancelRequest {
            turn_id: accepted.turn_id.clone(),
        },
    )
    .await
    .expect("cancel");
    let stream = Arc::new(RecordingStream::default());
    let request = dto::ConversationRetryRequest {
        conversation_id: chat.clone(),
        turn_id: accepted.turn_id.clone(),
        client_operation_id: "retry-1".into(),
    };
    let retried = conversation_retry(&harness.context, request.clone(), stream.clone())
        .await
        .expect("retry");
    assert_ne!(retried.turn_id, accepted.turn_id);
    run_generation(&harness).await;
    assert!(matches!(
        last_event(&stream),
        GenerationEvent::Completed { .. }
    ));
    let replayed = conversation_retry(
        &harness.context,
        request,
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect("replayed retry");
    assert_eq!(replayed, retried);
    let succeeded = conversation_retry(
        &harness.context,
        dto::ConversationRetryRequest {
            conversation_id: chat.clone(),
            turn_id: retried.turn_id.clone(),
            client_operation_id: "retry-2".into(),
        },
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect_err("a settled reply is not retried");
    assert_eq!(succeeded.code, ApiErrorCode::Conflict);
    let foreign = conversation_retry(
        &harness.context,
        dto::ConversationRetryRequest {
            conversation_id: chat,
            turn_id: uuid::Uuid::new_v4().to_string(),
            client_operation_id: "retry-3".into(),
        },
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect_err("an unknown turn");
    assert_eq!(foreign.code, ApiErrorCode::NotFound);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_user_message_is_trimmed_refused_blank_and_replayed() {
    let harness = harness(Reply::Text("Hello."));
    let chat = launch(&harness, "add-launch").await;
    let request = |text: &str, key: &str| dto::ConversationAddUserMessageRequest {
        conversation_id: chat.clone(),
        text: text.into(),
        expected_revision: conversation(&harness, &chat).revision.get(),
        client_operation_id: key.into(),
    };
    let blank = conversation_add_user_message(&harness.context, request("  \n ", "add-blank"))
        .await
        .expect_err("blank");
    assert_eq!(blank.code, ApiErrorCode::InvalidInput);
    let first = request("  Director note  ", "add-1");
    let added = conversation_add_user_message(&harness.context, first.clone())
        .await
        .expect("added");
    assert_eq!(added.message.role, dto::MessageRole::User);
    assert_eq!(
        added.message.parts,
        vec![dto::MessagePartView::Text {
            text: "Director note".into()
        }]
    );
    let replayed = conversation_add_user_message(&harness.context, first)
        .await
        .expect("replayed");
    assert_eq!(replayed.message.id, added.message.id);
    let view = open(&harness, &chat).await;
    assert_eq!(view.messages.items.len(), 1);
    let mut stale = request("Late", "add-2");
    stale.expected_revision = 1;
    let error = conversation_add_user_message(&harness.context, stale)
        .await
        .expect_err("stale");
    assert_eq!(error.code, ApiErrorCode::Conflict);
}

#[tokio::test(flavor = "multi_thread")]
async fn cancelling_an_unknown_or_settled_turn_succeeds() {
    let harness = harness(Reply::Text("Hello."));
    let chat = launch(&harness, "cancel-launch").await;
    let accepted = send(
        &harness,
        &chat,
        "cancel-send",
        "Hi",
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect("send");
    run_generation(&harness).await;
    for turn_id in [accepted.turn_id.clone(), uuid::Uuid::new_v4().to_string()] {
        generation_cancel(&harness.context, dto::GenerationCancelRequest { turn_id })
            .await
            .expect("cancel of a settled or unknown turn");
    }
    let turn = ConversationReader::get_turn(
        harness.context.backend().database(),
        accepted.turn_id.parse().expect("turn id"),
    )
    .expect("turn");
    assert_eq!(turn.status, GenerationTurnStatus::Succeeded);
    let error = generation_cancel(
        &harness.context,
        dto::GenerationCancelRequest {
            turn_id: "not-an-id".into(),
        },
    )
    .await
    .expect_err("a malformed id");
    assert_eq!(error.code, ApiErrorCode::InvalidInput);
}

#[tokio::test(flavor = "multi_thread")]
async fn cancelling_a_regenerate_keeps_the_partial_candidate() {
    let harness = harness(Reply::PartialUntilCancelled("Half a"));
    let chat = launch(&harness, "partial-launch").await;
    let database = harness.context.backend().database();
    let stored = conversation(&harness, &chat);
    let user = stored
        .participants
        .iter()
        .find(|participant| participant.role == ParticipantRole::User)
        .expect("user")
        .id;
    let appended = ConversationRepository::append_user_message(
        database,
        &SendConversation {
            conversation_id: stored.id,
            branch_id: stored.active_branch_id,
            expected_revision: stored.revision,
            operation: crate::conversation::edit_operation("partial-message".into(), &[b"m"])
                .expect("token"),
            message: MessageDraft {
                role: MessageRole::User,
                author_participant_id: Some(user),
                parts: vec![MessagePart::Text { text: "Hi".into() }],
                visibility: MessageVisibility::Visible,
                pinned: false,
                scene_edited: false,
            },
            swap_roles: false,
        },
        harness.context.now(),
    )
    .expect("user message")
    .value;
    let first = conversation_continue(
        &harness.context,
        continue_request(&harness, &chat, "partial-continue"),
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect("continue");
    let worker = ConversationGenerationWorker::new(harness.context.clone());
    let (ran, cancelled) = tokio::join!(worker.run_once(), async {
        harness.provider.entered.notified().await;
        generation_cancel(
            &harness.context,
            dto::GenerationCancelRequest {
                turn_id: first.turn_id.clone(),
            },
        )
        .await
    });
    assert!(ran.expect("worker ran"));
    cancelled.expect("cancel");
    let view = open(&harness, &chat).await;
    let reply = view.messages.items.last().expect("reply").clone();
    assert_eq!(reply.role, dto::MessageRole::Assistant);
    assert_ne!(reply.id, appended.id.to_string());
    assert_eq!(
        reply.parts,
        vec![dto::MessagePartView::Text {
            text: "Half a".into()
        }]
    );

    let regenerated = conversation_regenerate(
        &harness.context,
        regenerate_request(&harness, &chat, &reply.id, "partial-regen"),
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect("regenerate");
    let (ran, cancelled) = tokio::join!(worker.run_once(), async {
        harness.provider.entered.notified().await;
        generation_cancel(
            &harness.context,
            dto::GenerationCancelRequest {
                turn_id: regenerated.turn_id.clone(),
            },
        )
        .await
    });
    assert!(ran.expect("worker ran"));
    cancelled.expect("cancel");
    let view = open(&harness, &chat).await;
    let shown = view.messages.items.last().expect("reply");
    assert_eq!(shown.id, reply.id);
    assert_eq!(shown.candidate_count, 2);
    assert_eq!(shown.candidate_index, Some(1));
}

#[tokio::test(flavor = "multi_thread")]
async fn an_archived_chat_is_restored_by_each_turn_operation() {
    let harness = harness(Reply::Text("Hello."));
    let (chat, reply) = replied_chat(&harness, "archived").await;
    let archive = || async {
        conversation_archive(
            &harness.context,
            dto::ConversationRequest {
                conversation_id: chat.clone(),
            },
        )
        .await
        .expect("archive");
    };
    archive().await;
    conversation_regenerate(
        &harness.context,
        regenerate_request(&harness, &chat, &reply, "archived-regen"),
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect("regenerate on an archived chat");
    assert_eq!(
        conversation(&harness, &chat).lifecycle,
        ConversationLifecycle::Active
    );
    run_generation(&harness).await;
    archive().await;
    conversation_continue(
        &harness.context,
        continue_request(&harness, &chat, "archived-continue"),
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect("continue on an archived chat");
    assert_eq!(
        conversation(&harness, &chat).lifecycle,
        ConversationLifecycle::Active
    );
}

pub(super) struct Cast {
    pub(super) chat: String,
    pub(super) group_id: lettuce_types::GroupId,
    pub(super) ada: lettuce_types::ConversationParticipantId,
    pub(super) bea: lettuce_types::ConversationParticipantId,
    pub(super) cleo: lettuce_types::ConversationParticipantId,
    pub(super) ada_character: lettuce_types::CharacterId,
    pub(super) cleo_character: lettuce_types::CharacterId,
}

pub(super) async fn group_cast(harness: &Harness, key: &str) -> Cast {
    let database = harness.context.backend().database();
    let ada_character = crate::launch::tests::seed_named_character(database, "Ada");
    let bea_character = crate::launch::tests::seed_named_character(database, "Bea");
    let cleo_character = crate::launch::tests::seed_named_character(database, "Cleo");
    let group_id = crate::launch::tests::seed_group(
        database,
        vec![
            crate::launch::tests::member(ada_character, 0),
            crate::launch::tests::member(bea_character, 1),
            crate::launch::tests::member(cleo_character, 2),
        ],
        None,
        |group| group.speaker_selection = lettuce_characters::SpeakerSelection::RoundRobin,
    );
    let chat = conversation_launch_group(
        &harness.context,
        dto::LaunchGroupRequest {
            group_id: group_id.to_string(),
            client_operation_id: format!("{key}-launch"),
        },
    )
    .await
    .expect("launch group")
    .conversation_id;
    let stored = conversation(harness, &chat);
    let participant = |character| {
        stored
            .participants
            .iter()
            .find(|participant| {
                participant.source == lettuce_conversations::ParticipantSource::Character(character)
            })
            .expect("member")
            .id
    };
    Cast {
        ada: participant(ada_character),
        bea: participant(bea_character),
        cleo: participant(cleo_character),
        chat,
        group_id,
        ada_character,
        cleo_character,
    }
}

pub(super) fn author_of(view: &dto::ConversationView, message: &str) -> Option<String> {
    view.messages
        .items
        .iter()
        .find(|item| item.id == message)
        .expect("message")
        .author_participant_id
        .clone()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_group_reply_regenerates_with_its_speaker_or_a_forced_one() {
    let harness = harness(Reply::Text("Hello."));
    let cast = group_cast(&harness, "group-regen").await;
    send(
        &harness,
        &cast.chat,
        "group-regen-send",
        "Hi all",
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect("send");
    run_generation(&harness).await;
    conversation_continue(
        &harness.context,
        continue_request(&harness, &cast.chat, "group-regen-continue"),
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect("continue");
    run_generation(&harness).await;
    let view = open(&harness, &cast.chat).await;
    let replies = view
        .messages
        .items
        .iter()
        .filter(|message| message.role == dto::MessageRole::Assistant)
        .map(|message| message.id.clone())
        .collect::<Vec<_>>();
    assert_eq!(replies.len(), 2);
    let first = &replies[0];
    let original = author_of(&view, first);
    assert_eq!(original.as_deref(), Some(&*cast.ada.to_string()));

    conversation_regenerate(
        &harness.context,
        regenerate_request(&harness, &cast.chat, first, "group-regen-1"),
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect("an older group reply regenerates");
    run_generation(&harness).await;
    let view = open(&harness, &cast.chat).await;
    assert_eq!(author_of(&view, first), original);

    let mut forced = regenerate_request(&harness, &cast.chat, first, "group-regen-2");
    forced.forced_speaker_participant_id = Some(cast.bea.to_string());
    conversation_regenerate(
        &harness.context,
        forced,
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect("forced speaker");
    run_generation(&harness).await;
    let view = open(&harness, &cast.chat).await;
    assert_eq!(
        author_of(&view, first).as_deref(),
        Some(&*cast.bea.to_string())
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_forced_speaker_that_is_not_a_member_is_invalid_input() {
    let harness = harness(Reply::Text("Hello."));
    let cast = group_cast(&harness, "group-forced").await;
    send(
        &harness,
        &cast.chat,
        "group-forced-send",
        "Hi all",
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect("send");
    run_generation(&harness).await;
    let view = open(&harness, &cast.chat).await;
    let reply = view
        .messages
        .items
        .iter()
        .find(|message| message.role == dto::MessageRole::Assistant)
        .expect("reply")
        .id
        .clone();
    let database = harness.context.backend().database();
    let group = lettuce_characters::GroupRepository::get(database, cast.group_id)
        .expect("group")
        .expect("group exists");
    lettuce_characters::GroupRepository::replace_members(
        database,
        cast.group_id,
        group.group.revision,
        vec![
            crate::launch::tests::member(cast.ada_character, 0),
            crate::launch::tests::member(cast.cleo_character, 1),
        ],
        harness.context.now(),
    )
    .expect("bea leaves the group");
    let mut request = regenerate_request(&harness, &cast.chat, &reply, "group-forced-1");
    request.forced_speaker_participant_id = Some(cast.bea.to_string());
    let error = conversation_regenerate(
        &harness.context,
        request,
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect_err("bea is no longer a member");
    assert_eq!(error.code, ApiErrorCode::InvalidInput);
    let mut request = continue_request(&harness, &cast.chat, "group-forced-2");
    request.forced_speaker_participant_id = Some(cast.bea.to_string());
    let error = conversation_continue(
        &harness.context,
        request,
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect_err("bea is no longer a member");
    assert_eq!(error.code, ApiErrorCode::InvalidInput);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_group_stop_keeps_the_partial_reply() {
    let harness = harness(Reply::PartialUntilCancelled("Half a"));
    let cast = group_cast(&harness, "group-stop").await;
    let accepted = send(
        &harness,
        &cast.chat,
        "group-stop-send",
        "Hi all",
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect("send");
    let worker = ConversationGenerationWorker::new(harness.context.clone());
    let (ran, cancelled) = tokio::join!(worker.run_once(), async {
        harness.provider.entered.notified().await;
        generation_cancel(
            &harness.context,
            dto::GenerationCancelRequest {
                turn_id: accepted.turn_id.clone(),
            },
        )
        .await
    });
    assert!(ran.expect("worker ran"));
    cancelled.expect("cancel");
    let view = open(&harness, &cast.chat).await;
    let reply = view.messages.items.last().expect("reply");
    assert_eq!(reply.role, dto::MessageRole::Assistant);
    assert_eq!(
        reply.parts,
        vec![dto::MessagePartView::Text {
            text: "Half a".into()
        }]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_reply_a_child_branch_shows_regenerates_on_its_owner_and_is_refused_from_the_child() {
    let harness = harness(Reply::Text("Hello."));
    let (chat, reply) = replied_chat(&harness, "branches").await;
    let database = harness.context.backend().database();
    let stored = conversation(&harness, &chat);
    let root = stored.active_branch_id;
    let fork = ConversationRepository::fork_branch(
        database,
        &lettuce_conversations::ForkBranch {
            conversation_id: stored.id,
            source_branch_id: root,
            at_message_id: Some(reply.parse().expect("message id")),
            expected_revision: stored.revision,
            operation: crate::conversation::edit_operation("branches-fork".into(), &[b"fork"])
                .expect("token"),
        },
        harness.context.now(),
    )
    .expect("fork at the reply");
    let child = fork.value.branch.id;
    assert_ne!(child, root);
    assert_eq!(conversation(&harness, &chat).active_branch_id, child);

    let refused = conversation_regenerate(
        &harness.context,
        regenerate_request(&harness, &chat, &reply, "branches-child"),
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect_err("the reply belongs to the parent branch");
    assert_eq!(refused.code, ApiErrorCode::Unsupported);

    let stored = conversation(&harness, &chat);
    ConversationRepository::select_branch(
        database,
        &lettuce_conversations::SelectBranch {
            conversation_id: stored.id,
            branch_id: root,
            expected_revision: stored.revision,
            operation: crate::conversation::edit_operation("branches-select".into(), &[b"select"])
                .expect("token"),
        },
        harness.context.now(),
    )
    .expect("select the root branch");
    conversation_regenerate(
        &harness.context,
        regenerate_request(&harness, &chat, &reply, "branches-root"),
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect("the owning branch regenerates its reply");
    run_generation(&harness).await;
    let view = open(&harness, &chat).await;
    let shown = view
        .messages
        .items
        .iter()
        .find(|message| message.id == reply)
        .expect("reply");
    assert_eq!(shown.candidate_count, 2);
    let aggregate = ConversationReader::get(database, stored.id).expect("conversation");
    assert_eq!(aggregate.branches.len(), 2);
    let stored = conversation(&harness, &chat);
    ConversationRepository::select_branch(
        database,
        &lettuce_conversations::SelectBranch {
            conversation_id: stored.id,
            branch_id: child,
            expected_revision: stored.revision,
            operation: crate::conversation::edit_operation("branches-back".into(), &[b"back"])
                .expect("token"),
        },
        harness.context.now(),
    )
    .expect("select the child branch");
    let view = open(&harness, &chat).await;
    let shown = view
        .messages
        .items
        .iter()
        .find(|message| message.id == reply)
        .expect("reply");
    assert_eq!(shown.candidate_count, 2);
    assert_eq!(view.messages.items.len(), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn retrying_a_turn_of_an_archived_chat_restores_it() {
    let harness = harness(Reply::Text("Hello."));
    let chat = launch(&harness, "retry-archived-launch").await;
    let accepted = send(
        &harness,
        &chat,
        "retry-archived-send",
        "Stop me",
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect("send");
    generation_cancel(
        &harness.context,
        dto::GenerationCancelRequest {
            turn_id: accepted.turn_id.clone(),
        },
    )
    .await
    .expect("cancel");
    conversation_archive(
        &harness.context,
        dto::ConversationRequest {
            conversation_id: chat.clone(),
        },
    )
    .await
    .expect("archive");
    conversation_retry(
        &harness.context,
        dto::ConversationRetryRequest {
            conversation_id: chat.clone(),
            turn_id: accepted.turn_id,
            client_operation_id: "retry-archived-1".into(),
        },
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect("retry on an archived chat");
    assert_eq!(
        conversation(&harness, &chat).lifecycle,
        ConversationLifecycle::Active
    );
}
