use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use lettuce_contracts::{
    self as dto, ApiErrorCode, ApiErrorDetails, ApiEvent, SpeechFailure, SpeechModelKind,
};
use lettuce_jobs::{
    FakeClock, JobState, JobStore, ResourceAvailability, WorkerId, handle::CancellationToken,
};
use lettuce_media::{AssetKind, AssetOrigin, AssetProvenanceV1, IngestRequest, RetentionClass};
use lettuce_settings::{SecretOwnerId, SecretValue};
use lettuce_speech::{
    AsrModelDescriptor, AsrModelId, AsrRuntime, AsrRuntimeError, AudioProvider,
    AudioProviderConfig, RuntimeSynthesis, RuntimeTranscription, SynthesisRequest,
    TranscriptionOptions, TranscriptionRequest, TtsOutputPolicy, TtsRuntime, TtsRuntimeError,
};
use lettuce_types::{
    AssetId, AudioProviderId, ContentHash, JobId, RequestId, Revision, TimestampMillis,
};

use super::{NoSpeech, SpeechHost};
use crate::api::tests::{Harness, Reply, api_events, harness_over, media_store};
use crate::api::{ApiContext, JobHandlers, JobRunner, NoModels, job_cancel, job_get};
use crate::{
    CaptureFormat, CaptureSession, CaptureSink, MicrophoneCapture, MicrophoneError,
    PreparedCapture, SPEECH_MAX_ATTEMPTS, speech_retry_delay,
};

const START: TimestampMillis = TimestampMillis::new(1_000_000);

fn wav(rate: u32, samples: &[i16]) -> Vec<u8> {
    let data = samples.len() * 2;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&u32::try_from(36 + data).expect("size").to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(rate * 2).to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&u32::try_from(data).expect("size").to_le_bytes());
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    bytes
}

fn tone() -> Vec<u8> {
    let samples = (0..16_000_i32)
        .map(|index| ((index % 100) * 200 - 10_000) as i16)
        .collect::<Vec<_>>();
    wav(16_000, &samples)
}

enum AsrMode {
    Text(&'static str),
    WaitForCancel,
}

struct FakeAsr {
    mode: AsrMode,
    calls: AtomicUsize,
}

impl AsrRuntime for FakeAsr {
    fn transcribe(
        &self,
        _: &AsrModelDescriptor,
        samples: &[f32],
        _: &str,
        _: &TranscriptionOptions,
        cancellation: &CancellationToken,
    ) -> Result<RuntimeTranscription, AsrRuntimeError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        assert!(!samples.is_empty());
        match self.mode {
            AsrMode::Text(text) => Ok(RuntimeTranscription {
                raw_text: text.to_owned(),
                detected_language: Some("en".to_owned()),
                segments: Vec::new(),
            }),
            AsrMode::WaitForCancel => loop {
                if cancellation.is_cancelled() {
                    return Err(AsrRuntimeError::Cancelled);
                }
                std::thread::sleep(Duration::from_millis(5));
            },
        }
    }
}

enum TtsMode {
    Fail(TtsRuntimeError),
    Speak,
    WaitForCancel,
}

struct FakeTts {
    mode: TtsMode,
    calls: AtomicUsize,
}

#[async_trait]
impl TtsRuntime for FakeTts {
    async fn synthesize(
        &self,
        _: &SynthesisRequest,
        _: Option<&SecretValue>,
        cancellation: &CancellationToken,
    ) -> Result<RuntimeSynthesis, TtsRuntimeError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match &self.mode {
            TtsMode::Fail(error) => Err(*error),
            TtsMode::Speak => Ok(RuntimeSynthesis {
                bytes: tone(),
                declared_mime_type: "audio/wav".to_owned(),
            }),
            TtsMode::WaitForCancel => {
                cancellation.cancelled().await;
                Err(TtsRuntimeError::Cancelled)
            }
        }
    }
}

struct FakeMic {
    outcome: Result<Vec<f32>, MicrophoneError>,
}

struct FakePrepared(Vec<f32>);

struct FakeSession;

impl MicrophoneCapture for FakeMic {
    fn prepare(&self) -> Result<Box<dyn PreparedCapture>, MicrophoneError> {
        self.outcome
            .clone()
            .map(|samples| Box::new(FakePrepared(samples)) as Box<dyn PreparedCapture>)
    }
}

impl PreparedCapture for FakePrepared {
    fn format(&self) -> CaptureFormat {
        CaptureFormat {
            sample_rate_hz: 16_000,
            channels: 1,
        }
    }

    fn start(
        self: Box<Self>,
        sink: Arc<dyn CaptureSink>,
    ) -> Result<Box<dyn CaptureSession>, MicrophoneError> {
        if !self.0.is_empty() {
            let half = self.0.len() / 2;
            sink.push(&self.0[..half]);
            sink.push(&self.0[half..]);
        }
        Ok(Box::new(FakeSession))
    }
}

impl CaptureSession for FakeSession {
    fn stop(self: Box<Self>) -> Result<(), MicrophoneError> {
        Ok(())
    }
}

struct FakeSpeech {
    tts: Arc<FakeTts>,
    asr: Arc<FakeAsr>,
    microphone: Option<Arc<dyn MicrophoneCapture>>,
}

impl SpeechHost for FakeSpeech {
    fn tts_runtime(&self, _: &ApiContext) -> Result<Arc<dyn TtsRuntime>, lettuce_contracts::ApiError> {
        Ok(self.tts.clone())
    }

    fn microphone(&self) -> Option<Arc<dyn MicrophoneCapture>> {
        self.microphone.clone()
    }

    fn asr_runtime(&self, _: &ApiContext) -> Arc<dyn AsrRuntime> {
        self.asr.clone()
    }
}

struct Env {
    harness: Harness,
    context: ApiContext,
    clock: FakeClock,
    root: PathBuf,
    tts: Arc<FakeTts>,
    asr: Arc<FakeAsr>,
}

impl Drop for Env {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).ok();
    }
}

fn env(tts: TtsMode, asr: AsrMode, microphone: Option<Arc<dyn MicrophoneCapture>>) -> Env {
    env_with_reply(tts, asr, microphone, Reply::Text(""))
}

fn env_with_reply(tts: TtsMode, asr: AsrMode, microphone: Option<Arc<dyn MicrophoneCapture>>, reply: Reply) -> Env {
    let root = std::env::temp_dir().join(format!("lettuce-api-speech-{}", RequestId::new()));
    std::fs::create_dir_all(&root).expect("root");
    let clock = FakeClock::new(lettuce_jobs::Clock::now(&lettuce_jobs::SystemClock));
    let backend = Arc::new(crate::AppBackend::open(root.join("media.sqlite3"), START).expect("backend"));
    let harness = harness_over(
        backend,
        reply,
        Arc::new(clock.clone()),
        Some(media_store(&root)),
        Some(root.clone()),
        Arc::new(NoModels),
        Arc::new(crate::api::tests::NoImages),
    );
    let tts = Arc::new(FakeTts {
        mode: tts,
        calls: AtomicUsize::new(0),
    });
    let asr = Arc::new(FakeAsr {
        mode: asr,
        calls: AtomicUsize::new(0),
    });
    let context = harness.context.with_speech(Arc::new(FakeSpeech {
        tts: tts.clone(),
        asr: asr.clone(),
        microphone,
    }));
    Env {
        harness,
        context,
        clock,
        root,
        tts,
        asr,
    }
}

fn install_whisper_file(root: &Path) {
    let folder = root.join("models").join("whisper").join("base");
    std::fs::create_dir_all(&folder).expect("model folder");
    std::fs::write(folder.join("ggml-base.bin"), b"a whisper model").expect("model file");
}

fn ingest_wav(context: &ApiContext, bytes: Vec<u8>) -> AssetId {
    context
        .media()
        .expect("media")
        .ingest(
            Cursor::new(bytes),
            IngestRequest::new(
                AssetKind::OtherAudio,
                AssetOrigin::Upload,
                RetentionClass::Persistent,
                AssetProvenanceV1::default(),
            ),
        )
        .expect("ingested")
        .asset
        .id
}

fn descriptor(id: &str) -> AsrModelDescriptor {
    AsrModelDescriptor {
        id: AsrModelId::new(id).expect("model id"),
        artifact_hash: ContentHash::parse("0".repeat(64)).expect("hash"),
        english_only: false,
    }
}

fn transcription_request(context: &ApiContext, id: &str) -> TranscriptionRequest {
    TranscriptionRequest {
        id: RequestId::new(),
        audio_asset_id: ingest_wav(context, tone()),
        model: descriptor(id),
        options: TranscriptionOptions::default(),
        created_at: START,
    }
}

fn synthesis_request(text: &str) -> SynthesisRequest {
    SynthesisRequest {
        id: RequestId::new(),
        provider: AudioProvider {
            id: AudioProviderId::new(),
            secret_owner_id: SecretOwnerId::new(),
            label: "Fish Speech".to_owned(),
            api_key_ref: None,
            config: AudioProviderConfig::FishSpeech {
                base_url: None,
                request_path: None,
            },
            revision: Revision::INITIAL,
            created_at: START,
            updated_at: START,
        },
        model_id: "speech".to_owned(),
        voice_id: "reference".to_owned(),
        prompt: None,
        text: text.to_owned(),
        output_asset_id: AssetId::new(),
        output_policy: TtsOutputPolicy::Retained,
        created_at: START,
    }
}

fn runner(context: &ApiContext) -> JobRunner {
    JobRunner::new(context.clone(), JobHandlers::standard())
}

async fn job(context: &ApiContext, job_id: JobId) -> dto::JobView {
    job_get(
        context,
        dto::JobGetRequest {
            job_id: job_id.to_string(),
        },
    )
    .await
    .expect("job view")
}

async fn run_to_idle(runner: &JobRunner) -> bool {
    let started = runner.run_once().await.expect("run once");
    runner.wait_idle().await;
    started
}

#[tokio::test(flavor = "multi_thread")]
async fn a_missing_whisper_model_fails_terminally_with_a_typed_failure() {
    let env = env(TtsMode::Speak, AsrMode::Text("unused"), None);
    let context = env.harness.context.clone();
    let admitted = context
        .backend()
        .speech_transcriptions()
        .admit(transcription_request(&context, "base"))
        .expect("admitted");
    let runner = runner(&context);

    assert!(run_to_idle(&runner).await);
    let view = job(&context, admitted.job.id).await;
    assert_eq!(view.state, dto::JobStateDto::Failed);
    let failure = view.failure.expect("failure");
    assert!(!failure.retryable);
    assert_eq!(
        failure.speech,
        Some(SpeechFailure::ModelRequired {
            model: SpeechModelKind::Whisper
        })
    );
    assert!(!run_to_idle(&runner).await, "the job was not requeued");
    let stored = context
        .backend()
        .database()
        .get(admitted.job.id)
        .expect("get")
        .expect("job");
    assert_eq!(stored.attempt.get(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_transient_provider_failure_retries_with_backoff_and_stops_at_the_cap() {
    let env = env(
        TtsMode::Fail(TtsRuntimeError::Unavailable),
        AsrMode::Text("unused"),
        None,
    );
    let context = env.context.clone();
    let admitted = context
        .backend()
        .tts_syntheses()
        .admit(synthesis_request("Hello."))
        .expect("admitted");
    env.clock.set(admitted.job.updated_at);
    let runner = runner(&context);

    for attempt in 1..SPEECH_MAX_ATTEMPTS {
        assert!(run_to_idle(&runner).await, "attempt {attempt} runs");
        let view = job(&context, admitted.job.id).await;
        assert_eq!(view.state, dto::JobStateDto::Queued, "attempt {attempt}");
        assert!(
            !run_to_idle(&runner).await,
            "the retry waits for its backoff after attempt {attempt}"
        );
        env.clock.advance(speech_retry_delay(attempt));
    }
    assert!(run_to_idle(&runner).await);
    let view = job(&context, admitted.job.id).await;
    assert_eq!(view.state, dto::JobStateDto::Failed);
    let failure = view.failure.expect("failure");
    assert!(!failure.retryable);
    assert_eq!(failure.speech, Some(SpeechFailure::RetriesExhausted));
    assert_eq!(env.tts.calls.load(Ordering::SeqCst), SPEECH_MAX_ATTEMPTS as usize);
}

#[tokio::test(flavor = "multi_thread")]
async fn terminal_synthesis_failures_never_requeue() {
    for (error, expected) in [
        (TtsRuntimeError::VoiceMissing, SpeechFailure::VoiceMissing),
        (
            TtsRuntimeError::ModelMissing,
            SpeechFailure::ModelRequired {
                model: SpeechModelKind::Kokoro,
            },
        ),
        (
            TtsRuntimeError::EspeakMissing,
            SpeechFailure::RuntimeMissing {
                runtime: dto::SpeechRuntimeKind::Espeak,
            },
        ),
    ] {
        let env = env(TtsMode::Fail(error), AsrMode::Text("unused"), None);
        let context = env.context.clone();
        let admitted = context
            .backend()
            .tts_syntheses()
            .admit(synthesis_request("Hello."))
            .expect("admitted");
        let runner = runner(&context);
        assert!(run_to_idle(&runner).await);
        let view = job(&context, admitted.job.id).await;
        assert_eq!(view.state, dto::JobStateDto::Failed, "{error:?}");
        assert_eq!(view.failure.expect("failure").speech, Some(expected));
        assert_eq!(env.tts.calls.load(Ordering::SeqCst), 1);
        assert!(!run_to_idle(&runner).await);
    }
}

#[tokio::test(start_paused = true)]
async fn a_transcription_past_thirty_minutes_remains_running_and_cancels() {
    let env = env(TtsMode::WaitForCancel, AsrMode::WaitForCancel, None);
    let context = env.context.clone();
    let transcription = context.backend().speech_transcriptions()
        .admit(transcription_request(&context, "base")).expect("admitted");
    let runner = runner(&context);
    assert!(runner.run_once().await.expect("run"));
    while env.asr.calls.load(Ordering::SeqCst) == 0 {
        tokio::task::yield_now().await;
    }
    for _ in 0..31 {
        tokio::time::advance(Duration::from_secs(60)).await;
        tokio::task::yield_now().await;
    }
    assert_eq!(job(&context, transcription.job.id).await.state, dto::JobStateDto::Running);
    job_cancel(&context, dto::JobCancelRequest {
        job_id: transcription.job.id.to_string(),
    }).await.expect("cancel");
    tokio::time::timeout(Duration::from_secs(1), runner.wait_idle()).await
        .expect("cancellation is prompt");
    assert_eq!(job(&context, transcription.job.id).await.state, dto::JobStateDto::Cancelled);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_running_local_transcription_and_synthesis_stop_when_cancelled() {
    let env = env(TtsMode::WaitForCancel, AsrMode::WaitForCancel, None);
    let context = env.context.clone();
    let transcription = context
        .backend()
        .speech_transcriptions()
        .admit(transcription_request(&context, "base"))
        .expect("admitted transcription");
    let synthesis = context
        .backend()
        .tts_syntheses()
        .admit(synthesis_request("Hello."))
        .expect("admitted synthesis");
    let runner = runner(&context);
    assert!(runner.run_once().await.expect("run"));
    for _ in 0..200 {
        if env.asr.calls.load(Ordering::SeqCst) == 1 && env.tts.calls.load(Ordering::SeqCst) == 1 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(env.asr.calls.load(Ordering::SeqCst), 1);
    assert_eq!(env.tts.calls.load(Ordering::SeqCst), 1);

    for job_id in [transcription.job.id, synthesis.job.id] {
        job_cancel(
            &context,
            dto::JobCancelRequest {
                job_id: job_id.to_string(),
            },
        )
        .await
        .expect("cancel");
    }
    runner.wait_idle().await;
    for job_id in [transcription.job.id, synthesis.job.id] {
        assert_eq!(
            job(&context, job_id).await.state,
            dto::JobStateDto::Cancelled
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn local_transcriptions_share_a_lane_and_remote_syntheses_do_not() {
    let env = env(TtsMode::WaitForCancel, AsrMode::WaitForCancel, None);
    let context = env.context.clone();
    let first = context
        .backend()
        .speech_transcriptions()
        .admit(transcription_request(&context, "base"))
        .expect("first");
    let second = context
        .backend()
        .speech_transcriptions()
        .admit(transcription_request(&context, "base"))
        .expect("second");
    let remote = [
        context
            .backend()
            .tts_syntheses()
            .admit(synthesis_request("One."))
            .expect("remote one"),
        context
            .backend()
            .tts_syntheses()
            .admit(synthesis_request("Two."))
            .expect("remote two"),
    ];
    let runner = runner(&context);
    assert!(runner.run_once().await.expect("run"));
    for _ in 0..200 {
        if env.asr.calls.load(Ordering::SeqCst) == 1 && env.tts.calls.load(Ordering::SeqCst) == 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(env.asr.calls.load(Ordering::SeqCst), 1, "one transcription at a time");
    assert_eq!(env.tts.calls.load(Ordering::SeqCst), 2, "remote syntheses run together");
    let running = [first.job.id, second.job.id]
        .into_iter()
        .filter(|id| {
            context
                .backend()
                .database()
                .get(*id)
                .expect("get")
                .expect("job")
                .state
                == JobState::Running
        })
        .count();
    assert_eq!(running, 1);
    for job_id in [first.job.id, second.job.id, remote[0].job.id, remote[1].job.id] {
        job_cancel(
            &context,
            dto::JobCancelRequest {
                job_id: job_id.to_string(),
            },
        )
        .await
        .expect("cancel");
    }
    runner.wait_idle().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn queued_and_running_speech_jobs_are_cancelled_by_a_restart() {
    let env = env(TtsMode::Speak, AsrMode::Text("unused"), None);
    let context = env.context.clone();
    let queued = context
        .backend()
        .speech_transcriptions()
        .admit(transcription_request(&context, "base"))
        .expect("queued");
    let running = context
        .backend()
        .tts_syntheses()
        .admit(synthesis_request("Hello."))
        .expect("running");
    context
        .backend()
        .tts_syntheses()
        .claim(
            running.job.id,
            WorkerId::new(),
            START,
            Duration::from_secs(30),
            &ResourceAvailability::all(),
        )
        .expect("claim")
        .expect("work");

    let restarted = context.restarted();
    restarted.recover_after_restart().expect("recovery");

    for job_id in [queued.job.id, running.job.id] {
        assert_eq!(
            context
                .backend()
                .database()
                .get(job_id)
                .expect("get")
                .expect("job")
                .state,
            JobState::Cancelled
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_replayed_transcription_returns_its_job_and_another_request_conflicts() {
    let env = env(TtsMode::Speak, AsrMode::Text("unused"), None);
    install_whisper_file(&env.root);
    let audio = env.root.join("sample.wav");
    std::fs::write(&audio, tone()).expect("sample");
    let request = dto::TranscribeFileRequest {
        request_id: RequestId::new().to_string(),
        source: dto::FileSource {
            uri: audio.to_string_lossy().into_owned(),
        },
        model_id: None,
        options: dto::TranscribeOptions::default(),
    };

    let first = super::transcribe_file(&env.context, request.clone())
        .await
        .expect("first");
    env.clock.advance(Duration::from_secs(5));
    let again = super::transcribe_file(&env.context, request.clone())
        .await
        .expect("replay");
    assert_eq!(first, again);

    let mut changed_source = request.clone();
    changed_source.source.uri = env.root.join("another.wav").to_string_lossy().into_owned();
    let source_error = super::transcribe_file(&env.context, changed_source)
        .await
        .expect_err("a different source conflicts before reading it");
    assert_eq!(source_error.code, ApiErrorCode::Conflict);

    let mut changed = request;
    changed.options.language = Some("fr".to_owned());
    let error = super::transcribe_file(&env.context, changed)
        .await
        .expect_err("another request under the same id");
    assert_eq!(error.code, ApiErrorCode::Conflict);
}

#[tokio::test(flavor = "multi_thread")]
async fn dictation_records_to_an_asset_and_transcribes_without_audio_over_the_api() {
    let samples = (0..8_000)
        .map(|index| ((index % 50) as f32 - 25.0) / 100.0)
        .collect::<Vec<_>>();
    let microphone = Arc::new(FakeMic {
        outcome: Ok(samples),
    });
    let env = env(
        TtsMode::Speak,
        AsrMode::Text("hello there"),
        Some(microphone),
    );
    install_whisper_file(&env.root);

    let started = super::dictation_start(
        &env.context,
        dto::DictationStartRequest {
            conversation_id: None,
        },
    )
    .await
    .expect("start");
    for _ in 0..200 {
        let seen = api_events(&env.harness).into_iter().any(|event| {
            matches!(&event, ApiEvent::DictationLevel { capture_id, level }
                if *capture_id == started.capture_id && *level > 0)
        });
        if seen {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(api_events(&env.harness).into_iter().any(|event| matches!(
        event,
        ApiEvent::DictationLevel { capture_id, level } if capture_id == started.capture_id && level > 0
    )));
    let second = super::dictation_start(
        &env.context,
        dto::DictationStartRequest {
            conversation_id: None,
        },
    )
    .await
    .expect_err("one capture at a time");
    assert_eq!(second.code, ApiErrorCode::Conflict);

    let accepted = super::dictation_stop(
        &env.context,
        dto::DictationStopRequest {
            capture_id: started.capture_id.clone(),
            model_id: None,
            options: dto::TranscribeOptions::default(),
        },
    )
    .await
    .expect("stop");
    let queued = job(&env.context, accepted.job_id.parse().expect("id")).await;
    assert_eq!(queued.kind, dto::JobKindDto::SpeechTranscribe);
    assert_eq!(queued.state, dto::JobStateDto::Queued);
    let scratch = crate::dictation_scratch_root(&env.root);
    assert_eq!(
        std::fs::read_dir(&scratch).expect("scratch").count(),
        0,
        "the scratch recording is gone once it is an asset"
    );

    let runner = runner(&env.context);
    assert!(run_to_idle(&runner).await);
    let done = job(&env.context, accepted.job_id.parse().expect("id")).await;
    assert_eq!(done.state, dto::JobStateDto::Succeeded);
    let Some(dto::JobResultDto::Transcription { transcription }) = done.result else {
        panic!("a transcription result");
    };
    assert_eq!(transcription.text, "hello there");
    assert!(transcription.audio.url.contains(&transcription.audio.asset_id));

    let ended = super::dictation_stop(
        &env.context,
        dto::DictationStopRequest {
            capture_id: started.capture_id,
            model_id: None,
            options: dto::TranscribeOptions::default(),
        },
    )
    .await
    .expect_err("already stopped");
    assert_eq!(ended.code, ApiErrorCode::Conflict);
    let unknown = super::dictation_stop(
        &env.context,
        dto::DictationStopRequest {
            capture_id: RequestId::new().to_string(),
            model_id: None,
            options: dto::TranscribeOptions::default(),
        },
    )
    .await
    .expect_err("unknown capture");
    assert_eq!(unknown.code, ApiErrorCode::NotFound);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_denied_microphone_permission_is_typed_and_needs_a_model_first() {
    let microphone = Arc::new(FakeMic {
        outcome: Err(MicrophoneError::PermissionDenied),
    });
    let env = env(TtsMode::Speak, AsrMode::Text("unused"), Some(microphone));
    let request = dto::DictationStartRequest {
        conversation_id: None,
    };

    let no_model = super::dictation_start(&env.context, request.clone())
        .await
        .expect_err("no model");
    assert_eq!(no_model.code, ApiErrorCode::ModelRequired);
    assert_eq!(
        no_model.details,
        Some(ApiErrorDetails::Speech {
            failure: SpeechFailure::ModelRequired {
                model: SpeechModelKind::Whisper
            }
        })
    );

    install_whisper_file(&env.root);
    env.context.speech_state().legacy_whisper_admitted().store(false, Ordering::Release);
    let denied = super::dictation_start(&env.context, request.clone())
        .await
        .expect_err("permission denied");
    assert_eq!(denied.code, ApiErrorCode::Unavailable);
    assert_eq!(
        denied.details,
        Some(ApiErrorDetails::Speech {
            failure: SpeechFailure::MicrophonePermissionDenied
        })
    );
    let again = super::dictation_start(&env.context, request)
        .await
        .expect_err("still denied, not stuck starting");
    assert_eq!(again.code, ApiErrorCode::Unavailable);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_cancelled_dictation_leaves_no_recording_and_cannot_be_stopped() {
    let microphone = Arc::new(FakeMic {
        outcome: Ok(vec![0.2; 1_600]),
    });
    let env = env(TtsMode::Speak, AsrMode::Text("unused"), Some(microphone));
    install_whisper_file(&env.root);
    let started = super::dictation_start(
        &env.context,
        dto::DictationStartRequest {
            conversation_id: None,
        },
    )
    .await
    .expect("start");
    let scratch = crate::dictation_scratch_root(&env.root);
    assert_eq!(std::fs::read_dir(&scratch).expect("scratch").count(), 1);

    super::dictation_cancel(
        &env.context,
        dto::DictationCancelRequest {
            capture_id: started.capture_id.clone(),
        },
    )
    .await
    .expect("cancel");
    assert_eq!(std::fs::read_dir(&scratch).expect("scratch").count(), 0);
    let stop = super::dictation_stop(
        &env.context,
        dto::DictationStopRequest {
            capture_id: started.capture_id.clone(),
            model_id: None,
            options: dto::TranscribeOptions::default(),
        },
    )
    .await
    .expect_err("cancelled");
    assert_eq!(stop.code, ApiErrorCode::Conflict);
    let cancel = super::dictation_cancel(
        &env.context,
        dto::DictationCancelRequest {
            capture_id: started.capture_id,
        },
    )
    .await
    .expect_err("cancelled twice");
    assert_eq!(cancel.code, ApiErrorCode::Conflict);
    let none = super::dictation_cancel(
        &env.context,
        dto::DictationCancelRequest {
            capture_id: RequestId::new().to_string(),
        },
    )
    .await
    .expect_err("unknown");
    assert_eq!(none.code, ApiErrorCode::NotFound);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_recording_with_no_samples_is_refused() {
    let microphone = Arc::new(FakeMic {
        outcome: Ok(Vec::new()),
    });
    let env = env(TtsMode::Speak, AsrMode::Text("unused"), Some(microphone));
    install_whisper_file(&env.root);
    let started = super::dictation_start(
        &env.context,
        dto::DictationStartRequest {
            conversation_id: None,
        },
    )
    .await
    .expect("start");
    let error = super::dictation_stop(
        &env.context,
        dto::DictationStopRequest {
            capture_id: started.capture_id,
            model_id: None,
            options: dto::TranscribeOptions::default(),
        },
    )
    .await
    .expect_err("empty recording");
    assert_eq!(error.code, ApiErrorCode::InvalidInput);
    assert_eq!(
        error.details,
        Some(ApiErrorDetails::Speech {
            failure: SpeechFailure::NoAudioCaptured
        })
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn startup_removes_recordings_a_stopped_process_left_behind() {
    let env = env(TtsMode::Speak, AsrMode::Text("unused"), None);
    let scratch = crate::dictation_scratch_root(&env.root);
    std::fs::create_dir_all(&scratch).expect("scratch");
    std::fs::write(scratch.join("left.wav"), b"half a recording").expect("leftover");
    env.context.recover_after_restart().expect("recovery");
    assert_eq!(std::fs::read_dir(&scratch).expect("scratch").count(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_default_speech_host_has_no_microphone_and_no_runtime() {
    let env = env(TtsMode::Speak, AsrMode::Text("unused"), None);
    install_whisper_file(&env.root);
    let context = env.harness.context.with_speech(Arc::new(NoSpeech));
    let error = super::dictation_start(
        &context,
        dto::DictationStartRequest {
            conversation_id: None,
        },
    )
    .await
    .expect_err("no microphone");
    assert_eq!(error.code, ApiErrorCode::Unsupported);
}

#[tokio::test(flavor = "multi_thread")]
async fn local_runtime_unavailability_and_missing_credentials_are_terminal() {
    for local in [true, false] {
        let env = env(TtsMode::Fail(TtsRuntimeError::Unavailable), AsrMode::Text("unused"), None);
        let mut request = synthesis_request("Hello.");
        request.provider.config = if local {
            AudioProviderConfig::Kokoro { variant: Some("int8".into()) }
        } else {
            AudioProviderConfig::Elevenlabs
        };
        if !local {
            request.provider.api_key_ref = Some(lettuce_settings::SecretRef::new());
        }
        let admitted = env.context.backend().tts_syntheses().admit(request).expect("admitted");
        let runner = runner(&env.context);
        assert!(run_to_idle(&runner).await);
        let view = job(&env.context, admitted.job.id).await;
        assert_eq!(view.state, dto::JobStateDto::Failed);
        assert!(!view.failure.expect("failure").retryable);
        assert!(!run_to_idle(&runner).await);
        assert_eq!(env.tts.calls.load(Ordering::SeqCst), usize::from(local));
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn synthesis_admission_replays_and_conflicts_even_after_provider_deletion() {
    use lettuce_speech::TtsConfigurationRepository;
    let env = env(TtsMode::Speak, AsrMode::Text("unused"), None);
    let provider = synthesis_request("Unused").provider;
    env.context.backend().database().upsert_audio_provider(provider.clone(), None).expect("provider");
    let request = dto::TtsSynthesizeRequest {
        request_id: RequestId::new().to_string(),
        provider_id: provider.id.to_string(),
        model_id: "speech".into(), voice_id: "reference".into(),
        prompt: None, text: "Hello.".into(), retained: false,
    };
    let accepted = super::tts_synthesize(&env.context, request.clone()).await.expect("accepted");
    let runner = runner(&env.context);
    assert!(run_to_idle(&runner).await);
    let completed = job(&env.context, accepted.job_id.parse().expect("job id")).await;
    assert_eq!(completed.state, dto::JobStateDto::Succeeded);
    assert!(matches!(completed.result, Some(dto::JobResultDto::Asset { asset }) if asset.url.starts_with("test-asset://host/")));
    env.context.backend().database().delete_audio_provider(provider.id, provider.revision).expect("delete provider");
    assert_eq!(super::tts_synthesize(&env.context, request.clone()).await.expect("replay"), accepted);
    let mut changed = request;
    changed.text = "Different text.".into();
    assert_eq!(super::tts_synthesize(&env.context, changed).await.expect_err("conflict").code, ApiErrorCode::Conflict);
    assert_eq!(env.tts.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_local_provider_draft_verifies_without_saving_its_configuration() {
    use lettuce_speech::TtsConfigurationRepository;
    let env = env(TtsMode::Speak, AsrMode::Text("unused"), None);
    let draft = dto::AudioProviderDraft {
        configuration: dto::AudioProviderConfiguration::Kokoro { variant: None },
        api_key: None,
    };
    assert!(super::audio_provider_verify(&env.context, dto::AudioProviderVerifyRequest::Draft { draft }).await.expect("verified"));
    assert!(env.context.backend().database().list_audio_providers().expect("providers").is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn shutdown_discards_an_active_capture_and_refuses_new_recordings() {
    let env = env(TtsMode::Speak, AsrMode::Text("unused"), Some(Arc::new(FakeMic { outcome: Ok(vec![0.25; 320]) })));
    install_whisper_file(&env.root);
    let started = super::dictation_start(&env.context, dto::DictationStartRequest { conversation_id: None }).await.expect("started");
    env.context.begin_shutdown();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if std::fs::read_dir(crate::dictation_scratch_root(&env.root)).expect("scratch").next().is_none() { break; }
            tokio::task::yield_now().await;
        }
    }).await.expect("capture is discarded on shutdown");
    assert!(std::fs::read_dir(crate::dictation_scratch_root(&env.root)).expect("scratch").next().is_none());
    assert_eq!(super::dictation_cancel(&env.context, dto::DictationCancelRequest { capture_id: started.capture_id }).await.expect_err("ended").code, ApiErrorCode::Conflict);
    assert_eq!(super::dictation_start(&env.context, dto::DictationStartRequest { conversation_id: None }).await.expect_err("shutdown").code, ApiErrorCode::Unavailable);
}

#[tokio::test(flavor = "multi_thread")]
async fn kokoro_api_inventories_blends_removes_and_reports_missing_dependencies() {
    let folder = std::env::temp_dir().join(format!("kokoro-api-{}", lettuce_types::OperationId::new()));
    let harness = crate::api::tests::harness_in(Reply::Text("Hello."), Arc::new(lettuce_jobs::SystemClock), None, Some(folder.clone()), Arc::new(NoModels));
    let root = crate::kokoro_root(&folder);
    let inventory = crate::api::kokoro_inventory(&harness.context, dto::KokoroInventoryRequest { variant: "int8".into(), selected_voice_id: None }).await.expect("inventory");
    assert!(inventory.model.is_none());
    assert!(crate::api::kokoro_inventory(&harness.context, dto::KokoroInventoryRequest { variant: "wrong".into(), selected_voice_id: None }).await.expect_err("invalid variant").code == ApiErrorCode::InvalidInput);
    let bytes = std::iter::repeat_n(0.5_f32, lettuce_speech::KOKORO_STYLE_DIMENSIONS).flat_map(f32::to_le_bytes).collect::<Vec<_>>();
    use sha2::{Digest, Sha256};
    let remote = lettuce_model_hub::RemoteKokoroVoice::pinned("af_heart", "ef".repeat(20), u64::try_from(bytes.len()).expect("size"), format!("{:x}", Sha256::digest(&bytes))).expect("voice pin");
    let store = lettuce_model_hub::KokoroVoiceInstallStore::open(&root).expect("store");
    let lettuce_model_hub::KokoroVoicePreparation::Download(mut download) = store.prepare(remote).expect("prepare") else { panic!("new download"); };
    download.append(&bytes).expect("bytes"); download.finish().expect("finish");
    assert_eq!(crate::api::kokoro_voices_installed(&harness.context).await.expect("installed")[0].id, "af_heart");
    use lettuce_speech::TtsConfigurationRepository;
    let mut provider = synthesis_request("kokoro voices").provider;
    provider.config = AudioProviderConfig::Kokoro { variant: Some("int8".into()) };
    harness.context.backend().database().upsert_audio_provider(provider.clone(), None).expect("provider");
    let provider_request = || dto::AudioProviderRequest { provider_id: provider.id.to_string() };
    let listed = super::audio_provider_voices(&harness.context, provider_request()).await.expect("provider voices");
    assert_eq!(listed.len(), 1); assert_eq!(listed[0].voice_id, "af_heart");
    assert_eq!(listed[0].labels.get("engine").map(String::as_str), Some("kokoro"));
    assert_eq!(listed[0].labels.get("category").map(String::as_str), Some("library"));
    assert_eq!(super::audio_provider_voices_refresh(&harness.context, provider_request()).await.expect("refresh local"), listed);
    let blend = crate::api::kokoro_blend(&harness.context, dto::KokoroBlendRequest { voices: vec![dto::KokoroVoiceBlendInput { voice_id: "af_heart".into(), weight: 25.0 }, dto::KokoroVoiceBlendInput { voice_id: "af_heart".into(), weight: 75.0 }] }).await.expect("blend");
    assert_eq!(blend.voices.len(), 1); assert_eq!(blend.voices[0].weight, 1.0); assert_eq!(blend.style_rows, 1);
    let error = crate::api::kokoro_phonemize(&harness.context, dto::KokoroPhonemizeRequest { variant: "int8".into(), voice_id: "af_heart".into(), text: "Hello.".into() }).await.expect_err("model missing");
    assert_eq!(error.code, ApiErrorCode::ModelRequired);
    assert!(matches!(error.details, Some(ApiErrorDetails::Speech { failure: SpeechFailure::ModelRequired { model: SpeechModelKind::Kokoro } })));
    let voice = || dto::KokoroVoiceRequest { voice_id: "af_heart".into() };
    assert!(crate::api::kokoro_uninstall_voice(&harness.context, voice()).await.expect("remove"));
    assert!(!crate::api::kokoro_uninstall_voice(&harness.context, voice()).await.expect("repeat removal"));
    std::fs::remove_dir_all(folder).expect("cleanup");
}

#[tokio::test]
async fn provider_and_user_voice_api_updates_use_revisions_and_keep_secrets_private() {
    use lettuce_speech::TtsConfigurationRepository;
    let env = env(TtsMode::Speak, AsrMode::Text("text"), None);
    let provider = synthesis_request("metadata").provider;
    env.context.backend().database().upsert_audio_provider(provider.clone(), None).expect("provider");
    let voices = env.context.backend().tts_configuration(env.context.secret_store().as_ref())
        .create_user_voice(provider.id, "Narrator".into(), "speech".into(), "reference".into(), None, START).expect("voice");
    let listed = super::audio_providers_list(&env.context).await.expect("list");
    assert_eq!(listed.len(), 1); assert!(!listed[0].has_api_key);
    let update = || dto::AudioProviderUpdateRequest { provider_id: provider.id.to_string(), expected_revision: 1,
        label: "Local service".into(), configuration: dto::AudioProviderConfiguration::FishSpeech { base_url: Some("http://localhost:9000".into()), request_path: None } };
    let changed = super::audio_provider_update(&env.context, update()).await.expect("update");
    assert_eq!(changed.revision, 2);
    assert_eq!(super::audio_provider_update(&env.context, update()).await.expect_err("stale").code, ApiErrorCode::Conflict);
    let mut second_provider = provider.clone(); second_provider.id = AudioProviderId::new(); second_provider.secret_owner_id = lettuce_settings::SecretOwnerId::new();
    env.context.backend().database().upsert_audio_provider(second_provider.clone(), None).expect("other provider");
    let changed_voice = super::user_voice_update(&env.context, dto::UserVoiceUpdateRequest { id: voices.id.to_string(), provider_id: second_provider.id.to_string(), expected_revision: 1,
        name: "Updated narrator".into(), model_id: "speech".into(), voice_id: "reference".into(), prompt: Some("Warm".into()) }).await.expect("update voice");
    assert_eq!(changed_voice.revision, 2);
    assert_eq!(changed_voice.provider_id, second_provider.id.to_string());
    let invalid_update = |expected_revision| dto::UserVoiceUpdateRequest { id: voices.id.to_string(), provider_id: AudioProviderId::new().to_string(),
        expected_revision, name: "Uncommitted".into(), model_id: "speech".into(), voice_id: "other".into(), prompt: None };
    assert_eq!(super::user_voice_update(&env.context, invalid_update(2)).await.expect_err("provider absent").code, ApiErrorCode::NotFound);
    assert_eq!(super::user_voice_update(&env.context, invalid_update(1)).await.expect_err("stale update").code, ApiErrorCode::Conflict);
    assert_eq!(super::user_voices_list(&env.context).await.expect("voices")[0], changed_voice);
    super::user_voice_delete(&env.context, dto::UserVoiceRequest { voice_id: voices.id.to_string() }).await.expect("delete voice");
    super::audio_provider_delete(&env.context, dto::AudioProviderDeleteRequest { provider_id: provider.id.to_string(), expected_revision: 2 }).await.expect("delete provider");
    super::audio_provider_delete(&env.context, dto::AudioProviderDeleteRequest { provider_id: second_provider.id.to_string(), expected_revision: 1 }).await.expect("delete other provider");
    assert!(super::audio_providers_list(&env.context).await.expect("empty").is_empty());
}

#[tokio::test]
async fn queued_local_synthesis_waits_for_folder_move_and_preload_returns_busy() {
    use lettuce_settings::DeviceSettingsStore;
    use lettuce_speech::TtsConfigurationRepository;
    use crate::api::JobHandler;
    let env = env(TtsMode::Speak, AsrMode::Text("text"), None);
    let mut device = env.context.backend().database().load_device_settings().expect("device");
    device.llm_models_dir = Some(env.root.to_string_lossy().into_owned());
    env.context.backend().database().save_device_settings(device).expect("root");
    let mut provider = synthesis_request("queued").provider;
    provider.config = AudioProviderConfig::Kokoro { variant: Some("int8".into()) };
    env.context.backend().database().upsert_audio_provider(provider.clone(), None).expect("provider");
    let accepted = super::tts_synthesize(&env.context, dto::TtsSynthesizeRequest {
        request_id: RequestId::new().to_string(), provider_id: provider.id.to_string(), model_id: "kokoro".into(),
        voice_id: "af_heart".into(), prompt: None, text: "queued".into(), retained: true,
    }).await.expect("synthesis");
    crate::api::local_models_dir_set(&env.context, dto::LocalModelsDirSetRequest {
        client_operation_id: "speech-folder-move".into(), path: env.root.with_file_name(format!("{}-other", env.root.file_name().expect("name").to_string_lossy())).to_string_lossy().into_owned(), move_existing: false,
    }).await.expect("move admission");
    let id: JobId = accepted.job_id.parse().expect("job id");
    let snapshot = env.context.backend().database().get(id).expect("get").expect("job");
    assert!(crate::api::jobs::speech::SpeechSynthesizeHandler.claim(&env.context, &snapshot, WorkerId::new()).await.expect("claim").is_none());
    assert_eq!(env.context.backend().database().get(id).expect("queued").expect("job").state, JobState::Queued);
    let error = super::whisper_preload(&env.context, dto::WhisperPreloadRequest { model_id: None, run: Default::default() }).await.expect_err("busy");
    assert_eq!(error.code, ApiErrorCode::Busy);
    assert!(matches!(error.details, Some(ApiErrorDetails::LocalModelsBusy { reason: dto::LocalModelsBusyReason::FolderMoveActive { .. } })));
}

#[tokio::test]
async fn asr_library_api_filters_exports_and_keeps_audio_as_an_asset_reference() {
    let env = env(TtsMode::Speak, AsrMode::Text("text"), None);
    let library = env.context.backend().asr_learning();
    let term = lettuce_speech::AsrVocabularyTerm::new("Ford", Some("en"), Some("names"), Some("conversation"), 4, START).expect("term");
    let term = library.save_vocabulary(term).expect("save term");
    library.save_vocabulary(lettuce_speech::AsrVocabularyTerm::new("Other", Some("en"), None, Some("other"), 1, START).expect("other")).expect("save other");
    let filter = || dto::AsrLearningFilter { language: Some("EN".into()), scopes: vec!["conversation".into()] };
    let vocabulary = super::asr_vocabulary_list(&env.context, filter()).await.expect("vocabulary");
    assert_eq!(vocabulary.len(), 1); assert_eq!(vocabulary[0].id, term.id.to_string());
    let asset = ingest_wav(&env.context, tone());
    let mut example = lettuce_speech::AsrVoiceExample::new(asset, "Ford", Some("fort".into()), Some("en"), Some("conversation"), START).expect("example");
    example.vocabulary_term_id = Some(term.id);
    library.save_voice_example(example.clone()).expect("save example");
    let examples = super::asr_voice_examples_list(&env.context, filter()).await.expect("examples");
    assert_eq!(examples.len(), 1); assert_eq!(examples[0].audio.asset_id, asset.to_string());
    assert_eq!(examples[0].audio, env.context.asset_ref(asset));
    let learned = super::asr_voice_example_suggest(&env.context, dto::AsrLearningItemRequest { id: example.id.to_string() })
        .await.expect("example suggestion").expect("suggestion");
    assert_eq!(learned.correct, "Ford");

    let suggestions = super::asr_suggestions(&env.context, dto::AsrSuggestionsRequest { before: "the fort waits".into(), after: "the Ford waits".into(), language: Some("en".into()), scope: Some("conversation".into()) }).await.expect("suggestions");
    assert_eq!(suggestions.len(), 1); assert_eq!(suggestions[0].correct, "Ford");
    let export = env.root.join("learning.json");
    super::asr_learning_export(&env.context, dto::AsrLearningExportRequest { target: dto::FileTarget { uri: export.to_string_lossy().into_owned() }, filter: filter() }).await.expect("export");
    let document: lettuce_transfer::AsrLearningDocument = serde_json::from_slice(&std::fs::read(export).expect("export bytes")).expect("document");
    document.validate().expect("validated export");
    assert_eq!(document.vocabulary.len(), 1); assert_eq!(document.voice_examples.len(), 1); assert_eq!(document.audio_assets.len(), 1);
    super::asr_voice_example_delete(&env.context, dto::AsrLearningItemRequest { id: example.id.to_string() }).await.expect("delete example");
    super::asr_vocabulary_delete(&env.context, dto::AsrLearningItemRequest { id: term.id.to_string() }).await.expect("delete term");
    assert!(super::asr_vocabulary_list(&env.context, filter()).await.expect("empty").is_empty());
}

#[test]
fn installed_speech_runtime_follows_the_retained_kokoro_root() {
    use lettuce_settings::DeviceSettingsStore;
    let env = env(TtsMode::Speak, AsrMode::Text("hello"), None);
    let host = super::InstalledSpeech::new(None);
    let initial = host.tts_runtime(&env.context).expect("initial runtime");
    let reused = host.tts_runtime(&env.context).expect("reused runtime");
    assert!(Arc::ptr_eq(&initial, &reused));
    let database = env.context.backend().database();
    let mut device = database.load_device_settings().expect("device settings");
    device.retained_model_roots.kokoro = Some(env.root.join("moved-kokoro").to_string_lossy().into_owned());
    database.save_device_settings(device).expect("moved root");
    let moved = host.tts_runtime(&env.context).expect("moved runtime");
    assert!(!Arc::ptr_eq(&initial, &moved));
}

#[tokio::test]
async fn provider_voice_search_keeps_openai_empty_and_rejects_other_provider_kinds() {
    use lettuce_speech::TtsConfigurationRepository;
    let env = env(TtsMode::Speak, AsrMode::Text("text"), None);
    let mut provider = synthesis_request("search").provider;
    provider.config = AudioProviderConfig::OpenAiCompatible { base_url: None, request_path: None };
    env.context.backend().database().upsert_audio_provider(provider.clone(), None).expect("provider");
    let provider_id = provider.id;
    let request = || dto::AudioProviderVoiceSearchRequest { provider_id: provider_id.to_string(), search: "narrator".into() };
    assert!(super::audio_provider_voices_search(&env.context, request()).await.expect("empty").is_empty());
    provider.config = AudioProviderConfig::FishSpeech { base_url: Some("http://localhost:9000".into()), request_path: None };
    env.context.backend().database().upsert_audio_provider(provider, Some(Revision::new(1))).expect("changed kind");
    assert_eq!(super::audio_provider_voices_search(&env.context, request()).await.expect_err("unsupported kind").code, ApiErrorCode::InvalidInput);
}

#[tokio::test]
async fn voice_design_preview_rejects_invalid_provider_before_a_billable_request() {
    use lettuce_speech::TtsConfigurationRepository;
    let env = env(TtsMode::Speak, AsrMode::Text("text"), None);
    let provider = synthesis_request("voice-design").provider;
    let provider_id = provider.id;
    let request = || dto::VoiceDesignPreviewRequest { provider_id: provider_id.to_string(), text_sample: "A sample. ".repeat(20),
        voice_description: "A warm narrator with a clear voice.".into(), model_id: None, num_previews: Some(1) };
    assert_eq!(super::voice_design_preview(&env.context, request()).await.expect_err("missing account").code, ApiErrorCode::NotFound);
    env.context.backend().database().upsert_audio_provider(provider, None).expect("provider");
    assert_eq!(super::voice_design_preview(&env.context, request()).await.expect_err("wrong provider kind").code, ApiErrorCode::InvalidInput);
}

#[tokio::test(flavor = "multi_thread")]
async fn retained_synthesis_cache_reuses_the_real_job_and_records_each_request_replay() {
    use lettuce_speech::TtsConfigurationRepository;
    let env = env(TtsMode::Speak, AsrMode::Text("unused"), None);
    let provider = synthesis_request("cached").provider;
    let provider_id = provider.id;
    env.context.backend().database().upsert_audio_provider(provider, None).expect("provider");
    let make = || dto::TtsSynthesizeRequest { request_id: RequestId::new().to_string(), provider_id: provider_id.to_string(),
        model_id: "speech".into(), voice_id: "reference".into(), prompt: None, text: "Cached narration.".into(), retained: true };
    let first = super::tts_synthesize(&env.context, make()).await.expect("admit");
    let runner = runner(&env.context);
    assert!(run_to_idle(&runner).await);
    let repeated = make();
    let reused = super::tts_synthesize(&env.context, repeated.clone()).await.expect("cached");
    assert_eq!(reused, first);
    assert!(!run_to_idle(&runner).await);
    assert_eq!(env.tts.calls.load(Ordering::SeqCst), 1);
    let mut conflict = repeated.clone(); conflict.text = "Another narration.".into();
    assert_eq!(super::tts_synthesize(&env.context, conflict).await.expect_err("conflict").code, ApiErrorCode::Conflict);
    env.context.backend().database().delete_audio_provider(provider_id, Revision::INITIAL).expect("delete provider");
    assert_eq!(super::tts_synthesize(&env.context, repeated).await.expect("replay after deletion"), reused);
}

#[tokio::test(flavor = "multi_thread")]
async fn message_playback_resolves_live_provider_voice_and_reuses_frozen_audio() {
    use lettuce_characters::{CharacterRepository, VoicePreference};
    use lettuce_speech::{SynthesisRepository, TtsConfigurationRepository};
    let env = env_with_reply(TtsMode::Speak, AsrMode::Text("unused"), None, Reply::Text("Hello {{char}}."));
    let database = env.context.backend().database();
    let provider = synthesis_request("message").provider;
    let provider_id = provider.id;
    database.upsert_audio_provider(provider, None).expect("provider");
    let set_voice = |voice_id: &str| {
        let character = CharacterRepository::get(database, env.harness.character_id).expect("read").expect("character").character;
        let mut defaults = character.defaults;
        defaults.voice = Some(VoicePreference::Provider { provider_id, voice_id: voice_id.into(), model_id: None, voice_name: Some("Narrator".into()) });
        CharacterRepository::update_defaults(database, env.harness.character_id, character.revision, defaults, env.context.now()).expect("voice");
    };
    set_voice("voice-a");
    let (chat, message_id) = crate::api::turns_tests::replied_chat(&env.harness, "speak").await;
    let request = || dto::MessageSpeakRequest { request_id: RequestId::new().to_string(), message_id: message_id.clone(), voice_override: None, swap_places: false };
    let original_request = request();
    let first = super::message_speak(&env.context, original_request.clone()).await.expect("speak");
    let frozen = SynthesisRepository::get(database, first.job_id.parse().expect("job")).expect("record");
    assert_eq!(frozen.request.text, "Hello Ada.");
    assert_eq!(frozen.request.voice_id, "voice-a");
    assert_eq!(frozen.request.model_id, "server-default");
    let runner = runner(&env.context);
    assert!(run_to_idle(&runner).await);
    set_voice("voice-b");
    assert_eq!(super::message_speak(&env.context, original_request.clone()).await.expect("frozen replay"), first);
    let second = super::message_speak(&env.context, request()).await.expect("live voice");
    assert_ne!(second, first);
    assert_eq!(SynthesisRepository::get(database, second.job_id.parse().expect("job")).expect("record").request.voice_id, "voice-b");
    assert!(run_to_idle(&runner).await);
    let reused = super::message_speak(&env.context, request()).await.expect("cache");
    assert_eq!(reused, second);
    assert!(!run_to_idle(&runner).await);
    assert_eq!(env.tts.calls.load(Ordering::SeqCst), 2);
    let mut conflict = original_request.clone(); conflict.swap_places = true;
    assert_eq!(super::message_speak(&env.context, conflict).await.expect_err("conflict").code, ApiErrorCode::Conflict);
    crate::api::conversation_delete(&env.context, dto::ConversationRequest { conversation_id: chat }).await.expect("delete chat");
    assert_eq!(super::message_speak(&env.context, original_request).await.expect("replay after deletion"), first);
}

#[tokio::test]
async fn audio_credential_rotation_uses_generation_cas_and_returns_no_secret() {
    use lettuce_settings::SecretPurpose;
    let env = env(TtsMode::Speak, AsrMode::Text("unused"), None);
    let secrets = env.context.secret_store();
    let provider = env.context.backend().tts_configuration(secrets.as_ref()).create_audio_provider(
        "Narrator".into(), AudioProviderConfig::Elevenlabs, Some(SecretValue::new("old-secret-canary").expect("key")), START,
    ).await.expect("seed provider");
    let status_request = || dto::AudioProviderRequest { provider_id: provider.id.to_string() };
    let status = super::audio_provider_credential_status(&env.context, status_request()).await.expect("status");
    assert_eq!(status.generation, 1); assert!(status.available);
    let request = || dto::AudioProviderApiKeyRotateRequest { provider_id: provider.id.to_string(), api_key: "new-secret-canary".into(), expected_generation: 1 };
    assert!(!format!("{:?}", request()).contains("new-secret-canary"));
    let changed = super::audio_provider_api_key_rotate(&env.context, request()).await.expect("rotate");
    assert_eq!(changed.generation, 2); assert!(changed.available);
    assert_eq!(super::audio_provider_api_key_rotate(&env.context, request()).await.expect_err("stale generation").code, ApiErrorCode::Conflict);
    assert_eq!(super::audio_provider_credential_status(&env.context, status_request()).await.expect("current"), changed);
    let loaded = secrets.load(&provider.api_key_ref.expect("reference"), &SecretPurpose::AudioApiKey { owner: provider.secret_owner_id }).await.expect("stored key");
    loaded.with(|value| assert_eq!(value, "new-secret-canary"));
    let listed = serde_json::to_string(&super::audio_providers_list(&env.context).await.expect("metadata")).expect("json");
    assert!(!listed.contains("secret-canary"));
}

#[tokio::test(flavor = "multi_thread")]
async fn retained_audio_from_a_previous_provider_configuration_is_not_reused() {
    use lettuce_speech::TtsConfigurationRepository;
    let env = env(TtsMode::Speak, AsrMode::Text("unused"), None);
    let provider = synthesis_request("config cache").provider;
    let provider_id = provider.id;
    env.context.backend().database().upsert_audio_provider(provider, None).expect("provider");
    let request = || dto::TtsSynthesizeRequest { request_id: RequestId::new().to_string(), provider_id: provider_id.to_string(),
        model_id: "speech".into(), voice_id: "reference".into(), text: "Same narration.".into(), prompt: None, retained: true };
    let original = request();
    let first = super::tts_synthesize(&env.context, original.clone()).await.expect("first");
    let runner = runner(&env.context); assert!(run_to_idle(&runner).await);
    super::audio_provider_update(&env.context, dto::AudioProviderUpdateRequest { provider_id: provider_id.to_string(), expected_revision: 1,
        label: "Changed server".into(), configuration: dto::AudioProviderConfiguration::FishSpeech {
            base_url: Some("http://localhost:9000".into()), request_path: None,
        } }).await.expect("new server");
    let second = super::tts_synthesize(&env.context, request()).await.expect("fresh synthesis");
    assert_ne!(first, second);
    assert!(run_to_idle(&runner).await);
    assert_eq!(env.tts.calls.load(Ordering::SeqCst), 2);
    assert_eq!(super::tts_synthesize(&env.context, original).await.expect("original replay"), first);
}

#[tokio::test]
async fn library_receipts_preserve_counts_and_original_results_after_edits_and_deletes() {
    let env = env(TtsMode::Speak, AsrMode::Text("Hello."), None);
    let context = &env.context;
    let vocabulary = dto::AsrVocabularySaveRequest { client_operation_id: "vocabulary-create".into(), id: None, term: "Lettuce".into(), language: Some("en".into()), category: None, scope: None, priority: None, use_count: None };
    let original = super::asr_vocabulary_save(context, vocabulary.clone()).await.expect("vocabulary");
    assert_eq!(original.priority, 50);
    let mut edit = vocabulary.clone(); edit.client_operation_id = "vocabulary-edit".into(); edit.id = Some(original.id.clone()); edit.term = "Lettuce AI".into();
    let edited = super::asr_vocabulary_save(context, edit).await.expect("edit");
    assert_eq!(edited.created_at, original.created_at);
    assert_eq!(super::asr_vocabulary_save(context, vocabulary.clone()).await.expect("original snapshot"), original);
    super::asr_vocabulary_delete(context, dto::AsrLearningItemRequest { id: original.id.clone() }).await.expect("delete");
    assert_eq!(super::asr_vocabulary_save(context, vocabulary).await.expect("replay after deletion"), original);
    let correction = dto::AsrCorrectionSaveRequest { client_operation_id: "correction-create".into(), id: None, wrong: "let us".into(), correct: "Lettuce".into(), language: Some("en".into()), scope: None, confidence: None, use_count: None, accepted_count: None, rejected_count: None, seen_count: None, last_seen_at: None, user_approved: Some(true) };
    let first = super::asr_correction_save(context, correction.clone()).await.expect("correction");
    assert_eq!((first.accepted_count, first.seen_count), (1, 1));
    assert!((0.35..=0.98).contains(&first.confidence));
    assert_eq!(super::asr_correction_save(context, correction.clone()).await.expect("replay"), first);
    let mut changed = correction.clone(); changed.correct = "Different".into();
    assert_eq!(super::asr_correction_save(context, changed).await.expect_err("different digest").code, ApiErrorCode::Conflict);
    let suggestion = dto::AsrSuggestionWriteRequest { client_operation_id: "approve".into(), suggestion: dto::AsrSuggestionView {
        wrong: first.wrong.clone(), correct: first.correct.clone(), language: first.language.clone(), scope: first.scope.clone(), confidence: first.confidence,
        accepted_count: first.accepted_count, rejected_count: first.rejected_count, seen_count: first.seen_count,
    } };
    let approved = super::asr_suggestion_approve(context, suggestion.clone()).await.expect("approve");
    assert_eq!((approved.accepted_count, approved.seen_count), (2, 2));
    assert_eq!(super::asr_suggestion_approve(context, suggestion.clone()).await.expect("approval replay"), approved);
    let mut ignored_request = suggestion; ignored_request.client_operation_id = "ignore".into();
    let ignored = super::asr_suggestion_ignore(context, ignored_request.clone()).await.expect("ignore");
    assert_eq!(ignored.ignored_count, 1);
    assert_eq!(super::asr_suggestion_ignore(context, ignored_request.clone()).await.expect("ignore replay"), ignored);
    ignored_request.client_operation_id = "ignore-again".into();
    assert_eq!(super::asr_suggestion_ignore(context, ignored_request).await.expect("second distinct ignore").ignored_count, 2);
    super::asr_correction_delete(context, dto::AsrLearningItemRequest { id: first.id.clone() }).await.expect("delete");
    assert_eq!(super::asr_correction_save(context, correction).await.expect("original correction after delete"), first);
}

fn provider_create_request(key: &str) -> dto::AudioProviderCreateRequest {
    dto::AudioProviderCreateRequest { client_operation_id: key.into(), label: "Hosted voices".into(),
        draft: dto::AudioProviderDraft { configuration: dto::AudioProviderConfiguration::Elevenlabs, api_key: Some("private-canary".into()) } }
}

#[tokio::test]
async fn provider_create_reuses_the_crash_secret_and_replays_after_deletion() {
    use lettuce_settings::{SecretPurpose, SecretRecord};
    let env = env(TtsMode::Speak, AsrMode::Text("Hello."), None);
    let context = &env.context;
    let request = provider_create_request("provider-create");
    let (id, owner, reference) = super::providers::provider_create_ids(&request.client_operation_id);
    context.secret_store().put(SecretRecord::new(reference, SecretPurpose::AudioApiKey { owner }),
        SecretValue::new("crashed-before-database").expect("secret"), None).await.expect("crash point after put");
    let restarted = context.restarted();
    let first = super::audio_provider_create(&restarted, request.clone()).await.expect("retry completes create after restart");
    assert_eq!(first.id, id.to_string());
    assert_eq!(super::audio_provider_create(context, request.clone()).await.expect("replay"), first);
    let keys = context.secret_store().audio_api_keys().await.expect("keys");
    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0].generation, 2, "successful replay does not overwrite the secret");
    let mut changed = request.clone(); changed.label = "Different".into();
    assert_eq!(super::audio_provider_create(context, changed).await.expect_err("conflict").code, ApiErrorCode::Conflict);
    assert_eq!(context.secret_store().audio_api_keys().await.expect("keys")[0].generation, 2);
    super::audio_provider_delete(context, dto::AudioProviderDeleteRequest { provider_id: first.id.clone(), expected_revision: first.revision }).await.expect("delete");
    assert_eq!(super::audio_provider_create(context, request).await.expect("original result after deletion"), first);
    assert!(super::audio_providers_list(context).await.expect("providers").iter().all(|provider| provider.id != first.id));
    assert!(context.secret_store().audio_api_keys().await.expect("deleted key").is_empty());
}

#[tokio::test]
async fn startup_sweeps_orphan_audio_secrets_and_preserves_committed_owners() {
    use lettuce_settings::{SecretPurpose, SecretRecord};
    let env = env(TtsMode::Speak, AsrMode::Text("Hello."), None);
    let context = &env.context;
    let retained = super::audio_provider_create(context, provider_create_request("retained-create")).await.expect("provider");
    let (_, owner, reference) = super::providers::provider_create_ids("orphan-create");
    context.secret_store().put(SecretRecord::new(reference, SecretPurpose::AudioApiKey { owner }),
        SecretValue::new("orphan-canary").expect("secret"), None).await.expect("orphan");
    assert_eq!(context.secret_store().audio_api_keys().await.expect("keys").len(), 2);
    let workers = crate::api::startup(context).await.expect("startup sweep");
    assert!(workers.steps().contains(&crate::api::StartupStep::SweepOrphanAudioSecrets));
    assert_eq!(context.secret_store().audio_api_keys().await.expect("keys").len(), 1);
    assert!(super::audio_providers_list(context).await.expect("providers").iter().any(|provider| provider.id == retained.id));
    workers.stop().await;
}

#[tokio::test]
async fn voice_create_examples_and_library_import_have_receipts() {
    let env = env(TtsMode::Speak, AsrMode::Text("Hello."), None);
    let context = &env.context;
    let provider = super::audio_provider_create(context, provider_create_request("voice-account")).await.expect("provider");
    let voice = dto::UserVoiceCreateRequest { client_operation_id: "voice-create".into(), provider_id: provider.id,
        name: "Narrator".into(), model_id: "eleven_multilingual_v2".into(), voice_id: "remote-voice".into(), prompt: None };
    let original = super::user_voice_create(context, voice.clone()).await.expect("voice");
    super::user_voice_delete(context, dto::UserVoiceRequest { voice_id: original.id.clone() }).await.expect("delete voice");
    assert_eq!(super::user_voice_create(context, voice).await.expect("deleted voice replay"), original);
    let asset = ingest_wav(context, tone());
    let example = dto::AsrVoiceExampleSaveRequest { client_operation_id: "example-save".into(), id: None, audio_asset_id: asset.to_string(), expected_text: "Lettuce".into(), whisper_output: Some("let us".into()), language: Some("en".into()), scope: None, vocabulary_term_id: None, correction_id: None };
    let saved = super::asr_voice_example_save(context, example.clone()).await.expect("example");
    assert_eq!(super::asr_voice_example_save(context, example.clone()).await.expect("example replay"), saved);
    let exported = env.root.join("library.json");
    super::asr_learning_export(context, dto::AsrLearningExportRequest { target: dto::FileTarget { uri: exported.to_string_lossy().into_owned() }, filter: dto::AsrLearningFilter { language: None, scopes: vec!["global".into()] } }).await.expect("export");
    let import = dto::AsrLearningImportRequest { client_operation_id: "library-import".into(), source: dto::FileSource { uri: exported.to_string_lossy().into_owned() } };
    let imported = super::asr_learning_import(context, import.clone()).await.expect("import");
    assert_eq!(imported.voice_example_count, 1);
    assert_eq!(super::asr_learning_import(context, import).await.expect("import replay"), imported);
    let examples = super::asr_voice_examples_list(context, dto::AsrLearningFilter { language: None, scopes: vec!["global".into()] }).await.expect("examples");
    assert_eq!(examples.len(), 2);
    super::asr_voice_example_delete(context, dto::AsrLearningItemRequest { id: saved.id }).await.expect("delete example");
    assert_eq!(super::asr_voice_example_save(context, example).await.expect("example replay after deletion").audio.asset_id, asset.to_string());
}

struct RefusingAudioSecretStore {
    store: Arc<dyn lettuce_settings::SecretStore>,
    refuse_inventory: bool,
}

#[async_trait]
impl lettuce_settings::SecretStore for RefusingAudioSecretStore {
    async fn audio_api_keys(&self) -> Result<Vec<lettuce_settings::SecretStatus>, lettuce_settings::SecretStoreError> {
        if self.refuse_inventory { return Err(lettuce_settings::SecretStoreError::Unavailable(lettuce_settings::SecretAvailability::BackendUnavailable)); }
        self.store.audio_api_keys().await
    }
    async fn put(&self, record: lettuce_settings::SecretRecord, value: SecretValue, expected: Option<u64>) -> Result<lettuce_settings::SecretStatus, lettuce_settings::SecretStoreError> {
        self.store.put(record, value, expected).await
    }
    async fn load(&self, reference: &lettuce_settings::SecretRef, purpose: &lettuce_settings::SecretPurpose) -> Result<SecretValue, lettuce_settings::SecretStoreError> {
        self.store.load(reference, purpose).await
    }
    async fn status(&self, reference: &lettuce_settings::SecretRef, purpose: &lettuce_settings::SecretPurpose) -> Result<lettuce_settings::SecretStatus, lettuce_settings::SecretStoreError> {
        self.store.status(reference, purpose).await
    }
    async fn delete(&self, _: &lettuce_settings::SecretRef, _: &lettuce_settings::SecretPurpose, _: Option<u64>) -> Result<lettuce_settings::SecretStatus, lettuce_settings::SecretStoreError> {
        Err(lettuce_settings::SecretStoreError::Unavailable(lettuce_settings::SecretAvailability::BackendUnavailable))
    }
}

#[tokio::test]
async fn startup_refuses_typed_audio_secret_inventory_or_delete_failures() {
    use lettuce_settings::{SecretPurpose, SecretRecord};
    for refuse_inventory in [true, false] {
        let env = env(TtsMode::Speak, AsrMode::Text("Hello."), None);
        let (_, owner, reference) = super::providers::provider_create_ids("startup-orphan");
        env.context.secret_store().put(SecretRecord::new(reference, SecretPurpose::AudioApiKey { owner }),
            SecretValue::new("orphan-canary").expect("secret"), None).await.expect("orphan");
        let context = env.context.with_secret_store(Arc::new(RefusingAudioSecretStore {
            store: env.context.secret_store().clone(), refuse_inventory,
        }));
        let error = crate::api::startup(&context).await.expect_err("startup cannot silently keep the orphan");
        assert_eq!(error.code, ApiErrorCode::Unavailable);
        assert_eq!(error.details, Some(ApiErrorDetails::Speech { failure: SpeechFailure::SecretStoreUnavailable }));
        assert_eq!(env.context.secret_store().audio_api_keys().await.expect("original inventory").len(), 1);
    }
}

#[tokio::test]
async fn legacy_learning_file_import_ingests_audio_once_and_replays_its_counted_result() {
    let env = env(TtsMode::Speak, AsrMode::Text("Hello."), None);
    let audio = env.root.join("legacy-voice.wav");
    std::fs::write(&audio, tone()).expect("legacy audio");
    let source = env.root.join("legacy-library.json");
    let document = serde_json::json!({
        "version": 2,
        "vocabulary": [{"id": 1, "term": "Lettuce AI", "normalizedTerm": "lettuce ai", "language": "en", "category": "product", "scope": "global", "priority": 80, "useCount": 7, "createdAt": "2026-01-01 00:00:00", "updatedAt": "2026-01-02 00:00:00"}],
        "voiceExamples": [{"id": 2, "audioPath": "legacy-voice.wav", "expectedText": "Lettuce AI", "normalizedExpectedText": "lettuce ai", "whisperOutput": "lettuce a eye", "normalizedWhisperOutput": "lettuce a eye", "language": "en", "scope": "global", "termId": 1, "correctionId": null, "createdAt": "2026-01-04 00:00:00"}],
    });
    std::fs::write(&source, serde_json::to_vec(&document).expect("document")).expect("library");
    let request = dto::AsrLearningImportRequest { client_operation_id: "legacy-import".into(), source: dto::FileSource { uri: source.to_string_lossy().into_owned() } };
    let first = super::asr_learning_import(&env.context, request.clone()).await.expect("legacy import");
    assert_eq!((first.vocabulary_count, first.voice_example_count), (1, 1));
    assert_eq!(super::asr_learning_import(&env.context, request.clone()).await.expect("replay"), first);
    let filter = dto::AsrLearningFilter { language: None, scopes: vec!["global".into()] };
    let examples = super::asr_voice_examples_list(&env.context, filter.clone()).await.expect("examples");
    assert_eq!(examples.len(), 1, "replay does not ingest another asset/example");
    assert!(examples[0].audio.url.starts_with("test-asset://"));
    let vocabulary = super::asr_vocabulary_list(&env.context, filter).await.expect("vocabulary");
    assert_eq!(vocabulary[0].use_count, 7);
    std::fs::write(&source, b"{\"version\":2}").expect("changed request");
    assert_eq!(super::asr_learning_import(&env.context, request).await.expect_err("changed content conflicts").code, ApiErrorCode::Conflict);
}

#[tokio::test]
async fn an_audio_secret_with_only_a_creation_receipt_is_retained_at_startup() {
    use lettuce_speech::TtsConfigurationRepository;
    let env = env(TtsMode::Speak, AsrMode::Text("Hello."), None);
    let created = super::audio_provider_create(&env.context, provider_create_request("receipt-only-owner")).await.expect("create");
    env.context.backend().database().delete_audio_provider(created.id.parse().expect("id"), lettuce_types::Revision::new(created.revision)).expect("metadata deletion without credential deletion");
    super::sweep_orphan_audio_secrets(&env.context).await.expect("sweep");
    assert_eq!(env.context.secret_store().audio_api_keys().await.expect("retained key").len(), 1);
}
