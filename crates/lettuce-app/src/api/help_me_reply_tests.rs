use std::sync::{Arc, Mutex};

use lettuce_contracts::{self as dto, ApiErrorCode};
use lettuce_conversations::{InferenceRequest, ProviderContextPart};
use lettuce_settings::GlobalSettingsStore;

use super::jobs::JobFeed;
use super::tests::{Harness, Reply, harness};
use super::turns_tests::{group_cast, replied_chat, run_generation};
use super::*;

#[derive(Default)]
struct RecordingJob(Mutex<Vec<dto::JobEvent>>, tokio::sync::Notify);

impl JobEventSink for RecordingJob {
    fn emit(&self, event: dto::JobEvent) -> bool {
        self.0.lock().expect("job events").push(event);
        self.1.notify_one();
        true
    }
}

impl RecordingJob {
    fn events(&self) -> Vec<dto::JobEvent> {
        self.0.lock().expect("job events").clone()
    }
}

fn request(chat: &str, key: &str) -> dto::ConversationHelpMeReplyRequest {
    dto::ConversationHelpMeReplyRequest {
        conversation_id: chat.into(),
        mode: dto::HelpMeReplyMode::New,
        current_draft: None,
        swap_places: false,
        client_operation_id: key.into(),
    }
}

fn set_help_me_reply(
    harness: &Harness,
    change: impl FnOnce(&mut lettuce_settings::GlobalSettings),
) {
    let database = harness.context.backend().database();
    let stored = database.load().expect("settings");
    let mut settings = stored.settings;
    change(&mut settings);
    database
        .save(settings, stored.default_model_profile_id, stored.revision)
        .expect("save settings");
}

async fn run_jobs(harness: &Harness) {
    let runner = JobRunner::new(harness.context.clone(), JobHandlers::standard());
    while runner.run_once().await.expect("runner") {}
    runner.wait_idle().await;
}

async fn watch(harness: &Harness, accepted: &dto::JobAccepted) -> Arc<RecordingJob> {
    let sink = Arc::new(RecordingJob::default());
    job_watch(
        &harness.context,
        dto::JobWatchRequest {
            job_id: accepted.job_id.clone(),
        },
        sink.clone(),
    )
    .await
    .expect("watch");
    sink
}

fn job_texts(request: &InferenceRequest) -> Vec<String> {
    request
        .context
        .messages
        .iter()
        .map(|message| {
            message
                .parts
                .iter()
                .filter_map(|part| match part {
                    ProviderContextPart::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .collect()
}

fn last_request(harness: &Harness) -> InferenceRequest {
    harness
        .provider
        .requests
        .lock()
        .expect("requests")
        .last()
        .expect("a provider request")
        .clone()
}

#[tokio::test(flavor = "multi_thread")]
async fn help_me_reply_streams_deltas_and_ends_with_the_cleaned_text() {
    let harness = harness(Reply::Text("\"user: Hello.\""));
    let (chat, _reply) = replied_chat(&harness, "help").await;
    let mut feed = JobFeed::start(&harness.context).await.expect("feed");
    let accepted = conversation_help_me_reply(&harness.context, request(&chat, "help-1"))
        .await
        .expect("accepted");
    let sink = watch(&harness, &accepted).await;
    run_jobs(&harness).await;
    feed.publish(&harness.context).await.expect("publish");
    let events = sink.events();
    let deltas = events
        .iter()
        .filter_map(|event| match event {
            dto::JobEvent::TextDelta { text, .. } => text.clone(),
            _ => None,
        })
        .collect::<String>();
    assert_eq!(deltas, "Hello.");
    let Some(dto::JobEvent::Completed { job }) = events.last() else {
        panic!("the job did not complete: {events:?}");
    };
    assert_eq!(job.kind, dto::JobKindDto::CreationRun);
    assert_eq!(
        job.result,
        Some(dto::JobResultDto::GeneratedText {
            text: "Hello.".into()
        })
    );
    assert!(
        harness
            .provider
            .requests
            .lock()
            .expect("requests")
            .last()
            .expect("request")
            .stream_sink
            .is_some()
    );

    let replay = conversation_help_me_reply(&harness.context, request(&chat, "help-1"))
        .await
        .expect("replayed");
    assert_eq!(replay, accepted);
    let mut changed_mode = request(&chat, "help-1");
    changed_mode.mode = dto::HelpMeReplyMode::Enrich;
    changed_mode.current_draft = None;
    let conflict = conversation_help_me_reply(&harness.context, changed_mode)
        .await
        .expect_err("changing New to Enrich is another request even without a draft");
    assert_eq!(conflict.code, ApiErrorCode::Conflict);
    let mut other = request(&chat, "help-1");
    other.swap_places = true;
    let conflict = conversation_help_me_reply(&harness.context, other)
        .await
        .expect_err("another request under the key");
    assert_eq!(conflict.code, ApiErrorCode::Conflict);
    let requests = harness.provider.requests.lock().expect("requests").len();
    run_jobs(&harness).await;
    assert_eq!(
        harness.provider.requests.lock().expect("requests").len(),
        requests
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_watch_that_attaches_late_gets_the_text_streamed_so_far() {
    let harness = harness(Reply::UntilCancelled);
    let chat = launch_chat(&harness).await;
    let accepted = conversation_help_me_reply(&harness.context, request(&chat, "late-1"))
        .await
        .expect("accepted");
    let initial = watch(&harness, &accepted).await;
    let runner = JobRunner::new(harness.context.clone(), JobHandlers::standard());
    assert!(runner.run_once().await.expect("started"));
    harness.provider.entered.notified().await;
    loop {
        let delivered = initial.1.notified();
        let text = initial
            .events()
            .into_iter()
            .filter_map(|event| match event {
                dto::JobEvent::TextDelta { text, .. } => text,
                _ => None,
            })
            .collect::<String>();
        if text == "Hello." {
            break;
        }
        delivered.await;
    }
    let sink = watch(&harness, &accepted).await;
    let events = sink.events();
    assert!(
        events.iter().any(|event| matches!(
            event,
            dto::JobEvent::TextDelta { text: Some(text), .. } if text == "Hello."
        )),
        "{events:?}"
    );
    job_cancel(
        &harness.context,
        dto::JobCancelRequest {
            job_id: accepted.job_id,
        },
    )
    .await
    .expect("cancel");
    runner.wait_idle().await;
}

async fn launch_chat(harness: &Harness) -> String {
    let chat = super::tests::launch(harness, "late-launch").await;
    for index in 0..2 {
        let stored = super::turns_tests::conversation(harness, &chat);
        let user = stored
            .participants
            .iter()
            .find(|participant| participant.role == lettuce_conversations::ParticipantRole::User)
            .expect("user")
            .id;
        lettuce_conversations::ConversationRepository::append_user_message(
            harness.context.backend().database(),
            &lettuce_conversations::SendConversation {
                conversation_id: stored.id,
                branch_id: stored.active_branch_id,
                expected_revision: stored.revision,
                operation: crate::conversation::edit_operation(
                    format!("late-message-{index}"),
                    &[b"m"],
                )
                .expect("token"),
                message: lettuce_conversations::MessageDraft {
                    role: lettuce_conversations::MessageRole::User,
                    author_participant_id: Some(user),
                    parts: vec![lettuce_conversations::MessagePart::Text {
                        text: format!("Message {index}"),
                    }],
                    visibility: lettuce_conversations::MessageVisibility::Visible,
                    pinned: false,
                    scene_edited: false,
                },
                swap_roles: false,
            },
            harness.context.now(),
        )
        .expect("append");
    }
    chat
}

#[tokio::test(flavor = "multi_thread")]
async fn help_me_reply_reads_only_the_last_history_count_messages_of_every_role() {
    let harness = harness(Reply::Text("Hello."));
    let (chat, _reply) = replied_chat(&harness, "history").await;
    for index in 0..3 {
        conversation_add_user_message(
            &harness.context,
            dto::ConversationAddUserMessageRequest {
                conversation_id: chat.clone(),
                text: format!("Older note {index}"),
                expected_revision: super::turns_tests::conversation(&harness, &chat)
                    .revision
                    .get(),
                client_operation_id: format!("history-add-{index}"),
            },
        )
        .await
        .expect("add");
    }
    set_help_me_reply(&harness, |settings| {
        settings.help_me_reply.history_count = 2
    });
    let accepted = conversation_help_me_reply(&harness.context, request(&chat, "history-1"))
        .await
        .expect("accepted");
    run_jobs(&harness).await;
    assert!(!accepted.job_id.is_empty());
    let text = job_texts(&last_request(&harness)).join("\n");
    assert!(text.contains("Older note 2"), "{text}");
    assert!(text.contains("Older note 1"), "{text}");
    assert!(!text.contains("Older note 0"), "{text}");
    assert!(!text.contains("Hi there"), "{text}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_group_help_me_reply_names_only_the_enabled_members() {
    let harness = harness(Reply::Text("Hello."));
    let cast = group_cast(&harness, "cast").await;
    super::tests::send(
        &harness,
        &cast.chat,
        "cast-send",
        "Hi all",
        Arc::new(super::tests::RecordingStream::default()),
    )
    .await
    .expect("send");
    run_generation(&harness).await;
    conversation_participant_update(
        &harness.context,
        dto::ConversationParticipantUpdateRequest {
            conversation_id: cast.chat.clone(),
            participant_id: cast.bea.to_string(),
            enabled: Some(false),
            muted: None,
            model: None,
            client_operation_id: "cast-disable".into(),
        },
    )
    .await
    .expect("disable Bea");
    conversation_help_me_reply(&harness.context, request(&cast.chat, "cast-help"))
        .await
        .expect("accepted");
    run_jobs(&harness).await;
    let text = job_texts(&last_request(&harness)).join("\n");
    assert!(text.contains("Ada"), "{text}");
    assert!(text.contains("Cleo"), "{text}");
    assert!(!text.contains("Bea"), "{text}");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_empty_reply_fails_with_the_no_reply_reason() {
    let harness = harness(Reply::Text("   "));
    let chat = launch_chat(&harness).await;
    let mut feed = JobFeed::start(&harness.context).await.expect("feed");
    let accepted = conversation_help_me_reply(&harness.context, request(&chat, "empty-1"))
        .await
        .expect("accepted");
    let sink = watch(&harness, &accepted).await;
    run_jobs(&harness).await;
    feed.publish(&harness.context).await.expect("publish");
    let Some(dto::JobEvent::Failed { job }) = sink.events().last().cloned() else {
        panic!("the job did not fail: {:?}", sink.events());
    };
    let failure = job.failure.expect("failure");
    assert_eq!(
        failure.reason,
        Some(dto::JobFailureReason::HelpMeReplyNoReply)
    );
    assert_eq!(job.result, None);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_chat_without_messages_fails_with_the_no_history_reason() {
    let harness = harness(Reply::Text("Hello."));
    let chat = super::tests::launch(&harness, "no-history-launch").await;
    let mut feed = JobFeed::start(&harness.context).await.expect("feed");
    let accepted = conversation_help_me_reply(&harness.context, request(&chat, "no-history"))
        .await
        .expect("accepted");
    let sink = watch(&harness, &accepted).await;
    run_jobs(&harness).await;
    feed.publish(&harness.context).await.expect("publish");
    let Some(dto::JobEvent::Failed { job }) = sink.events().last().cloned() else {
        panic!("the job did not fail: {:?}", sink.events());
    };
    assert_eq!(
        job.failure.expect("failure").reason,
        Some(dto::JobFailureReason::HelpMeReplyNoHistory)
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

#[tokio::test(flavor = "multi_thread")]
async fn a_disabled_help_me_reply_is_refused_before_a_job_exists() {
    let harness = harness(Reply::Text("Hello."));
    let (chat, _reply) = replied_chat(&harness, "disabled").await;
    set_help_me_reply(&harness, |settings| settings.help_me_reply.enabled = false);
    let error = conversation_help_me_reply(&harness.context, request(&chat, "disabled-1"))
        .await
        .expect_err("disabled");
    assert_eq!(error.code, ApiErrorCode::Unsupported);
    let jobs = jobs_list(
        &harness.context,
        dto::JobsListRequest {
            kinds: Some(vec![dto::JobKindDto::CreationRun]),
            ..dto::JobsListRequest::default()
        },
    )
    .await
    .expect("jobs");
    assert!(jobs.items.is_empty());
    let unknown = conversation_help_me_reply(
        &harness.context,
        request(&uuid::Uuid::new_v4().to_string(), "disabled-2"),
    )
    .await
    .expect_err("an unknown conversation");
    assert!(matches!(
        unknown.code,
        ApiErrorCode::NotFound | ApiErrorCode::Unsupported
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn cancelling_help_me_reply_ends_the_job_cancelled_with_no_text() {
    let harness = harness(Reply::UntilCancelled);
    let chat = launch_chat(&harness).await;
    let mut feed = JobFeed::start(&harness.context).await.expect("feed");
    let accepted = conversation_help_me_reply(&harness.context, request(&chat, "cancel-1"))
        .await
        .expect("accepted");
    let sink = watch(&harness, &accepted).await;
    let runner = JobRunner::new(harness.context.clone(), JobHandlers::standard());
    assert!(runner.run_once().await.expect("started"));
    harness.provider.entered.notified().await;
    job_cancel(
        &harness.context,
        dto::JobCancelRequest {
            job_id: accepted.job_id.clone(),
        },
    )
    .await
    .expect("cancel");
    runner.wait_idle().await;
    feed.publish(&harness.context).await.expect("publish");
    let Some(dto::JobEvent::Cancelled { job }) = sink.events().last().cloned() else {
        panic!("the job was not cancelled: {:?}", sink.events());
    };
    assert_eq!(job.state, dto::JobStateDto::Cancelled);
    assert_eq!(job.result, None);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_feature_disabled_after_admission_fails_the_owned_job() {
    let harness = harness(Reply::Text("Hello."));
    let (chat, _) = replied_chat(&harness, "disabled-after-admission").await;
    let accepted = conversation_help_me_reply(
        &harness.context,
        request(&chat, "disabled-after-admission-key"),
    )
    .await
    .expect("admitted");
    set_help_me_reply(&harness, |settings| settings.help_me_reply.enabled = false);
    run_jobs(&harness).await;
    let job = job_get(
        &harness.context,
        dto::JobGetRequest {
            job_id: accepted.job_id,
        },
    )
    .await
    .expect("job");
    assert!(matches!(job.state, dto::JobStateDto::Failed));
    assert_eq!(
        job.failure.expect("typed failure").reason,
        Some(dto::JobFailureReason::HelpMeReplyDisabled)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn two_text_handlers_cannot_own_the_same_stream_or_provider_call() {
    use lettuce_jobs::{JobStore, WorkerId};
    let harness = harness(Reply::Text("One reply"));
    let (chat, _) = replied_chat(&harness, "two-text-owners").await;
    let accepted =
        conversation_help_me_reply(&harness.context, request(&chat, "two-text-owners-key"))
            .await
            .expect("admission");
    let job_id = accepted.job_id.parse().expect("job id");
    let job = JobStore::get(harness.context.backend().database(), job_id)
        .expect("job")
        .expect("exists");
    let first = TextFeatureHandler
        .claim(&harness.context, &job, WorkerId::new())
        .await
        .expect("first claim")
        .expect("owned");
    let second = TextFeatureHandler
        .claim(&harness.context, &job, WorkerId::new())
        .await
        .expect("second claim");
    assert!(second.is_none());
    let before = harness.provider.requests.lock().expect("requests").len();
    struct Progress;
    impl JobProgressSink for Progress {
        fn text_delta(&self, _: Option<String>, _: Option<String>) {}
    }
    first
        .run(harness.context.clone(), Arc::new(Progress))
        .await
        .expect("owned run");
    assert_eq!(
        harness.provider.requests.lock().expect("requests").len(),
        before + 1
    );
    let ended = JobStore::get(harness.context.backend().database(), job_id)
        .expect("job")
        .expect("exists");
    assert_eq!(ended.state, lettuce_jobs::JobState::Succeeded);
}
