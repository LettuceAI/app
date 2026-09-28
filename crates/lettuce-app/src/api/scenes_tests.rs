use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use lettuce_contracts::{self as dto, ApiErrorCode, ApiEvent};
use lettuce_conversations::{SceneFollowUpRepository, SceneFollowUpState};
use lettuce_database::Database;
use lettuce_image_generation::{
    ImageProviderError, ImageProviderPort, ProviderImage, ProviderImageOutput, ProviderImageRequest,
};
use lettuce_jobs::SystemClock;
use lettuce_media::LocalMediaBlobStore;
use lettuce_models::{CapabilityStatus, ModelProfileRepository, ProviderProtocol};
use lettuce_platform::{DirectorySnapshot, FilesystemAuthority, ManagedRoot};
use lettuce_settings::{GlobalSettingsStore, SceneGenerationMode};
use lettuce_types::{ConversationId, MessageId, TimestampMillis};

use super::jobs::JobFeed;
use super::tests::{Harness, RecordingStream, Reply, api_events, harness_over, launch, send};
use super::*;
use crate::AppBackend;

enum Outcome {
    Image,
    Empty,
    Fail(&'static str),
    UntilCancelled,
}

struct ScriptedImages {
    outcomes: Mutex<VecDeque<Outcome>>,
    prompts: Mutex<Vec<String>>,
    entered: tokio::sync::Notify,
}

impl ScriptedImages {
    fn new(outcomes: Vec<Outcome>) -> Arc<Self> {
        Arc::new(Self {
            outcomes: Mutex::new(outcomes.into()),
            prompts: Mutex::new(Vec::new()),
            entered: tokio::sync::Notify::new(),
        })
    }

    fn calls(&self) -> usize {
        self.prompts.lock().expect("prompts").len()
    }
}

fn png() -> ProviderImage {
    let mut bytes = b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR".to_vec();
    bytes.extend_from_slice(&2_u32.to_be_bytes());
    bytes.extend_from_slice(&3_u32.to_be_bytes());
    bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
    bytes.extend_from_slice(b"generated image bytes");
    ProviderImage {
        bytes,
        declared_mime_type: Some("image/png".into()),
        text: None,
    }
}

#[async_trait::async_trait]
impl ImageProviderPort for ScriptedImages {
    async fn generate(
        &self,
        request: ProviderImageRequest,
    ) -> Result<ProviderImageOutput, ImageProviderError> {
        self.prompts
            .lock()
            .expect("prompts")
            .push(request.prompt.clone());
        let outcome = self
            .outcomes
            .lock()
            .expect("outcomes")
            .pop_front()
            .expect("a scripted outcome");
        match outcome {
            Outcome::Image => Ok(ProviderImageOutput {
                images: vec![png()],
                usage: None,
            }),
            Outcome::Empty => Ok(ProviderImageOutput {
                images: Vec::new(),
                usage: None,
            }),
            Outcome::Fail(message) => Err(ImageProviderError::Failed(message.into())),
            Outcome::UntilCancelled => {
                self.entered.notify_one();
                request.cancellation.cancelled().await;
                Err(ImageProviderError::Cancelled)
            }
        }
    }
}

struct SceneHarness {
    harness: Harness,
    images: Arc<ScriptedImages>,
    _root: std::path::PathBuf,
}

fn scene_harness(reply: Reply, outcomes: Vec<Outcome>, mode: SceneGenerationMode) -> SceneHarness {
    let root = std::env::temp_dir().join(format!(
        "lettuce-scenes-{}",
        lettuce_types::OperationId::new()
    ));
    std::fs::create_dir_all(&root).expect("root");
    let path = root.join("app.db");
    let backend = Arc::new(AppBackend::open(&path, TimestampMillis::new(1)).expect("backend"));
    let authority = FilesystemAuthority::new(&DirectorySnapshot::new(&root).expect("snapshot"))
        .expect("authority");
    let media = Arc::new(LocalMediaBlobStore::new(
        authority.managed_files(),
        authority
            .read_capability(ManagedRoot::MediaBlobs)
            .expect("read capability"),
        authority
            .write_capability(ManagedRoot::MediaBlobs)
            .expect("write capability"),
        Database::open(&path).expect("blob catalog"),
        Database::open(&path).expect("asset catalog"),
    ));
    let images = ScriptedImages::new(outcomes);
    let harness = harness_over(
        backend,
        reply,
        Arc::new(SystemClock),
        Some(media),
        None,
        Arc::new(NoModels),
        images.clone(),
    );
    let database = harness.context.backend().database();
    let image_model =
        crate::launch::tests::seed_model(database, ProviderProtocol::OpenAiCompatible, "scenes");
    let mut model = ModelProfileRepository::get(database, image_model)
        .expect("model")
        .expect("exists");
    let revision = model.revision;
    model.config.capabilities.output_modalities.image = CapabilityStatus::Supported;
    ModelProfileRepository::upsert(database, model, Some(revision)).expect("image output");
    let stored = GlobalSettingsStore::load(database).expect("settings");
    let writer_id = stored.default_model_profile_id.expect("the chat model");
    let mut writer = ModelProfileRepository::get(database, writer_id)
        .expect("writer")
        .expect("exists");
    let revision = writer.revision;
    writer.config.capabilities.input_modalities.image = CapabilityStatus::Supported;
    ModelProfileRepository::upsert(database, writer, Some(revision)).expect("vision writer");
    let stored = GlobalSettingsStore::load(database).expect("settings");
    let mut settings = stored.settings;
    settings.image_generation.scene_writer_model_profile_id = Some(writer_id);
    settings.image_generation.scene_enabled = true;
    settings.image_generation.scene_model_profile_id = Some(image_model);
    settings.image_generation.scene_mode = mode;
    GlobalSettingsStore::save(
        database,
        settings,
        stored.default_model_profile_id,
        stored.revision,
    )
    .expect("scene settings");
    SceneHarness {
        harness,
        images,
        _root: root,
    }
}

const TAGGED: &str = "Hello. <img>a harbor</img>";

async fn reply_with_scene(harness: &Harness, key: &str) -> (String, String) {
    let chat = launch(harness, &format!("{key}-launch")).await;
    send(
        harness,
        &chat,
        &format!("{key}-send"),
        "Show me",
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect("send");
    let worker = ConversationGenerationWorker::new(harness.context.clone());
    assert!(worker.run_once().await.expect("worker ran"));
    let view = open_chat(&harness.context, &chat).await;
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

async fn open_chat(context: &ApiContext, chat: &str) -> dto::ConversationView {
    conversation_open(
        context,
        dto::ConversationOpenRequest {
            conversation_id: chat.into(),
        },
    )
    .await
    .expect("open")
}

async fn reply_of(context: &ApiContext, chat: &str, reply: &str) -> dto::TimelineMessage {
    open_chat(context, chat)
        .await
        .messages
        .items
        .into_iter()
        .find(|message| message.id == reply)
        .expect("reply")
}

fn media_count(message: &dto::TimelineMessage) -> usize {
    message
        .parts
        .iter()
        .filter(|part| matches!(part, dto::MessagePartView::Media { .. }))
        .count()
}

async fn run_jobs(context: &ApiContext) {
    let runner = JobRunner::new(context.clone(), JobHandlers::standard());
    while runner.run_once().await.expect("runner") {
        runner.wait_idle().await;
    }
    runner.wait_idle().await;
}

fn follow_up_state(context: &ApiContext, chat: &str, reply: &str) -> Option<SceneFollowUpState> {
    context
        .backend()
        .database()
        .get_follow_up(
            chat.parse::<ConversationId>().expect("chat"),
            reply.parse::<MessageId>().expect("message"),
        )
        .expect("follow-up")
        .map(|follow_up| follow_up.state)
}

fn approve(reply: &str, prompt: Option<&str>) -> dto::MessageSceneImageApproveRequest {
    dto::MessageSceneImageApproveRequest {
        message_id: reply.into(),
        prompt: prompt.map(str::to_owned),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn an_automatic_scene_image_starts_with_the_reply_and_lands_on_the_message() {
    let scene = scene_harness(
        Reply::Text(TAGGED),
        vec![Outcome::Image],
        SceneGenerationMode::Auto,
    );
    let context = &scene.harness.context;
    let mut feed = super::conversation_feed::ConversationFeed::start(context)
        .await
        .expect("feed");
    let (chat, reply) = reply_with_scene(&scene.harness, "auto").await;
    let shown = reply_of(context, &chat, &reply).await;
    let image = shown.scene_image.clone().expect("a pending image");
    assert_eq!(image.state, dto::SceneImageState::Approved);
    assert_eq!(image.mode, dto::SceneImageMode::Auto);
    assert_eq!(image.prompt, "a harbor");
    assert!(image.job_id.is_some());
    assert_eq!(media_count(&shown), 0);
    assert!(
        !shown.parts.iter().any(|part| matches!(
            part,
            dto::MessagePartView::Text { text } if text.contains("<img>")
        )),
        "the tag is stripped from the text"
    );

    run_jobs(context).await;
    let shown = reply_of(context, &chat, &reply).await;
    assert_eq!(shown.scene_image, None);
    assert_eq!(media_count(&shown), 1);
    assert_eq!(
        follow_up_state(context, &chat, &reply),
        Some(SceneFollowUpState::Done)
    );
    let prompts = scene.images.prompts.lock().expect("prompts").clone();
    assert_eq!(prompts.len(), 1);
    assert!(prompts[0].ends_with("a harbor"), "{}", prompts[0]);
    feed.publish(context).await.expect("publish");
    assert!(api_events(&scene.harness).iter().any(|event| matches!(
        event,
        ApiEvent::MessageSceneImageChanged { message_id, .. } if *message_id == reply
    )));
}

#[tokio::test(flavor = "multi_thread")]
async fn an_ask_first_follow_up_survives_a_restart_and_its_approval_runs_the_job() {
    let scene = scene_harness(
        Reply::Text(TAGGED),
        vec![Outcome::Image],
        SceneGenerationMode::AskFirst,
    );
    let (chat, reply) = reply_with_scene(&scene.harness, "ask").await;
    let shown = reply_of(&scene.harness.context, &chat, &reply).await;
    let before = shown.scene_image.expect("waiting for approval");
    assert_eq!(before.state, dto::SceneImageState::Pending);
    assert_eq!(before.mode, dto::SceneImageMode::AskFirst);
    assert_eq!(before.job_id, None);

    let restarted = scene.harness.context.restarted();
    restarted.recover_after_restart().expect("recovery");
    let after = reply_of(&restarted, &chat, &reply).await;
    assert_eq!(after.scene_image, Some(before));
    assert_eq!(scene.images.calls(), 0);

    let blank = message_scene_image_approve(&restarted, approve(&reply, Some("  \n ")))
        .await
        .expect_err("a blank prompt");
    assert_eq!(blank.code, ApiErrorCode::InvalidInput);
    let accepted =
        message_scene_image_approve(&restarted, approve(&reply, Some("  a red harbor ")))
            .await
            .expect("approved");
    let again = message_scene_image_approve(&restarted, approve(&reply, None))
        .await
        .expect("a second approval returns the job");
    assert_eq!(again, accepted);
    run_jobs(&restarted).await;
    let shown = reply_of(&restarted, &chat, &reply).await;
    assert_eq!(media_count(&shown), 1);
    let prompts = scene.images.prompts.lock().expect("prompts").clone();
    assert_eq!(prompts.len(), 1);
    assert!(prompts[0].ends_with("a red harbor"), "{}", prompts[0]);
}

#[tokio::test(flavor = "multi_thread")]
async fn dismissing_a_follow_up_is_final_and_repeatable() {
    let scene = scene_harness(
        Reply::Text(TAGGED),
        Vec::new(),
        SceneGenerationMode::AskFirst,
    );
    let context = &scene.harness.context;
    let (chat, reply) = reply_with_scene(&scene.harness, "dismiss").await;
    message_scene_image_dismiss(
        context,
        dto::MessageSceneRequest {
            message_id: reply.clone(),
        },
    )
    .await
    .expect("dismissed");
    message_scene_image_dismiss(
        context,
        dto::MessageSceneRequest {
            message_id: reply.clone(),
        },
    )
    .await
    .expect("dismissed again");
    assert_eq!(reply_of(context, &chat, &reply).await.scene_image, None);
    let refused = message_scene_image_approve(context, approve(&reply, None))
        .await
        .expect_err("dismissed");
    assert_eq!(refused.code, ApiErrorCode::Conflict);
    assert_eq!(scene.images.calls(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stopped_reply_asks_for_no_scene_image() {
    let scene = scene_harness(
        Reply::PartialUntilCancelled("Half of it <img>a harbor</img>"),
        Vec::new(),
        SceneGenerationMode::Auto,
    );
    let context = &scene.harness.context;
    let chat = launch(&scene.harness, "stopped-launch").await;
    let accepted = send(
        &scene.harness,
        &chat,
        "stopped-send",
        "Show me",
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect("send");
    let worker = ConversationGenerationWorker::new(context.clone());
    let (ran, cancelled) = tokio::join!(worker.run_once(), async {
        scene.harness.provider.entered.notified().await;
        generation_cancel(
            context,
            dto::GenerationCancelRequest {
                turn_id: accepted.turn_id.clone(),
            },
        )
        .await
    });
    assert!(ran.expect("worker ran"));
    cancelled.expect("cancel");
    let view = open_chat(context, &chat).await;
    let reply = view.messages.items.last().expect("the kept partial reply");
    assert_eq!(reply.role, dto::MessageRole::Assistant);
    assert_eq!(reply.scene_image, None);
    assert_eq!(follow_up_state(context, &chat, &reply.id), None);
    run_jobs(context).await;
    assert_eq!(scene.images.calls(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_manual_image_takes_a_written_or_typed_prompt_and_a_second_one_adds_another() {
    let scene = scene_harness(
        Reply::Text(TAGGED),
        vec![Outcome::Image, Outcome::Image],
        SceneGenerationMode::Manual,
    );
    let context = &scene.harness.context;
    let (chat, reply) = reply_with_scene(&scene.harness, "manual").await;
    assert_eq!(reply_of(context, &chat, &reply).await.scene_image, None);
    assert_eq!(follow_up_state(context, &chat, &reply), None);

    let mut feed = JobFeed::start(context).await.expect("feed");
    let written = message_scene_prompt_generate(
        context,
        dto::MessageSceneRequest {
            message_id: reply.clone(),
        },
    )
    .await
    .expect("prompt job");
    let sink = Arc::new(RecordingJob::default());
    job_watch(
        context,
        dto::JobWatchRequest {
            job_id: written.job_id.clone(),
        },
        sink.clone(),
    )
    .await
    .expect("watch");
    run_jobs(context).await;
    feed.publish(context).await.expect("publish");
    let Some(dto::JobEvent::Completed { job }) = sink.0.lock().expect("events").last().cloned()
    else {
        panic!(
            "the scene prompt job did not complete: {:?}",
            sink.0.lock().expect("events")
        );
    };
    assert!(matches!(
        job.result,
        Some(dto::JobResultDto::GeneratedText { .. })
    ));

    let blank = message_scene_image_generate(
        context,
        dto::MessageSceneImageGenerateRequest {
            message_id: reply.clone(),
            prompt: "   ".into(),
        },
    )
    .await
    .expect_err("blank");
    assert_eq!(blank.code, ApiErrorCode::InvalidInput);
    for prompt in ["  manual harbor ", "second harbor"] {
        message_scene_image_generate(
            context,
            dto::MessageSceneImageGenerateRequest {
                message_id: reply.clone(),
                prompt: prompt.into(),
            },
        )
        .await
        .expect("image job");
        let busy = message_scene_image_generate(
            context,
            dto::MessageSceneImageGenerateRequest {
                message_id: reply.clone(),
                prompt: "again".into(),
            },
        )
        .await
        .expect_err("one image job at a time");
        assert_eq!(busy.code, ApiErrorCode::Busy);
        run_jobs(context).await;
    }
    assert_eq!(media_count(&reply_of(context, &chat, &reply).await), 2);
    let prompts = scene.images.prompts.lock().expect("prompts").clone();
    assert!(prompts[0].ends_with("manual harbor"), "{}", prompts[0]);
    assert!(prompts[1].ends_with("second harbor"), "{}", prompts[1]);
}

#[derive(Default)]
struct RecordingJob(Mutex<Vec<dto::JobEvent>>);

impl JobEventSink for RecordingJob {
    fn emit(&self, event: dto::JobEvent) -> bool {
        self.0.lock().expect("job events").push(event);
        true
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn cancelling_the_image_job_dismisses_the_follow_up_and_adds_nothing() {
    let scene = scene_harness(
        Reply::Text(TAGGED),
        vec![Outcome::UntilCancelled],
        SceneGenerationMode::AskFirst,
    );
    let context = &scene.harness.context;
    let (chat, reply) = reply_with_scene(&scene.harness, "cancel").await;
    let accepted = message_scene_image_approve(context, approve(&reply, None))
        .await
        .expect("approved");
    let runner = JobRunner::new(context.clone(), JobHandlers::standard());
    assert!(runner.run_once().await.expect("started"));
    scene.images.entered.notified().await;
    assert_eq!(
        follow_up_state(context, &chat, &reply),
        Some(SceneFollowUpState::Running)
    );
    job_cancel(
        context,
        dto::JobCancelRequest {
            job_id: accepted.job_id.clone(),
        },
    )
    .await
    .expect("cancel");
    runner.wait_idle().await;
    let job = job_get(
        context,
        dto::JobGetRequest {
            job_id: accepted.job_id,
        },
    )
    .await
    .expect("job");
    assert_eq!(job.state, dto::JobStateDto::Cancelled);
    let shown = reply_of(context, &chat, &reply).await;
    assert_eq!(shown.scene_image, None);
    assert_eq!(media_count(&shown), 0);
    assert_eq!(
        follow_up_state(context, &chat, &reply),
        Some(SceneFollowUpState::Dismissed)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_missing_image_is_retried_three_times_then_fails_typed() {
    let scene = scene_harness(
        Reply::Text(TAGGED),
        vec![Outcome::Empty, Outcome::Empty, Outcome::Empty],
        SceneGenerationMode::Auto,
    );
    let context = &scene.harness.context;
    let (chat, reply) = reply_with_scene(&scene.harness, "retries").await;
    run_jobs(context).await;
    assert_eq!(scene.images.calls(), 3);
    let shown = reply_of(context, &chat, &reply).await;
    let image = shown.scene_image.clone().expect("a failed image");
    assert_eq!(image.state, dto::SceneImageState::Failed);
    assert_eq!(image.failure, Some(dto::SceneImageFailure::NoImage));
    assert_eq!(media_count(&shown), 0);

    let failed = scene_harness(
        Reply::Text(TAGGED),
        vec![Outcome::Fail("Quota exceeded")],
        SceneGenerationMode::Auto,
    );
    let (chat, reply) = reply_with_scene(&failed.harness, "quota").await;
    run_jobs(&failed.harness.context).await;
    assert_eq!(failed.images.calls(), 1);
    let image = reply_of(&failed.harness.context, &chat, &reply)
        .await
        .scene_image
        .expect("a failed image");
    assert_eq!(image.failure, Some(dto::SceneImageFailure::Failed));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_follow_up_whose_job_a_restart_cancelled_fails_as_interrupted() {
    let scene = scene_harness(Reply::Text(TAGGED), Vec::new(), SceneGenerationMode::Auto);
    let (chat, reply) = reply_with_scene(&scene.harness, "restart").await;
    assert_eq!(
        follow_up_state(&scene.harness.context, &chat, &reply),
        Some(SceneFollowUpState::Approved)
    );
    let restarted = scene.harness.context.restarted();
    restarted.recover_after_restart().expect("recovery");
    let image = reply_of(&restarted, &chat, &reply)
        .await
        .scene_image
        .expect("an interrupted image");
    assert_eq!(image.state, dto::SceneImageState::Failed);
    assert_eq!(image.failure, Some(dto::SceneImageFailure::Interrupted));
    assert_eq!(scene.images.calls(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_group_reply_asks_for_no_scene_image() {
    let scene = scene_harness(Reply::Text(TAGGED), Vec::new(), SceneGenerationMode::Auto);
    let cast = super::turns_tests::group_cast(&scene.harness, "scene-group").await;
    send(
        &scene.harness,
        &cast.chat,
        "scene-group-send",
        "Show me",
        Arc::new(RecordingStream::default()),
    )
    .await
    .expect("send");
    super::turns_tests::run_generation(&scene.harness).await;
    let view = open_chat(&scene.harness.context, &cast.chat).await;
    assert!(
        view.messages
            .items
            .iter()
            .all(|message| message.scene_image.is_none())
    );
    let refused = message_scene_image_generate(
        &scene.harness.context,
        dto::MessageSceneImageGenerateRequest {
            message_id: view.messages.items.last().expect("reply").id.clone(),
            prompt: "a harbor".into(),
        },
    )
    .await
    .expect_err("a group chat");
    assert_eq!(refused.code, ApiErrorCode::Unsupported);
}

#[tokio::test(flavor = "multi_thread")]
async fn deleting_a_chat_removes_what_its_feature_jobs_stored() {
    let scene = scene_harness(
        Reply::Text(TAGGED),
        vec![Outcome::Image],
        SceneGenerationMode::Auto,
    );
    let context = &scene.harness.context;
    let (chat, reply) = reply_with_scene(&scene.harness, "purge").await;
    let image_job: lettuce_types::JobId = reply_of(context, &chat, &reply)
        .await
        .scene_image
        .expect("an image")
        .job_id
        .expect("its job")
        .parse()
        .expect("job id");
    let helper = conversation_help_me_reply(
        context,
        dto::ConversationHelpMeReplyRequest {
            conversation_id: chat.clone(),
            mode: dto::HelpMeReplyMode::Enrich,
            current_draft: Some("a private draft".into()),
            swap_places: false,
            client_operation_id: "purge-help".into(),
        },
    )
    .await
    .expect("help me reply");
    run_jobs(context).await;
    let helper: lettuce_types::JobId = helper.job_id.parse().expect("job id");
    let database = context.backend().database();
    assert!(database.local_model_job(helper).expect("detail").is_some());
    assert!(lettuce_image_generation::ImageGenerationRepository::get(database, image_job).is_ok());
    conversation_delete(
        context,
        dto::ConversationRequest {
            conversation_id: chat,
        },
    )
    .await
    .expect("delete");
    assert!(database.local_model_job(helper).expect("detail").is_none());
    assert!(lettuce_image_generation::ImageGenerationRepository::get(database, image_job).is_err());
}
