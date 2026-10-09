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
async fn reset_webview_failure_keeps_the_cutover_and_reports_a_terminal_failure_without_restart() {
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
        &["stop", "clear"]
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
        &["stop", "clear", "restart"]
    );
    drop(h);
    std::fs::remove_dir_all(root).expect("cleanup");
}
