use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use async_trait::async_trait;
use lettuce_contracts::{self as dto, ApiErrorCode, ApiErrorDetails, ApiEvent};
use lettuce_jobs::{
    CancellationPolicy, IdempotencyKey, JobKind, JobSpec, JobState, JobStore, JobSubject,
    OutcomeRef, RecoveryPolicy, ResourceClass, SubjectKind, SystemClock,
};
use lettuce_model_hub::PinnedArtifact;
use lettuce_types::{AssetId, JobId, OperationId};
use sha2::{Digest, Sha256};

use super::*;
use crate::api::tests::{Harness, Reply, api_events, harness, harness_in};
use crate::api::{ApiContext, JobEventSink};
use crate::{
    ArtifactBody, ArtifactInstallPlan, ArtifactSource, ArtifactSourceClient, ArtifactSourceError,
    KokoroDownloadSource, KokoroVoiceDownloadSource, PlannedArtifact, WhisperDownloadSource,
};

#[derive(Default)]
struct RecordingJob(Mutex<Vec<dto::JobEvent>>);

impl JobEventSink for RecordingJob {
    fn emit(&self, event: dto::JobEvent) {
        self.0.lock().expect("job events").push(event);
    }
}

impl RecordingJob {
    fn events(&self) -> Vec<dto::JobEvent> {
        self.0.lock().expect("job events").clone()
    }
}

fn spec(kind: JobKind, subject: &str, key: &str) -> JobSpec {
    JobSpec::new(
        kind,
        JobSubject::new(SubjectKind::Maintenance, subject).expect("subject"),
        OutcomeRef::ArtifactInstallation(AssetId::new()),
    )
    .with_idempotency_key(IdempotencyKey::new(key).expect("key"))
    .with_resources(vec![ResourceClass::Cpu])
    .with_policies(
        RecoveryPolicy::MarkInterrupted,
        CancellationPolicy::Cooperative,
    )
}

fn create(harness: &Harness, spec: JobSpec) -> JobId {
    harness
        .context
        .backend()
        .database()
        .create_or_get(spec)
        .expect("job")
        .job
        .id
}

fn state(context: &ApiContext, job_id: JobId) -> JobState {
    context
        .backend()
        .database()
        .get(job_id)
        .expect("job")
        .expect("job exists")
        .state
}

fn job_updates(harness: &Harness, job_id: JobId) -> Vec<dto::JobStateDto> {
    api_events(harness)
        .into_iter()
        .filter_map(|event| match event {
            ApiEvent::JobUpdated { job } if job.id == job_id.to_string() => Some(job.state),
            _ => None,
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn jobs_list_filters_and_pages_newest_first() {
    let harness = harness(Reply::Text("Hello."));
    let first = create(&harness, spec(JobKind::Maintenance, "one", "list-1"));
    let second = create(&harness, spec(JobKind::Maintenance, "two", "list-2"));
    let other = create(&harness, spec(JobKind::BackupExport, "one", "list-3"));
    let context = &harness.context;

    let page = jobs_list(
        context,
        dto::JobsListRequest {
            kinds: Some(vec![dto::JobKindDto::Maintenance]),
            limit: Some(1),
            ..dto::JobsListRequest::default()
        },
    )
    .await
    .expect("first page");
    assert_eq!(page.items.len(), 1);
    let rest = jobs_list(
        context,
        dto::JobsListRequest {
            kinds: Some(vec![dto::JobKindDto::Maintenance]),
            cursor: page.next_cursor.clone(),
            limit: Some(1),
            ..dto::JobsListRequest::default()
        },
    )
    .await
    .expect("second page");
    let mut listed = [page.items[0].id.clone(), rest.items[0].id.clone()];
    listed.sort();
    let mut expected = [first.to_string(), second.to_string()];
    expected.sort();
    assert_eq!(listed, expected);
    assert!(page.items[0].created_at >= rest.items[0].created_at);
    assert_eq!(rest.next_cursor, None);

    let by_subject = jobs_list(
        context,
        dto::JobsListRequest {
            subject: Some(dto::JobSubjectDto {
                kind: dto::JobSubjectKindDto::Maintenance,
                id: "one".into(),
            }),
            states: Some(vec![dto::JobStateDto::Queued]),
            ..dto::JobsListRequest::default()
        },
    )
    .await
    .expect("by subject");
    let mut ids = by_subject
        .items
        .iter()
        .map(|job| job.id.clone())
        .collect::<Vec<_>>();
    ids.sort();
    let mut expected = vec![first.to_string(), other.to_string()];
    expected.sort();
    assert_eq!(ids, expected);
    assert_eq!(by_subject.items[0].state, dto::JobStateDto::Queued);
    assert_eq!(
        by_subject.items[0].progress.label_code.as_deref(),
        Some("queued")
    );

    let none = jobs_list(
        context,
        dto::JobsListRequest {
            states: Some(vec![dto::JobStateDto::Succeeded]),
            ..dto::JobsListRequest::default()
        },
    )
    .await
    .expect("empty");
    assert!(none.items.is_empty());

    let error = jobs_list(
        context,
        dto::JobsListRequest {
            cursor: Some("not-a-cursor".into()),
            ..dto::JobsListRequest::default()
        },
    )
    .await
    .expect_err("bad cursor");
    assert_eq!(
        error.details,
        Some(ApiErrorDetails::InvalidField {
            field: "cursor".into()
        })
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn job_get_reads_a_job_and_rejects_unknown_ids() {
    let harness = harness(Reply::Text("Hello."));
    let job_id = create(&harness, spec(JobKind::Maintenance, "get", "get-1"));
    let view = job_get(
        &harness.context,
        dto::JobGetRequest {
            job_id: job_id.to_string(),
        },
    )
    .await
    .expect("job");
    assert_eq!(view.kind, dto::JobKindDto::Maintenance);
    assert_eq!(view.subject.id, "get");
    assert_eq!(view.failure, None);
    let missing = job_get(
        &harness.context,
        dto::JobGetRequest {
            job_id: JobId::new().to_string(),
        },
    )
    .await
    .expect_err("missing");
    assert_eq!(missing.code, ApiErrorCode::NotFound);
    let invalid = job_get(
        &harness.context,
        dto::JobGetRequest {
            job_id: "nope".into(),
        },
    )
    .await
    .expect_err("invalid");
    assert_eq!(invalid.code, ApiErrorCode::InvalidInput);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_watched_queued_job_is_cancelled_and_its_stream_ends() {
    let harness = harness(Reply::Text("Hello."));
    let context = &harness.context;
    let mut feed = JobFeed::start(context).await.expect("feed");
    let job_id = create(&harness, spec(JobKind::Maintenance, "cancel", "cancel-1"));
    let stream = Arc::new(RecordingJob::default());
    let view = job_watch(
        context,
        dto::JobWatchRequest {
            job_id: job_id.to_string(),
        },
        stream.clone(),
    )
    .await
    .expect("watch");
    assert_eq!(view.state, dto::JobStateDto::Queued);
    assert!(matches!(
        stream.events().as_slice(),
        [dto::JobEvent::Progress { job }] if job.state == dto::JobStateDto::Queued
    ));

    job_cancel(
        context,
        dto::JobCancelRequest {
            job_id: job_id.to_string(),
        },
    )
    .await
    .expect("cancel");
    assert_eq!(state(context, job_id), JobState::Cancelled);
    feed.publish(context).await.expect("publish");
    let events = stream.events();
    assert!(matches!(
        events.last(),
        Some(dto::JobEvent::Cancelled { job }) if job.state == dto::JobStateDto::Cancelled
    ));
    assert_eq!(
        job_updates(&harness, job_id).last(),
        Some(&dto::JobStateDto::Cancelled)
    );

    feed.publish(context).await.expect("publish again");
    assert_eq!(stream.events().len(), events.len());
    job_cancel(
        context,
        dto::JobCancelRequest {
            job_id: job_id.to_string(),
        },
    )
    .await
    .expect("an ended job is left alone");

    let late = Arc::new(RecordingJob::default());
    let view = job_watch(
        context,
        dto::JobWatchRequest {
            job_id: job_id.to_string(),
        },
        late.clone(),
    )
    .await
    .expect("late watch");
    assert_eq!(view.state, dto::JobStateDto::Cancelled);
    assert!(matches!(
        late.events().as_slice(),
        [dto::JobEvent::Cancelled { .. }]
    ));
    feed.publish(context).await.expect("publish");
    assert_eq!(late.events().len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn conversation_turns_are_not_cancelled_through_jobs() {
    let harness = harness(Reply::Text("Hello."));
    let job_id = create(
        &harness,
        spec(JobKind::ConversationGeneration, "turn", "generation-1"),
    );
    let error = job_cancel(
        &harness.context,
        dto::JobCancelRequest {
            job_id: job_id.to_string(),
        },
    )
    .await
    .expect_err("generation job");
    assert_eq!(error.code, ApiErrorCode::Unsupported);
    assert_eq!(state(&harness.context, job_id), JobState::Queued);
}

struct Body {
    chunks: Vec<Vec<u8>>,
    start: u64,
    pace: Option<Duration>,
}

#[async_trait]
impl ArtifactBody for Body {
    fn start(&self) -> u64 {
        self.start
    }

    async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, ArtifactSourceError> {
        if let Some(pace) = self.pace {
            tokio::time::sleep(pace).await;
        }
        if self.chunks.is_empty() {
            return Ok(None);
        }
        Ok(Some(self.chunks.remove(0)))
    }
}

/// Serves `bytes` in one chunk, or one byte at a time with `pace` between
/// chunks.
struct FakeSources {
    bytes: Vec<u8>,
    pace: Option<Duration>,
    opens: Mutex<u32>,
}

#[async_trait]
impl ArtifactSourceClient for Arc<FakeSources> {
    async fn open(
        &self,
        _source: &ArtifactSource,
        offset: u64,
        _expected_size: u64,
    ) -> Result<Box<dyn ArtifactBody>, ArtifactSourceError> {
        *self.opens.lock().expect("opens") += 1;
        let rest = self.bytes[usize::try_from(offset).expect("offset")..].to_vec();
        let chunks = match self.pace {
            Some(_) => rest.chunks(1).map(<[u8]>::to_vec).collect(),
            None => vec![rest],
        };
        Ok(Box::new(Body {
            chunks,
            start: offset,
            pace: self.pace,
        }))
    }
}

struct Sources(Arc<FakeSources>);

#[async_trait]
impl InstallSources for Sources {
    async fn artifacts(
        &self,
        _context: &ApiContext,
        _finish: &InstallFinish,
    ) -> Result<Box<dyn ArtifactSourceClient>, lettuce_contracts::ApiError> {
        Ok(Box::new(Arc::clone(&self.0)))
    }

    fn whisper(&self) -> Result<Box<dyn WhisperDownloadSource>, lettuce_contracts::ApiError> {
        NetworkInstallSources.whisper()
    }

    fn kokoro_model(&self) -> Result<Box<dyn KokoroDownloadSource>, lettuce_contracts::ApiError> {
        NetworkInstallSources.kokoro_model()
    }

    fn kokoro_voices(
        &self,
    ) -> Result<Box<dyn KokoroVoiceDownloadSource>, lettuce_contracts::ApiError> {
        NetworkInstallSources.kokoro_voices()
    }
}

fn runner(
    context: &ApiContext,
    bytes: &[u8],
    pace: Option<Duration>,
) -> (JobRunner, Arc<FakeSources>) {
    let sources = Arc::new(FakeSources {
        bytes: bytes.to_vec(),
        pace,
        opens: Mutex::new(0),
    });
    let runner = JobRunner::new(
        context.clone(),
        JobHandlers::new(vec![Arc::new(ArtifactInstallHandler::new(Arc::new(
            Sources(Arc::clone(&sources)),
        )))]),
    );
    (runner, sources)
}

fn temp_root(label: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("lettuce-api-{label}-{}", OperationId::new()));
    std::fs::create_dir_all(&root).expect("root");
    root
}

fn plan(root: &std::path::Path, install_id: &str, bytes: &[u8]) -> ArtifactInstallPlan {
    let source = ArtifactSource::HuggingFace {
        repository: "owner/repo".into(),
        revision: "a".repeat(40),
        path: format!("{install_id}.bin"),
    };
    ArtifactInstallPlan {
        install_id: install_id.into(),
        root: root.to_path_buf(),
        artifacts: vec![PlannedArtifact {
            artifact: PinnedArtifact {
                source_identity: source.identity(),
                local_segments: vec![format!("{install_id}.bin")],
                byte_size: bytes.len() as u64,
                sha256: Some(format!("{:x}", Sha256::digest(bytes))),
            },
            source,
        }],
    }
}

fn parse(accepted: &dto::JobAccepted) -> JobId {
    accepted.job_id.parse().expect("job id")
}

#[tokio::test(flavor = "multi_thread")]
async fn an_artifact_install_runs_to_success_and_its_watch_sees_the_end() {
    let harness = harness(Reply::Text("Hello."));
    let context = &harness.context;
    let root = temp_root("install");
    let bytes = b"model bytes".to_vec();
    let (runner, sources) = runner(context, &bytes, None);
    let mut feed = JobFeed::start(context).await.expect("feed");
    let accepted = admit_install(
        context,
        InstallWork::Artifact {
            plan: plan(&root, "upscaler", &bytes),
            finish: Box::new(InstallFinish::Files),
        },
    )
    .await
    .expect("admit");
    let job_id = parse(&accepted);
    let again = admit_install(
        context,
        InstallWork::Artifact {
            plan: plan(&root, "upscaler", &bytes),
            finish: Box::new(InstallFinish::Files),
        },
    )
    .await
    .expect("admit again");
    assert_eq!(again, accepted);
    let stream = Arc::new(RecordingJob::default());
    job_watch(
        context,
        dto::JobWatchRequest {
            job_id: accepted.job_id.clone(),
        },
        stream.clone(),
    )
    .await
    .expect("watch");

    assert!(runner.run_once().await.expect("run"));
    runner.wait_idle().await;
    assert_eq!(state(context, job_id), JobState::Succeeded);
    assert_eq!(
        std::fs::read(root.join("upscaler.bin")).expect("installed"),
        bytes
    );
    assert_eq!(*sources.opens.lock().expect("opens"), 1);
    feed.publish(context).await.expect("publish");
    let view = job_get(
        context,
        dto::JobGetRequest {
            job_id: accepted.job_id.clone(),
        },
    )
    .await
    .expect("job");
    assert_eq!(view.result, Some(dto::JobResultDto::ArtifactInstalled));
    assert_eq!(view.progress.label_code.as_deref(), Some("install"));
    assert!(matches!(
        stream.events().first(),
        Some(dto::JobEvent::Progress { job }) if job.state == dto::JobStateDto::Queued
    ));
    assert!(matches!(
        stream.events().last(),
        Some(dto::JobEvent::Completed { .. })
    ));
    assert_eq!(
        job_updates(&harness, job_id).last(),
        Some(&dto::JobStateDto::Succeeded)
    );
    assert!(!runner.run_once().await.expect("nothing left"));
    std::fs::remove_dir_all(root).ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failing_finisher_fails_the_install() {
    let harness = harness(Reply::Text("Hello."));
    let context = &harness.context;
    let root = temp_root("finisher");
    let bytes = b"bundle bytes".to_vec();
    let (runner, _) = runner(context, &bytes, None);
    let accepted = admit_install(
        context,
        InstallWork::Artifact {
            plan: plan(&root, "bundle", &bytes),
            finish: Box::new(InstallFinish::HuggingFaceBundle {
                paths: lettuce_image_generation::sd_runtime::layout::DiffusionPaths::legacy_layout(
                    &root,
                    root.join("image"),
                ),
                bundle_id: "missing".into(),
            }),
        },
    )
    .await
    .expect("admit");
    assert!(runner.run_once().await.expect("run"));
    runner.wait_idle().await;
    let view = job_get(
        context,
        dto::JobGetRequest {
            job_id: accepted.job_id,
        },
    )
    .await
    .expect("job");
    assert_eq!(view.state, dto::JobStateDto::Failed);
    assert!(view.failure.is_some());
    std::fs::remove_dir_all(root).ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn jobs_without_runnable_work_are_not_claimed() {
    let harness = harness(Reply::Text("Hello."));
    let context = &harness.context;
    let root = temp_root("unclaimed");
    let (runner, sources) = runner(context, b"bytes", None);
    let orphan = crate::ArtifactInstallCoordinator::new(context.backend().database())
        .admit(&plan(&root, "orphan", b"bytes"))
        .expect("admit")
        .job
        .id;
    assert!(!runner.run_once().await.expect("run"));
    assert_eq!(state(context, orphan), JobState::Queued);

    let accepted = admit_install(
        context,
        InstallWork::Artifact {
            plan: plan(&root, "cancelled", b"bytes"),
            finish: Box::new(InstallFinish::Files),
        },
    )
    .await
    .expect("admit");
    job_cancel(
        context,
        dto::JobCancelRequest {
            job_id: accepted.job_id.clone(),
        },
    )
    .await
    .expect("cancel");
    assert!(!runner.run_once().await.expect("run"));
    assert_eq!(state(context, parse(&accepted)), JobState::Cancelled);
    assert_eq!(*sources.opens.lock().expect("opens"), 0);

    let cancelled = recover_queued_installs(context).expect("recover");
    assert_eq!(cancelled, vec![orphan]);
    assert_eq!(state(context, orphan), JobState::Cancelled);
    std::fs::remove_dir_all(root).ok();
}

async fn until_running(context: &ApiContext, job_id: JobId) {
    for _ in 0..500 {
        if state(context, job_id) == JobState::Running
            && context
                .backend()
                .database()
                .get(job_id)
                .expect("job")
                .expect("job exists")
                .progress
                .bytes
                .is_some()
        {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("the install did not start");
}

async fn until_ended(context: &ApiContext, job_id: JobId) -> JobState {
    for _ in 0..500 {
        let current = state(context, job_id);
        if current.is_terminal() {
            return current;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("the install did not end");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_running_install_is_cancelled_by_the_user() {
    let harness = harness(Reply::Text("Hello."));
    let context = &harness.context;
    let root = temp_root("user-cancel");
    let bytes = vec![7_u8; 4096];
    let (runner, _) = runner(context, &bytes, Some(Duration::from_millis(5)));
    let accepted = admit_install(
        context,
        InstallWork::Artifact {
            plan: plan(&root, "slow", &bytes),
            finish: Box::new(InstallFinish::Files),
        },
    )
    .await
    .expect("admit");
    let job_id = parse(&accepted);
    assert!(runner.run_once().await.expect("run"));
    until_running(context, job_id).await;
    job_cancel(
        context,
        dto::JobCancelRequest {
            job_id: accepted.job_id,
        },
    )
    .await
    .expect("cancel");
    runner.wait_idle().await;
    assert_eq!(until_ended(context, job_id).await, JobState::Cancelled);
    std::fs::remove_dir_all(root).ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn shutdown_stops_the_runner_and_cancels_its_install() {
    let harness = harness(Reply::Text("Hello."));
    let context = harness.context.clone();
    let root = temp_root("shutdown");
    let bytes = vec![3_u8; 4096];
    let (runner, _) = runner(&context, &bytes, Some(Duration::from_millis(5)));
    let accepted = admit_install(
        &context,
        InstallWork::Artifact {
            plan: plan(&root, "slow", &bytes),
            finish: Box::new(InstallFinish::Files),
        },
    )
    .await
    .expect("admit");
    let job_id = parse(&accepted);
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let running = {
        let runner = runner.clone();
        tokio::spawn(async move {
            runner
                .run(async move {
                    let _ = stopped.await;
                })
                .await;
        })
    };
    until_running(&context, job_id).await;
    context.begin_shutdown();
    stop.send(()).expect("stop");
    tokio::time::timeout(Duration::from_secs(10), running)
        .await
        .expect("the runner stopped")
        .expect("runner task");
    assert_eq!(state(&context, job_id), JobState::Cancelled);
    std::fs::remove_dir_all(root).ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn startup_recovers_before_it_starts_the_workers() {
    let root = temp_root("startup");
    let harness = harness_in(
        Reply::Text("Hello."),
        Arc::new(SystemClock),
        None,
        Some(root.clone()),
        Arc::new(crate::api::NoModels),
    );
    let context = &harness.context;
    let orphan = crate::ArtifactInstallCoordinator::new(context.backend().database())
        .admit(&plan(&root, "orphan", b"bytes"))
        .expect("admit")
        .job
        .id;
    let workers = crate::api::startup(context).await.expect("startup");
    tokio::time::timeout(Duration::from_secs(30), workers.started())
        .await
        .expect("workers started");
    assert_eq!(
        workers.steps(),
        vec![
            crate::api::StartupStep::RecoverAfterRestart,
            crate::api::StartupStep::DetectLegacyDatabase,
            crate::api::StartupStep::ResumeMemoryJobs,
            crate::api::StartupStep::ResumeCompanionFollowUps,
            crate::api::StartupStep::RecoverQueuedInstalls,
            crate::api::StartupStep::AdoptLegacyEmbedding,
            crate::api::StartupStep::SweepOrphanMedia,
            crate::api::StartupStep::StartWorkers,
        ]
    );
    assert_eq!(state(context, orphan), JobState::Cancelled);
    let status = crate::api::app_status(context).await.expect("status");
    assert!(!status.legacy_database_detected);
    tokio::task::spawn_blocking(move || workers.stop())
        .await
        .expect("stopped");
    std::fs::remove_dir_all(root).ok();
}
