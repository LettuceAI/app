use std::time::Duration;

use lettuce_jobs::{
    CancellationPolicy, CancellationReason, Claim, FiniteFraction, IdempotencyKey, JobError,
    JobErrorCode, JobKind, JobMutation, JobOutcome, JobPriority, JobSnapshot, JobState, JobStore,
    JobSubject, OutcomeRef, ProgressSnapshot, RecoveryPolicy, ResourceAvailability, ResourceClass,
    StageSnapshot, StoreError, SubjectKind, WorkerId, handle::JobHandle,
};
use lettuce_settings::{SecretPurpose, SecretStore, SecretStoreError};
use lettuce_speech::{
    AudioProviderKind, SynthesisRecord, SynthesisRepository, SynthesisRepositoryError,
    SynthesisRequest, SynthesisState, TtsAudioError, TtsAudioSink, TtsRuntime, TtsRuntimeError,
    TtsSynthesisValidationError,
};
use lettuce_types::{JobId, TimestampMillis};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TtsSynthesisAdmission {
    pub record: SynthesisRecord,
    pub job: JobSnapshot,
    pub created: bool,
}

#[derive(Debug)]
pub struct TtsSynthesisClaimedWork {
    pub record: SynthesisRecord,
    pub claim: Claim,
    pub handle: JobHandle,
    pub job: JobSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TtsSynthesisSuccess {
    pub record: SynthesisRecord,
    pub job: JobSnapshot,
    pub replayed: bool,
}

#[derive(Debug)]
pub enum TtsSynthesisRunResult {
    Succeeded(Box<TtsSynthesisSuccess>),
    Cancelled {
        error: TtsSynthesisError,
        job: JobSnapshot,
    },
    Failed {
        error: TtsSynthesisError,
        job: JobSnapshot,
    },
    RetryScheduled {
        error: TtsSynthesisError,
        job: JobSnapshot,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum TtsSynthesisError {
    #[error("TTS synthesis request is invalid: {0}")]
    Invalid(#[from] TtsSynthesisValidationError),
    #[error("TTS synthesis runtime failed: {0}")]
    Runtime(#[from] TtsRuntimeError),
    #[error("TTS synthesis output failed: {0}")]
    Audio(#[from] TtsAudioError),
    #[error("TTS synthesis secret failed: {0}")]
    Secret(#[from] SecretStoreError),
    #[error("TTS synthesis persistence failed: {0}")]
    Repository(#[from] SynthesisRepositoryError),
    #[error("TTS synthesis job failed: {0}")]
    Jobs(#[from] StoreError),
    #[error("TTS synthesis job and request are inconsistent")]
    InvalidWork,
}

#[derive(Debug)]
pub struct TtsSynthesisCoordinator<'a, R: ?Sized, J: ?Sized> {
    syntheses: &'a R,
    jobs: &'a J,
}

impl<'a, R: ?Sized, J: ?Sized> TtsSynthesisCoordinator<'a, R, J> {
    #[must_use]
    pub const fn new(syntheses: &'a R, jobs: &'a J) -> Self {
        Self { syntheses, jobs }
    }
}

impl<R: SynthesisRepository + ?Sized, J: JobStore + ?Sized> TtsSynthesisCoordinator<'_, R, J> {
    pub fn admit(
        &self,
        request: SynthesisRequest,
    ) -> Result<TtsSynthesisAdmission, TtsSynthesisError> {
        request.validate()?;
        let subject = JobSubject::new(SubjectKind::SpeechRequest, request.id.to_string())
            .map_err(|_| TtsSynthesisError::InvalidWork)?;
        let key = IdempotencyKey::new(format!("speech-synthesize-{}", request.id))
            .map_err(|_| TtsSynthesisError::InvalidWork)?;
        let resources = match request.provider.config.provider_kind() {
            AudioProviderKind::Kokoro => vec![
                ResourceClass::ModelLoad,
                ResourceClass::Cpu,
                ResourceClass::DiskWrite,
            ],
            _ => vec![ResourceClass::Network, ResourceClass::DiskWrite],
        };
        let admitted = self.jobs.create_or_get(
            lettuce_jobs::JobSpec::new(
                JobKind::SpeechSynthesize,
                subject,
                OutcomeRef::Request(request.id),
            )
            .with_idempotency_key(key)
            .with_priority(JobPriority::Interactive)
            .with_resources(resources)
            .with_policies(RecoveryPolicy::Restart, CancellationPolicy::Cooperative),
        )?;
        let record = self.syntheses.admit(SynthesisRecord {
            job_id: admitted.job.id,
            request,
            state: SynthesisState::Pending,
        })?;
        validate_job_record(&admitted.job, &record)?;
        Ok(TtsSynthesisAdmission {
            record,
            job: admitted.job,
            created: admitted.created,
        })
    }

    pub fn claim(
        &self,
        job_id: JobId,
        worker_id: WorkerId,
        now: TimestampMillis,
        lease_for: Duration,
        allowed: &ResourceAvailability,
    ) -> Result<Option<TtsSynthesisClaimedWork>, TtsSynthesisError> {
        let record = self.syntheses.get(job_id)?;
        let job = self
            .jobs
            .get(job_id)?
            .ok_or(TtsSynthesisError::InvalidWork)?;
        validate_job_record(&job, &record)?;
        if job.state == JobState::Succeeded {
            return Ok(None);
        }
        let at = now.max(job.updated_at);
        let Some(claim) = self.jobs.claim(job_id, worker_id, at, lease_for, allowed)? else {
            return Ok(None);
        };
        if claim.input_ref != OutcomeRef::Request(record.request.id) {
            return Err(TtsSynthesisError::InvalidWork);
        }
        let handle = JobHandle::new(job_id);
        self.jobs.append_and_transition(JobMutation::Start {
            claim: claim.claim.clone(),
            at,
        })?;
        let job = self.jobs.append_and_transition(JobMutation::StageChanged {
            claim: claim.claim.clone(),
            stage: StageSnapshot::new("speech-synthesis", false).expect("constant stage is valid"),
            at,
        })?;
        Ok(Some(TtsSynthesisClaimedWork {
            record,
            claim,
            handle,
            job,
        }))
    }

    pub fn replay(&self, job_id: JobId) -> Result<Option<TtsSynthesisSuccess>, TtsSynthesisError> {
        let record = self.syntheses.get(job_id)?;
        let job = self
            .jobs
            .get(job_id)?
            .ok_or(TtsSynthesisError::InvalidWork)?;
        validate_job_record(&job, &record)?;
        match (&record.state, job.state) {
            (SynthesisState::Succeeded { .. }, JobState::Succeeded) => {
                Ok(Some(TtsSynthesisSuccess {
                    record,
                    job,
                    replayed: true,
                }))
            }
            _ => Ok(None),
        }
    }

    pub async fn run<S, E, A>(
        &self,
        work: TtsSynthesisClaimedWork,
        secrets: &S,
        runtime: &E,
        audio: &A,
        cancellation_reason: CancellationReason,
        now: TimestampMillis,
    ) -> Result<TtsSynthesisRunResult, TtsSynthesisError>
    where
        S: SecretStore + ?Sized,
        E: TtsRuntime + ?Sized,
        A: TtsAudioSink + ?Sized,
    {
        validate_job_record(&work.job, &work.record)?;
        if work.job.state != JobState::Running
            || work.claim.claim.job_id != work.job.id
            || work.handle.id() != work.job.id
        {
            return Err(TtsSynthesisError::InvalidWork);
        }
        if matches!(work.record.state, SynthesisState::Succeeded { .. }) {
            return self.finish_success(work, true, now);
        }
        let result = self.execute(&work, secrets, runtime, audio, now).await;
        match result {
            Ok(result) => match self.syntheses.settle(work.job.id, result) {
                Ok(record) => {
                    self.finish_success(TtsSynthesisClaimedWork { record, ..work }, false, now)
                }
                Err(error) => self.finish_error(work, error.into(), now),
            },
            Err(error)
                if matches!(
                    error,
                    TtsSynthesisError::Runtime(TtsRuntimeError::Cancelled)
                ) || work.handle.cancellation_token().is_cancelled() =>
            {
                self.finish_cancellation(work, error, cancellation_reason, now)
            }
            Err(error) => self.finish_error(work, error, now),
        }
    }

    async fn execute<S, E, A>(
        &self,
        work: &TtsSynthesisClaimedWork,
        secrets: &S,
        runtime: &E,
        audio: &A,
        now: TimestampMillis,
    ) -> Result<lettuce_speech::SynthesisResult, TtsSynthesisError>
    where
        S: SecretStore + ?Sized,
        E: TtsRuntime + ?Sized,
        A: TtsAudioSink + ?Sized,
    {
        check_cancelled(&work.handle)?;
        let credential = match work.record.request.provider.api_key_ref {
            Some(reference) => Some(
                secrets
                    .load(
                        &reference,
                        &SecretPurpose::AudioApiKey {
                            owner: work.record.request.provider.secret_owner_id,
                        },
                    )
                    .await?,
            ),
            None => None,
        };
        check_cancelled(&work.handle)?;
        let output = runtime
            .synthesize(
                &work.record.request,
                credential.as_ref(),
                &work.handle.cancellation_token(),
            )
            .await?;
        check_cancelled(&work.handle)?;
        let result = audio.ingest(work.job.id, &work.record.request, output, now)?;
        result.validate_for(&work.record.request)?;
        Ok(result)
    }

    fn finish_success(
        &self,
        work: TtsSynthesisClaimedWork,
        replayed: bool,
        now: TimestampMillis,
    ) -> Result<TtsSynthesisRunResult, TtsSynthesisError> {
        if !matches!(work.record.state, SynthesisState::Succeeded { .. }) {
            return Err(TtsSynthesisError::InvalidWork);
        }
        let at = now.max(work.job.updated_at);
        self.jobs.append_and_transition(JobMutation::Progress {
            claim: work.claim.claim.clone(),
            progress: ProgressSnapshot {
                fraction: Some(FiniteFraction::new(1.0).expect("constant progress is valid")),
                ..ProgressSnapshot::default()
            },
            at,
        })?;
        let job = self.jobs.append_and_transition(JobMutation::Succeed {
            claim: work.claim.claim,
            outcome: JobOutcome::Success {
                result_ref: OutcomeRef::Request(work.record.request.id),
            },
            at,
        })?;
        Ok(TtsSynthesisRunResult::Succeeded(Box::new(
            TtsSynthesisSuccess {
                record: work.record,
                job,
                replayed,
            },
        )))
    }

    fn finish_cancellation(
        &self,
        work: TtsSynthesisClaimedWork,
        error: TtsSynthesisError,
        reason: CancellationReason,
        now: TimestampMillis,
    ) -> Result<TtsSynthesisRunResult, TtsSynthesisError> {
        let at = now.max(work.job.updated_at);
        self.jobs
            .append_and_transition(JobMutation::RequestCancellation {
                id: work.job.id,
                reason,
                at,
            })?;
        self.jobs
            .append_and_transition(JobMutation::RequestCleanup {
                claim: work.claim.claim.clone(),
                at,
            })?;
        let job = self
            .jobs
            .append_and_transition(JobMutation::FinishCancellation {
                claim: work.claim.claim,
                at,
            })?;
        Ok(TtsSynthesisRunResult::Cancelled { error, job })
    }

    fn finish_error(
        &self,
        work: TtsSynthesisClaimedWork,
        error: TtsSynthesisError,
        now: TimestampMillis,
    ) -> Result<TtsSynthesisRunResult, TtsSynthesisError> {
        let at = now.max(work.job.updated_at);
        let (code, retryable, message) = classify_error(&error);
        if retryable {
            let job = self
                .jobs
                .append_and_transition(JobMutation::RetryScheduled {
                    claim: work.claim.claim,
                    at,
                })?;
            return Ok(TtsSynthesisRunResult::RetryScheduled { error, job });
        }
        let job = self.jobs.append_and_transition(JobMutation::Fail {
            claim: work.claim.claim,
            error: JobError::new(code, false, message).expect("constant TTS error label is valid"),
            at,
        })?;
        Ok(TtsSynthesisRunResult::Failed { error, job })
    }
}

fn validate_job_record(
    job: &JobSnapshot,
    record: &SynthesisRecord,
) -> Result<(), TtsSynthesisError> {
    record.validate()?;
    if job.id != record.job_id
        || job.kind != JobKind::SpeechSynthesize
        || job.subject.kind != SubjectKind::SpeechRequest
        || job.subject.id.as_str() != record.request.id.to_string()
    {
        return Err(TtsSynthesisError::InvalidWork);
    }
    if job.state == JobState::Succeeded
        && (!matches!(record.state, SynthesisState::Succeeded { .. })
            || job.outcome.as_ref()
                != Some(&JobOutcome::Success {
                    result_ref: OutcomeRef::Request(record.request.id),
                }))
    {
        return Err(TtsSynthesisError::InvalidWork);
    }
    Ok(())
}

fn check_cancelled(handle: &JobHandle) -> Result<(), TtsSynthesisError> {
    if handle.cancellation_token().is_cancelled() {
        Err(TtsRuntimeError::Cancelled.into())
    } else {
        Ok(())
    }
}

fn classify_error(error: &TtsSynthesisError) -> (JobErrorCode, bool, &'static str) {
    use lettuce_media::MediaStoreError;

    match error {
        TtsSynthesisError::Invalid(_)
        | TtsSynthesisError::Runtime(TtsRuntimeError::Rejected)
        | TtsSynthesisError::Audio(TtsAudioError::Invalid(_))
        | TtsSynthesisError::Audio(TtsAudioError::Media(
            MediaStoreError::EmptyInput
            | MediaStoreError::InputTooLarge
            | MediaStoreError::UnsupportedFormat
            | MediaStoreError::InvalidHeader
            | MediaStoreError::KindMismatch
            | MediaStoreError::MimeMismatch
            | MediaStoreError::PixelLimitExceeded
            | MediaStoreError::InvalidDimensions
            | MediaStoreError::InvalidMetadata,
        ))
        | TtsSynthesisError::InvalidWork => (
            JobErrorCode::InvalidInput,
            false,
            "TTS synthesis input or output is invalid",
        ),
        TtsSynthesisError::Runtime(TtsRuntimeError::Cancelled) => (
            JobErrorCode::Cancelled,
            false,
            "TTS synthesis was cancelled",
        ),
        TtsSynthesisError::Runtime(TtsRuntimeError::Unavailable) => (
            JobErrorCode::CapabilityUnavailable,
            true,
            "TTS provider is unavailable",
        ),
        TtsSynthesisError::Runtime(TtsRuntimeError::Failed)
        | TtsSynthesisError::Audio(_)
        | TtsSynthesisError::Secret(_)
        | TtsSynthesisError::Repository(_)
        | TtsSynthesisError::Jobs(_) => (
            JobErrorCode::ResourceUnavailable,
            true,
            "TTS synthesis resource is unavailable",
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use lettuce_database::Database;
    use lettuce_media::{LocalMediaBlobStore, MediaAssetRepository, RetentionClass};
    use lettuce_platform::{DirectorySnapshot, FilesystemAuthority, ManagedRoot};
    use lettuce_settings::{
        InMemorySecretStore, SecretOwnerId, SecretRecord, SecretRef, SecretValue,
    };
    use lettuce_speech::{AudioProvider, AudioProviderConfig, RuntimeSynthesis, TtsOutputPolicy};
    use lettuce_types::{AssetId, AudioProviderId, RequestId, Revision};

    use super::*;

    const NOW: TimestampMillis = TimestampMillis::new(1_000);

    struct Runtime {
        calls: Arc<Mutex<u32>>,
        outcome: Result<(), TtsRuntimeError>,
        cancel_after_response: bool,
    }

    struct LocalRuntime {
        calls: Arc<Mutex<u32>>,
    }

    #[async_trait]
    impl TtsRuntime for LocalRuntime {
        async fn synthesize(
            &self,
            _: &SynthesisRequest,
            credential: Option<&lettuce_settings::SecretValue>,
            _: &lettuce_jobs::handle::CancellationToken,
        ) -> Result<RuntimeSynthesis, TtsRuntimeError> {
            assert!(credential.is_none());
            *self.calls.lock().expect("calls") += 1;
            Ok(RuntimeSynthesis {
                bytes: wav_fixture(),
                declared_mime_type: "audio/wav".into(),
            })
        }
    }

    #[async_trait]
    impl TtsRuntime for Runtime {
        async fn synthesize(
            &self,
            _: &SynthesisRequest,
            credential: Option<&lettuce_settings::SecretValue>,
            cancellation: &lettuce_jobs::handle::CancellationToken,
        ) -> Result<RuntimeSynthesis, TtsRuntimeError> {
            *self.calls.lock().expect("calls") += 1;
            assert!(credential.is_some());
            self.outcome?;
            if self.cancel_after_response {
                cancellation.cancel();
            }
            Ok(RuntimeSynthesis {
                bytes: wav_fixture(),
                declared_mime_type: "audio/wav".into(),
            })
        }
    }

    fn wav_fixture() -> Vec<u8> {
        let samples = [0_i16, 1, -1, 0];
        let data_size = u32::try_from(samples.len() * 2).expect("WAV data size");
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data_size).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16_u32.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&16_000_u32.to_le_bytes());
        wav.extend_from_slice(&32_000_u32.to_le_bytes());
        wav.extend_from_slice(&2_u16.to_le_bytes());
        wav.extend_from_slice(&16_u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_size.to_le_bytes());
        for sample in samples {
            wav.extend_from_slice(&sample.to_le_bytes());
        }
        wav
    }

    fn media_store(
        database_path: &std::path::Path,
        root: &std::path::Path,
    ) -> LocalMediaBlobStore<Database, Database> {
        let snapshot = DirectorySnapshot::new(root).expect("directory snapshot");
        let authority = FilesystemAuthority::new(&snapshot).expect("filesystem authority");
        LocalMediaBlobStore::new(
            authority.managed_files(),
            authority
                .read_capability(ManagedRoot::MediaBlobs)
                .expect("read capability"),
            authority
                .write_capability(ManagedRoot::MediaBlobs)
                .expect("write capability"),
            Database::open(database_path).expect("blob database"),
            Database::open(database_path).expect("asset database"),
        )
    }

    fn request(reference: SecretRef, policy: TtsOutputPolicy) -> SynthesisRequest {
        SynthesisRequest {
            id: RequestId::new(),
            provider: AudioProvider {
                id: AudioProviderId::new(),
                secret_owner_id: SecretOwnerId::new(),
                label: "OpenAI-compatible speech".into(),
                api_key_ref: Some(reference),
                config: AudioProviderConfig::OpenAiCompatible {
                    base_url: Some("https://audio.example".into()),
                    request_path: Some("/v1/audio/speech".into()),
                },
                revision: Revision::INITIAL,
                created_at: NOW,
                updated_at: NOW,
            },
            model_id: "tts-1".into(),
            voice_id: "alloy".into(),
            prompt: Some("Speak warmly.".into()),
            text: "Welcome back.".into(),
            output_asset_id: AssetId::new(),
            output_policy: policy,
            created_at: NOW,
        }
    }

    async fn secret_store(request: &SynthesisRequest) -> InMemorySecretStore {
        let store = InMemorySecretStore::new();
        store
            .put(
                SecretRecord::new(
                    request.provider.api_key_ref.expect("secret reference"),
                    SecretPurpose::AudioApiKey {
                        owner: request.provider.secret_owner_id,
                    },
                ),
                SecretValue::new("secret").expect("secret"),
                None,
            )
            .await
            .expect("stored secret");
        store
    }

    #[tokio::test]
    async fn durable_runner_routes_kokoro_without_a_secret_or_remote_call() {
        let root = std::env::temp_dir().join(format!("lettuce-tts-{}", RequestId::new()));
        std::fs::create_dir_all(&root).expect("create root");
        let path = root.join("app.sqlite3");
        let database = Database::open(&path).expect("database");
        let media = media_store(&path, &root.join("media"));
        let coordinator = TtsSynthesisCoordinator::new(&database, &database);
        let mut request = request(SecretRef::new(), TtsOutputPolicy::Retained);
        request.provider.label = "Local speech".into();
        request.provider.api_key_ref = None;
        request.provider.config = AudioProviderConfig::Kokoro {
            variant: Some("int8".into()),
        };
        request.model_id = "int8".into();
        request.voice_id = "af_heart".into();
        request.prompt = Some(r#"{"speed":1.0}"#.into());
        let output_asset_id = request.output_asset_id;
        let admitted = coordinator.admit(request).expect("admitted");
        let work = coordinator
            .claim(
                admitted.job.id,
                WorkerId::new(),
                NOW,
                Duration::from_secs(30),
                &ResourceAvailability::all(),
            )
            .expect("claim")
            .expect("work");
        let remote_calls = Arc::new(Mutex::new(0));
        let kokoro_calls = Arc::new(Mutex::new(0));
        let runtime = crate::ApplicationTtsRuntime::new(
            Arc::new(LocalRuntime {
                calls: remote_calls.clone(),
            }),
            Arc::new(LocalRuntime {
                calls: kokoro_calls.clone(),
            }),
        );

        let result = coordinator
            .run(
                work,
                &InMemorySecretStore::new(),
                &runtime,
                &media,
                CancellationReason::User,
                TimestampMillis::new(2_000),
            )
            .await
            .expect("run");

        assert!(matches!(
            result,
            TtsSynthesisRunResult::Succeeded(success) if success.job.state == JobState::Succeeded
        ));
        assert_eq!(*kokoro_calls.lock().expect("Kokoro calls"), 1);
        assert_eq!(*remote_calls.lock().expect("remote calls"), 0);
        assert!(
            MediaAssetRepository::get(&database, output_asset_id)
                .expect("asset read")
                .is_some()
        );
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[tokio::test]
    async fn persists_preview_and_retained_audio_and_replays() {
        let root = std::env::temp_dir().join(format!("lettuce-tts-{}", RequestId::new()));
        std::fs::create_dir_all(&root).expect("create root");
        let path = root.join("app.sqlite3");
        let calls = Arc::new(Mutex::new(0));
        let mut preview_id = None;
        let mut retained_id = None;
        let mut preview_job = None;
        {
            let database = Database::open(&path).expect("database");
            let media = media_store(&path, &root.join("media"));
            let coordinator = TtsSynthesisCoordinator::new(&database, &database);
            for policy in [
                TtsOutputPolicy::Preview {
                    expires_at: TimestampMillis::new(10_000),
                },
                TtsOutputPolicy::Retained,
            ] {
                let reference = SecretRef::new();
                let request = request(reference, policy);
                let secrets = secret_store(&request).await;
                let admitted = coordinator.admit(request).expect("admitted");
                let work = coordinator
                    .claim(
                        admitted.job.id,
                        WorkerId::new(),
                        NOW,
                        Duration::from_secs(30),
                        &ResourceAvailability::all(),
                    )
                    .expect("claim")
                    .expect("work");
                let outcome = coordinator
                    .run(
                        work,
                        &secrets,
                        &Runtime {
                            calls: calls.clone(),
                            outcome: Ok(()),
                            cancel_after_response: false,
                        },
                        &media,
                        CancellationReason::User,
                        TimestampMillis::new(2_000),
                    )
                    .await
                    .expect("run");
                let TtsSynthesisRunResult::Succeeded(success) = outcome else {
                    panic!("expected success");
                };
                let SynthesisState::Succeeded { result } = success.record.state else {
                    panic!("expected result");
                };
                let asset = MediaAssetRepository::get(&database, result.audio_asset_id)
                    .expect("asset read")
                    .expect("asset");
                match policy {
                    TtsOutputPolicy::Preview { expires_at } => {
                        assert_eq!(asset.retention, RetentionClass::Temporary { expires_at });
                        preview_id = Some(result.audio_asset_id);
                        preview_job = Some(success.job.id);
                    }
                    TtsOutputPolicy::Retained => {
                        assert_eq!(asset.retention, RetentionClass::Persistent);
                        retained_id = Some(result.audio_asset_id);
                    }
                }
            }
        }
        let database = Database::open(&path).expect("reopen database");
        let replay = TtsSynthesisCoordinator::new(&database, &database)
            .replay(preview_job.expect("preview job"))
            .expect("replay")
            .expect("completed synthesis");
        assert!(replay.replayed);
        let SynthesisState::Succeeded { result } = replay.record.state else {
            panic!("expected replay result");
        };
        assert_eq!(result.audio_asset_id, preview_id.expect("preview asset"));
        assert_ne!(preview_id, retained_id);
        assert_eq!(*calls.lock().expect("calls"), 2);
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[tokio::test]
    async fn cancellation_after_response_does_not_admit_audio() {
        let root = std::env::temp_dir().join(format!("lettuce-tts-{}", RequestId::new()));
        std::fs::create_dir_all(&root).expect("create root");
        let path = root.join("app.sqlite3");
        let database = Database::open(&path).expect("database");
        let media = media_store(&path, &root.join("media"));
        let coordinator = TtsSynthesisCoordinator::new(&database, &database);
        let request = request(
            SecretRef::new(),
            TtsOutputPolicy::Preview {
                expires_at: TimestampMillis::new(10_000),
            },
        );
        let output_asset_id = request.output_asset_id;
        let secrets = secret_store(&request).await;
        let admitted = coordinator.admit(request).expect("admitted");
        let work = coordinator
            .claim(
                admitted.job.id,
                WorkerId::new(),
                NOW,
                Duration::from_secs(30),
                &ResourceAvailability::all(),
            )
            .expect("claim")
            .expect("work");
        let outcome = coordinator
            .run(
                work,
                &secrets,
                &Runtime {
                    calls: Arc::new(Mutex::new(0)),
                    outcome: Ok(()),
                    cancel_after_response: true,
                },
                &media,
                CancellationReason::User,
                TimestampMillis::new(2_000),
            )
            .await
            .expect("cancelled run");
        assert!(matches!(
            outcome,
            TtsSynthesisRunResult::Cancelled { job, .. } if job.state == JobState::Cancelled
        ));
        assert!(
            MediaAssetRepository::get(&database, output_asset_id)
                .expect("asset read")
                .is_none()
        );
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[tokio::test]
    async fn unavailable_runtime_requeues_without_losing_the_request() {
        let root = std::env::temp_dir().join(format!("lettuce-tts-{}", RequestId::new()));
        std::fs::create_dir_all(&root).expect("create root");
        let path = root.join("app.sqlite3");
        let database = Database::open(&path).expect("database");
        let media = media_store(&path, &root.join("media"));
        let coordinator = TtsSynthesisCoordinator::new(&database, &database);
        let request = request(SecretRef::new(), TtsOutputPolicy::Retained);
        let secrets = secret_store(&request).await;
        let admitted = coordinator.admit(request).expect("admitted");
        let calls = Arc::new(Mutex::new(0));
        let work = coordinator
            .claim(
                admitted.job.id,
                WorkerId::new(),
                NOW,
                Duration::from_secs(30),
                &ResourceAvailability::all(),
            )
            .expect("claim")
            .expect("work");
        let outcome = coordinator
            .run(
                work,
                &secrets,
                &Runtime {
                    calls: calls.clone(),
                    outcome: Err(TtsRuntimeError::Unavailable),
                    cancel_after_response: false,
                },
                &media,
                CancellationReason::User,
                TimestampMillis::new(2_000),
            )
            .await
            .expect("retry result");
        assert!(matches!(
            outcome,
            TtsSynthesisRunResult::RetryScheduled { job, .. } if job.state == JobState::Queued
        ));
        assert!(matches!(
            SynthesisRepository::get(&database, admitted.job.id)
                .expect("pending request")
                .state,
            SynthesisState::Pending
        ));

        let work = coordinator
            .claim(
                admitted.job.id,
                WorkerId::new(),
                TimestampMillis::new(3_000),
                Duration::from_secs(30),
                &ResourceAvailability::all(),
            )
            .expect("second claim")
            .expect("second work");
        let outcome = coordinator
            .run(
                work,
                &secrets,
                &Runtime {
                    calls: calls.clone(),
                    outcome: Ok(()),
                    cancel_after_response: false,
                },
                &media,
                CancellationReason::User,
                TimestampMillis::new(4_000),
            )
            .await
            .expect("successful retry");
        assert!(matches!(outcome, TtsSynthesisRunResult::Succeeded(_)));
        assert_eq!(*calls.lock().expect("calls"), 2);
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}
