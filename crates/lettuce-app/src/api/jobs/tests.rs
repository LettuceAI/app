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
    fn emit(&self, event: dto::JobEvent) -> bool {
        self.0.lock().expect("job events").push(event);
        true
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
async fn embedding_install_admission_preserves_detail_and_conflicts_with_changed_setup() {
    let root = temp_root("embedding-admission");
    let harness = harness_in(Reply::Text("Hello."), Arc::new(SystemClock), None, Some(root.clone()), Arc::new(crate::api::NoModels));
    let pin = lettuce_model_hub::EmbeddingPin {
        family: lettuce_model_hub::EmbeddingModelFamily::LettuceEmbV4,
        revision: "a".repeat(40), files: Vec::new(),
    };
    let work = |enabled| InstallWork::Artifact {
        plan: plan(&root, "embedding", b"pinned bytes"),
        finish: Box::new(InstallFinish::Embedding { root: root.clone(), pin: pin.clone(), enable_dynamic_memory: enabled }),
    };
    let detail = |enabled| serde_json::to_value(super::local::LocalModelJobDetail::EmbeddingInstall {
        root: root.clone(), pin: pin.clone(), enable_dynamic_memory: enabled,
    }).expect("detail");
    let accepted = admit_install_with_detail(&harness.context, work(true), Some(detail(true))).await.expect("admission");
    assert_eq!(harness.context.backend().database().local_model_job(parse(&accepted)).expect("read").expect("detail").detail, detail(true));
    assert_eq!(admit_install_with_detail(&harness.context, work(true), Some(detail(true))).await.expect("replay"), accepted);
    assert_eq!(admit_install_with_detail(&harness.context, work(false), Some(detail(false))).await.expect_err("changed setup").code, ApiErrorCode::Conflict);
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
            crate::api::StartupStep::CompletePendingRewinds,
            crate::api::StartupStep::DetectLegacyDatabase,
            crate::api::StartupStep::AdoptLegacyEmbedding,
            crate::api::StartupStep::ResumeMemoryJobs,
            crate::api::StartupStep::ResumeCompanionFollowUps,
            crate::api::StartupStep::RecoverQueuedInstalls,
            crate::api::StartupStep::SweepOrphanMedia,
            crate::api::StartupStep::StartWorkers,
        ]
    );
    assert_eq!(state(context, orphan), JobState::Cancelled);
    until_updated(&harness, orphan, dto::JobStateDto::Cancelled).await;
    let status = crate::api::app_status(context).await.expect("status");
    assert!(!status.legacy_database_detected);
    workers.stop().await;
    std::fs::remove_dir_all(root).ok();
}

/// A download source that cannot be built.
struct BrokenSources;

#[async_trait]
impl InstallSources for BrokenSources {
    async fn artifacts(
        &self,
        _context: &ApiContext,
        _finish: &InstallFinish,
    ) -> Result<Box<dyn ArtifactSourceClient>, lettuce_contracts::ApiError> {
        Err(crate::api::error::api_error(
            ApiErrorCode::Unavailable,
            "the saved token cannot be read",
        ))
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

#[tokio::test(flavor = "multi_thread")]
async fn an_install_whose_source_cannot_be_built_fails_typed() {
    let harness = harness(Reply::Text("Hello."));
    let context = &harness.context;
    let root = temp_root("broken-source");
    let runner = JobRunner::new(
        context.clone(),
        JobHandlers::new(vec![Arc::new(ArtifactInstallHandler::new(Arc::new(
            BrokenSources,
        )))]),
    );
    let accepted = admit_install(
        context,
        InstallWork::Artifact {
            plan: plan(&root, "broken", b"bytes"),
            finish: Box::new(InstallFinish::Files),
        },
    )
    .await
    .expect("admit");
    assert!(!runner.run_once().await.expect("run"));
    let view = job_get(
        context,
        dto::JobGetRequest {
            job_id: accepted.job_id,
        },
    )
    .await
    .expect("job");
    assert_eq!(view.state, dto::JobStateDto::Failed);
    assert_eq!(
        view.failure
            .map(|failure| (failure.code, failure.retryable)),
        Some((dto::JobFailureCode::ResourceUnavailable, true))
    );
    assert!(!runner.run_once().await.expect("nothing left"));
    std::fs::remove_dir_all(root).ok();
}

/// Closed after its first event.
#[derive(Default)]
struct ClosingJob(Mutex<u32>);

impl JobEventSink for ClosingJob {
    fn emit(&self, _event: dto::JobEvent) -> bool {
        let mut count = self.0.lock().expect("count");
        *count += 1;
        *count < 2
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_closed_watch_is_dropped() {
    let harness = harness(Reply::Text("Hello."));
    let context = &harness.context;
    let job_id = create(&harness, spec(JobKind::Maintenance, "closing", "closing-1"));
    let sink = Arc::new(ClosingJob::default());
    job_watch(
        context,
        dto::JobWatchRequest {
            job_id: job_id.to_string(),
        },
        sink.clone(),
    )
    .await
    .expect("watch");
    assert!(context.jobs().watching(job_id));
    let view = job_get(
        context,
        dto::JobGetRequest {
            job_id: job_id.to_string(),
        },
    )
    .await
    .expect("job");
    context
        .jobs()
        .deliver(job_id, dto::JobEvent::Progress { job: view }, false);
    assert!(!context.jobs().watching(job_id));
    assert_eq!(*sink.0.lock().expect("count"), 2);
}

async fn until_updated(harness: &Harness, job_id: JobId, state: dto::JobStateDto) {
    for _ in 0..500 {
        if job_updates(harness, job_id).contains(&state) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("no {state:?} update for {job_id}");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_feed_publishes_only_after_a_committed_change() {
    let harness = harness(Reply::Text("Hello."));
    let context = harness.context.clone();
    let feed = JobFeed::start(&context).await.expect("feed");
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let running = tokio::spawn({
        let context = context.clone();
        async move {
            feed.run(context, async move {
                let _ = stopped.await;
            })
            .await;
        }
    });
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert!(api_events(&harness).is_empty());
    let job_id = create(&harness, spec(JobKind::Maintenance, "fed", "fed-1"));
    until_updated(&harness, job_id, dto::JobStateDto::Queued).await;
    job_cancel(
        &context,
        dto::JobCancelRequest {
            job_id: job_id.to_string(),
        },
    )
    .await
    .expect("cancel");
    until_updated(&harness, job_id, dto::JobStateDto::Cancelled).await;
    stop.send(()).expect("stop");
    running.await.expect("feed task");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_idle_runner_wakes_for_a_new_install() {
    let harness = harness(Reply::Text("Hello."));
    let context = harness.context.clone();
    let root = temp_root("idle-wake");
    let (runner, _) = runner(&context, b"bytes", None);
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
    tokio::time::sleep(Duration::from_millis(100)).await;
    let accepted = admit_install(
        &context,
        InstallWork::Artifact {
            plan: plan(&root, "later", b"bytes"),
            finish: Box::new(InstallFinish::Files),
        },
    )
    .await
    .expect("admit");
    assert_eq!(
        until_ended(&context, parse(&accepted)).await,
        JobState::Succeeded
    );
    stop.send(()).expect("stop");
    running.await.expect("runner task");
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn whisper_and_kokoro_share_the_download_queue() {
    let root = std::path::PathBuf::from("/models");
    let artifact = InstallWork::Artifact {
        plan: plan(&root, "gguf", b"bytes"),
        finish: Box::new(InstallFinish::Files),
    };
    let whisper = InstallWork::Whisper {
        model: lettuce_model_hub::RemoteWhisperModel {
            model_id: "base".into(),
            filename: "ggml-base.bin".into(),
            source_revision: "a".repeat(40),
            byte_size: 1,
            sha256: "0".repeat(64),
            english_only: false,
            quantized: false,
            recommended: false,
            recommended_for_mobile: false,
            recommended_for_desktop: false,
        },
        install_root: root,
    };
    assert_eq!(artifact.lane(), whisper.lane());
}

/// Claims fail as the database would while it is briefly unavailable.
struct UnavailableStorage;

#[async_trait]
impl JobHandler for UnavailableStorage {
    fn kinds(&self) -> &[JobKind] {
        &[JobKind::Maintenance]
    }

    fn lane(&self, _context: &ApiContext, _job: &JobSnapshot) -> Option<JobLane> {
        Some(JobLane("maintenance".into()))
    }

    async fn claim(
        &self,
        _context: &ApiContext,
        _job: &JobSnapshot,
        _worker_id: lettuce_jobs::WorkerId,
    ) -> Result<Option<Box<dyn ClaimedJob>>, lettuce_contracts::ApiError> {
        Err(crate::api::error::api_error(
            ApiErrorCode::Unavailable,
            "job storage failed",
        ))
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_transient_claim_failure_leaves_the_job_queued_for_a_retry() {
    let harness = harness(Reply::Text("Hello."));
    let context = &harness.context;
    let job_id = create(&harness, spec(JobKind::Maintenance, "retry", "retry-1"));
    let runner = JobRunner::new(
        context.clone(),
        JobHandlers::new(vec![Arc::new(UnavailableStorage)]),
    );
    let error = runner.run_once().await.expect_err("retried later");
    assert_eq!(error.code, ApiErrorCode::Unavailable);
    assert_eq!(state(context, job_id), JobState::Queued);
}

struct HealthModels { constant: bool }

#[async_trait]
impl crate::api::ModelLoader for HealthModels {
    fn installed(&self, _context: &ApiContext, model: dto::RequiredModel) -> bool { model == dto::RequiredModel::Embedding }
    async fn prepare(&self, _context: &ApiContext) -> bool { true }
    fn embedding(&self, _context: &ApiContext) -> crate::api::ModelLoad<Arc<dyn crate::MemoryEmbeddingEngine>> {
        crate::api::ModelLoad::Loaded(crate::api::embedding_health::tests::engine(self.constant))
    }
    fn emotion(&self, _context: &ApiContext) -> crate::api::ModelLoad<Arc<dyn crate::CompanionEmotionEngine>> { crate::api::ModelLoad::NotInstalled }
}

#[tokio::test(flavor = "multi_thread")]
async fn embedding_install_worker_checks_health_before_activating_dynamic_memory() {
    use lettuce_model_hub::{EmbeddingArtifactRole, EmbeddingFileDigest, EmbeddingModelFamily, EmbeddingPin, EmbeddingPinnedFile};
    use lettuce_settings::GlobalSettingsStore;
    let bytes = b"downloaded fixture bytes";
    for (bad_health, enable) in [(true, true), (false, true), (false, false)] {
        let folder = temp_root("embedding-health-install");
        let root = crate::embedding_models_root(&folder);
        let harness = harness_in(Reply::Text("Hello."), Arc::new(SystemClock), None, Some(folder), Arc::new(HealthModels { constant: bad_health }));
        let pin = EmbeddingPin { family: EmbeddingModelFamily::LettuceEmbV4, revision: "a".repeat(40),
            files: [(EmbeddingArtifactRole::Model, "model.onnx"), (EmbeddingArtifactRole::Tokenizer, "tokenizer.json")].into_iter().map(|(role, name)|
                EmbeddingPinnedFile { role, remote_path: name.into(), byte_size: bytes.len() as u64,
                    digest: EmbeddingFileDigest::Sha256(format!("{:x}", Sha256::digest(bytes))) }).collect() };
        let plan = ArtifactInstallPlan { install_id: "embedding-health".into(), root: root.clone(), artifacts: pin.files.iter().map(|file| {
            let source = ArtifactSource::HuggingFace { repository: "test/embedding".into(), revision: pin.revision.clone(), path: file.remote_path.clone() };
            PlannedArtifact { artifact: PinnedArtifact { source_identity: source.identity(), local_segments: pin.local_segments(file), byte_size: file.byte_size,
                sha256: Some(format!("{:x}", Sha256::digest(bytes))) }, source }
        }).collect() };
        let detail = serde_json::to_value(super::local::LocalModelJobDetail::EmbeddingInstall {
            root: root.clone(), pin: pin.clone(), enable_dynamic_memory: enable,
        }).expect("detail");
        let accepted = admit_install_with_detail(&harness.context, InstallWork::Artifact { plan,
            finish: Box::new(InstallFinish::Embedding { root: root.clone(), pin, enable_dynamic_memory: enable }) }, Some(detail)).await.expect("admit");
        let (runner, _) = runner(&harness.context, bytes, None);
        assert!(runner.run_once().await.expect("claim")); runner.wait_idle().await;
        assert_eq!(state(&harness.context, parse(&accepted)), if bad_health { JobState::Failed } else { JobState::Succeeded });
        let settings = GlobalSettingsStore::load(harness.context.backend().database()).expect("settings").settings;
        assert_eq!(settings.dynamic_memory.enabled, !bad_health && enable);
        if !bad_health && enable { assert_eq!(settings.dynamic_memory.min_similarity_basis_points, Some(3200)); }
        assert!(crate::EmbeddingModelCoordinator::new(&root, harness.context.backend().database()).active().expect("installed").is_some());
    }
}
