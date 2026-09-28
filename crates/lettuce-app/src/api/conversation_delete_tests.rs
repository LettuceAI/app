use std::sync::Arc;

use lettuce_contracts::{self as dto, ApiEvent, GenerationEvent};
use lettuce_conversations::{
    ConversationOverviewReader, ConversationReader, ConversationRepositoryError, InferencePort,
};
use lettuce_types::ConversationId;

use super::tests::{
    Harness, RecordingEvents, RecordingStream, Reply, StdFiles, harness, launch, send,
};
use super::*;
use crate::AppBackend;

/// An API context over `backend` that answers every provider call with
/// `inference`.
pub(crate) fn context_over(
    backend: Arc<AppBackend>,
    inference: Arc<dyn InferencePort>,
) -> ApiContext {
    ApiContext::new(ApiContextParts {
        backend,
        secret_store: Arc::new(lettuce_settings::InMemorySecretStore::new()),
        inference,
        image_provider: Arc::new(super::tests::NoImages),
        models: Arc::new(NoModels),
        media: None,
        events: Arc::new(RecordingEvents::default()),
        clock: Arc::new(lettuce_jobs::SystemClock),
        files: Arc::new(StdFiles),
        app_folder: None,
        resource_dir: None,
        database_files: None,
        asset_url_base: "test-asset://host".into(),
    })
}

fn gone(context: &ApiContext, conversation_id: &str) -> bool {
    matches!(
        ConversationReader::get(
            context.backend().database(),
            conversation_id.parse::<ConversationId>().expect("id"),
        ),
        Err(ConversationRepositoryError::NotFound)
    )
}

async fn delete(harness: &Harness, conversation_id: &str) -> Result<(), dto::ApiError> {
    conversation_delete(
        &harness.context,
        dto::ConversationRequest {
            conversation_id: conversation_id.into(),
        },
    )
    .await
}

/// A delete during a streaming reply cancels the reply, waits for it to
/// settle and then deletes the chat; the caller never sees `Busy`. A
/// repeated delete succeeds.
#[tokio::test(flavor = "multi_thread")]
async fn deleting_a_chat_while_it_streams_cancels_the_reply_first() {
    let harness = harness(Reply::UntilCancelled);
    let chat = launch(&harness, "delete-streaming").await;
    let stream = Arc::new(RecordingStream::default());
    let accepted = send(
        &harness,
        &chat,
        "delete-streaming-send",
        "Wait",
        stream.clone(),
    )
    .await
    .expect("send");
    let worker = ConversationGenerationWorker::new(harness.context.clone());
    let (ran, deleted) = tokio::join!(worker.run_once(), async {
        harness.provider.entered.notified().await;
        delete(&harness, &chat).await
    });
    assert!(ran.expect("worker ran"));
    deleted.expect("the delete waited for the cancelled reply");
    assert!(gone(&harness.context, &chat));
    assert!(stream.events().iter().any(|event| matches!(
        event,
        GenerationEvent::Cancelled { turn_id } if *turn_id == accepted.turn_id
    )));
    harness
        .events
        .until(|events| {
            events.iter().any(|event| {
                matches!(event, ApiEvent::GenerationSettled { turn_id, .. } if *turn_id == accepted.turn_id)
            })
        })
        .await;
    delete(&harness, &chat)
        .await
        .expect("deleting a deleted chat succeeds");
}

/// A delete of a chat whose reply is still queued settles the reply at once.
#[tokio::test(flavor = "multi_thread")]
async fn deleting_a_chat_with_a_queued_reply_settles_it_and_deletes() {
    let harness = harness(Reply::Text("Never sent."));
    let chat = launch(&harness, "delete-queued").await;
    let stream = Arc::new(RecordingStream::default());
    send(
        &harness,
        &chat,
        "delete-queued-send",
        "Hello",
        stream.clone(),
    )
    .await
    .expect("send");
    delete(&harness, &chat).await.expect("delete");
    assert!(gone(&harness.context, &chat));
    assert!(
        stream
            .events()
            .iter()
            .any(|event| matches!(event, GenerationEvent::Cancelled { .. }))
    );
    assert!(
        harness
            .provider
            .requests
            .lock()
            .expect("requests")
            .is_empty()
    );
}

/// The app stopping between a delete's cancellation and its purge leaves the
/// cancelled turn to restart recovery; the next delete then completes.
#[tokio::test(flavor = "multi_thread")]
async fn a_delete_interrupted_between_cancel_and_purge_completes_next_time() {
    let harness = harness(Reply::UntilCancelled);
    let chat = launch(&harness, "delete-crash").await;
    let conversation_id: ConversationId = chat.parse().expect("id");
    send(
        &harness,
        &chat,
        "delete-crash-send",
        "Wait",
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect("send");
    let worker = ConversationGenerationWorker::new(harness.context.clone());
    let running = tokio::spawn(async move { worker.run_once().await });
    harness.provider.entered.notified().await;
    running.abort();
    let _ = running.await;
    harness
        .context
        .blocking(move |context| {
            super::conversation_delete::cancel_conversation_work(context, conversation_id)
        })
        .await
        .expect("the cancellation is recorded");
    let turn = ConversationOverviewReader::live_turn(
        harness.context.backend().database(),
        conversation_id,
    )
    .expect("live turn")
    .expect("the reply is still unsettled");
    let job_id = ConversationReader::get_turn(harness.context.backend().database(), turn)
        .expect("turn")
        .attempts
        .iter()
        .find_map(|attempt| attempt.job_id)
        .expect("job");
    assert_eq!(
        lettuce_jobs::JobStore::get(harness.context.backend().database(), job_id)
            .expect("job")
            .expect("job exists")
            .state,
        lettuce_jobs::JobState::CancellationRequested,
        "the cancellation was recorded before the app stopped"
    );
    harness
        .context
        .recover_after_restart()
        .expect("restart recovery");
    delete(&harness, &chat).await.expect("delete after restart");
    assert!(gone(&harness.context, &chat));
}

/// A reply whose job already ended without settling its turn does not keep
/// the chat busy: the delete settles the turn and completes.
#[tokio::test(flavor = "multi_thread")]
async fn a_live_reply_whose_job_already_ended_does_not_block_a_delete() {
    let harness = harness(Reply::Text("Never sent."));
    let chat = launch(&harness, "delete-ended-job").await;
    let accepted = send(
        &harness,
        &chat,
        "delete-ended-job-send",
        "Hello",
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect("send");
    let database = harness.context.backend().database();
    let job_id = ConversationReader::get_turn(database, accepted.turn_id.parse().expect("turn"))
        .expect("turn")
        .attempts
        .iter()
        .find_map(|attempt| attempt.job_id)
        .expect("job");
    let at = harness.context.now();
    let requested = lettuce_jobs::JobStore::append_and_transition(
        database,
        lettuce_jobs::JobMutation::RequestCancellation {
            id: job_id,
            reason: lettuce_jobs::CancellationReason::User,
            at,
        },
    )
    .expect("request cancellation");
    lettuce_jobs::JobStore::append_and_transition(
        database,
        lettuce_jobs::JobMutation::FinishQueuedCancellation {
            id: job_id,
            at: requested.updated_at,
        },
    )
    .expect("the job ends on its own");
    assert!(
        ConversationOverviewReader::live_turn(database, chat.parse().expect("id"))
            .expect("live turn")
            .is_some(),
        "the turn is still unsettled"
    );
    delete(&harness, &chat).await.expect("delete");
    assert!(gone(&harness.context, &chat));
}

/// A job attached to a turn between the delete reading it and settling it
/// moves the turn on; the delete reads it again and cancels the queued job
/// instead of failing.
#[tokio::test(flavor = "multi_thread")]
async fn a_job_attached_while_the_delete_settles_its_turn_is_cancelled_next() {
    let harness = harness(Reply::Text("Never sent."));
    let chat = launch(&harness, "delete-race").await;
    let conversation_id: ConversationId = chat.parse().expect("id");
    let database = harness.context.backend().database();
    let conversation = ConversationReader::get(database, conversation_id)
        .expect("conversation")
        .conversation;
    let user = conversation
        .participants
        .iter()
        .find(|participant| participant.role == lettuce_conversations::ParticipantRole::User)
        .expect("user")
        .id;
    let begun = lettuce_conversations::ConversationRepository::begin_send(
        database,
        &lettuce_conversations::SendConversation {
            conversation_id,
            branch_id: conversation.active_branch_id,
            expected_revision: conversation.revision,
            operation: crate::conversation::edit_operation("delete-race-send".into(), &[b"race"])
                .expect("token"),
            message: lettuce_conversations::MessageDraft {
                role: lettuce_conversations::MessageRole::User,
                author_participant_id: Some(user),
                parts: vec![lettuce_conversations::MessagePart::Text {
                    text: "Hello".into(),
                }],
                visibility: lettuce_conversations::MessageVisibility::Visible,
                pinned: false,
                scene_edited: false,
            },
            swap_roles: false,
        },
        harness.context.now(),
    )
    .expect("a turn without a job yet")
    .value;
    let scheduled = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let hook: super::conversation_delete::BeforeSettle = {
        let scheduled = Arc::clone(&scheduled);
        Arc::new(move |context: &ApiContext| {
            if !scheduled.swap(true, std::sync::atomic::Ordering::SeqCst) {
                context
                    .backend()
                    .conversation_generation_dispatcher()
                    .schedule(&begun, context.now())
                    .expect("the send's job is attached meanwhile");
            }
        })
    };
    super::conversation_delete::delete_with(
        &harness.context,
        dto::ConversationRequest {
            conversation_id: chat.clone(),
        },
        hook,
    )
    .await
    .expect("the delete reads the turn again");
    assert!(scheduled.load(std::sync::atomic::Ordering::SeqCst));
    assert!(gone(&harness.context, &chat));
    assert!(
        harness
            .provider
            .requests
            .lock()
            .expect("requests")
            .is_empty()
    );
}
