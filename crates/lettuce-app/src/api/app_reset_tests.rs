use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_jobs::{JobStore, SystemClock};
use lettuce_platform::{DirectorySnapshot, FilesystemAuthority};
use lettuce_types::{OperationId, RequestId, TimestampMillis};

use super::app_reset::{AppResetHandler, AppResetHost};
use super::tests::{NoImages, Reply, harness_over_files};
use super::{ApiContext, ApiDatabaseFiles, NoModels};
use crate::{AppBackend, AppDatabaseLocation};

#[derive(Default)]
struct Host {
    calls: Mutex<Vec<&'static str>>,
    fail_clear: bool,
    fail_restart: bool,
    hold_clear: Option<Arc<tokio::sync::Notify>>,
    entered_clear: tokio::sync::Notify,
}

#[async_trait]
impl AppResetHost for Host {
    async fn preflight(&self) -> Result<(), ApiError> {
        Ok(())
    }
    async fn stop_workers(&self) -> Result<(), ApiError> {
        self.calls.lock().expect("calls").push("stop");
        Ok(())
    }
    async fn clear_webview_storage(&self) -> Result<(), ApiError> {
        self.calls.lock().expect("calls").push("clear");
        self.entered_clear.notify_one();
        if let Some(release) = &self.hold_clear {
            release.notified().await;
        }
        if self.fail_clear {
            return Err(super::app_reset::reset_error(
                dto::AppDataResetStage::WebviewStorage,
                None,
            ));
        }
        Ok(())
    }
    async fn prepare_restart(&self) -> Result<(), ApiError> {
        self.calls.lock().expect("calls").push("restart");
        if self.fail_restart {
            return Err(super::app_reset::reset_error(
                dto::AppDataResetStage::Restart,
                None,
            ));
        }
        Ok(())
    }
    fn exit_for_restart(&self) {
        self.calls.lock().expect("calls").push("exit");
    }
}

struct Progress;

impl super::jobs::JobProgressSink for Progress {
    fn text_delta(&self, _text: Option<String>, _reasoning: Option<String>) {}
    fn image_progress(&self, _progress: dto::ImageProgress) {}
}

#[tokio::test]
async fn reset_resumes_webview_clear_and_restart_after_shutdown_after_cutover() {
    use super::jobs::{JobHandler, JobProgressSink};
    let (root, location, h) = harness();
    let host = Arc::new(Host {
        hold_clear: Some(Arc::new(tokio::sync::Notify::new())),
        ..Host::default()
    });
    h.context.attach_reset_host(host.clone()).expect("host");
    let accepted = super::app_data_reset(
        &h.context,
        dto::AppDataResetRequest {
            client_operation_id: RequestId::new().to_string(),
        },
    )
    .await
    .expect("admit");
    let id = accepted.job_id.parse().expect("id");
    let job = h
        .context
        .backend()
        .database()
        .get(id)
        .expect("job")
        .expect("exists");
    let work = AppResetHandler
        .claim(&h.context, &job, lettuce_jobs::WorkerId::new())
        .await
        .expect("claim")
        .expect("work");
    let context = h.context.clone();
    let progress: Arc<dyn JobProgressSink> = Arc::new(Progress);
    let task = tokio::spawn(async move { work.run(context, progress).await });
    host.entered_clear.notified().await;
    task.abort();
    assert!(task.await.expect_err("aborted reset task").is_cancelled());
    let active = location.active_path().expect("fresh active");
    let backend =
        Arc::new(AppBackend::open(&active, TimestampMillis::new(20)).expect("fresh backend"));
    let fresh = ApiContext::new(super::ApiContextParts {
        backend,
        secret_store: h.context.secret_store().clone(),
        inference: h.provider.clone(),
        image_provider: Arc::new(NoImages),
        models: Arc::new(NoModels),
        speech: Arc::new(super::NoSpeech),
        media: None,
        events: h.events.clone(),
        clock: Arc::new(SystemClock),
        files: Arc::new(super::tests::StdFiles),
        app_folder: Some(root.clone()),
        resource_dir: None,
        database_files: Some(ApiDatabaseFiles { location, active }),
        asset_url_base: "test-asset://host".into(),
    });
    fresh.recover_after_restart().expect("recovery");
    let resumed_host = Arc::new(Host::default());
    fresh
        .attach_reset_host(resumed_host.clone())
        .expect("resumed host");
    run(&fresh).await;
    assert_eq!(
        resumed_host.calls.lock().expect("calls").as_slice(),
        &["stop", "clear", "restart", "exit"]
    );
    let view = super::job_get(
        &fresh,
        dto::JobGetRequest {
            job_id: accepted.job_id,
        },
    )
    .await
    .expect("terminal");
    assert_eq!(view.state, dto::JobStateDto::Succeeded);
    drop((h, fresh));
    std::fs::remove_dir_all(root).expect("cleanup");
}

fn harness() -> (
    std::path::PathBuf,
    AppDatabaseLocation,
    super::tests::Harness,
) {
    let root = std::env::temp_dir().join(format!("lettuce-api-reset-{}", OperationId::new()));
    let authority = FilesystemAuthority::new(&DirectorySnapshot::new(&root).expect("snapshot"))
        .expect("authority");
    let location =
        AppDatabaseLocation::new(root.join("private-persistent-v2"), &authority).expect("location");
    let active = location.active_path().expect("active");
    let backend = Arc::new(AppBackend::open(&active, TimestampMillis::new(10)).expect("backend"));
    let media = Arc::new(super::ApiMediaStore::new(
        authority.managed_files(),
        authority
            .read_capability(lettuce_platform::ManagedRoot::MediaBlobs)
            .expect("read"),
        authority
            .write_capability(lettuce_platform::ManagedRoot::MediaBlobs)
            .expect("write"),
        lettuce_database::Database::open(&active).expect("blobs"),
        lettuce_database::Database::open(&active).expect("assets"),
    ));
    let h = harness_over_files(
        backend,
        Reply::Text("ok"),
        Arc::new(SystemClock),
        Some(media),
        Some(root.clone()),
        Arc::new(NoModels),
        Arc::new(NoImages),
        Some(ApiDatabaseFiles {
            location: location.clone(),
            active,
        }),
    );
    (root, location, h)
}

async fn run(context: &ApiContext) {
    let runner = super::jobs::JobRunner::new(
        context.clone(),
        super::jobs::JobHandlers::new(vec![Arc::new(AppResetHandler)]),
    );
    assert!(runner.run_once().await.expect("run reset"));
    runner.wait_idle().await;
}

#[tokio::test]
async fn reset_missing_shell_fails_typed_before_admission_or_file_changes() {
    let (root, location, h) = harness();
    let before = location.active_path().expect("active");
    let bytes = std::fs::read(&before).expect("source bytes");
    let error = super::app_data_reset(
        &h.context,
        dto::AppDataResetRequest {
            client_operation_id: RequestId::new().to_string(),
        },
    )
    .await
    .expect_err("missing host");
    assert_eq!(error.code, ApiErrorCode::Unavailable);
    assert!(matches!(
        error.details,
        Some(dto::ApiErrorDetails::AppDataReset {
            stage: dto::AppDataResetStage::Preflight,
            ..
        })
    ));
    assert_eq!(location.active_path().expect("active"), before);
    assert_eq!(std::fs::read(&before).expect("preserved"), bytes);
    drop(h);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test]
async fn reset_is_a_durable_job_keeps_the_source_and_replays_in_the_fresh_database() {
    let (root, location, h) = harness();
    let old = location.active_path().expect("old active");
    let model_file = root.join("retained-model.gguf");
    std::fs::write(&model_file, b"retained model").expect("model file");
    let object = h
        .context
        .media()
        .expect("media")
        .ingest(
            super::tests::png_bytes().as_slice(),
            lettuce_media::IngestRequest::new(
                lettuce_media::AssetKind::OtherImage,
                lettuce_media::AssetOrigin::Upload,
                lettuce_media::RetentionClass::Library,
                lettuce_media::AssetProvenanceV1::default(),
            ),
        )
        .expect("media object");
    let hash = object.blob.content_hash.clone();
    let host = Arc::new(Host::default());
    h.context.attach_reset_host(host.clone()).expect("host");
    let request = dto::AppDataResetRequest {
        client_operation_id: RequestId::new().to_string(),
    };
    let accepted = super::app_data_reset(&h.context, request.clone())
        .await
        .expect("admit");
    assert_eq!(
        super::app_data_reset(&h.context, request.clone())
            .await
            .expect("queued replay"),
        accepted
    );
    run(&h.context).await;
    let view = super::job_get(
        &h.context,
        dto::JobGetRequest {
            job_id: accepted.job_id.clone(),
        },
    )
    .await
    .expect("terminal job");
    assert_eq!(view.state, dto::JobStateDto::Succeeded);
    let Some(dto::JobResultDto::AppDataReset { kept_file }) = view.result else {
        panic!("reset result");
    };
    assert!(!old.exists());
    assert!(old.parent().expect("directory").join(kept_file).exists());
    assert_ne!(location.active_path().expect("fresh active"), old);
    assert_eq!(
        std::fs::read(&model_file).expect("retained model"),
        b"retained model"
    );
    let lifecycle = location.file_lifecycle().await.expect("lifecycle");
    assert!(
        lifecycle
            .kept_media_hashes()
            .expect("kept hashes")
            .contains(&hash)
    );
    drop(lifecycle);
    assert_eq!(
        host.calls.lock().expect("calls").as_slice(),
        &["stop", "clear", "restart", "exit"]
    );
    assert_eq!(
        super::app_data_reset(&h.context, request)
            .await
            .expect("fresh replay"),
        accepted
    );
    drop(h);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test]
async fn reset_webview_failure_keeps_the_cutover_reports_a_terminal_failure_and_restarts() {
    let (root, location, h) = harness();
    let old = location.active_path().expect("old active");
    let host = Arc::new(Host {
        fail_clear: true,
        ..Host::default()
    });
    h.context.attach_reset_host(host.clone()).expect("host");
    let accepted = super::app_data_reset(
        &h.context,
        dto::AppDataResetRequest {
            client_operation_id: RequestId::new().to_string(),
        },
    )
    .await
    .expect("admit");
    run(&h.context).await;
    let view = super::job_get(
        &h.context,
        dto::JobGetRequest {
            job_id: accepted.job_id,
        },
    )
    .await
    .expect("terminal");
    assert_eq!(view.state, dto::JobStateDto::Failed);
    assert_eq!(
        view.failure.expect("typed failure").reason,
        Some(dto::JobFailureReason::ResetWebviewStorage)
    );
    assert_ne!(location.active_path().expect("new active"), old);
    assert_eq!(
        host.calls.lock().expect("calls").as_slice(),
        &["stop", "clear", "restart", "exit"],
        "the process never keeps running on the closed database"
    );
    drop(h);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test]
async fn reset_cancelled_before_claim_preserves_the_active_database() {
    let (root, location, h) = harness();
    let old = location.active_path().expect("old active");
    let host = Arc::new(Host::default());
    h.context.attach_reset_host(host.clone()).expect("host");
    let accepted = super::app_data_reset(
        &h.context,
        dto::AppDataResetRequest {
            client_operation_id: RequestId::new().to_string(),
        },
    )
    .await
    .expect("admit");
    super::job_cancel(
        &h.context,
        dto::JobCancelRequest {
            job_id: accepted.job_id.clone(),
        },
    )
    .await
    .expect("cancel");
    let id = accepted.job_id.parse().expect("id");
    assert_eq!(
        h.context
            .backend()
            .database()
            .get(id)
            .expect("job")
            .expect("exists")
            .state,
        lettuce_jobs::JobState::Cancelled
    );
    assert_eq!(location.active_path().expect("unchanged"), old);
    assert!(host.calls.lock().expect("calls").is_empty());
    drop(h);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test]
async fn reset_claimed_by_another_worker_does_not_start_or_change_files() {
    use super::jobs::JobHandler;
    let (root, location, h) = harness();
    let old = location.active_path().expect("active");
    let host = Arc::new(Host::default());
    h.context.attach_reset_host(host.clone()).expect("host");
    let accepted = super::app_data_reset(
        &h.context,
        dto::AppDataResetRequest {
            client_operation_id: RequestId::new().to_string(),
        },
    )
    .await
    .expect("admit");
    let id = accepted.job_id.parse().expect("id");
    let job = h
        .context
        .backend()
        .database()
        .get(id)
        .expect("job")
        .expect("exists");
    let first = AppResetHandler
        .claim(&h.context, &job, lettuce_jobs::WorkerId::new())
        .await
        .expect("first")
        .expect("claim");
    assert!(
        AppResetHandler
            .claim(&h.context, &job, lettuce_jobs::WorkerId::new())
            .await
            .expect("second")
            .is_none()
    );
    assert_eq!(location.active_path().expect("unchanged"), old);
    assert!(host.calls.lock().expect("calls").is_empty());
    drop((first, h));
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test]
async fn reset_rejects_cancel_after_cutover_and_reports_restart_failure() {
    use super::jobs::{JobHandler, JobProgressSink};
    let (root, location, h) = harness();
    let release = Arc::new(tokio::sync::Notify::new());
    let host = Arc::new(Host {
        fail_restart: true,
        hold_clear: Some(release.clone()),
        ..Host::default()
    });
    h.context.attach_reset_host(host.clone()).expect("host");
    let accepted = super::app_data_reset(
        &h.context,
        dto::AppDataResetRequest {
            client_operation_id: RequestId::new().to_string(),
        },
    )
    .await
    .expect("admit");
    let id = accepted.job_id.parse().expect("id");
    let job = h
        .context
        .backend()
        .database()
        .get(id)
        .expect("job")
        .expect("exists");
    let work = AppResetHandler
        .claim(&h.context, &job, lettuce_jobs::WorkerId::new())
        .await
        .expect("claim")
        .expect("work");
    let context = h.context.clone();
    let progress: Arc<dyn JobProgressSink> = Arc::new(Progress);
    let task = tokio::spawn(async move { work.run(context, progress).await });
    host.entered_clear.notified().await;
    assert_eq!(
        super::job_cancel(
            &h.context,
            dto::JobCancelRequest {
                job_id: accepted.job_id.clone()
            }
        )
        .await
        .expect_err("irreversible")
        .code,
        ApiErrorCode::Conflict
    );
    release.notify_one();
    task.await.expect("task").expect("settled");
    let view = super::job_get(
        &h.context,
        dto::JobGetRequest {
            job_id: accepted.job_id,
        },
    )
    .await
    .expect("terminal");
    assert_eq!(view.state, dto::JobStateDto::Failed);
    assert_eq!(
        view.failure.expect("failure").reason,
        Some(dto::JobFailureReason::ResetRestart)
    );
    assert!(location.active_path().expect("active").exists());
    assert_eq!(
        host.calls.lock().expect("calls").as_slice(),
        &["stop", "clear", "restart", "exit"],
        "a failed relaunch still ends the process, which the user reopens"
    );
    drop(h);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test]
async fn reset_failing_after_workers_stopped_restarts_on_the_preserved_database() {
    let (root, location, h) = harness();
    let old = location.active_path().expect("active");
    let leaked = lettuce_database::Database::open(&old).expect("a handle reset cannot close");
    let host = Arc::new(Host::default());
    h.context.attach_reset_host(host.clone()).expect("host");
    let accepted = super::app_data_reset(
        &h.context,
        dto::AppDataResetRequest {
            client_operation_id: RequestId::new().to_string(),
        },
    )
    .await
    .expect("admit");
    run(&h.context).await;
    let view = super::job_get(
        &h.context,
        dto::JobGetRequest {
            job_id: accepted.job_id,
        },
    )
    .await
    .expect("terminal");
    assert_eq!(view.state, dto::JobStateDto::Failed);
    assert_eq!(
        view.failure.expect("typed failure").reason,
        Some(dto::JobFailureReason::ResetDatabase)
    );
    assert_eq!(
        host.calls.lock().expect("calls").as_slice(),
        &["stop", "restart", "exit"],
        "stopped workers never leave the process running"
    );
    assert_eq!(location.active_path().expect("unchanged"), old);
    drop(leaked);
    drop(h);
    drop(lettuce_database::Database::open(&old).expect("source stays writable"));
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test]
async fn reset_retried_after_a_crash_that_left_its_target_file_starts_a_new_file() {
    let (root, location, h) = harness();
    let old = location.active_path().expect("active");
    let request = RequestId::new();
    {
        let lifecycle = location.file_lifecycle().await.expect("lifecycle");
        let abandoned = lifecycle
            .begin_file(
                &format!("reset-{request}.sqlite3"),
                crate::DatabaseFileKind::Reset,
                TimestampMillis::new(15),
            )
            .expect("crashed attempt's target");
        drop(lettuce_database::Database::open(&abandoned).expect("partly written"));
    }
    let host = Arc::new(Host::default());
    h.context.attach_reset_host(host.clone()).expect("host");
    let accepted = super::app_data_reset(
        &h.context,
        dto::AppDataResetRequest {
            client_operation_id: request.to_string(),
        },
    )
    .await
    .expect("admit");
    run(&h.context).await;
    let view = super::job_get(
        &h.context,
        dto::JobGetRequest {
            job_id: accepted.job_id,
        },
    )
    .await
    .expect("terminal");
    assert_eq!(view.state, dto::JobStateDto::Succeeded);
    assert_ne!(location.active_path().expect("fresh"), old);
    drop(h);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test]
async fn a_reset_job_restored_into_another_database_file_never_runs() {
    let (root, location, h) = harness();
    let host = Arc::new(Host::default());
    h.context.attach_reset_host(host.clone()).expect("host");
    let accepted = super::app_data_reset(
        &h.context,
        dto::AppDataResetRequest {
            client_operation_id: RequestId::new().to_string(),
        },
    )
    .await
    .expect("admit");
    let source = location.active_path().expect("source");
    let copy = format!("{}.sqlite3", OperationId::new());
    drop(h);
    let lifecycle = location.file_lifecycle().await.expect("lifecycle");
    let target = lifecycle
        .begin_file(
            &copy,
            crate::DatabaseFileKind::Restore,
            TimestampMillis::new(20),
        )
        .expect("restore target");
    std::fs::copy(&source, &target).expect("restored copy holding the queued reset");
    lifecycle
        .activate_file(&copy, TimestampMillis::new(30))
        .expect("restore cutover");
    drop(lifecycle);
    let backend =
        Arc::new(AppBackend::open(&target, TimestampMillis::new(40)).expect("restored backend"));
    let restored = harness_over_files(
        backend,
        Reply::Text("ok"),
        Arc::new(SystemClock),
        None,
        Some(root.clone()),
        Arc::new(NoModels),
        Arc::new(NoImages),
        Some(ApiDatabaseFiles {
            location: location.clone(),
            active: target.clone(),
        }),
    );
    restored
        .context
        .attach_reset_host(host.clone())
        .expect("host");
    run(&restored.context).await;
    let view = super::job_get(
        &restored.context,
        dto::JobGetRequest {
            job_id: accepted.job_id,
        },
    )
    .await
    .expect("terminal");
    assert_eq!(view.state, dto::JobStateDto::Failed);
    assert_eq!(
        view.failure.expect("typed failure").reason,
        Some(dto::JobFailureReason::ResetDatabase)
    );
    assert!(
        host.calls.lock().expect("calls").is_empty(),
        "nothing stops for a reset bound to another file"
    );
    assert_eq!(location.active_path().expect("unchanged"), target);
    drop(restored);
    std::fs::remove_dir_all(root).expect("cleanup");
}

fn fence(location: &AppDatabaseLocation) {
    let fence =
        lettuce_database::Database::lock_file_writes(&location.active_path().expect("active"))
            .expect("fence");
    fence.set_fenced(true).expect("freeze");
}

#[tokio::test]
async fn api_writes_after_the_cutover_fence_fail_typed() {
    let (root, location, h) = harness();
    fence(&location);
    let error = super::conversation_launch_direct(
        &h.context,
        dto::LaunchDirectRequest {
            character_id: h.character_id.to_string(),
            title: None,
            scene_id: None,
            starter_id: None,
            client_operation_id: RequestId::new().to_string(),
        },
    )
    .await
    .expect_err("the kept database accepts no writes");
    assert_eq!(error.code, ApiErrorCode::Unavailable);
    assert_eq!(
        error.details,
        Some(dto::ApiErrorDetails::DatabaseWriteFenced)
    );
    drop(h);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test]
async fn a_generation_whose_settlement_hits_the_fence_ends_its_stream_typed() {
    let (root, location, h) = harness();
    let conversation = super::tests::launch(&h, "fenced-generation-launch").await;
    let release = Arc::new(tokio::sync::Notify::new());
    *h.provider.response_release.lock().expect("release") = Some(release.clone());
    let stream = Arc::new(super::tests::RecordingStream::default());
    let accepted = super::tests::send(
        &h,
        &conversation,
        "fenced-generation-send",
        "Hello",
        stream.clone(),
    )
    .await
    .expect("send");
    let worker = super::ConversationGenerationWorker::new(h.context.clone());
    let generation = tokio::spawn(async move { worker.run_once().await });
    h.provider.entered.notified().await;
    fence(&location);
    release.notify_one();
    let error = generation
        .await
        .expect("worker task")
        .expect_err("settlement cannot be written");
    assert_eq!(
        error.details,
        Some(dto::ApiErrorDetails::DatabaseWriteFenced)
    );
    assert_eq!(
        stream.events().last(),
        Some(&dto::GenerationEvent::Failed {
            turn_id: accepted.turn_id,
            code: dto::GenerationFailureCode::DatabaseWriteFenced,
        })
    );
    drop(h);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test]
async fn a_reset_requested_during_a_generation_waits_and_keeps_its_reply() {
    let (root, location, h) = harness();
    let old = location.active_path().expect("active");
    let conversation = super::tests::launch(&h, "reset-generation-launch").await;
    let release = Arc::new(tokio::sync::Notify::new());
    *h.provider.response_release.lock().expect("release") = Some(release.clone());
    let stream = Arc::new(super::tests::RecordingStream::default());
    super::tests::send(
        &h,
        &conversation,
        "reset-generation-send",
        "Hello",
        stream.clone(),
    )
    .await
    .expect("send");
    let worker = super::ConversationGenerationWorker::new(h.context.clone());
    let generation = tokio::spawn(async move { worker.run_once().await });
    h.provider.entered.notified().await;
    let host = Arc::new(Host::default());
    h.context.attach_reset_host(host.clone()).expect("host");
    super::app_data_reset(
        &h.context,
        dto::AppDataResetRequest {
            client_operation_id: RequestId::new().to_string(),
        },
    )
    .await
    .expect("admit");
    let context = h.context.clone();
    let reset = tokio::spawn(async move { run(&context).await });
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    assert!(
        host.calls.lock().expect("calls").is_empty(),
        "the reset waits for the running generation"
    );
    assert_eq!(location.active_path().expect("unchanged"), old);
    release.notify_one();
    assert!(generation.await.expect("worker task").expect("settled"));
    reset.await.expect("reset task");
    assert!(
        matches!(
            stream.events().last(),
            Some(dto::GenerationEvent::Completed { .. })
        ),
        "the reply settled in the database the reset keeps: {:?}",
        stream.events()
    );
    assert_eq!(
        host.calls.lock().expect("calls").as_slice(),
        &["stop", "clear", "restart", "exit"]
    );
    drop(h);
    std::fs::remove_dir_all(root).expect("cleanup");
}
